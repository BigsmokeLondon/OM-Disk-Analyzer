"""Install / uninstall helpers for OM Disk Analyzer on Windows (current user only, no admin rights).

Used by the setup program (installer.py) and by the app itself for "--uninstall".
"""
import json
import os
import shutil
import subprocess
import sys

APP_NAME = "OM Disk Analyzer"
COMPANY = "OM Industries Inc"
EXE_NAME = "OM Disk Analyzer.exe"
UNINSTALL_KEY = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\OMDiskAnalyzer"
INFO_FILE = "install.json"
NO_WINDOW = 0x08000000 if os.name == "nt" else 0


def default_dir():
    base = os.environ.get("LOCALAPPDATA") or os.path.expanduser("~")
    return os.path.join(base, "Programs", APP_NAME)


def _shell_folder(csidl, fallback):
    try:
        import ctypes
        buf = ctypes.create_unicode_buffer(520)
        if ctypes.windll.shell32.SHGetFolderPathW(None, csidl, None, 0, buf) == 0 and buf.value:
            return buf.value
    except Exception:
        pass
    return fallback


def start_menu_dir():
    programs = _shell_folder(0x02, os.path.join(os.environ.get("APPDATA", ""), "Microsoft", "Windows",
                                                "Start Menu", "Programs"))
    return os.path.join(programs, COMPANY)


def desktop_dir():
    return _shell_folder(0x10, os.path.join(os.path.expanduser("~"), "Desktop"))


def make_shortcut(lnk, target, args="", desc=""):
    """Create a .lnk with Windows' own shortcut object (values passed via environment, no quoting issues)."""
    ps = ("$s=(New-Object -ComObject WScript.Shell).CreateShortcut($env:OM_LNK);"
          "$s.TargetPath=$env:OM_TARGET;$s.Arguments=$env:OM_ARGS;"
          "$s.WorkingDirectory=$env:OM_WD;$s.IconLocation=$env:OM_TARGET+',0';"
          "$s.Description=$env:OM_DESC;$s.Save()")
    env = dict(os.environ, OM_LNK=lnk, OM_TARGET=target, OM_ARGS=args, OM_WD=os.path.dirname(target),
               OM_DESC=desc)
    subprocess.run(["powershell", "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", ps],
                   env=env, check=True, creationflags=NO_WINDOW, capture_output=True)


def installed_location():
    try:
        import winreg
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, UNINSTALL_KEY) as k:
            return winreg.QueryValueEx(k, "InstallLocation")[0]
    except OSError:
        return None


def install(payload_exe, target_dir, version, start_menu=True, desktop=False, register=True, log=print):
    """Copy the program to target_dir and add shortcuts + an 'Apps & features' entry. Returns the exe path."""
    os.makedirs(target_dir, exist_ok=True)
    exe = os.path.join(target_dir, EXE_NAME)
    log("Copying the program...")
    shutil.copy2(payload_exe, exe)
    info = {"version": version, "shortcuts": [], "registered": False}
    if start_menu:
        log("Adding the Start menu entry...")
        folder = start_menu_dir()
        os.makedirs(folder, exist_ok=True)
        for name, args, desc in ((APP_NAME, "", "See what is using space on your drives"),
                                 (f"Uninstall {APP_NAME}", "--uninstall", f"Remove {APP_NAME}")):
            lnk = os.path.join(folder, name + ".lnk")
            make_shortcut(lnk, exe, args, desc)
            info["shortcuts"].append(lnk)
    if desktop:
        log("Adding the desktop shortcut...")
        lnk = os.path.join(desktop_dir(), APP_NAME + ".lnk")
        make_shortcut(lnk, exe, "", "See what is using space on your drives")
        info["shortcuts"].append(lnk)
    if register:
        log("Registering with Windows...")
        import winreg
        with winreg.CreateKey(winreg.HKEY_CURRENT_USER, UNINSTALL_KEY) as k:
            size_kb = os.path.getsize(exe) // 1024
            for name, value in (("DisplayName", APP_NAME), ("DisplayVersion", version), ("Publisher", COMPANY),
                                ("DisplayIcon", exe + ",0"), ("InstallLocation", target_dir),
                                ("UninstallString", f'"{exe}" --uninstall')):
                winreg.SetValueEx(k, name, 0, winreg.REG_SZ, value)
            for name, value in (("NoModify", 1), ("NoRepair", 1), ("EstimatedSize", size_kb)):
                winreg.SetValueEx(k, name, 0, winreg.REG_DWORD, value)
        info["registered"] = True
    with open(os.path.join(target_dir, INFO_FILE), "w", encoding="utf-8") as f:
        json.dump(info, f, indent=1)
    log("Done.")
    return exe


def install_info(exe_path):
    try:
        with open(os.path.join(os.path.dirname(exe_path), INFO_FILE), encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError):
        return None


def uninstall(exe_path):
    """Remove shortcuts, the 'Apps & features' entry and (after this program exits) its files."""
    info = install_info(exe_path)
    if info is None:
        return False
    for lnk in info.get("shortcuts", []):
        try:
            os.remove(lnk)
        except OSError:
            pass
    try:
        os.rmdir(start_menu_dir())  # only if now empty
    except OSError:
        pass
    if info.get("registered"):
        try:
            import winreg
            winreg.DeleteKey(winreg.HKEY_CURRENT_USER, UNINSTALL_KEY)
        except OSError:
            pass
    folder = os.path.dirname(exe_path)
    # only the files the setup put there; the folder itself goes if it is then empty
    cmd = (f'ping 127.0.0.1 -n 3 > nul & del /f /q "{exe_path}" & del /f /q "{os.path.join(folder, INFO_FILE)}"'
           f' & rmdir "{folder}"')
    # one string: cmd strips the outer quotes and keeps the inner ones around each path
    subprocess.Popen(f'cmd /d /c "{cmd}"', creationflags=NO_WINDOW | 0x00000008, close_fds=True)  # DETACHED
    return True


def frozen_exe():
    return sys.executable if getattr(sys, "frozen", False) else None
