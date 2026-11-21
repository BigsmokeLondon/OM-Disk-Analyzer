"""Setup program for OM Disk Analyzer (Windows, current user, no administrator rights).

Packaged by tools/build_all.py into "OM Disk Analyzer Setup.exe", which carries the program inside.
Options: --silent [--target DIR] [--no-start-menu] [--desktop] [--no-register]   install without the window
         --payload FILE                                               program to install (when run from source)
         --screenshot FILE                                            picture of the setup window (for checks)
"""
import os
import subprocess
import sys
import threading
import tkinter as tk
from tkinter import ttk, filedialog, messagebox

import om_install
from omdisk import CREAM, GOLD, MUTED, NAVY, ORANGE, VERSION, brand_image, capture_window

BASE = getattr(sys, "_MEIPASS", os.path.dirname(os.path.abspath(__file__)))


def payload_path(argv):
    if "--payload" in argv:
        return argv[argv.index("--payload") + 1]
    inside = os.path.join(BASE, "payload", om_install.EXE_NAME)
    if os.path.exists(inside):
        return inside
    return os.path.join(BASE, "dist", om_install.EXE_NAME)


class Setup:
    def __init__(self, root, payload, screenshot=None):
        self.root, self.payload, self.screenshot = root, payload, screenshot
        self.k = k = max(1.0, root.winfo_fpixels("1i") / 96.0)
        root.title(f"{om_install.APP_NAME} {VERSION} Setup")
        root.geometry(f"{int(640 * k)}x{int(420 * k)}")
        root.resizable(False, False)
        icon = brand_image("icon", 64)
        if icon is not None:
            root.iconphoto(True, icon)
        ttk.Style(root).theme_use("clam")
        side = tk.Frame(root, bg=NAVY, width=int(200 * k))
        side.pack(side="left", fill="y")
        side.pack_propagate(False)
        badge = brand_image("badge", int(160 * k))
        if badge is not None:
            tk.Label(side, image=badge, bg=NAVY).pack(pady=(int(40 * k), int(12 * k)))
        tk.Label(side, text="O.M. INDUSTRIES", bg=NAVY, fg=CREAM, font=("Segoe UI", 11, "bold")).pack()
        tk.Label(side, text="INC.", bg=NAVY, fg=GOLD, font=("Segoe UI", 10, "bold")).pack()
        self.body = tk.Frame(root, bg="white", padx=int(24 * k), pady=int(20 * k))
        self.body.pack(side="left", fill="both", expand=True)
        self.target = tk.StringVar(value=om_install.installed_location() or om_install.default_dir())
        self.start_menu = tk.BooleanVar(value=True)
        self.desktop = tk.BooleanVar(value=False)
        self.launch = tk.BooleanVar(value=True)
        self.page_options()

    def _clear(self):
        for w in self.body.winfo_children():
            w.destroy()

    def _title(self, text, sub):
        tk.Label(self.body, text=text, bg="white", fg=NAVY, font=("Segoe UI Semibold", 15)).pack(anchor="w")
        tk.Label(self.body, text=sub, bg="white", fg=MUTED, font=("Segoe UI", 9), justify="left",
                 wraplength=int(390 * self.k)).pack(anchor="w", pady=(2, int(14 * self.k)))

    def page_options(self):
        self._clear()
        upgrade = om_install.installed_location() is not None
        self._title(f"{'Update' if upgrade else 'Install'} {om_install.APP_NAME} {VERSION}",
                    "See what is using space on your drives. The program is installed for your user account "
                    "only, so no administrator rights are needed.")
        tk.Label(self.body, text="Install to:", bg="white", font=("Segoe UI", 9, "bold")).pack(anchor="w")
        row = tk.Frame(self.body, bg="white")
        row.pack(fill="x", pady=(2, 12))
        ttk.Entry(row, textvariable=self.target).pack(side="left", fill="x", expand=True)
        ttk.Button(row, text="Browse...", command=self._browse).pack(side="left", padx=(6, 0))
        for var, text in ((self.start_menu, "Add to the Start menu"), (self.desktop, "Add a desktop shortcut")):
            tk.Checkbutton(self.body, text=text, variable=var, bg="white", activebackground="white",
                           font=("Segoe UI", 9)).pack(anchor="w")
        bar = tk.Frame(self.body, bg="white")
        bar.pack(side="bottom", fill="x")
        ttk.Button(bar, text="Cancel", command=self.root.destroy).pack(side="right")
        ttk.Button(bar, text="Install", command=self.run_install).pack(side="right", padx=8)

    def _browse(self):
        d = filedialog.askdirectory(parent=self.root, initialdir=os.path.dirname(self.target.get()))
        if d:
            self.target.set(os.path.join(os.path.normpath(d), om_install.APP_NAME))

    def run_install(self):
        self._clear()
        self._title("Installing...", f"Please wait while {om_install.APP_NAME} is set up.")
        bar = ttk.Progressbar(self.body, mode="indeterminate")
        bar.pack(fill="x")
        bar.start(10)
        msg = tk.StringVar(value="Starting...")
        tk.Label(self.body, textvariable=msg, bg="white", fg=MUTED, font=("Segoe UI", 9)).pack(anchor="w", pady=6)
        result = {}

        def work():
            try:
                result["exe"] = om_install.install(self.payload, self.target.get(), VERSION,
                                                   start_menu=self.start_menu.get(), desktop=self.desktop.get(),
                                                   log=lambda t: self.root.after(0, msg.set, t))
            except PermissionError:
                result["error"] = f"{om_install.APP_NAME} seems to be running. Close it and try again."
            except Exception as ex:  # shown to the person instead of a crash
                result["error"] = str(ex)

        def wait():
            if t.is_alive():
                self.root.after(100, wait)
            elif "error" in result:
                messagebox.showerror("Setup", f"The installation did not finish:\n{result['error']}",
                                     parent=self.root)
                self.page_options()
            else:
                self.page_done(result["exe"])
        t = threading.Thread(target=work, daemon=True)
        t.start()
        wait()

    def page_done(self, exe):
        self._clear()
        where = "the Start menu (under OM Industries Inc)" if self.start_menu.get() else "its folder"
        self._title("All done", f"{om_install.APP_NAME} is installed. You can open it from {where}. To remove "
                                "it, use 'Uninstall OM Disk Analyzer' there or Windows Settings > Apps.")
        tk.Checkbutton(self.body, text=f"Open {om_install.APP_NAME} now", variable=self.launch, bg="white",
                       activebackground="white", font=("Segoe UI", 9)).pack(anchor="w")

        def finish():
            if self.launch.get():
                subprocess.Popen([exe], close_fds=True)
            self.root.destroy()
        bar = tk.Frame(self.body, bg="white")
        bar.pack(side="bottom", fill="x")
        ttk.Button(bar, text="Finish", command=finish).pack(side="right")


def main(argv):
    payload = payload_path(argv)
    if "--silent" in argv:
        target = argv[argv.index("--target") + 1] if "--target" in argv else om_install.default_dir()
        om_install.install(payload, target, VERSION, start_menu="--no-start-menu" not in argv,
                           desktop="--desktop" in argv, register="--no-register" not in argv, log=lambda t: None)
        return 0
    if not os.path.exists(payload):
        messagebox.showerror("Setup", "The program to install is missing from this setup file.")
        return 1
    try:
        import ctypes
        ctypes.windll.shcore.SetProcessDpiAwareness(1)
    except Exception:
        pass
    shot = argv[argv.index("--screenshot") + 1] if "--screenshot" in argv else None
    root = tk.Tk()
    if shot:
        from omdisk_app import show_without_focus
        root.withdraw()
        Setup(root, payload, shot)
        show_without_focus(root)
        frames = [0]

        def tick():
            frames[0] += 1
            if frames[0] >= 120:
                root.update()
                capture_window(int(root.wm_frame(), 16), os.path.abspath(shot))
                root.destroy()
                return
            root.after(16, tick)
        root.after(16, tick)
    else:
        Setup(root, payload)
    root.mainloop()
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
