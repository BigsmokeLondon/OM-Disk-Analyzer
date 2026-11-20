"""Build every Windows output of OM Disk Analyzer into dist/ (run on Windows, from the app folder):

  1. OM Disk Analyzer.exe                       the program (one file)
  2. OM Disk Analyzer <ver> Portable.zip        unzip anywhere; settings stay in its Data folder
  3. OM Disk Analyzer <ver> Setup.exe           installer with Start menu entry and uninstaller
  4. OM Disk Analyzer <ver> - source code.zip   all source files and logos

Usage: python tools/build_all.py        (needs PyInstaller: pip install pyinstaller)
macOS installers are made on a Mac with tools/build_mac.sh, or by the GitHub workflow in .github/.
"""
import os
import shutil
import subprocess
import sys
import zipfile

APP = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, APP)
from omdisk import VERSION  # noqa: E402

NAME = "OM Disk Analyzer"
DIST = os.path.join(APP, "dist")
WORK = os.path.join(APP, ".build")
SOURCE = ["main.py", "omdisk.py", "omdisk_app.py", "omdisk_settings.py", "om_install.py", "installer.py",
          "README.md", "tools/make_assets.py", "tools/build_all.py", "tools/build_mac.sh",
          ".github/workflows/build.yml"]


def pyinstaller(script, name, extra):
    cmd = [sys.executable, "-m", "PyInstaller", "--noconfirm", "--clean", "--onefile", "--windowed",
           "--name", name, "--icon", os.path.join(APP, "assets", "om.ico"),
           "--add-data", f"{os.path.join(APP, 'assets')};assets",
           "--distpath", DIST, "--workpath", WORK, "--specpath", WORK] + extra + [os.path.join(APP, script)]
    subprocess.run(cmd, check=True, cwd=APP)


def build_program():
    pyinstaller("main.py", NAME, [])
    return os.path.join(DIST, NAME + ".exe")


def build_portable(exe):
    out = os.path.join(DIST, f"{NAME} {VERSION} Portable.zip")
    top = f"{NAME} Portable"
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        z.write(exe, f"{top}/{NAME}.exe")
        z.writestr(f"{top}/portable.ini", "; Keeps settings and scan history in the Data folder next to the program.\n"
                                          "[portable]\nenabled=1\n")
        z.writestr(f"{top}/Data/", "")
        z.writestr(f"{top}/Read me.txt",
                   f"{NAME} {VERSION} Portable - {chr(169)} OM Industries Inc\r\n\r\n"
                   "Unzip this folder anywhere (also a USB stick) and double-click the program.\r\n"
                   "Nothing is installed: settings and scan history stay in the Data folder.\r\n"
                   "To remove it, delete the folder.\r\n")
    return out


def build_setup(exe):
    name = f"{NAME} {VERSION} Setup"
    pyinstaller("installer.py", name, ["--add-data", f"{exe};payload"])
    return os.path.join(DIST, name + ".exe")


def zip_source():
    out = os.path.join(DIST, f"{NAME} {VERSION} - source code.zip")
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        for rel in SOURCE:
            z.write(os.path.join(APP, rel), f"{NAME}/{rel}")
        for f in sorted(os.listdir(os.path.join(APP, "assets"))):
            z.write(os.path.join(APP, "assets", f), f"{NAME}/assets/{f}")
    return out


if __name__ == "__main__":
    os.makedirs(DIST, exist_ok=True)
    for old in os.listdir(DIST):  # earlier builds
        if old.startswith(NAME):
            os.remove(os.path.join(DIST, old))
    program = build_program()
    results = [program, build_portable(program), build_setup(program), zip_source()]
    shutil.rmtree(WORK, ignore_errors=True)
    for r in results:
        print("built:", r)
