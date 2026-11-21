//! Small wrappers around the operating system: drives, volume facts, file owners, times.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Clone, Debug, Default)]
pub struct DriveInfo {
    pub root: String,
    pub label: String,
    #[allow(dead_code)]
    pub kind: String,
    pub total: u64,
    pub free: u64,
}

#[derive(Clone, Debug)]
pub struct VolumeInfo {
    pub cluster: u64,
    pub file_system: String,
    pub label: String,
    #[allow(dead_code)]
    pub kind: String,
}

impl Default for VolumeInfo {
    fn default() -> Self {
        Self { cluster: 4096, file_system: String::new(), label: String::new(), kind: "Disk".into() }
    }
}

/// Seconds since 1970 from a Windows FILETIME (100 ns ticks since 1601).
pub fn filetime_to_unix(ft: u64) -> i64 {
    if ft == 0 {
        0
    } else {
        (ft / 10_000_000) as i64 - 11_644_473_600
    }
}

pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// "C:\" for anything on drive C.
pub fn drive_root(path: &Path) -> String {
    let s = path.to_string_lossy();
    if s.len() >= 2 && s.as_bytes()[1] == b':' {
        format!("{}\\", &s[..2].to_uppercase())
    } else if s.starts_with("\\\\") {
        let parts: Vec<&str> = s.trim_start_matches('\\').splitn(3, '\\').collect();
        if parts.len() >= 2 {
            format!("\\\\{}\\{}\\", parts[0], parts[1])
        } else {
            s.to_string()
        }
    } else if let Some(rest) = s.strip_prefix("/Volumes/") {
        // macOS: an external disk is /Volumes/<name>
        match rest.split('/').next() {
            Some(name) if !name.is_empty() => format!("/Volumes/{name}"),
            _ => "/".into(),
        }
    } else {
        "/".into()
    }
}

/// What this system calls its recycle bin, file manager, and the matching menu words.
pub const TRASH_NAME: &str = if cfg!(windows) { "Recycle Bin" } else { "Trash" };
pub const FILE_MANAGER: &str = if cfg!(windows) {
    "Explorer"
} else if cfg!(target_os = "macos") {
    "Finder"
} else {
    "file manager"
};
pub const SHOW_IN_FM: &str = if cfg!(windows) {
    "Show in Explorer"
} else if cfg!(target_os = "macos") {
    "Reveal in Finder"
} else {
    "Show in file manager"
};
pub const OPEN_IN_FM: &str = if cfg!(windows) {
    "Open in Explorer"
} else if cfg!(target_os = "macos") {
    "Open in Finder"
} else {
    "Open in file manager"
};

/// The folder shown when the program starts for the first time.
pub fn default_root() -> String {
    if cfg!(windows) {
        "C:\\".into()
    } else {
        "/".into()
    }
}

/// Whether deleting is offered: on Windows only in administrator mode, elsewhere always
/// (the Trash can bring anything back).
pub fn can_delete() -> bool {
    !cfg!(windows) || is_admin()
}

#[cfg(windows)]
fn wide(s: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

#[cfg(windows)]
pub fn list_drives() -> Vec<DriveInfo> {
    use windows_sys::Win32::Storage::FileSystem::GetLogicalDrives;
    let mask = unsafe { GetLogicalDrives() };
    let mut out = Vec::new();
    for i in 0..26u32 {
        if mask >> i & 1 == 1 {
            let root = format!("{}:\\", (b'A' + i as u8) as char);
            let v = volume_info(Path::new(&root));
            if v.kind == "CD Drive" {
                continue;
            }
            let (total, free) = disk_space(Path::new(&root));
            if total == 0 {
                continue;
            }
            out.push(DriveInfo { root, label: v.label, kind: v.kind, total, free });
        }
    }
    out
}

/// macOS and Linux: the main disk "/" plus every disk mounted under /Volumes.
#[cfg(not(windows))]
pub fn list_drives() -> Vec<DriveInfo> {
    let mut root_label = String::new();
    let mut vols: Vec<(String, PathBuf)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir("/Volumes") {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let p = e.path();
            // macOS lists the startup disk in /Volumes as a link to "/"
            if std::fs::canonicalize(&p).map(|c| c == Path::new("/")).unwrap_or(false) {
                root_label = name;
            } else {
                vols.push((name, p));
            }
        }
    }
    vols.sort();
    let mut out = Vec::new();
    let (total, free) = disk_space(Path::new("/"));
    out.push(DriveInfo { root: "/".into(), label: root_label, kind: "Disk".into(), total, free });
    for (label, p) in vols {
        let (total, free) = disk_space(&p);
        if total > 0 {
            out.push(DriveInfo { root: p.to_string_lossy().into_owned(), label, kind: "Disk".into(), total, free });
        }
    }
    out
}

/// (total, free) bytes from the text of `df -Pk <path>`. The mount-point column can hold
/// spaces, so the numbers are found by their shape: total used avail NN%.
#[cfg(not(windows))]
fn parse_df(text: &str) -> Option<(u64, u64)> {
    let line = text.lines().nth(1)?;
    let t: Vec<&str> = line.split_whitespace().collect();
    for i in 0..t.len().saturating_sub(3) {
        if t[i + 3].ends_with('%') {
            if let (Ok(total), Ok(avail)) = (t[i].parse::<u64>(), t[i + 2].parse::<u64>()) {
                return Some((total * 1024, avail * 1024));
            }
        }
    }
    None
}

/// (total, free) bytes of the drive holding `path`.
#[cfg(windows)]
pub fn disk_space(path: &Path) -> (u64, u64) {
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let w = wide(&path.to_string_lossy());
    let (mut avail, mut total, mut free) = (0u64, 0u64, 0u64);
    let ok = unsafe { GetDiskFreeSpaceExW(w.as_ptr(), &mut avail, &mut total, &mut free) };
    if ok == 0 {
        (0, 0)
    } else {
        (total, free)
    }
}

#[cfg(not(windows))]
pub fn disk_space(path: &Path) -> (u64, u64) {
    match std::process::Command::new("df").arg("-Pk").arg(path).output() {
        Ok(o) if o.status.success() => parse_df(&String::from_utf8_lossy(&o.stdout)).unwrap_or((0, 0)),
        _ => (0, 0),
    }
}

#[cfg(windows)]
pub fn volume_info(path: &Path) -> VolumeInfo {
    use windows_sys::Win32::Storage::FileSystem::{GetDiskFreeSpaceW, GetDriveTypeW, GetVolumeInformationW};
    let root = drive_root(path);
    let w = wide(&root);
    let mut v = VolumeInfo::default();
    unsafe {
        let (mut spc, mut bps, mut fr, mut tot) = (0u32, 0u32, 0u32, 0u32);
        if GetDiskFreeSpaceW(w.as_ptr(), &mut spc, &mut bps, &mut fr, &mut tot) != 0 && spc * bps > 0 {
            v.cluster = (spc as u64) * (bps as u64);
        }
        let mut label = [0u16; 261];
        let mut fs = [0u16; 261];
        if GetVolumeInformationW(
            w.as_ptr(),
            label.as_mut_ptr(),
            261,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            fs.as_mut_ptr(),
            261,
        ) != 0
        {
            v.label = from_wide(&label);
            v.file_system = from_wide(&fs);
        }
        v.kind = match GetDriveTypeW(w.as_ptr()) {
            2 => "Removable Disk",
            3 => "Local Disk",
            4 => "Network Drive",
            5 => "CD Drive",
            6 => "RAM Disk",
            _ => "Disk",
        }
        .into();
    }
    v
}

/// macOS and Linux: the file system's block size stands in for the cluster size.
#[cfg(not(windows))]
pub fn volume_info(path: &Path) -> VolumeInfo {
    use std::os::unix::fs::MetadataExt;
    let cluster = std::fs::metadata(path)
        .map(|m| m.blksize())
        .ok()
        .filter(|b| *b >= 512 && b.is_power_of_two())
        .unwrap_or(4096);
    let root = drive_root(path);
    let label = root.strip_prefix("/Volumes/").unwrap_or("").to_string();
    VolumeInfo { cluster, label, ..VolumeInfo::default() }
}

static SID_NAMES: Mutex<Option<HashMap<Vec<u8>, String>>> = Mutex::new(None);

/// Account name of a file's or folder's owner ("" when it can't be read).
#[cfg(windows)]
pub fn owner_of(path: &Path) -> String {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT};
    use windows_sys::Win32::Security::{GetLengthSid, LookupAccountSidW, OWNER_SECURITY_INFORMATION, PSID};
    let w = wide(&path.to_string_lossy());
    unsafe {
        let mut sid: PSID = std::ptr::null_mut();
        let mut sd = std::ptr::null_mut();
        let rc = GetNamedSecurityInfoW(
            w.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            &mut sid,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut sd,
        );
        if rc != 0 || sid.is_null() {
            return String::new();
        }
        let len = GetLengthSid(sid) as usize;
        let key = std::slice::from_raw_parts(sid as *const u8, len).to_vec();
        if let Some(n) = SID_NAMES.lock().unwrap().get_or_insert_with(HashMap::new).get(&key) {
            let n = n.clone();
            LocalFree(sd as _);
            return n;
        }
        let mut name = [0u16; 256];
        let mut dom = [0u16; 256];
        let (mut nl, mut dl, mut use_) = (256u32, 256u32, 0i32);
        let found = LookupAccountSidW(
            std::ptr::null(),
            sid,
            name.as_mut_ptr(),
            &mut nl,
            dom.as_mut_ptr(),
            &mut dl,
            &mut use_ as *mut i32 as *mut _,
        ) != 0;
        LocalFree(sd as _);
        let n = if found { from_wide(&name) } else { String::new() };
        SID_NAMES.lock().unwrap().get_or_insert_with(HashMap::new).insert(key, n.clone());
        n
    }
}

#[cfg(not(windows))]
pub fn owner_of(path: &Path) -> String {
    use std::os::unix::fs::MetadataExt;
    static NAMES: Mutex<Option<HashMap<u32, String>>> = Mutex::new(None);
    let uid = match std::fs::symlink_metadata(path) {
        Ok(m) => m.uid(),
        Err(_) => return String::new(),
    };
    let mut guard = NAMES.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .get_or_insert_with(HashMap::new)
        .entry(uid)
        .or_insert_with(|| {
            // `id` knows directory-service accounts too (macOS keeps users out of /etc/passwd)
            std::process::Command::new("id")
                .arg("-un")
                .arg(uid.to_string())
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| uid.to_string())
        })
        .clone()
}

/// Lower the scanning thread's priority so the computer stays responsive.
#[cfg(windows)]
pub fn set_thread_priority(level: i32) {
    use windows_sys::Win32::System::Threading::{GetCurrentThread, SetThreadPriority};
    unsafe {
        SetThreadPriority(GetCurrentThread(), level);
    }
}

#[cfg(not(windows))]
pub fn set_thread_priority(_level: i32) {}

/// Where settings and scan history live: a `Data` folder beside a portable copy, else the
/// user's application data folder (shared with the earlier version).
pub fn data_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let portable = dir.join("Data");
            if portable.is_dir() {
                return portable;
            }
        }
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        return PathBuf::from(appdata).join("OM Industries Inc").join("OM Disk Analyzer");
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    let base = if cfg!(target_os = "macos") {
        home.join("Library").join("Application Support")
    } else {
        std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".local").join("share"))
    };
    base.join("OM Industries Inc").join("OM Disk Analyzer")
}

pub fn open_path(path: &Path) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer").arg(path).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(path).spawn();
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
}

/// Open a web address in the default browser. Only https:// addresses are accepted.
pub fn open_url(url: &str) {
    if !url.starts_with("https://") || url.contains(char::is_whitespace) {
        return;
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // rundll32 hands the address straight to the browser; no command line parsing of "&" etc.
        let _ = std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .creation_flags(0x0800_0000)
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

pub fn show_in_explorer(path: &Path) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let arg = format!("/select,\"{}\"", path.display());
        let _ = std::process::Command::new("explorer").raw_arg(arg).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        // Finder opens the folder with the item selected
        let _ = std::process::Command::new("open").arg("-R").arg(path).spawn();
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        if let Some(p) = path.parent() {
            open_path(p);
        }
    }
}

const MENU_KEYS: [&str; 2] = [
    r"HKCU\Software\Classes\Directory\shell\OMDiskAnalyzer",
    r"HKCU\Software\Classes\Drive\shell\OMDiskAnalyzer",
];

/// Add or remove "Scan with OM Disk Analyzer" in File Explorer's right-click menu (this user only).
pub fn set_explorer_menu(on: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const NO_WINDOW: u32 = 0x0800_0000;
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        for key in MENU_KEYS {
            let run = |args: &[&str]| -> Result<(), String> {
                let st = std::process::Command::new("reg")
                    .args(args)
                    .creation_flags(NO_WINDOW)
                    .status()
                    .map_err(|e| e.to_string())?;
                if st.success() || !on {
                    Ok(())
                } else {
                    Err(format!("reg {:?} failed", args.first()))
                }
            };
            if on {
                let icon = format!("\"{}\",0", exe.display());
                let cmd = format!("\"{}\" \"%1\"", exe.display());
                run(&["add", key, "/ve", "/d", "Scan with OM Disk Analyzer", "/f"])?;
                run(&["add", key, "/v", "Icon", "/d", &icon, "/f"])?;
                run(&["add", &format!("{key}\\command"), "/ve", "/d", &cmd, "/f"])?;
            } else {
                run(&["delete", key, "/f"])?;
            }
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = on;
        Err("Only available on Windows".into())
    }
}

pub fn explorer_menu_present() -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("reg")
            .args(["query", MENU_KEYS[0]])
            .creation_flags(0x0800_0000)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Size on disk Windows reports for a compressed or sparse file.
#[cfg(windows)]
pub fn compressed_size(path: &Path) -> Option<u64> {
    use windows_sys::Win32::Storage::FileSystem::GetCompressedFileSizeW;
    let w = wide(&path.to_string_lossy());
    let mut high = 0u32;
    let low = unsafe { GetCompressedFileSizeW(w.as_ptr(), &mut high) };
    if low == u32::MAX && std::io::Error::last_os_error().raw_os_error().unwrap_or(0) != 0 {
        None
    } else {
        Some(((high as u64) << 32) | low as u64)
    }
}

#[cfg(not(windows))]
pub fn compressed_size(_path: &Path) -> Option<u64> {
    None
}

/// Whether this copy runs with administrator rights.
#[cfg(windows)]
pub fn is_admin() -> bool {
    unsafe { windows_sys::Win32::UI::Shell::IsUserAnAdmin() != 0 }
}

#[cfg(not(windows))]
pub fn is_admin() -> bool {
    false
}

/// Delete a file or folder: to the Recycle Bin, or permanently.
pub fn delete_path(path: &Path, is_dir: bool, recycle: bool) -> Result<(), String> {
    if !recycle {
        let r = if is_dir { std::fs::remove_dir_all(path) } else { std::fs::remove_file(path) };
        return r.map_err(|e| e.to_string());
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Shell::{SHFileOperationW, FO_DELETE, SHFILEOPSTRUCTW};
        const FOF_SILENT: u16 = 0x0004;
        const FOF_NOCONFIRMATION: u16 = 0x0010;
        const FOF_ALLOWUNDO: u16 = 0x0040;
        const FOF_NOERRORUI: u16 = 0x0400;
        let mut from = wide(&path.to_string_lossy());
        from.push(0); // the list of paths ends with two zeros
        let mut op: SHFILEOPSTRUCTW = unsafe { std::mem::zeroed() };
        op.wFunc = FO_DELETE;
        op.pFrom = from.as_ptr();
        op.fFlags = FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI;
        let rc = unsafe { SHFileOperationW(&mut op) };
        if rc != 0 {
            return Err(format!("Windows error {rc:#x}"));
        }
        if op.fAnyOperationsAborted != 0 {
            return Err("cancelled".into());
        }
        if path.exists() {
            return Err("it is still there (it may be in use)".into());
        }
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        let _ = is_dir;
        let home = std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| "no home folder".to_string())?;
        // a plain move into ~/.Trash; for another disk (or if that fails) Finder does it
        if trash_into(path, &home.join(".Trash")).is_ok() {
            return Ok(());
        }
        finder_trash(path)
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let _ = is_dir;
        Err("the Trash is only available on macOS and Windows".into())
    }
}

/// Move a file or folder into a trash folder without overwriting what is already there.
#[cfg(unix)]
fn trash_into(path: &Path, trash: &Path) -> Result<(), String> {
    std::fs::create_dir_all(trash).map_err(|e| e.to_string())?;
    let name = path.file_name().ok_or_else(|| "no name".to_string())?.to_string_lossy().into_owned();
    let mut dest = trash.join(&name);
    let mut n = 1;
    while std::fs::symlink_metadata(&dest).is_ok() {
        n += 1;
        dest = trash.join(format!("{name} {n}"));
    }
    std::fs::rename(path, &dest).map_err(|e| e.to_string())
}

/// Ask Finder to move an item to the Trash (macOS may ask once for permission to control Finder).
#[cfg(target_os = "macos")]
fn finder_trash(path: &Path) -> Result<(), String> {
    let p = path.to_string_lossy().replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!("tell application \"Finder\" to delete (POSIX file \"{p}\")");
    let out = std::process::Command::new("osascript").arg("-e").arg(script).output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    if std::fs::symlink_metadata(path).is_ok() {
        return Err("it is still there (it may be in use)".into());
    }
    Ok(())
}

/// Quote one command-line argument the way Windows programs read them back.
fn quote_arg(a: &str) -> String {
    if !a.is_empty() && !a.contains([' ', '\t', '"']) {
        return a.to_string();
    }
    let mut out = String::from("\"");
    let mut slashes = 0;
    for c in a.chars() {
        match c {
            '\\' => slashes += 1,
            '"' => {
                out.push_str(&"\\".repeat(slashes * 2 + 1));
                out.push('"');
                slashes = 0;
            }
            _ => {
                out.push_str(&"\\".repeat(slashes));
                slashes = 0;
                out.push(c);
            }
        }
    }
    out.push_str(&"\\".repeat(slashes * 2)); // backslashes before the closing quote are doubled
    out.push('"');
    out
}

/// Start this program again with administrator rights and these arguments (Windows asks for
/// permission). `--elevated` is added so the new copy doesn't ask again.
#[cfg(windows)]
pub fn restart_elevated(args: &[String]) -> Result<(), String> {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let file = wide(&exe.to_string_lossy());
    let verb = wide("runas");
    let mut list: Vec<String> = args.iter().map(|a| quote_arg(a)).collect();
    list.push("--elevated".into());
    let params = wide(&list.join(" "));
    let dir = exe.parent().map(|d| wide(&d.to_string_lossy()));
    let rc = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            params.as_ptr(),
            dir.as_ref().map(|d| d.as_ptr()).unwrap_or(std::ptr::null()),
            1,
        )
    } as isize;
    if rc > 32 {
        Ok(())
    } else if rc == 5 {
        Err("permission was not given".into())
    } else {
        Err(format!("Windows refused (code {rc})"))
    }
}

#[cfg(not(windows))]
pub fn restart_elevated(_args: &[String]) -> Result<(), String> {
    Err("only available on Windows".into())
}

/// Start this program again as administrator, opening `folder`.
pub fn restart_as_admin(folder: &str) -> Result<(), String> {
    restart_elevated(&[folder.to_string()])
}

/// True when the signed-in account is an administrator but this copy runs without those
/// rights (Windows' "limited" token), so starting elevated is possible.
#[cfg(windows)]
pub fn can_elevate() -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::{GetTokenInformation, TokenElevationType, TOKEN_QUERY};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    const LIMITED: i32 = 3; // TokenElevationTypeLimited
    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut kind: i32 = 0;
        let mut len = 0u32;
        let ok = GetTokenInformation(
            token,
            TokenElevationType,
            &mut kind as *mut i32 as *mut _,
            std::mem::size_of::<i32>() as u32,
            &mut len,
        ) != 0;
        CloseHandle(token);
        ok && kind == LIMITED
    }
}

#[cfg(not(windows))]
pub fn can_elevate() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delete_permanently() {
        let dir = std::env::temp_dir().join(format!("om-delete-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("sub").join("a.txt"), b"hello").unwrap();
        std::fs::write(dir.join("b.txt"), b"x").unwrap();
        delete_path(&dir.join("b.txt"), false, false).unwrap();
        assert!(!dir.join("b.txt").exists());
        delete_path(&dir, true, false).unwrap();
        assert!(!dir.exists());
        assert!(delete_path(&dir, true, false).is_err());
    }

    #[test]
    fn quoting() {
        assert_eq!(quote_arg(r"C:\"), r"C:\");
        assert_eq!(quote_arg(r"C:\Program Files\"), r#""C:\Program Files\\""#);
        assert_eq!(quote_arg(r#"a "b"#), r#""a \"b""#);
        assert_eq!(quote_arg(""), r#""""#);
    }
}

#[cfg(test)]
mod elevation_probe {
    #[test]
    fn report() {
        println!("can_elevate={} is_admin={}", super::can_elevate(), super::is_admin());
    }
}

#[cfg(all(test, unix))]
mod unix_tests {
    use super::*;

    #[test]
    fn df_text_is_read_even_with_spaces_in_names() {
        let mac = "Filesystem 1024-blocks Used Available Capacity Mounted on\n/dev/disk3s1s1 971350180 15400000 400000000 4% /\n";
        assert_eq!(parse_df(mac), Some((971350180 * 1024, 400000000 * 1024)));
        let odd = "Filesystem 1024-blocks Used Available Capacity Mounted on\nmy server:/share 1000 400 600 40% /Volumes/My Disk\n";
        assert_eq!(parse_df(odd), Some((1000 * 1024, 600 * 1024)));
        assert_eq!(parse_df("nothing useful"), None);
    }

    #[test]
    fn real_disk_space_and_drive_list() {
        let (total, free) = disk_space(Path::new("/"));
        assert!(total > 0 && free <= total);
        let drives = list_drives();
        assert_eq!(drives[0].root, "/");
        assert!(drives[0].total > 0);
    }

    #[test]
    fn macos_volume_roots() {
        assert_eq!(drive_root(Path::new("/Volumes/Backup/Photos/2024")), "/Volumes/Backup");
        assert_eq!(drive_root(Path::new("/Volumes/Backup")), "/Volumes/Backup");
        assert_eq!(drive_root(Path::new("/Users/me")), "/");
        assert_eq!(drive_root(Path::new("/Volumes/")), "/");
    }

    #[test]
    fn block_size_and_owner_of_a_real_file() {
        let v = volume_info(Path::new("/"));
        assert!(v.cluster >= 512 && v.cluster.is_power_of_two());
        let dir = std::env::temp_dir().join(format!("omda-sys-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("x.txt");
        std::fs::write(&f, b"hi").unwrap();
        let o = owner_of(&f);
        assert!(!o.is_empty());
        assert_eq!(o, owner_of(&f), "second lookup comes from the cache");
        assert_eq!(owner_of(&dir.join("missing")), "");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn trash_moves_and_never_overwrites() {
        let base = std::env::temp_dir().join(format!("omda-trash-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let trash = base.join(".Trash");
        for round in 0..3 {
            let d = base.join(format!("work{round}")).join("Photos");
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("a.jpg"), format!("{round}")).unwrap();
            trash_into(&d, &trash).unwrap();
            assert!(!d.exists());
        }
        let mut names: Vec<String> =
            std::fs::read_dir(&trash).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        assert_eq!(names, ["Photos", "Photos 2", "Photos 3"]);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn labels_follow_the_system() {
        assert!(can_delete());
        assert_eq!(default_root(), "/");
        assert_eq!(TRASH_NAME, "Trash");
    }
}
