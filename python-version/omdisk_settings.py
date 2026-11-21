"""Settings, the Options window and the File Explorer context-menu hook (OM Disk Analyzer)."""
import copy
import json
import os
import struct
import sys
import tkinter as tk
from tkinter import ttk, colorchooser, filedialog, messagebox

from omdisk import APP_NAME, COMPANY, EDITION, IS_WIN, NAVY, BG, MUTED, PANEL, make_buckets, png_bytes

TAB_NAMES = ("Chart", "Details", "Extensions", "Users", "Age of Files", "Top 100 Files", "History", "Treemap")
DETAIL_COLUMNS = [("name", "Name"), ("size", "Size"), ("alloc", "Allocated"), ("files", "Files"),
                  ("dirs", "Folders"), ("pct", "% of Parent"), ("mtime", "Last Change"), ("atime", "Last Access"),
                  ("owner", "Owner")]
EXPORT_COLUMNS = [("name", "Name"), ("path", "Full Path"), ("size", "Size"), ("alloc", "Allocated"),
                  ("files", "Files"), ("dirs", "Folders"), ("pct", "% of Parent"), ("mtime", "Last Change"),
                  ("atime", "Last Access"), ("owner", "Owner (slower)"), ("level", "Level")]
BOLD_MODES = ["% of the scanned folder", "% of the parent folder", "MB"]
PRIORITIES = ["Idle", "Lower priority", "Normal priority", "Higher priority"]
CLUSTERS = {"Original": 0, "512 Bytes": 512, "4 KB": 4096, "8 KB": 8192, "16 KB": 16384, "32 KB": 32768,
            "64 KB": 65536}
STARTUP_SCANS = ["Nothing", "Last scanned folder", "Drive C:\\", "This folder:"]
AGE_BASES = ["Last change", "Last access", "Creation date"]

DEFAULTS = {
    # View
    "long_tips": True, "compressed_color": True, "show_time": False, "gradient": True,
    "grad_start": "#ffff70", "grad_end": "#ff7a00", "recent_count": 12, "bold_value": 15,
    "bold_mode": BOLD_MODES[0],
    # Scan
    "follow_links": False, "show_errors": False, "ext_stats": True, "owner_stats": False,
    "thread_priority": "Lower priority", "cluster": "Original",
    # Include / exclude
    "exclude_dirs": [], "include_files": [],
    # Columns / export
    "details_cols": [k for k, _ in DETAIL_COLUMNS],
    "export_cols": ["path", "size", "alloc", "files", "dirs", "pct", "mtime", "atime"],
    "export_scope": "full", "export_header": True, "export_units": True, "export_bytes": False,
    "export_file": "",
    # Startup
    "startup_scan": "Nothing", "startup_folder": "", "startup_tab": "Details", "expand_levels": 1,
    # Shell extensions
    "shell_menu": False,
    # Age of files
    "age_basis": "Last change", "age_days": [30, 90, 180, 365, 730, 1825],
    # remembered state
    "recent": [], "last_path": "",
}


def program_dir():
    return os.path.dirname(sys.executable) if getattr(sys, "frozen", False) else \
        os.path.dirname(os.path.abspath(__file__))


def is_portable():
    """Portable copy: a 'portable.ini' next to the program keeps all settings in a 'Data' folder beside it."""
    return os.path.exists(os.path.join(program_dir(), "portable.ini"))


def data_dir():
    if is_portable():
        return os.path.join(program_dir(), "Data")
    if IS_WIN:
        base = os.environ.get("APPDATA") or os.path.expanduser("~")
    elif sys.platform == "darwin":
        base = os.path.expanduser("~/Library/Application Support")
    else:
        base = os.environ.get("XDG_CONFIG_HOME") or os.path.expanduser("~/.config")
    return os.path.join(base, COMPANY, APP_NAME)


def settings_file():
    return os.path.join(data_dir(), "settings.json")


def load_settings():
    s = copy.deepcopy(DEFAULTS)
    try:
        with open(settings_file(), encoding="utf-8") as f:
            saved = json.load(f)
        s.update({k: v for k, v in saved.items() if k in DEFAULTS and type(v) is type(DEFAULTS[k])})
    except (OSError, ValueError, AttributeError):
        pass
    return s


def save_settings(s):
    try:
        os.makedirs(data_dir(), exist_ok=True)
        with open(settings_file(), "w", encoding="utf-8") as f:
            json.dump(s, f, indent=1)
    except OSError:
        pass


def cluster_bytes(label, original):
    return CLUSTERS.get(label, 0) or original


# ------------------------------------------------------- explorer menu hook --
SHELL_KEYS = ((r"Software\Classes\Directory\shell\OMDiskAnalyzer", "%1"),
              (r"Software\Classes\Drive\shell\OMDiskAnalyzer", "%1"),
              (r"Software\Classes\Directory\Background\shell\OMDiskAnalyzer", "%V"))
MENU_TEXT = "Analyze with OM Disk Analyzer"


def launch_command(arg):
    """Command line Explorer runs: this program, opened on the clicked folder."""
    if getattr(sys, "frozen", False):
        return f'"{sys.executable}" "{arg}"'
    exe = sys.executable
    pyw = os.path.join(os.path.dirname(exe), "pythonw.exe")
    if os.path.exists(pyw):
        exe = pyw
    script = os.path.join(os.path.dirname(os.path.abspath(__file__)), "main.py")
    return f'"{exe}" "{script}" "{arg}"'


def logo_rgb(n=32):
    navy, orange, white = (0x14, 0x28, 0x4B), (0xF2, 0x8C, 0x28), (255, 255, 255)
    px = bytearray()
    for y in range(n):
        for x in range(n):
            c = navy
            if 4 <= x < 28 and 4 <= y < 28:
                c = orange
            if 8 <= y < 24 and (8 <= x < 15 or 17 <= x < 24):
                c = navy
            if 10 <= y < 22 and (10 <= x < 13 or 19 <= x < 22):
                c = white
            px += bytes(c)
    return px


def icon_file():
    """The OM Industries .ico for the menu entry (from the logo pack; a drawn one if it is missing)."""
    if getattr(sys, "frozen", False):
        return f"{sys.executable},0"  # the packaged program carries the OM icon itself
    pack = os.path.join(os.path.dirname(os.path.abspath(__file__)), "assets", "om.ico")
    if os.path.exists(pack):
        return pack
    path = os.path.join(data_dir(), "om-disk-analyzer.ico")
    png = png_bytes(32, 32, logo_rgb(32))
    os.makedirs(data_dir(), exist_ok=True)
    with open(path, "wb") as f:
        f.write(struct.pack("<HHH", 0, 1, 1) + struct.pack("<BBBBHHII", 32, 32, 0, 0, 1, 32, len(png), 22) + png)
    return path


def shell_menu_installed():
    if not IS_WIN:
        return False
    import winreg
    try:
        winreg.CloseKey(winreg.OpenKey(winreg.HKEY_CURRENT_USER, SHELL_KEYS[0][0] + r"\command"))
        return True
    except OSError:
        return False


def set_shell_menu(enable):
    """Add or remove 'Analyze with OM Disk Analyzer' on folders, drives and folder backgrounds
    for the current user only (no administrator rights needed)."""
    if not IS_WIN:
        raise OSError("The Explorer menu is only available on Windows.")
    import winreg
    hk = winreg.HKEY_CURRENT_USER
    icon = icon_file() if enable else ""
    for key, arg in SHELL_KEYS:
        if enable:
            with winreg.CreateKey(hk, key) as k:
                winreg.SetValueEx(k, "", 0, winreg.REG_SZ, MENU_TEXT)
                winreg.SetValueEx(k, "Icon", 0, winreg.REG_SZ, icon)
            with winreg.CreateKey(hk, key + r"\command") as k:
                winreg.SetValueEx(k, "", 0, winreg.REG_SZ, launch_command(arg))
        else:
            for sub in (key + r"\command", key):
                try:
                    winreg.DeleteKey(hk, sub)
                except FileNotFoundError:
                    pass
    try:
        import ctypes
        ctypes.windll.shell32.SHChangeNotify(0x08000000, 0, None, None)  # SHCNE_ASSOCCHANGED
    except Exception:
        pass


# ------------------------------------------------------------ options window --
class OptionsDialog(tk.Toplevel):
    PAGES = ("View", "Scan", "Include / Exclude", "Columns / Export", "Startup", "Shell Extensions", "Age of Files")

    def __init__(self, app, page=0):
        super().__init__(app.root)
        self.app = app
        self.k = app.k
        self.s = copy.deepcopy(app.settings)
        self.v = {}
        self.title(f"{APP_NAME} {EDITION} - Options")
        self.transient(app.root)
        self.configure(bg=BG)
        self.resizable(False, False)
        style = ttk.Style(self)
        style.configure("Opt.TNotebook.Tab", padding=(9, 3), font=("Segoe UI", 9))
        style.configure("Opt.TCheckbutton", background=PANEL)
        style.configure("Opt.TRadiobutton", background=PANEL)
        style.configure("Opt.TLabel", background=PANEL)
        style.configure("OptMuted.TLabel", background=PANEL, foreground=MUTED)
        style.configure("Opt.TLabelframe", background=PANEL)
        style.configure("Opt.TLabelframe.Label", background=PANEL, foreground=NAVY)
        self.nb = ttk.Notebook(self, style="Opt.TNotebook")
        self.nb.pack(fill="both", expand=True, padx=8, pady=(8, 4))
        for build in (self._page_view, self._page_scan, self._page_filter, self._page_columns, self._page_startup,
                      self._page_shell, self._page_age):
            build()
        bar = ttk.Frame(self, padding=(8, 6))
        bar.pack(fill="x")
        ttk.Button(bar, text="OK", width=12, command=self.ok).pack(side="left")
        ttk.Button(bar, text="Help  (F1)", width=12, command=self.help).pack(side="right")
        ttk.Button(bar, text="Cancel", width=12, command=self.destroy).pack(side="right", padx=8)
        self.bind("<Escape>", lambda e: self.destroy())
        self.bind("<F1>", lambda e: self.help())
        self.nb.select(page)
        k = self.k
        self.geometry(f"{int(560 * k)}x{int(640 * k)}+{app.root.winfo_rootx() + int(120 * k)}"
                      f"+{app.root.winfo_rooty() + int(40 * k)}")
        if not app.screenshot:
            self.grab_set()
            self.focus_set()

    # ------------------------------------------------------------ helpers --
    def _page(self, title):
        f = tk.Frame(self.nb, bg=PANEL, padx=int(14 * self.k), pady=int(10 * self.k))
        self.nb.add(f, text=title)
        return f

    def _bool(self, parent, key, text, pady=4):
        var = tk.BooleanVar(value=bool(self.s[key]))
        self.v[key] = var
        ttk.Checkbutton(parent, text=text, variable=var, style="Opt.TCheckbutton").pack(anchor="w", pady=pady)
        return var

    def _var(self, key, cls=tk.StringVar):
        var = cls(value=self.s[key])
        self.v[key] = var
        return var

    def _group(self, parent, title):
        g = ttk.LabelFrame(parent, text=title, style="Opt.TLabelframe", padding=(10, 6))
        g.pack(fill="x", pady=(4, 8))
        return g

    @staticmethod
    def _sep(parent):
        ttk.Separator(parent).pack(fill="x", pady=10)

    # -------------------------------------------------------------- pages --
    def _page_view(self):
        p = self._page("View")
        self._bool(p, "compressed_color", "Different colour for compressed files and folders")
        self._bool(p, "long_tips", "Show long tooltips in the directory tree")
        self._bool(p, "show_time", "Show time with date values")
        self._bool(p, "gradient", "Show gradient bars as size indicators in the directory tree")
        row = tk.Frame(p, bg=PANEL)
        row.pack(anchor="w", padx=(22, 0), pady=(2, 6))
        ttk.Label(row, text="Gradient colours:", style="Opt.TLabel").pack(side="left", padx=(0, 8))
        self._c0 = tk.Button(row, width=3, relief="groove", bg=self.s["grad_start"],
                             command=lambda: self._pick("grad_start"))
        self._c0.pack(side="left")
        self._grad = tk.Canvas(row, width=int(170 * self.k), height=int(20 * self.k), bg=PANEL,
                               highlightthickness=0)
        self._grad.pack(side="left", padx=8)
        self._c1 = tk.Button(row, width=3, relief="groove", bg=self.s["grad_end"],
                             command=lambda: self._pick("grad_end"))
        self._c1.pack(side="left")
        self._draw_grad()
        self._sep(p)
        row = tk.Frame(p, bg=PANEL)
        row.pack(anchor="w")
        ttk.Spinbox(row, from_=0, to=50, width=5, textvariable=self._var("recent_count", tk.IntVar)).pack(side="left")
        ttk.Label(row, text="  Number of folders in the recently used list", style="Opt.TLabel").pack(side="left")
        self._sep(p)
        ttk.Label(p, text="Show folders in bold that are larger than", style="Opt.TLabel").pack(anchor="w")
        row = tk.Frame(p, bg=PANEL)
        row.pack(anchor="w", pady=4)
        ttk.Spinbox(row, from_=0, to=100000, width=7, textvariable=self._var("bold_value", tk.IntVar)).pack(side="left")
        ttk.Combobox(row, values=BOLD_MODES, state="readonly", width=26,
                     textvariable=self._var("bold_mode")).pack(side="left", padx=6)

    def _pick(self, key):
        rgb, hx = colorchooser.askcolor(color=self.s[key], parent=self, title="Choose a bar colour")
        if hx:
            self.s[key] = hx
            (self._c0 if key == "grad_start" else self._c1).configure(bg=hx)
            self._draw_grad()

    def _draw_grad(self):
        c = self._grad
        c.delete("all")
        w, h = int(c.cget("width")), int(c.cget("height"))
        a = [int(self.s["grad_start"][i:i + 2], 16) for i in (1, 3, 5)]
        b = [int(self.s["grad_end"][i:i + 2], 16) for i in (1, 3, 5)]
        for x in range(0, w, 2):
            f = x / max(1, w - 1)
            col = "#%02x%02x%02x" % tuple(int(p + (q - p) * f) for p, q in zip(a, b))
            c.create_rectangle(x, 0, x + 2, h, fill=col, outline="")

    def _page_scan(self):
        p = self._page("Scan")
        g = self._group(p, "Scan options")
        self._bool(g, "follow_links", "Follow mount points, junctions and symbolic links")
        self._bool(g, "show_errors", "Show error messages after a scan")
        self._bool(g, "ext_stats", "Create statistics on file extensions")
        self._bool(g, "owner_stats", "Create statistics on file owners (slower)")
        g = self._group(p, "Thread options")
        row = tk.Frame(g, bg=PANEL)
        row.pack(anchor="w", pady=4)
        ttk.Label(row, text="Thread priority:", style="Opt.TLabel", width=18).pack(side="left")
        ttk.Combobox(row, values=PRIORITIES, state="readonly", width=24,
                     textvariable=self._var("thread_priority")).pack(side="left")
        g = self._group(p, "User defined cluster size")
        ttk.Combobox(g, values=list(CLUSTERS), state="readonly", width=34,
                     textvariable=self._var("cluster")).pack(anchor="w", pady=4)
        ttk.Label(g, text="Used to estimate the allocated space of every file.", style="OptMuted.TLabel").pack(
            anchor="w")
        ttk.Label(p, text="Scan options take effect from the next scan.", style="OptMuted.TLabel").pack(
            anchor="w", pady=6)

    def _list_editor(self, parent, key, title, hint):
        g = self._group(parent, title)
        ttk.Label(g, text=hint, style="OptMuted.TLabel", wraplength=int(460 * self.k)).pack(anchor="w")
        row = tk.Frame(g, bg=PANEL)
        row.pack(fill="x", pady=4)
        entry = ttk.Entry(row)
        entry.pack(side="left", fill="x", expand=True)
        lb = tk.Listbox(g, height=6, font=("Segoe UI", 9), activestyle="none", relief="solid", bd=1)
        for item in self.s[key]:
            lb.insert("end", item)

        def add(_e=None):
            for part in entry.get().replace(";", ",").split(","):
                part = part.strip()
                if part and part not in lb.get(0, "end"):
                    lb.insert("end", part)
            entry.delete(0, "end")

        def remove():
            for i in reversed(lb.curselection()):
                lb.delete(i)
        ttk.Button(row, text="Add", command=add).pack(side="left", padx=4)
        ttk.Button(row, text="Remove", command=remove).pack(side="left")
        entry.bind("<Return>", add)
        lb.pack(fill="x")
        self.v[key] = lb

    def _page_filter(self):
        p = self._page("Include / Exclude")
        self._list_editor(p, "exclude_dirs", "Exclude these folders",
                          "Folder names or full paths; * and ? work as wildcards (for example node_modules, "
                          "*\\Temp, C:\\Windows\\WinSxS).")
        self._list_editor(p, "include_files", "Only count files that match",
                          "Leave empty to count every file. For example *.mp4, *.mkv or report*.")
        ttk.Label(p, text="These filters take effect from the next scan.", style="OptMuted.TLabel").pack(anchor="w")

    def _page_columns(self):
        p = self._page("Columns / Export")
        row = tk.Frame(p, bg=PANEL)
        row.pack(fill="x", pady=(0, 6))
        ttk.Label(row, text="Visible columns of:", style="Opt.TLabel").pack(side="left", padx=(0, 10))
        self._col_target = tk.StringVar(value="Details list")
        box = ttk.Combobox(row, values=["Details list", "CSV export"], state="readonly", width=30,
                           textvariable=self._col_target)
        box.pack(side="left", fill="x", expand=True)
        self._col_frame = tk.Frame(p, bg=PANEL, relief="solid", bd=1, padx=8, pady=4)
        self._col_frame.pack(fill="x")
        self._col_vars = {
            "details_cols": {k: tk.BooleanVar(value=k in self.s["details_cols"]) for k, _ in DETAIL_COLUMNS},
            "export_cols": {k: tk.BooleanVar(value=k in self.s["export_cols"]) for k, _ in EXPORT_COLUMNS}}
        box.bind("<<ComboboxSelected>>", lambda e: self._show_cols())
        self._show_cols()
        g = tk.Frame(p, bg=PANEL)
        g.pack(fill="x", pady=(10, 0))
        scope = self._var("export_scope")
        r = tk.Frame(g, bg=PANEL)
        r.pack(fill="x", pady=2)
        ttk.Label(r, text="Export:", style="Opt.TLabel", width=12, font=("Segoe UI", 9, "bold")).pack(side="left")
        ttk.Radiobutton(r, text="Full branch", value="full", variable=scope, style="Opt.TRadiobutton").pack(
            side="left")
        ttk.Radiobutton(r, text="Expanded folders only", value="expanded", variable=scope,
                        style="Opt.TRadiobutton").pack(side="left", padx=20)
        r = tk.Frame(g, bg=PANEL)
        r.pack(fill="x", pady=2)
        ttk.Label(r, text="Header:", style="Opt.TLabel", width=12, font=("Segoe UI", 9, "bold")).pack(side="left")
        self._bool(r, "export_header", "Include in export", pady=0)
        r = tk.Frame(g, bg=PANEL)
        r.pack(fill="x", pady=2)
        ttk.Label(r, text="Units:", style="Opt.TLabel", width=12, font=("Segoe UI", 9, "bold")).pack(side="left")
        units = tk.BooleanVar(value=self.s["export_units"])
        self.v["export_units"] = units
        ttk.Checkbutton(r, text="Include in export", variable=units, style="Opt.TCheckbutton").pack(side="left")
        raw = tk.BooleanVar(value=self.s["export_bytes"])
        self.v["export_bytes"] = raw
        ttk.Checkbutton(r, text="Always use byte values", variable=raw, style="Opt.TCheckbutton").pack(
            side="left", padx=20)
        r = tk.Frame(g, bg=PANEL)
        r.pack(fill="x", pady=(8, 2))
        ttk.Label(r, text="Default export file:", style="Opt.TLabel", font=("Segoe UI", 9, "bold")).pack(side="left")
        fvar = self._var("export_file")
        ttk.Entry(r, textvariable=fvar).pack(side="left", fill="x", expand=True, padx=6)
        ttk.Button(r, text="...", width=3, command=lambda: fvar.set(filedialog.asksaveasfilename(
            parent=self, defaultextension=".csv", filetypes=[("CSV file", "*.csv")]) or fvar.get())).pack(side="left")

    def _show_cols(self):
        for w in self._col_frame.winfo_children():
            w.destroy()
        key = "details_cols" if self._col_target.get() == "Details list" else "export_cols"
        cols = DETAIL_COLUMNS if key == "details_cols" else EXPORT_COLUMNS
        for k, label in cols:
            ttk.Checkbutton(self._col_frame, text=label, variable=self._col_vars[key][k],
                            style="Opt.TCheckbutton").pack(anchor="w", pady=1)

    def _page_startup(self):
        p = self._page("Startup")
        g = self._group(p, "When the program starts")
        row = tk.Frame(g, bg=PANEL)
        row.pack(fill="x", pady=4)
        ttk.Label(row, text="Scan:", style="Opt.TLabel", width=12).pack(side="left")
        ttk.Combobox(row, values=STARTUP_SCANS, state="readonly", width=24,
                     textvariable=self._var("startup_scan")).pack(side="left")
        row = tk.Frame(g, bg=PANEL)
        row.pack(fill="x", pady=4)
        ttk.Label(row, text="Folder:", style="Opt.TLabel", width=12).pack(side="left")
        fvar = self._var("startup_folder")
        ttk.Entry(row, textvariable=fvar).pack(side="left", fill="x", expand=True)
        ttk.Button(row, text="...", width=3, command=lambda: fvar.set(
            os.path.normpath(filedialog.askdirectory(parent=self) or fvar.get() or "."))).pack(side="left", padx=4)
        ttk.Label(g, text='Used when "This folder:" is chosen above. A folder opened from the Explorer '
                          "menu is always scanned.", style="OptMuted.TLabel",
                  wraplength=int(460 * self.k)).pack(anchor="w")
        row = tk.Frame(g, bg=PANEL)
        row.pack(fill="x", pady=(10, 4))
        ttk.Label(row, text="Open tab:", style="Opt.TLabel", width=12).pack(side="left")
        ttk.Combobox(row, values=TAB_NAMES, state="readonly", width=24,
                     textvariable=self._var("startup_tab")).pack(side="left")
        g = self._group(p, "After a scan")
        row = tk.Frame(g, bg=PANEL)
        row.pack(fill="x", pady=4)
        ttk.Label(row, text="Expand the directory tree to level", style="Opt.TLabel").pack(side="left")
        ttk.Spinbox(row, from_=1, to=6, width=4, textvariable=self._var("expand_levels", tk.IntVar)).pack(
            side="left", padx=6)

    def _page_shell(self):
        p = self._page("Shell Extensions")
        g = self._group(p, "Explorer context menu")
        installed = shell_menu_installed()
        self.s["shell_menu"] = installed
        self._bool(g, "shell_menu", f"Show {APP_NAME} in the Explorer context menu of the current user")
        if not IS_WIN:
            g.winfo_children()[-1].state(["disabled"])
            ttk.Label(g, text="The File Explorer menu is only available on Windows.",
                      style="OptMuted.TLabel").pack(anchor="w")
        ttk.Label(g, text=f'Right-click any folder, drive or the empty space inside a folder in File Explorer and '
                          f'choose "{MENU_TEXT}" to open it here. On Windows 11 the entry is under '
                          '"Show more options". Only your own user account is changed; no administrator rights '
                          "are needed.", style="OptMuted.TLabel", wraplength=int(470 * self.k),
                  justify="left").pack(anchor="w", pady=(4, 6))
        self._shell_state = ttk.Label(g, style="Opt.TLabel",
                                      text="Status: " + ("installed" if installed else "not installed"))
        self._shell_state.pack(anchor="w")

    def _page_age(self):
        p = self._page("Age of Files")
        g = self._group(p, "Group files by")
        ttk.Combobox(g, values=AGE_BASES, state="readonly", width=24,
                     textvariable=self._var("age_basis")).pack(anchor="w", pady=4)
        g = self._group(p, "Age intervals (in days)")
        row = tk.Frame(g, bg=PANEL)
        row.pack(anchor="w", pady=4)
        self._age_vars = []
        days = list(self.s["age_days"]) + [""] * (6 - len(self.s["age_days"]))
        for d in days[:6]:
            var = tk.StringVar(value=str(d))
            ttk.Spinbox(row, from_=1, to=36500, width=6, textvariable=var).pack(side="left", padx=(0, 6))
            var.trace_add("write", lambda *_: self._age_preview())
            self._age_vars.append(var)
        self._age_text = ttk.Label(g, style="OptMuted.TLabel", justify="left", wraplength=int(470 * self.k))
        self._age_text.pack(anchor="w", pady=(6, 0))
        self._age_preview()
        ttk.Label(p, text="Takes effect from the next scan.", style="OptMuted.TLabel").pack(anchor="w")

    def _age_days(self):
        out = []
        for v in self._age_vars:
            try:
                d = int(v.get())
                if d > 0:
                    out.append(d)
            except ValueError:
                pass
        return sorted(set(out))

    def _age_preview(self):
        self._age_text.configure(text="Groups:  " + "  ·  ".join(label for _, label in make_buckets(self._age_days())))

    # ------------------------------------------------------------ buttons --
    def ok(self):
        s = self.s
        for key, var in self.v.items():
            if isinstance(var, tk.Listbox):
                s[key] = list(var.get(0, "end"))
                continue
            try:
                val = var.get()
            except (tk.TclError, ValueError):
                continue
            s[key] = val
        for key in ("details_cols", "export_cols"):
            cols = DETAIL_COLUMNS if key == "details_cols" else EXPORT_COLUMNS
            chosen = [k for k, _ in cols if self._col_vars[key][k].get()]
            s[key] = chosen or [cols[0][0]]
        s["age_days"] = self._age_days() or list(DEFAULTS["age_days"])
        s["recent_count"] = max(0, min(50, int(s["recent_count"])))
        want, have = bool(s["shell_menu"]), shell_menu_installed()
        if want != have:
            try:
                set_shell_menu(want)
            except OSError as ex:
                messagebox.showerror(APP_NAME, f"The Explorer menu could not be changed:\n{ex}", parent=self)
                s["shell_menu"] = have
        self.app.apply_settings(s)
        self.destroy()

    def help(self):
        messagebox.showinfo(f"{APP_NAME} - Options", (
            "View: colours, tooltips, dates and which folders are shown in bold.\n"
            "Scan: what a scan follows and counts, its speed and the cluster size.\n"
            "Include / Exclude: folders to skip and which files to count.\n"
            "Columns / Export: the columns of the Details list and of CSV exports.\n"
            "Startup: what is scanned and shown when the program opens.\n"
            "Shell Extensions: adds the program to the right-click menu of File Explorer.\n"
            "Age of Files: how files are grouped on the Age of Files tab."), parent=self)
