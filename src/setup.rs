//! Setup and uninstall for OM Disk Analyzer: for the current user only, no administrator
//! rights. A copy of the program whose file name contains "Setup" (or started with --setup)
//! shows the install wizard; the installed copy started with --uninstall removes itself.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

use egui::{pos2, vec2, Align, Button, Color32, Context, Layout, Rect, RichText, Stroke, Ui, ViewportCommand};
use serde::{Deserialize, Serialize};

use crate::app::{bold, load_texture, regular, setup_fonts, setup_style};
use crate::draw::*;
use crate::{sys, APP_NAME, COMPANY, EDITION};

pub const EXE_NAME: &str = "OM Disk Analyzer.exe";
const UNINSTALL_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\OMDiskAnalyzer";
const INFO_FILE: &str = "install.json";

pub fn short_version() -> String {
    let v = crate::VERSION;
    v.strip_suffix(".0").unwrap_or(v).to_string()
}

#[derive(Serialize, Deserialize, Default)]
struct InstallInfo {
    version: String,
    shortcuts: Vec<String>,
    registered: bool,
}

pub fn default_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    base.join("Programs").join(APP_NAME)
}

fn start_menu_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    base.join("Microsoft").join("Windows").join("Start Menu").join("Programs").join(COMPANY)
}

/// Run a helper program without a console window and return what it printed.
fn run_hidden(program: &str, args: &[&str], envs: &[(&str, &str)]) -> Result<String, String> {
    let mut c = std::process::Command::new(program);
    c.args(args);
    for (k, v) in envs {
        c.env(k, v);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    let out = c.output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// Windows' own shortcut object, with every value passed through the environment (no quoting issues).
fn make_shortcut(lnk: &Path, target: &Path, args: &str, desc: &str) -> Result<(), String> {
    let ps = "$s=(New-Object -ComObject WScript.Shell).CreateShortcut($env:OM_LNK);\
              $s.TargetPath=$env:OM_TARGET;$s.Arguments=$env:OM_ARGS;\
              $s.WorkingDirectory=$env:OM_WD;$s.IconLocation=$env:OM_TARGET+',0';\
              $s.Description=$env:OM_DESC;$s.Save()";
    let wd = target.parent().map(|p| p.display().to_string()).unwrap_or_default();
    run_hidden(
        "powershell",
        &["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", ps],
        &[
            ("OM_LNK", &lnk.display().to_string()),
            ("OM_TARGET", &target.display().to_string()),
            ("OM_ARGS", args),
            ("OM_WD", &wd),
            ("OM_DESC", desc),
        ],
    )
    .map(|_| ())
}

fn desktop_dir() -> PathBuf {
    run_hidden("powershell", &["-NoProfile", "-NonInteractive", "-Command", "[Environment]::GetFolderPath('Desktop')"], &[])
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var("USERPROFILE").unwrap_or_default()).join("Desktop"))
}

fn reg(args: &[&str]) -> Result<String, String> {
    run_hidden("reg", args, &[])
}

/// Where Setup put the program before, if it did.
pub fn installed_location() -> Option<PathBuf> {
    let out = reg(&["query", UNINSTALL_KEY, "/v", "InstallLocation"]).ok()?;
    out.lines().find_map(|l| l.split_once("REG_SZ").map(|(_, v)| PathBuf::from(v.trim())))
}

enum Msg {
    Log(String),
    Done(Result<PathBuf, String>),
}

/// Copy this program into `dir`, add shortcuts and the Settings > Apps entry.
fn install(dir: &Path, start_menu: bool, desktop: bool, log: &dyn Fn(String)) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("Can't create {}: {e}", dir.display()))?;
    let exe = dir.join(EXE_NAME);
    log("Copying the program...".into());
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    if me != exe {
        std::fs::copy(&me, &exe).map_err(|e| {
            format!("Can't copy the program to {} ({e}). If OM Disk Analyzer is open, close it and try again.", exe.display())
        })?;
    }
    let mut info = InstallInfo { version: short_version(), ..Default::default() };
    if start_menu {
        log("Adding the Start menu entry...".into());
        let folder = start_menu_dir();
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        for (name, args, desc) in [
            (APP_NAME.to_string(), "", "See what is using space on your drives".to_string()),
            (format!("Uninstall {APP_NAME}"), "--uninstall", format!("Remove {APP_NAME}")),
        ] {
            let lnk = folder.join(format!("{name}.lnk"));
            make_shortcut(&lnk, &exe, args, &desc)?;
            info.shortcuts.push(lnk.display().to_string());
        }
    }
    if desktop {
        log("Adding the desktop shortcut...".into());
        let lnk = desktop_dir().join(format!("{APP_NAME}.lnk"));
        make_shortcut(&lnk, &exe, "", "See what is using space on your drives")?;
        info.shortcuts.push(lnk.display().to_string());
    }
    log("Registering with Windows (Settings > Apps)...".into());
    let size_kb = std::fs::metadata(&exe).map(|m| m.len() / 1024).unwrap_or(0).to_string();
    let exe_s = exe.display().to_string();
    let values = [
        ("DisplayName", "REG_SZ", APP_NAME.to_string()),
        ("DisplayVersion", "REG_SZ", short_version()),
        ("Publisher", "REG_SZ", COMPANY.to_string()),
        ("DisplayIcon", "REG_SZ", format!("{exe_s},0")),
        ("InstallLocation", "REG_SZ", dir.display().to_string()),
        ("UninstallString", "REG_SZ", format!("\"{exe_s}\" --uninstall")),
        ("NoModify", "REG_DWORD", "1".into()),
        ("NoRepair", "REG_DWORD", "1".into()),
        ("EstimatedSize", "REG_DWORD", size_kb),
    ];
    for (name, kind, value) in values {
        reg(&["add", UNINSTALL_KEY, "/v", name, "/t", kind, "/d", &value, "/f"])?;
    }
    info.registered = true;
    let json = serde_json::to_string_pretty(&info).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(INFO_FILE), json).map_err(|e| e.to_string())?;
    log("Done.".into());
    Ok(exe)
}

fn install_info(exe: &Path) -> Option<InstallInfo> {
    let s = std::fs::read_to_string(exe.parent()?.join(INFO_FILE)).ok()?;
    serde_json::from_str(&s).ok()
}

/// Remove shortcuts, the Settings > Apps entry, the File Explorer menu and (optionally) settings.
/// The program's own files go once it has closed (see `delete_after_exit`).
fn uninstall(exe: &Path, remove_data: bool) -> Result<(), String> {
    let info = install_info(exe).ok_or("This copy wasn't installed by Setup.")?;
    for lnk in &info.shortcuts {
        let _ = std::fs::remove_file(lnk);
    }
    let _ = std::fs::remove_dir(start_menu_dir()); // only when it is now empty
    if info.registered {
        let _ = reg(&["delete", UNINSTALL_KEY, "/f"]);
    }
    if sys::explorer_menu_present() {
        let _ = sys::set_explorer_menu(false);
    }
    if remove_data {
        let _ = std::fs::remove_dir_all(sys::data_dir());
        if let Some(company) = sys::data_dir().parent() {
            let _ = std::fs::remove_dir(company);
        }
    }
    Ok(())
}

/// Delete the program and its install note a moment after this process exits.
fn delete_after_exit(exe: &Path) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let folder = exe.parent().unwrap_or(Path::new("."));
        let cmd = format!(
            "ping 127.0.0.1 -n 3 > nul & del /f /q \"{}\" & del /f /q \"{}\" & rmdir \"{}\"",
            exe.display(),
            folder.join(INFO_FILE).display(),
            folder.display()
        );
        // one string: cmd strips the outer quotes and keeps the inner ones around each path
        let _ = std::process::Command::new("cmd")
            .raw_arg(format!("/d /c \"{cmd}\""))
            .creation_flags(0x0800_0000 | 0x0000_0008)
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = exe;
    }
}

// ------------------------------------------------------------------ window --

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Install,
    Uninstall,
}

enum Page {
    Options,
    Working,
    Done(Result<PathBuf, String>),
}

struct SetupApp {
    mode: Mode,
    page: Page,
    dir: String,
    previous: Option<PathBuf>,
    start_menu: bool,
    desktop: bool,
    launch: bool,
    remove_data: bool,
    log: Vec<String>,
    rx: Option<Receiver<Msg>>,
    badge: egui::TextureHandle,
    installed_copy: bool,
    screenshot: Option<PathBuf>,
    frames: u32,
    shot_sent: bool,
    shot_done: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

pub fn run(mode: Mode, screenshot: Option<PathBuf>) -> eframe::Result<()> {
    let title = match mode {
        Mode::Install => format!("{APP_NAME} {} Setup", short_version()),
        Mode::Uninstall => format!("Uninstall {APP_NAME}"),
    };
    let icon = image::load_from_memory(include_bytes!("../assets/icon_256.png")).ok().map(|img| {
        let img = img.to_rgba8();
        let (w, h) = img.dimensions();
        egui::IconData { rgba: img.into_raw(), width: w, height: h }
    });
    let mut vp = egui::ViewportBuilder::default().with_title(title).with_inner_size([660.0, 430.0]).with_resizable(false);
    if let Some(icon) = icon {
        vp = vp.with_icon(std::sync::Arc::new(icon));
    }
    if screenshot.is_some() {
        vp = vp.with_active(false);
        #[cfg(windows)]
        {
            vp = vp.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }
    let options = eframe::NativeOptions { viewport: vp, ..Default::default() };
    eframe::run_native(
        "OM Disk Analyzer Setup",
        options,
        Box::new(move |cc| {
            setup_fonts(&cc.egui_ctx);
            setup_style(&cc.egui_ctx);
            let badge = load_texture(&cc.egui_ctx, "om-badge", include_bytes!("../assets/badge_256.png"));
            let previous = installed_location();
            let me = std::env::current_exe().ok();
            let installed_copy = me.as_deref().and_then(install_info).is_some();
            Ok(Box::new(SetupApp {
                mode,
                page: Page::Options,
                dir: previous.clone().unwrap_or_else(default_dir).display().to_string(),
                previous,
                start_menu: true,
                desktop: false,
                launch: true,
                remove_data: false,
                log: Vec::new(),
                rx: None,
                badge,
                installed_copy,
                screenshot,
                frames: 0,
                shot_sent: false,
                shot_done: Default::default(),
            }))
        }),
    )
}

impl SetupApp {
    fn banner(&self, ui: &mut Ui) {
        let r = ui.max_rect();
        let p = ui.painter();
        p.rect_filled(r, 0.0, NAVY);
        let side = 150.0;
        let img = Rect::from_center_size(pos2(r.center().x, r.top() + 40.0 + side / 2.0), vec2(side, side));
        p.circle_filled(img.center(), side / 2.0 + 6.0, CREAM);
        p.image(self.badge.id(), img, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        let y = img.bottom() + 26.0;
        p.text(pos2(r.center().x, y), egui::Align2::CENTER_CENTER, "O.M. INDUSTRIES", bold(17.0), CREAM);
        p.text(pos2(r.center().x, y + 22.0), egui::Align2::CENTER_CENTER, "INC.", bold(15.0), GOLD);
        p.line_segment([pos2(r.left() + 30.0, y + 42.0), pos2(r.right() - 30.0, y + 42.0)], Stroke::new(2.0, RUST));
        p.text(pos2(r.center().x, y + 62.0), egui::Align2::CENTER_CENTER, format!("{APP_NAME}"), regular(13.5), CREAM);
        p.text(pos2(r.center().x, y + 80.0), egui::Align2::CENTER_CENTER, format!("{EDITION} {}", short_version()), regular(13.0), GOLD);
    }

    fn start(&mut self, ctx: &Context) {
        let (tx, rx) = channel();
        self.rx = Some(rx);
        self.log.clear();
        self.page = Page::Working;
        let ctx = ctx.clone();
        match self.mode {
            Mode::Install => {
                let dir = PathBuf::from(self.dir.trim());
                let (sm, dt) = (self.start_menu, self.desktop);
                std::thread::spawn(move || {
                    let log = |s: String| {
                        let _ = tx.send(Msg::Log(s));
                        ctx.request_repaint();
                    };
                    let r = install(&dir, sm, dt, &log);
                    let _ = tx.send(Msg::Done(r));
                    ctx.request_repaint();
                });
            }
            Mode::Uninstall => {
                let remove = self.remove_data;
                std::thread::spawn(move || {
                    let r = std::env::current_exe()
                        .map_err(|e| e.to_string())
                        .and_then(|exe| uninstall(&exe, remove).map(|_| exe));
                    let _ = tx.send(Msg::Done(r));
                    ctx.request_repaint();
                });
            }
        }
    }

    fn content(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        if let Some(rx) = &self.rx {
            while let Ok(m) = rx.try_recv() {
                match m {
                    Msg::Log(s) => self.log.push(s),
                    Msg::Done(r) => self.page = Page::Done(r),
                }
            }
        }
        let v = short_version();
        match (&self.page, self.mode) {
            (Page::Options, Mode::Install) => {
                ui.label(RichText::new(format!("Install {APP_NAME} {v}")).font(bold(20.0)).color(NAVY));
                ui.label(RichText::new("See what is using space on your drives.").color(MUTED));
                ui.add_space(14.0);
                ui.label("Install into this folder:");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.dir).desired_width(300.0));
                    if ui.button("Browse...").clicked() {
                        if let Some(p) = rfd::FileDialog::new().pick_folder() {
                            self.dir = p.join(APP_NAME).display().to_string();
                        }
                    }
                });
                if let Some(prev) = &self.previous {
                    ui.label(RichText::new(format!("An earlier copy is installed in {}; it will be updated.", prev.display())).color(RUST));
                }
                ui.add_space(10.0);
                ui.checkbox(&mut self.start_menu, "Add to the Start menu (with an Uninstall entry)");
                ui.checkbox(&mut self.desktop, "Add a desktop shortcut");
                ui.add_space(10.0);
                ui.label(
                    RichText::new("Installs for your user only; no administrator rights needed. Remove it any time from Settings > Apps.")
                        .color(MUTED),
                );
                self.buttons(ui, "Install");
            }
            (Page::Options, Mode::Uninstall) => {
                ui.label(RichText::new(format!("Remove {APP_NAME}")).font(bold(20.0)).color(NAVY));
                ui.add_space(14.0);
                if self.installed_copy {
                    ui.label(format!("This removes {APP_NAME} {v}, its Start menu and desktop shortcuts and its entry in Settings > Apps."));
                    ui.add_space(10.0);
                    ui.checkbox(&mut self.remove_data, "Also remove my settings and scan history");
                    self.buttons(ui, "Remove");
                } else {
                    ui.label("This copy wasn't installed by Setup (for example the portable copy).");
                    ui.label("To remove it, close it and delete its folder.");
                    self.close_button(ui);
                }
            }
            (Page::Working, _) => {
                ui.label(RichText::new(if self.mode == Mode::Install { "Installing..." } else { "Removing..." }).font(bold(20.0)).color(NAVY));
                ui.add_space(12.0);
                ui.add(egui::Spinner::new());
                for l in &self.log {
                    ui.label(l);
                }
            }
            (Page::Done(r), mode) => {
                match r {
                    Ok(exe) => {
                        let exe = exe.clone();
                        if mode == Mode::Install {
                            ui.label(RichText::new(format!("{APP_NAME} {v} is installed")).font(bold(20.0)).color(NAVY));
                            ui.add_space(10.0);
                            ui.label(format!("You'll find it in {}.", if self.start_menu { "the Start menu, under OM Industries Inc" } else { "its folder" }));
                            ui.label(RichText::new(exe.display().to_string()).color(MUTED));
                            ui.add_space(10.0);
                            ui.checkbox(&mut self.launch, format!("Start {APP_NAME} now"));
                            ui.with_layout(Layout::bottom_up(Align::Max), |ui| {
                                if ui.add(Button::new("Finish").min_size(vec2(100.0, 28.0))).clicked() {
                                    if self.launch {
                                        let _ = std::process::Command::new(&exe).spawn();
                                    }
                                    ctx.send_viewport_cmd(ViewportCommand::Close);
                                }
                            });
                        } else {
                            ui.label(RichText::new(format!("{APP_NAME} has been removed")).font(bold(20.0)).color(NAVY));
                            ui.add_space(10.0);
                            ui.label("Its files are deleted as soon as this window closes.");
                            ui.with_layout(Layout::bottom_up(Align::Max), |ui| {
                                if ui.add(Button::new("Close").min_size(vec2(100.0, 28.0))).clicked() {
                                    delete_after_exit(&exe);
                                    ctx.send_viewport_cmd(ViewportCommand::Close);
                                }
                            });
                        }
                    }
                    Err(e) => {
                        let e = e.clone();
                        ui.label(RichText::new("Something went wrong").font(bold(20.0)).color(ERROR));
                        ui.add_space(10.0);
                        ui.label(e);
                        ui.with_layout(Layout::bottom_up(Align::Max), |ui| {
                            ui.horizontal(|ui| {
                                if ui.add(Button::new("Close").min_size(vec2(90.0, 28.0))).clicked() {
                                    ctx.send_viewport_cmd(ViewportCommand::Close);
                                }
                                if ui.add(Button::new("Back").min_size(vec2(90.0, 28.0))).clicked() {
                                    self.page = Page::Options;
                                }
                            });
                        });
                    }
                }
            }
        }
    }

    fn buttons(&mut self, ui: &mut Ui, go: &str) {
        let ctx = ui.ctx().clone();
        ui.with_layout(Layout::bottom_up(Align::Max), |ui| {
            ui.horizontal(|ui| {
                if ui.add(Button::new("Cancel").min_size(vec2(90.0, 28.0))).clicked() {
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
                let b = Button::new(RichText::new(go).color(Color32::WHITE).font(bold(14.0))).fill(RUST).min_size(vec2(110.0, 28.0));
                if ui.add(b).clicked() {
                    self.start(&ctx);
                }
            });
        });
    }

    fn close_button(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        ui.with_layout(Layout::bottom_up(Align::Max), |ui| {
            if ui.add(Button::new("Close").min_size(vec2(90.0, 28.0))).clicked() {
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        });
    }

    fn handle_screenshot(&mut self, ctx: &Context) {
        use std::sync::atomic::Ordering;
        let Some(path) = self.screenshot.clone() else { return };
        self.frames += 1;
        if self.frames >= 60 && !self.shot_sent {
            self.shot_sent = true;
            let done = self.shot_done.clone();
            let ctx2 = ctx.clone();
            ctx.send_viewport_cmd(ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |img| {
                let [w, h] = img.size;
                let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| [c.r(), c.g(), c.b(), 255]).collect();
                if let Some(i) = image::RgbaImage::from_raw(w as u32, h as u32, bytes) {
                    let _ = i.save(&path);
                }
                done.store(true, Ordering::SeqCst);
                ctx2.request_repaint();
            })));
        }
        if self.shot_done.load(Ordering::SeqCst) {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        ctx.send_viewport_cmd(ViewportCommand::RequestPaintWhileHidden);
        ctx.request_repaint();
    }
}

impl eframe::App for SetupApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        egui::Panel::left("banner").resizable(false).default_size(220.0).frame(egui::Frame::new()).show(ui, |ui| {
            ui.set_min_width(220.0);
            self.banner(ui);
        });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(PANEL).inner_margin(egui::Margin::same(22)))
            .show(ui, |ui| self.content(ui));
        self.handle_screenshot(&ctx);
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn shortcut_is_made() {
        let dir = std::env::temp_dir().join(format!("om-setup-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let lnk = dir.join("OM test.lnk");
        let target = std::env::current_exe().unwrap();
        make_shortcut(&lnk, &target, "--uninstall", "test").unwrap();
        assert!(lnk.is_file());
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(short_version(), "2.0");
    }
}
