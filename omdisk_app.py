"""Main window of OM Disk Analyzer Professional (OM Industries Inc)."""
from omdisk import *  # noqa: F401,F403  (shared helpers, scanner, tree widget)
from omdisk import _win32
from omdisk_settings import (DETAIL_COLUMNS, EXPORT_COLUMNS, TAB_NAMES, OptionsDialog, cluster_bytes, data_dir,
                             is_portable, load_settings, save_settings, set_shell_menu)

IS_MAC = sys.platform == "darwin"


class App:
    TAB_NAMES = ("Chart", "Details", "Extensions", "Users", "Age of Files", "Top 100 Files", "History", "Treemap")

    def __init__(self, root, start_path=None, screenshot=None):
        self.root = root
        self.screenshot = screenshot
        self.scanner = None
        self.tree_root = None
        self.sel_node = None
        self.sel_files = False
        self.settings = load_settings()
        self.capture_target = None
        self.unit = tk.StringVar(value="MB")
        self.mode = tk.StringVar(value="size")
        self.sort_key = tk.StringVar(value="size")
        self.path_var = tk.StringVar(value=start_path or ("C:\\" if IS_WIN else os.path.expanduser("~")))
        self.vol = (4096, "", "", "Disk")
        self._redraw_job = None
        self._tip_win = None
        self._map_items, self._pie_items = {}, {}
        self._details_paths, self._top_paths = {}, {}
        self.frames = self.ready_frames = 0

        f = max(1.0, root.winfo_fpixels("1i") / 96.0)
        self.k = f
        w = min(int(1620 * f), int(root.winfo_screenwidth() * 0.96))
        h = min(int(690 * f), int(root.winfo_screenheight() * 0.88))
        root.geometry(f"{w}x{h}")
        root.minsize(int(900 * min(f, 1.25)), int(560 * min(f, 1.25)))
        root.configure(bg=BG)
        self._icon()
        self._style()
        self._menu()
        self._toolbar()
        self._infobar()
        self._statusbar()
        self._body()
        self._bind_keys()
        self._update_title()
        self._apply_view_settings()
        st = self.settings
        if st["startup_tab"] in TAB_NAMES:
            self.nb.select(TAB_NAMES.index(st["startup_tab"]))
        if not start_path and not screenshot:
            choice = st["startup_scan"]
            if choice == "Last scanned folder" and st["last_path"]:
                start_path = st["last_path"]
            elif choice == "Drive C:\\":
                start_path = "C:\\"
            elif choice == "This folder:" and st["startup_folder"]:
                start_path = st["startup_folder"]
            if start_path and os.path.isdir(start_path):
                self.path_var.set(start_path)
            else:
                start_path = None
        if start_path:
            root.after(50, self.start_scan)

    # ---------------------------------------------------------- settings --
    def fdate(self, t):
        return fmt_time(t) if self.settings.get("show_time") else fmt_date(t)

    def _apply_view_settings(self):
        cols = [c for c in self.settings["details_cols"] if c in dict(DETAIL_COLUMNS)] or ["name"]
        self.details.configure(displaycolumns=cols)
        recent = self.settings["recent"][:self.settings["recent_count"]]
        self.path_box.configure(values=list_drives() + [p for p in recent if p not in list_drives()])

    def options(self, page=0):
        return OptionsDialog(self, page)

    def apply_settings(self, new):
        self.settings = new
        self.settings["recent"] = self.settings["recent"][:self.settings["recent_count"]]
        if not self.screenshot:
            save_settings(self.settings)
        self._apply_view_settings()
        self.refresh_all()
        self.st_msg.set("Options saved. Scan options apply from the next scan.")

    def _remember_path(self, path):
        st = self.settings
        st["last_path"] = path
        st["recent"] = ([path] + [p for p in st["recent"] if p != path])[:st["recent_count"]]
        if not self.screenshot:
            save_settings(st)
        self._apply_view_settings()

    # ------------------------------------------------------------ look --
    def _icon(self):
        icons = [im for im in (brand_image("icon", s) for s in (256, 64, 48, 32, 16)) if im is not None]
        if icons:
            self._icon_imgs = icons
            self.root.iconphoto(True, *icons)
            return
        img = tk.PhotoImage(width=32, height=32)
        img.put(NAVY, to=(0, 0, 32, 32))
        img.put(ORANGE, to=(4, 4, 28, 28))
        img.put(NAVY, to=(8, 8, 15, 24))
        img.put(NAVY, to=(17, 8, 24, 24))
        img.put("white", to=(10, 10, 13, 22))
        img.put("white", to=(19, 10, 22, 22))
        self._icon_img = img
        self.root.iconphoto(True, img)

    def _style(self):
        s = ttk.Style(self.root)
        s.theme_use("clam")
        s.configure(".", font=("Segoe UI", 9), background=BG, foreground=TEXT)
        s.configure("TFrame", background=BG)
        s.configure("Panel.TFrame", background=PANEL)
        s.configure("TButton", padding=(8, 3), background=BG)
        s.configure("Toolbutton", padding=(5, 2), background=BG, font=("Segoe UI", 9))
        s.map("Toolbutton", background=[("selected", "#C5DDF7"), ("active", "#E1ECF8")],
              relief=[("selected", "sunken")])
        s.configure("Unit.Toolbutton", font=("Segoe UI", 8, "bold"))
        s.configure("Treeview", background=PANEL, fieldbackground=PANEL, rowheight=int(21 * self.k),
                    bordercolor=LINE, font=("Segoe UI", 10))
        s.map("Treeview", background=[("selected", SELECT)], foreground=[("selected", TEXT)])
        s.configure("Treeview.Heading", background=PANEL, font=("Segoe UI", 10), padding=(4, 2), relief="flat")
        s.map("Treeview.Heading", background=[("active", "#E5F1FB")])
        s.configure("TNotebook", background=BG, borderwidth=0, tabmargins=(2, 2, 0, 0))
        s.configure("TNotebook.Tab", padding=(8, 3), background=BG, font=("Segoe UI", 11))
        s.map("TNotebook.Tab", background=[("selected", PANEL)])
        s.configure("Horizontal.TProgressbar", background=GOLD, troughcolor="#DDD", bordercolor=LINE)
        s.configure("Status.TLabel", background=BG, padding=(6, 2), font=("Segoe UI", 10))

    def _menu(self):
        m = tk.Menu(self.root)
        f = tk.Menu(m, tearoff=False)
        f.add_command(label="Select directory...", accelerator="Ctrl+O", command=self.browse)
        f.add_command(label="Scan / Refresh", accelerator="F5", command=self.start_scan)
        f.add_command(label="Stop scan", accelerator="Esc", command=self.stop_scan)
        f.add_separator()
        f.add_command(label="Export to CSV...", accelerator="Ctrl+E", command=self.export_csv)
        f.add_separator()
        f.add_command(label="Exit", command=self.root.destroy)
        m.add_cascade(label="File", menu=f)
        b = tk.Menu(m, tearoff=False)
        b.add_command(label="Up one level", accelerator="Backspace", command=self.go_up)
        b.add_command(label="Expand two levels", accelerator="Ctrl+Plus", command=lambda: self.expand(2))
        b.add_command(label="Expand all below", command=lambda: self.expand(64))
        b.add_command(label="Collapse all", accelerator="Ctrl+Minus", command=self.collapse)
        b.add_separator()
        b.add_command(label="Open", command=lambda: self.sel_node and self._open_path(self.sel_node.path))
        b.add_command(label="Show in Explorer", command=lambda: self.sel_node and self._explore(self.sel_node.path))
        b.add_command(label="Copy path", command=lambda: self.sel_node and self._copy(self.sel_node.path))
        b.add_command(label="Scan this branch", command=lambda: self.sel_node and self.start_scan(self.sel_node.path))
        m.add_cascade(label="Branch", menu=b)
        v = tk.Menu(m, tearoff=False)
        for key, label in MODES.items():
            v.add_radiobutton(label=label, value=key, variable=self.mode, command=self.refresh_all)
        v.add_separator()
        for u in ("Auto", "KB", "MB", "GB"):
            v.add_radiobutton(label=f"Unit: {u}", value=u, variable=self.unit, command=self.refresh_all)
        v.add_separator()
        for i, name in enumerate(self.TAB_NAMES):
            v.add_command(label=name, command=lambda i=i: self.nb.select(i))
        m.add_cascade(label="View", menu=v)
        s = tk.Menu(m, tearoff=False)
        for key, label in (("size", "By size"), ("name", "By name"), ("files", "By file count"),
                           ("mtime", "By last change")):
            s.add_radiobutton(label=label, value=key, variable=self.sort_key, command=self.refresh_all)
        m.add_cascade(label="Sort", menu=s)
        fs = tk.Menu(m, tearoff=False)
        fs.add_command(label="Search files by name...", accelerator="Ctrl+F", command=self.search)
        fs.add_command(label="Largest files", command=lambda: self.nb.select(5))
        fs.add_command(label="Files by age", command=lambda: self.nb.select(4))
        m.add_cascade(label="File Search", menu=fs)
        t = tk.Menu(m, tearoff=False)
        t.add_command(label="Export to CSV...", command=self.export_csv)
        t.add_command(label="Clear scan history", command=self.clear_history)
        t.add_separator()
        t.add_command(label="Explorer context menu...", command=lambda: self.options(5))
        t.add_command(label="Options...", accelerator="Ctrl+Alt+O", command=self.options)
        m.add_cascade(label="Tools", menu=t)
        h = tk.Menu(m, tearoff=False)
        h.add_command(label=f"About {APP_NAME}", command=self.about)
        m.add_cascade(label="Help", menu=h)
        self.root.config(menu=m)

    def _toolbar(self):
        bar = tk.Frame(self.root, bg=BG, bd=1, relief="groove")
        bar.pack(fill="x", padx=2, pady=(2, 0))
        self.path_box = ttk.Combobox(bar, textvariable=self.path_var, width=46, font=("Segoe UI", 10),
                                     values=list_drives())
        self.path_box.pack(side="left", padx=(4, 6), pady=3)
        self.path_box.bind("<Return>", lambda e: self.start_scan())
        self.path_box.bind("<<ComboboxSelected>>", lambda e: self.start_scan())

        def tb(text, cmd, tip):
            b = ttk.Button(bar, text=text, style="Toolbutton", command=cmd)
            b.pack(side="left", padx=1)
            self._tip(b, tip)
            return b

        def sep():
            ttk.Separator(bar, orient="vertical").pack(side="left", fill="y", padx=5, pady=3)

        self.scan_btn = tb("⟳ Scan", self.start_scan, "Scan or refresh (F5)")
        self.stop_btn = tb("■", self.stop_scan, "Stop scanning (Esc)")
        self.stop_btn.state(["disabled"])
        tb("📂", self.browse, "Select directory (Ctrl+O)")
        tb("⇧", self.go_up, "Up one level")
        sep()
        tb("⊞", lambda: self.expand(2), "Expand two levels")
        tb("⊟", self.collapse, "Collapse all")
        tb("🔍", self.search, "Search files by name (Ctrl+F)")
        tb("💾", self.export_csv, "Export to CSV (Ctrl+E)")
        tb("⚙", self.options, "Options (Ctrl+Alt+O)")
        sep()
        for key, text, tip in (("files", "000", "Show file count"), ("size", "Size", "Show size"),
                               ("alloc", "Alloc", "Show allocated space"), ("percent", "%", "Show percent")):
            r = ttk.Radiobutton(bar, text=text, value=key, variable=self.mode, style="Toolbutton",
                                command=self.refresh_all)
            r.pack(side="left", padx=1)
            self._tip(r, tip)
        sep()
        for key, text in (("name", "A↓Z"), ("size", "9↓1")):
            r = ttk.Radiobutton(bar, text=text, value=key, variable=self.sort_key, style="Toolbutton",
                                command=self.refresh_all)
            r.pack(side="left", padx=1)
            self._tip(r, "Sort by name" if key == "name" else "Sort by size")
        sep()
        for u in ("KB", "MB", "GB", "Auto"):
            ttk.Radiobutton(bar, text=u, value=u, variable=self.unit, style="Unit.Toolbutton",
                            command=self.refresh_all).pack(side="left", padx=1)
        s = self.k
        logo = brand_image("icon", int(26 * s))
        if logo is not None:
            brand = tk.Frame(bar, bg=BG, cursor="hand2")
            brand.pack(side="right", padx=8)
            parts = [tk.Label(brand, image=logo, bg=BG, bd=0),
                     tk.Label(brand, text="O.M. INDUSTRIES", bg=BG, fg=NAVY, font=("Segoe UI", 11, "bold")),
                     tk.Label(brand, text="INC.", bg=BG, fg=ORANGE, font=("Segoe UI", 11, "bold"))]
            for i, w in enumerate(parts):
                w.pack(side="left", padx=(0, 6 if i == 0 else 3))
                w.bind("<Button-1>", lambda e: self.about())
            self._tip(parts[1], f"About {APP_NAME} - {COMPANY}")
        else:
            brand = tk.Canvas(bar, width=int(190 * s), height=int(26 * s), bg=BG, highlightthickness=0)
            brand.pack(side="right", padx=6)
            brand.create_rectangle(2, 3 * s, 24 * s, 23 * s, fill=ORANGE, outline="")
            brand.create_text(13 * s, 13 * s, text="OM", fill="white", font=("Segoe UI Black", 8))
            brand.create_text(30 * s, 13 * s, text=COMPANY, anchor="w", fill=NAVY, font=("Segoe UI Semibold", 10))

    def _tip(self, widget, text):
        def show(_e):
            hide(None)
            tw = tk.Toplevel(widget)
            tw.wm_overrideredirect(True)
            tw.geometry(f"+{widget.winfo_rootx()}+{widget.winfo_rooty() + widget.winfo_height() + 2}")
            tk.Label(tw, text=text, bg="#FFFFE1", relief="solid", bd=1, font=("Segoe UI", 9)).pack()
            self._tip_win = tw

        def hide(_e):
            if self._tip_win is not None:
                self._tip_win.destroy()
                self._tip_win = None
        widget.bind("<Enter>", show, add="+")
        widget.bind("<Leave>", hide, add="+")

    def _infobar(self):
        self.info = tk.Text(self.root, height=1, bd=1, relief="groove", bg=BG, font=("Segoe UI", 9),
                            padx=6, pady=2, cursor="arrow", wrap="none")
        self.info.tag_configure("b", font=("Segoe UI", 9, "bold"))
        self.info.pack(fill="x", padx=2)
        self._set_info([("", f"Welcome to {APP_NAME} {EDITION} by {COMPANY}.")])

    def _set_info(self, parts):
        self.info.configure(state="normal")
        self.info.delete("1.0", "end")
        for label, value in parts:
            if label:
                self.info.insert("end", label + " ", "b")
            self.info.insert("end", value + "      ")
        self.info.configure(state="disabled")

    def _statusbar(self):
        sb = tk.Frame(self.root, bg=BG)
        sb.pack(fill="x", side="bottom")
        self.st_free = tk.StringVar()
        self.st_files = tk.StringVar()
        self.st_cluster = tk.StringVar()
        self.st_msg = tk.StringVar(value="Ready")
        for var, wch in ((self.st_free, 30), (self.st_files, 14), (self.st_cluster, 30)):
            ttk.Label(sb, textvariable=var, style="Status.TLabel", width=wch, relief="sunken").pack(
                side="left", padx=1)
        ttk.Label(sb, text=f" © {COMPANY}", style="Status.TLabel", relief="sunken", compound="left",
                  image=brand_image("icon", int(16 * self.k)) or "").pack(side="right", padx=1)
        self.progress = ttk.Progressbar(sb, mode="indeterminate", length=int(120 * self.k))
        self._msg_label = ttk.Label(sb, textvariable=self.st_msg, style="Status.TLabel", relief="sunken")
        self._msg_label.pack(side="left", padx=1, fill="x", expand=True)

    def _body(self):
        pw = ttk.PanedWindow(self.root, orient="horizontal")
        pw.pack(fill="both", expand=True, padx=2, pady=2)
        left = ttk.PanedWindow(pw, orient="vertical")
        right = ttk.Frame(pw)
        pw.add(left, weight=3)
        pw.add(right, weight=5)
        tree_holder = tk.Frame(left, bd=1, relief="sunken", bg=PANEL)
        self.tree = SizeTree(tree_holder, self)
        self.tree.pack(fill="both", expand=True)
        left.add(tree_holder, weight=4)
        drives_holder = tk.Frame(left, bd=1, relief="sunken", bg=PANEL)
        self.drives = tk.Canvas(drives_holder, bg=PANEL, highlightthickness=0, height=int(140 * self.k))
        self.drives.pack(fill="both", expand=True)
        left.add(drives_holder, weight=1)
        self.drives.bind("<Configure>", lambda e: self._draw_drives())
        self.drives.bind("<Button-1>", self._drive_click)
        self._drive_rows = []
        self.drive_list = []
        self.root.after(30, self._load_drives)

        self.nb = ttk.Notebook(right)
        self.nb.pack(fill="both", expand=True)
        self.pie = tk.Canvas(self.nb, bg=PANEL, highlightthickness=0)
        self.nb.add(self.pie, text=" ◕ Chart ")
        self.details = self._make_list("☰ Details", [
            ("name", "Name", 190, "w"), ("size", "Size", 95, "e"), ("alloc", "Allocated", 95, "e"),
            ("files", "Files", 70, "e"), ("dirs", "Folders", 62, "e"), ("pct", "% of Parent", 92, "e"),
            ("mtime", "Last Change", 110, "w"), ("atime", "Last Access", 110, "w"), ("owner", "Owner", 110, "w")],
            toolbar=True)
        self.types = self._make_list("⁂ Extensions", [
            ("ext", "Extension", 90, "w"), ("kind", "Description", 110, "w"), ("size", "Size", 95, "e"),
            ("alloc", "Allocated", 95, "e"), ("pct", "% of Total", 90, "e"), ("count", "Files", 80, "e")])
        self.users = self._make_list("👥 Users", [
            ("user", "Owner", 200, "w"), ("size", "Size", 100, "e"), ("pct", "% of Total", 90, "e"),
            ("count", "Files", 80, "e")])
        self.ages = self._make_list("⌛ Age of Files", [
            ("age", "Last changed", 150, "w"), ("size", "Size", 100, "e"), ("pct", "% of Total", 90, "e"),
            ("count", "Files", 80, "e")])
        self.topf = self._make_list("⇩ Top 100 Files", [
            ("name", "Name", 200, "w"), ("size", "Size", 95, "e"), ("path", "Path", 330, "w"),
            ("mtime", "Last Change", 120, "w")])
        self.hist = self._make_list("🕘 History", [
            ("date", "Scan date", 130, "w"), ("path", "Path", 260, "w"), ("size", "Size", 100, "e"),
            ("alloc", "Allocated", 100, "e"), ("files", "Files", 80, "e"), ("dirs", "Folders", 80, "e"),
            ("diff", "Change", 100, "e")])
        self.tmap = tk.Canvas(self.nb, bg=PANEL, highlightthickness=0)
        self.nb.add(self.tmap, text=" ▦ Treemap ")
        self.nb.select(1)
        self.details.bind("<Double-1>", self._details_open)
        self.details.bind("<Button-2>" if IS_MAC else "<Button-3>", self._details_menu)
        self.topf.bind("<Double-1>", self._topfile_open)
        for c in (self.pie, self.tmap):
            c.bind("<Configure>", lambda e: self._schedule_redraw())
        self.nb.bind("<<NotebookTabChanged>>", lambda e: self._schedule_redraw())
        self.tmap.bind("<Motion>", self._map_hover)
        self.tmap.bind("<Double-1>", self._map_click)
        self.pie.bind("<Button-1>", self._pie_click)
        self._fill_history()

    def _make_list(self, title, columns, toolbar=False):
        frame = ttk.Frame(self.nb, style="Panel.TFrame")
        if toolbar:
            tb = tk.Frame(frame, bg=BG)
            tb.pack(fill="x")
            for text, cmd in (("⧉ Copy", self._copy_details), ("💾 Export", self.export_csv),
                              ("📂 Explorer", lambda: self.sel_node and self._explore(self.sel_node.path)),
                              ("⟳ Rescan", lambda: self.sel_node and self.start_scan(self.sel_node.path))):
                ttk.Button(tb, text=text, style="Toolbutton", command=cmd).pack(side="left", padx=1, pady=2)
        inner = ttk.Frame(frame, style="Panel.TFrame")
        inner.pack(fill="both", expand=True)
        tv = ttk.Treeview(inner, columns=[c[0] for c in columns], show="headings", selectmode="extended")
        for key, label, width, anchor in columns:
            tv.heading(key, text=label, anchor=anchor, command=lambda t=tv, c=key: self._sort_list(t, c))
            tv.column(key, width=int(width * self.k), anchor=anchor, stretch=key in ("name", "path", "user"),
                      minwidth=30)
        tv.tag_configure("odd", background="#FAFAFA")
        ys = ttk.Scrollbar(inner, orient="vertical", command=tv.yview)
        tv.configure(yscrollcommand=ys.set)
        tv.pack(side="left", fill="both", expand=True)
        ys.pack(side="right", fill="y")
        self.nb.add(frame, text=f" {title} ")
        return tv

    @staticmethod
    def _sort_list(tv, col):
        def keyf(v):
            parts = v.replace(",", "").replace("%", "").split()
            try:
                mult = {"KB": 1024, "MB": 1024 ** 2, "GB": 1024 ** 3, "TB": 1024 ** 4}.get(
                    parts[1] if len(parts) > 1 else "", 1)
                return (0, float(parts[0]) * mult, "")
            except (ValueError, IndexError):
                return (1, 0, v.lower())
        rows = [(tv.set(i, col), i) for i in tv.get_children("")]
        desc = not getattr(tv, "_desc", {}).get(col, False)
        tv._desc = {col: desc}
        rows.sort(key=lambda r: keyf(r[0]), reverse=desc)
        for idx, (_, i) in enumerate(rows):
            tv.move(i, "", idx)

    def _bind_keys(self):
        r = self.root
        r.bind("<F5>", lambda e: self.start_scan())
        r.bind("<Escape>", lambda e: self.stop_scan())
        r.bind("<Control-o>", lambda e: self.browse())
        r.bind("<Control-e>", lambda e: self.export_csv())
        r.bind("<Control-f>", lambda e: self.search())
        r.bind("<Control-Alt-o>", lambda e: self.options())
        r.bind("<Control-plus>", lambda e: self.expand(2))
        r.bind("<Control-minus>", lambda e: self.collapse())
        self.tree.c.bind("<BackSpace>", lambda e: self.go_up())

    def _update_title(self):
        _, _, label, kind = self.vol
        drive = os.path.splitdrive(self.path_var.get())[0] or self.path_var.get()
        where = f" - Drive: {label or kind} ({drive})" if self.tree_root else ""
        edition = EDITION + (" Portable" if is_portable() else "")
        self.root.title(f"{APP_NAME} {edition} - {MODES[self.mode.get()]}{where}  |  {COMPANY}")

    # ------------------------------------------------------------ drives --
    def _load_drives(self):
        out = []
        for d in list_drives():
            try:
                u = shutil.disk_usage(d)
                out.append((d, u.total, u.free))
            except OSError:
                pass
        self.drive_list = out
        self._draw_drives()
        self._update_status()

    def _draw_drives(self):
        c, k = self.drives, self.k
        c.delete("all")
        W = c.winfo_width()
        rh = int(26 * k)
        for label, x, a in (("Name", 8 * k, "w"), ("Size", 150 * k, "e"), ("Free", 230 * k, "e"),
                            ("% Free", 245 * k, "w")):
            c.create_text(x, rh / 2, text=label, anchor=a, font=("Segoe UI", 10), fill=TEXT)
        c.create_line(0, rh, W, rh, fill=LINE)
        for x in (120 * k, 158 * k, 238 * k, 340 * k):
            c.create_line(x, 0, x, rh, fill=LINE)
        self._drive_rows = []
        for i, (d, total, free) in enumerate(self.drive_list):
            y = rh + 4 + i * rh
            cy = y + rh / 2
            c.create_rectangle(8 * k, cy - 4 * k, 22 * k, cy + 4 * k, fill="#D7DCE3", outline="#59606B")
            c.create_rectangle(17 * k, cy - 1 * k, 20 * k, cy + 1 * k, fill="#2BB673", outline="")
            c.create_text(28 * k, cy, text=d, anchor="w", font=("Segoe UI", 10))
            c.create_text(150 * k, cy, text=fmt_short(total), anchor="e", font=("Segoe UI", 10))
            c.create_text(230 * k, cy, text=fmt_short(free), anchor="e", font=("Segoe UI", 10))
            frac = free / total if total else 0
            bx, bw = 245 * k, 90 * k
            c.create_rectangle(bx, y + 3, bx + bw, y + rh - 3, fill="#E6E6E6", outline="#BDBDBD")
            c.create_rectangle(bx, y + 3, bx + bw * frac, y + rh - 3,
                               fill="#22D11F" if frac > 0.1 else "#E5484D", outline="")
            c.create_text(bx + bw / 2, cy, text=f"{frac * 100:.0f} %", font=("Segoe UI", 10))
            self._drive_rows.append((y, y + rh, d))

    def _drive_click(self, e):
        for y0, y1, d in self._drive_rows:
            if y0 <= e.y < y1:
                self.path_var.set(d)
                self.start_scan()

    # ------------------------------------------------------------ scanning --
    def browse(self):
        p = filedialog.askdirectory(initialdir=self.path_var.get() or None, title="Select directory to scan")
        if p:
            self.path_var.set(os.path.normpath(p))
            self.start_scan()

    def start_scan(self, path=None):
        path = os.path.normpath(path or self.path_var.get().strip())
        if IS_WIN and len(path) == 2 and path[1] == ":":
            path += "\\"
        if not os.path.isdir(path):
            messagebox.showerror(APP_NAME, f"This folder can't be found:\n{path}")
            return
        if self.scanner and not self.scanner.done:
            self.scanner.stop_evt.set()
        self.path_var.set(path)
        self._remember_path(path)
        self.vol = volume_info(path)
        cluster = cluster_bytes(self.settings["cluster"], self.vol[0])
        self.vol = (cluster,) + tuple(self.vol[1:])
        self.scanner = Scanner(path, cluster, self.settings)
        self.scanner.start()
        self.tree.clear("Scanning...")
        self.scan_btn.state(["disabled"])
        self.stop_btn.state(["!disabled"])
        self.progress.pack(side="right", padx=4)
        self.progress.start(12)
        self._poll()

    def _show_errors(self, items, total):
        win = tk.Toplevel(self.root)
        win.title(f"Scan errors - {APP_NAME}")
        win.geometry(f"{int(760 * self.k)}x{int(380 * self.k)}")
        ttk.Label(win, text=f"{total:,} items could not be read" + (" (first 1,000 shown)" if total > 1000 else ""),
                  padding=6).pack(anchor="w")
        tv = ttk.Treeview(win, columns=("path", "error"), show="headings")
        tv.heading("path", text="Path")
        tv.heading("error", text="Error")
        tv.column("path", width=int(480 * self.k))
        tv.column("error", width=int(240 * self.k))
        for path, msg in items:
            tv.insert("", "end", values=(path, msg))
        tv.pack(fill="both", expand=True, padx=6)
        ttk.Button(win, text="Close", command=win.destroy).pack(pady=6)

    def stop_scan(self):
        if self.scanner and not self.scanner.done:
            self.scanner.stop_evt.set()

    def _poll(self):
        sc = self.scanner
        if sc is None:
            return
        if not sc.done:
            cur = sc.current if len(sc.current) < 80 else "..." + sc.current[-77:]
            self.st_msg.set(f"Scanning: {sc.files:,} files, {sc.dirs:,} folders, {fmt_size(sc.bytes, 'Auto')}  {cur}")
            self.tree.hint = f"Scanning {sc.files:,} files in {sc.dirs:,} folders..."
            self.tree.draw()
            self.root.after(120, self._poll)
            return
        self.progress.stop()
        self.progress.pack_forget()
        self.scan_btn.state(["!disabled"])
        self.stop_btn.state(["disabled"])
        state = "Scan stopped" if sc.cancelled else "Scan complete"
        self.st_msg.set(f"{state}: {sc.files:,} files in {sc.dirs:,} folders, {sc.elapsed:.1f} s"
                        + (f", {sc.errors:,} items not readable" if sc.errors else ""))
        self.tree_root = sc.root
        self.tree.set_root(sc.root)
        levels = int(self.settings.get("expand_levels", 1) or 1)
        if levels > 1:
            self.tree.expand(sc.root, levels)
        self._fill_stats(sc)
        if not self.screenshot and not sc.cancelled:
            self._save_history(sc.root, sc.finished_at)
        self._fill_history()
        if self.settings.get("show_errors") and sc.error_list and not self.screenshot:
            self._show_errors(sc.error_list, sc.errors)
        self._update_title()

    # ---------------------------------------------------------------- tree --
    def metric(self, node, files_row=False):
        m = self.mode.get()
        if m == "alloc":
            return node.own_alloc if files_row else node.alloc
        if m == "files":
            return node.own_files if files_row else node.files
        return node.own_size if files_row else node.size

    def metric_text(self, node, files_row, parent_val):
        m = self.mode.get()
        v = self.metric(node, files_row)
        if m == "files":
            return f"{v:,} Files"
        if m == "percent":
            return pct_text(v, parent_val) if node.parent is not None or files_row else "100.0 %"
        return fmt_size(v, self.unit.get())

    def sorted_entries(self, node):
        ents = [(ch, False) for ch in node.children]
        if node.own_files:
            ents.append((node, True))
        k = self.sort_key.get()
        if k == "name":
            ents.sort(key=lambda e: "" if e[1] else e[0].name.lower())
        elif k == "files":
            ents.sort(key=lambda e: e[0].own_files if e[1] else e[0].files, reverse=True)
        elif k == "mtime":
            ents.sort(key=lambda e: e[0].mtime, reverse=True)
        else:
            ents.sort(key=lambda e: self.metric(e[0], e[1]), reverse=True)
        return ents

    def refresh_all(self):
        self.tree.refresh()
        self._update_title()
        if self.scanner and self.scanner.done:
            self._fill_stats(self.scanner)
        self._fill_details()
        self._fill_history()
        self._schedule_redraw()

    def reveal(self, node):
        self.tree.reveal(node)

    def expand(self, levels):
        if self.tree_root:
            self.tree.expand(self.sel_node or self.tree_root, levels)

    def collapse(self):
        self.tree.collapse_all()

    def go_up(self):
        if self.sel_node and self.sel_node.parent:
            self.reveal(self.sel_node.parent)
        elif self.tree_root:
            parent = os.path.dirname(self.tree_root.path.rstrip("\\/"))
            if parent and os.path.normpath(parent) != os.path.normpath(self.tree_root.path):
                self.start_scan(parent + "\\" if parent.endswith(":") else parent)

    def on_tree_select(self, entry):
        node, files_row = entry if entry else (None, False)
        changed = node is not self.sel_node or files_row != self.sel_files
        self.sel_node, self.sel_files = node, files_row
        if node:
            u = self.unit.get()
            self._set_info([("", node.path + ("\\[Files]" if files_row else "")),
                            ("Size:", fmt_size(node.own_size if files_row else node.size, u)),
                            ("Allocated:", fmt_size(node.own_alloc if files_row else node.alloc, u)),
                            ("Files:", f"{node.own_files if files_row else node.files:,}"),
                            ("Folders:", f"{0 if files_row else node.dirs:,}"),
                            ("Last Change:", self.fdate(node.mtime)), ("Last Access:", self.fdate(node.atime)),
                            ("Owner:", get_owner(node.path) or "-")])
        self._update_status()
        if changed:
            self._fill_details()
            self._schedule_redraw()

    def _update_status(self):
        try:
            u = shutil.disk_usage(self.path_var.get())
            self.st_free.set(f"Free Space: {fmt_short(u.free)}  (of {fmt_short(u.total)})")
        except OSError:
            self.st_free.set("")
        if self.sel_node:
            n = self.sel_node.own_files if self.sel_files else self.sel_node.files
            self.st_files.set(f"{n:,}  Files")
        cl, fs, _, _ = self.vol
        if self.tree_root:
            self.st_cluster.set(f"{cl:,} Bytes per Cluster ({fs or '?'})")

    def tree_menu(self, e, node, files_row):
        m = tk.Menu(self.root, tearoff=False)
        m.add_command(label="Open", command=lambda: self._open_path(node.path))
        m.add_command(label="Show in Explorer", command=lambda: self._explore(node.path))
        m.add_command(label="Copy path", command=lambda: self._copy(node.path))
        m.add_separator()
        m.add_command(label="Expand two levels", command=lambda: self.tree.expand(node, 2))
        m.add_command(label="Scan this branch", command=lambda: self.start_scan(node.path))
        m.tk_popup(e.x_root, e.y_root)

    def _open_path(self, p):
        try:
            if IS_WIN:
                os.startfile(p)
            else:
                subprocess.Popen(["open" if IS_MAC else "xdg-open", p])
        except OSError as ex:
            messagebox.showerror(APP_NAME, str(ex))

    def _explore(self, p):
        if IS_WIN:
            subprocess.Popen(["explorer", "/select,", p])
        elif IS_MAC:
            subprocess.Popen(["open", "-R", p])  # reveal in Finder
        else:
            self._open_path(os.path.dirname(p))

    def _copy(self, text):
        self.root.clipboard_clear()
        self.root.clipboard_append(text)
        self.st_msg.set(f"Copied: {text}")

    # ------------------------------------------------------------- lists --
    @staticmethod
    def _clear(tv):
        tv.delete(*tv.get_children())

    def _fill_details(self):
        tv = self.details
        self._clear(tv)
        self._details_paths = {}
        node = self.sel_node
        if not node:
            return
        u, cl = self.unit.get(), self.vol[0]
        rows = [] if self.sel_files else [
            (ch.size, "📁 " + ch.name, ch.alloc, ch.files, ch.dirs, ch.mtime, ch.atime, ch.path)
            for ch in node.children]
        try:
            with os.scandir(node.path) as it:
                for e in it:
                    try:
                        if e.is_file(follow_symlinks=False):
                            st = e.stat(follow_symlinks=False)
                            rows.append((st.st_size, "🗎 " + e.name, -(-st.st_size // cl) * cl, 1, 0,
                                         st.st_mtime, st.st_atime, e.path))
                    except OSError:
                        pass
        except OSError:
            pass
        rows.sort(key=lambda r: r[0], reverse=True)
        total = node.own_size if self.sel_files else node.size
        for i, (sz, name, al, fl, dr, mt, at, path) in enumerate(rows[:2000]):
            owner = get_owner(path) if i < 300 and "owner" in self.settings["details_cols"] else ""
            iid = tv.insert("", "end", values=(name, fmt_size(sz, u), fmt_size(al, u), f"{fl:,}", f"{dr:,}",
                                               pct_text(sz, total), self.fdate(mt), self.fdate(at), owner),
                            tags=("odd",) if i % 2 else ())
            self._details_paths[iid] = path

    def _copy_details(self):
        lines = ["\t".join(self.details.heading(c, "text") for c in self.details["columns"])]
        sel = self.details.selection() or self.details.get_children()
        for iid in sel:
            lines.append("\t".join(str(v) for v in self.details.item(iid, "values")))
        self._copy("\n".join(lines))
        self.st_msg.set(f"Copied {len(sel)} rows to the clipboard")

    def _details_open(self, e):
        path = self._details_paths.get(self.details.identify_row(e.y))
        if not path or not self.sel_node:
            return
        for ch in self.sel_node.children:
            if ch.path == path:
                self.reveal(ch)
                return
        self._open_path(path)

    def _details_menu(self, e):
        iid = self.details.identify_row(e.y)
        path = self._details_paths.get(iid)
        if not path:
            return
        self.details.selection_set(iid)
        m = tk.Menu(self.root, tearoff=False)
        m.add_command(label="Open", command=lambda: self._open_path(path))
        m.add_command(label="Show in Explorer", command=lambda: self._explore(path))
        m.add_command(label="Copy path", command=lambda: self._copy(path))
        m.tk_popup(e.x_root, e.y_root)

    def _topfile_open(self, e):
        path = self._top_paths.get(self.topf.identify_row(e.y))
        if path:
            self._explore(path)

    def _fill_users(self):
        tv = self.users
        self._clear(tv)
        node = self.sel_node
        if not node:
            return
        sc = self.scanner
        if sc is not None and sc.done and sc.owner_stats and node is self.tree_root and not self.sel_files:
            total = node.size or 1
            for i, (user, (sz, cnt)) in enumerate(sorted(sc.owner_stats.items(), key=lambda kv: kv[1][0],
                                                         reverse=True)):
                tv.insert("", "end", values=("👤 " + user, fmt_size(sz, self.unit.get()), pct_text(sz, total),
                                             f"{cnt:,}"), tags=("odd",) if i % 2 else ())
            return
        stats = {}
        if not self.sel_files:
            for ch in node.children:
                rec = stats.setdefault(get_owner(ch.path) or "(unknown)", [0, 0])
                rec[0] += ch.size
                rec[1] += ch.files
        try:
            with os.scandir(node.path) as it:
                for e in it:
                    try:
                        if e.is_file(follow_symlinks=False):
                            rec = stats.setdefault(get_owner(e.path) or "(unknown)", [0, 0])
                            rec[0] += e.stat(follow_symlinks=False).st_size
                            rec[1] += 1
                    except OSError:
                        pass
        except OSError:
            pass
        total = (node.own_size if self.sel_files else node.size) or 1
        for i, (user, (sz, cnt)) in enumerate(sorted(stats.items(), key=lambda kv: kv[1][0], reverse=True)):
            tv.insert("", "end", values=("👤 " + user, fmt_size(sz, self.unit.get()), pct_text(sz, total),
                                         f"{cnt:,}"), tags=("odd",) if i % 2 else ())

    def _fill_stats(self, sc):
        u = self.unit.get()
        total = sc.root.size or 1
        self._clear(self.types)
        if not sc.want_ext:
            self.types.insert("", "end", values=("", "Statistics on file extensions are turned off in Options "
                                                     "(Scan page).", "", "", "", ""))
        for i, (ext, (sz, cnt, al)) in enumerate(sorted(sc.ext_stats.items(), key=lambda kv: kv[1][0],
                                                        reverse=True)[:500]):
            desc = EXT_TO_KIND.get(ext, (ext.upper() + " file") if ext else "No extension")
            self.types.insert("", "end", values=("." + ext if ext else "(none)", desc, fmt_size(sz, u),
                                                 fmt_size(al, u), pct_text(sz, total), f"{cnt:,}"),
                              tags=("odd",) if i % 2 else ())
        self._clear(self.topf)
        self._top_paths = {}
        for i, (sz, path, mt) in enumerate(sc.top_files[:100]):
            iid = self.topf.insert("", "end", values=(os.path.basename(path), fmt_size(sz, u),
                                                      os.path.dirname(path), fmt_time(mt)),
                                   tags=("odd",) if i % 2 else ())
            self._top_paths[iid] = path
        self._clear(self.ages)
        self.ages.heading("age", text=sc.age_basis)
        for i, ((_, label), (sz, cnt)) in enumerate(zip(sc.buckets, sc.age_stats)):
            self.ages.insert("", "end", values=(label, fmt_size(sz, u), pct_text(sz, total), f"{cnt:,}"),
                             tags=("odd",) if i % 2 else ())

    # ----------------------------------------------------------- history --
    @staticmethod
    def _history_file():
        return os.path.join(data_dir(), "history.json")

    def _load_history(self):
        try:
            with open(self._history_file(), encoding="utf-8") as f:
                return json.load(f)
        except (OSError, ValueError):
            return []

    def _save_history(self, node, when):
        hist = self._load_history()
        hist.append({"date": when, "path": node.path, "size": node.size, "alloc": node.alloc,
                     "files": node.files, "dirs": node.dirs})
        try:
            os.makedirs(os.path.dirname(self._history_file()), exist_ok=True)
            with open(self._history_file(), "w", encoding="utf-8") as f:
                json.dump(hist[-500:], f)
        except OSError:
            pass

    def clear_history(self):
        if messagebox.askyesno(APP_NAME, "Remove all saved scan history?"):
            try:
                os.remove(self._history_file())
            except OSError:
                pass
            self._fill_history()

    def _fill_history(self):
        tv = self.hist
        self._clear(tv)
        u = self.unit.get()
        prev, rows = {}, []
        for h in self._load_history():
            p = prev.get(h["path"])
            diff = "" if p is None else ("+" if h["size"] >= p else "-") + fmt_size(abs(h["size"] - p), u)
            prev[h["path"]] = h["size"]
            rows.append((fmt_time(h["date"]), h["path"], fmt_size(h["size"], u), fmt_size(h["alloc"], u),
                         f"{h['files']:,}", f"{h['dirs']:,}", diff))
        for i, r in enumerate(reversed(rows)):
            tv.insert("", "end", values=r, tags=("odd",) if i % 2 else ())

    # ------------------------------------------------------------- search --
    def search(self):
        base = self.tree_root.path if self.tree_root else self.path_var.get()
        win = tk.Toplevel(self.root)
        win.title(f"File Search - {APP_NAME}")
        win.geometry(f"{int(820 * self.k)}x{int(480 * self.k)}")
        top = ttk.Frame(win, padding=6)
        top.pack(fill="x")
        ttk.Label(top, text="Find files named").pack(side="left")
        pat = tk.StringVar(value="*.zip")
        ent = ttk.Entry(top, textvariable=pat, width=24)
        ent.pack(side="left", padx=6)
        ttk.Label(top, text=f"in  {base}").pack(side="left")
        go = ttk.Button(top, text="Search")
        go.pack(side="right")
        tv = ttk.Treeview(win, columns=("name", "size", "path", "mtime"), show="headings")
        for c, t, w in (("name", "Name", 200), ("size", "Size", 90), ("path", "Folder", 360),
                        ("mtime", "Last Change", 120)):
            tv.heading(c, text=t, command=lambda c=c: self._sort_list(tv, c))
            tv.column(c, width=int(w * self.k), anchor="e" if c == "size" else "w")
        tv.pack(fill="both", expand=True, padx=6)
        msg = tk.StringVar(value="Use * and ? as wildcards, e.g. *.mp4 or report*")
        ttk.Label(win, textvariable=msg, padding=6).pack(fill="x")
        state = {"stop": threading.Event()}

        def worker(pattern, stop, q):
            n = 0
            for d, _dirs, files in os.walk(base):
                if stop.is_set():
                    break
                for f in files:
                    if fnmatch.fnmatch(f.lower(), pattern):
                        p = os.path.join(d, f)
                        try:
                            st = os.stat(p)
                        except OSError:
                            continue
                        q.put((f, st.st_size, d, st.st_mtime))
                        n += 1
                        if n >= 5000:
                            stop.set()
                            break
            q.put(None)

        def pump(q):
            if not win.winfo_exists():
                return
            try:
                while True:
                    item = q.get_nowait()
                    if item is None:
                        msg.set(f"{len(tv.get_children()):,} files found")
                        return
                    f, sz, d, mt = item
                    tv.insert("", "end", values=(f, fmt_size(sz, self.unit.get()), d, fmt_time(mt)))
            except queue.Empty:
                pass
            msg.set(f"Searching... {len(tv.get_children()):,} found")
            win.after(100, lambda: pump(q))

        def start(_e=None):
            state["stop"].set()
            state["stop"] = threading.Event()
            q = queue.Queue()
            tv.delete(*tv.get_children())
            threading.Thread(target=worker, args=(pat.get().strip().lower() or "*", state["stop"], q),
                             daemon=True).start()
            pump(q)

        def open_row(e):
            i = tv.identify_row(e.y)
            if i:
                self._explore(os.path.join(tv.set(i, "path"), tv.set(i, "name")))

        go.configure(command=start)
        ent.bind("<Return>", start)
        tv.bind("<Double-1>", open_row)
        win.protocol("WM_DELETE_WINDOW", lambda: (state["stop"].set(), win.destroy()))
        ent.focus_set()

    # ------------------------------------------------------------- charts --
    def _schedule_redraw(self):
        if self._redraw_job:
            self.root.after_cancel(self._redraw_job)
        self._redraw_job = self.root.after(40, self._redraw)

    def _redraw(self):
        self._redraw_job = None
        tab = self.nb.index(self.nb.select())
        if tab == 0:
            self._draw_pie()
        elif tab == 3:
            self._fill_users()
        elif tab == 7:
            self._draw_map()

    def _chart_items(self, node, limit):
        items = [(self.metric(ch), ch.name, ch) for ch in node.children if self.metric(ch) > 0]
        if self.metric(node, True):
            items.append((self.metric(node, True), "[Files]", None))
        items.sort(key=lambda t: t[0], reverse=True)
        if len(items) > limit:
            rest = sum(t[0] for t in items[limit - 1:])
            items = items[:limit - 1] + [(rest, f"Other ({len(items) - limit + 1} items)", "other")]
        return items

    def _val_text(self, v):
        return f"{v:,} Files" if self.mode.get() == "files" else fmt_size(v, self.unit.get())

    @staticmethod
    def _empty_canvas(c, msg):
        w, h = c.winfo_width(), c.winfo_height()
        badge = brand_image("badge", 160)
        if badge is not None and h > badge.height() + 80:
            c.create_image(w / 2, h / 2 - 20, image=badge, anchor="s")
            c.create_text(w / 2, h / 2, text=msg, font=("Segoe UI", 10), fill=MUTED, anchor="n")
        else:
            c.create_text(w / 2, h / 2, text=msg, font=("Segoe UI", 10), fill=MUTED)

    def _draw_pie(self):
        c, k = self.pie, self.k
        c.delete("all")
        self._pie_items = {}
        node = self.sel_node
        if not node:
            self._empty_canvas(c, "Scan a drive or folder to see what takes up space.")
            return
        w, h = c.winfo_width(), c.winfo_height()
        total = self.metric(node)
        c.create_text(16 * k, 18 * k, text=node.path, anchor="w", font=("Segoe UI Semibold", 12), fill=NAVY)
        c.create_text(16 * k, 38 * k, text=f"{self._val_text(total)} · {node.files:,} files · {node.dirs:,} folders",
                      anchor="w", font=("Segoe UI", 9), fill=MUTED)
        items = self._chart_items(node, 11)
        if not items or total <= 0:
            self._empty_canvas(c, "This folder is empty.")
            return
        legend_w = min(320 * k, w * 0.45)
        r = max(40, min((w - legend_w - 60 * k) / 2, (h - 90 * k) / 2))
        cx, cy = 24 * k + r, 60 * k + (h - 60 * k) / 2
        start = 90.0
        for i, (sz, name, ref) in enumerate(items):
            color = FILES_COLOR if ref is None else OTHER_COLOR if ref == "other" else PALETTE[i % len(PALETTE)]
            ext = max(-359.99, -360.0 * sz / total)
            tag = f"p{i}"
            if abs(ext) > 0.05:
                c.create_arc(cx - r, cy - r, cx + r, cy + r, start=start, extent=ext, fill=color, outline=PANEL,
                             width=2, style="pieslice", tags=(tag,))
            start += ext
            ly, lx = (70 + i * 26) * k, cx + r + 30 * k
            if ly < h - 10:
                c.create_rectangle(lx, ly - 6 * k, lx + 12 * k, ly + 6 * k, fill=color, outline="", tags=(tag,))
                c.create_text(lx + 20 * k, ly, text=name if len(name) < 28 else name[:26] + "…", anchor="w",
                              font=("Segoe UI", 9), fill=TEXT, tags=(tag,))
                c.create_text(w - 14, ly, text=f"{self._val_text(sz)}   {100 * sz / total:4.1f}%", anchor="e",
                              font=("Segoe UI", 9), fill=MUTED, tags=(tag,))
            if isinstance(ref, Node):
                self._pie_items[tag] = ref
        c.create_oval(cx - r * 0.55, cy - r * 0.55, cx + r * 0.55, cy + r * 0.55, fill=PANEL, outline="")
        c.create_text(cx, cy - 9 * k, text=self._val_text(total), font=("Segoe UI Semibold", 13), fill=NAVY)
        c.create_text(cx, cy + 13 * k, text="total", font=("Segoe UI", 9), fill=MUTED)

    def _pie_click(self, e):
        for item in self.pie.find_overlapping(e.x, e.y, e.x, e.y)[::-1]:
            for tag in self.pie.gettags(item):
                if tag in self._pie_items:
                    self.reveal(self._pie_items[tag])
                    return

    def _draw_map(self):
        c, k = self.tmap, self.k
        c.delete("all")
        self._map_items = {}
        node = self.sel_node
        if not node:
            self._empty_canvas(c, "Scan a drive or folder to see its treemap.")
            return
        w, h = c.winfo_width(), c.winfo_height()
        c.create_text(10, 14 * k, text=f"{node.path}   ({self._val_text(self.metric(node))})", anchor="w",
                      font=("Segoe UI Semibold", 9), fill=NAVY)
        c.create_text(w - 10, 14 * k, text="double-click a box to open it", anchor="e", font=("Segoe UI", 8),
                      fill=MUTED)
        self._map_level(c, node, 6, 28 * k, w - 12, h - 34 * k, 0, None)

    def _map_level(self, c, node, x, y, w, h, level, base):
        k = self.k
        items = self._chart_items(node, 60 if level == 0 else 30)
        rects = squarify([(sz, (name, ref)) for sz, name, ref in items], x, y, w, h)
        for i, (rx, ry, rw, rh, (name, ref)) in enumerate(rects):
            if level == 0:
                color = FILES_COLOR if ref is None else OTHER_COLOR if ref == "other" else PALETTE[i % len(PALETTE)]
            else:
                color = lighten(base, 0.25 + 0.5 * (i % 3) / 3) if ref not in (None, "other") else lighten(base, 0.8)
            tag = f"m{len(self._map_items)}"
            self._map_items[tag] = (ref if isinstance(ref, Node) else node, name)
            c.create_rectangle(rx, ry, rx + rw, ry + rh, fill=color, outline=PANEL, width=1 if level else 2,
                               tags=(tag,))
            if level == 0 and isinstance(ref, Node) and ref.children and rw > 70 * k and rh > 50 * k:
                c.create_text(rx + 5, ry + 9 * k, text=self._trim(name, rw), anchor="w",
                              font=("Segoe UI Semibold", 8), fill="white", tags=(tag,))
                self._map_level(c, ref, rx + 3, ry + 18 * k, rw - 6, rh - 21 * k, 1, color)
            elif rw > 50 * k and rh > 18 * k:
                c.create_text(rx + rw / 2, ry + rh / 2, text=self._trim(name, rw), width=rw - 6,
                              font=("Segoe UI", 8), fill=TEXT if level else "white", tags=(tag,))

    def _trim(self, name, width):
        n = max(3, int(width / (6.5 * self.k)))
        return name if len(name) <= n else name[:n - 1] + "…"

    def _map_target(self, e):
        for item in self.tmap.find_overlapping(e.x, e.y, e.x, e.y)[::-1]:
            for tag in self.tmap.gettags(item):
                if tag in self._map_items:
                    return self._map_items[tag]
        return None

    def _map_hover(self, e):
        t = self._map_target(e)
        if t:
            node, name = t
            self.st_msg.set(f"{node.path}   {self._val_text(self.metric(node))}" if name == node.name
                            else f"{name} in {node.path}")

    def _map_click(self, e):
        t = self._map_target(e)
        if t:
            self.reveal(t[0])

    # -------------------------------------------------------------- misc --
    def export_csv(self):
        if not self.tree_root:
            messagebox.showinfo(APP_NAME, "Scan a drive or folder first.")
            return
        default = self.settings.get("export_file", "")
        p = filedialog.asksaveasfilename(defaultextension=".csv", filetypes=[("CSV file", "*.csv")],
                                         initialdir=os.path.dirname(default) or None,
                                         initialfile=os.path.basename(default) or "disk-report.csv",
                                         title="Export folder sizes")
        if not p:
            return
        try:
            n = self.write_export(p)
        except OSError as ex:
            messagebox.showerror(APP_NAME, f"The export could not be saved:\n{ex}")
            return
        self.st_msg.set(f"Exported {n:,} folders to {p}")

    def write_export(self, p):
        st = self.settings
        cols = [c for c in st["export_cols"] if c in dict(EXPORT_COLUMNS)] or ["path"]
        raw, units, unit = st["export_bytes"], st["export_units"], self.unit.get()
        factor = 1 if unit in ("Auto", "Bytes") else {"KB": 1024, "MB": 1024 ** 2, "GB": 1024 ** 3}[unit]

        def size(v):
            if raw:
                return f"{v} Bytes" if units else v
            if units:
                return fmt_size(v, unit)
            return v if factor == 1 else round(v / factor, 1)

        def cell(c, n):
            if c == "name":
                return n.name
            if c == "path":
                return n.path
            if c in ("size", "alloc"):
                return size(n.size if c == "size" else n.alloc)
            if c == "files":
                return n.files
            if c == "dirs":
                return n.dirs
            if c == "pct":
                return pct_text(n.size, n.parent.size) if n.parent else "100.0 %"
            if c == "mtime":
                return self.fdate(n.mtime)
            if c == "atime":
                return self.fdate(n.atime)
            if c == "owner":
                return get_owner(n.path, cache=False)
            return n.depth
        only_open = st["export_scope"] == "expanded"
        count = 0
        with open(p, "w", newline="", encoding="utf-8-sig") as f:
            wr = csv.writer(f)
            if st["export_header"]:
                names = dict(EXPORT_COLUMNS)
                wr.writerow([names[c].replace(" (slower)", "") for c in cols])
            stack = [self.tree_root]
            while stack:
                n = stack.pop()
                wr.writerow([cell(c, n) for c in cols])
                count += 1
                if not only_open or n in self.tree.opened:
                    stack.extend(reversed(n.children))
        return count

    def about(self):
        k = self.k
        win = tk.Toplevel(self.root)
        win.title(f"About {APP_NAME}")
        win.configure(bg=CREAM)
        win.resizable(False, False)
        win.transient(self.root)
        badge = brand_image("badge", int(200 * k))
        if badge is not None:
            tk.Label(win, image=badge, bg=CREAM).pack(padx=int(40 * k), pady=(int(20 * k), int(8 * k)))
        tk.Label(win, text=f"{APP_NAME} {EDITION}", bg=CREAM, fg=NAVY,
                 font=("Segoe UI Semibold", 14)).pack()
        tk.Label(win, text=f"Version {VERSION}", bg=CREAM, fg=ORANGE, font=("Segoe UI", 10, "bold")).pack()
        tk.Frame(win, bg=GOLD, height=2, width=int(260 * k)).pack(pady=int(10 * k))
        tk.Label(win, text="See what is using space on your drives: folder sizes, charts, file types,\n"
                           "owners, largest and oldest files, and scan history.",
                 bg=CREAM, fg=NAVY, font=("Segoe UI", 9), justify="center").pack(padx=int(24 * k))
        tk.Label(win, text=f"© {time.localtime().tm_year} {COMPANY}. All rights reserved.", bg=CREAM, fg=MUTED,
                 font=("Segoe UI", 9)).pack(pady=(int(10 * k), 0))
        ttk.Button(win, text="OK", width=12, command=win.destroy).pack(pady=int(14 * k))
        win.bind("<Escape>", lambda e: win.destroy())
        win.bind("<Return>", lambda e: win.destroy())
        if not self.screenshot:
            win.grab_set()
            win.focus_set()
        return win

    # -------------------------------------------------------- screenshot --
    def run_screenshot_loop(self):
        self.frames += 1
        sc = self.scanner
        if sc is not None and sc.done and self.tree_root is sc.root:
            self.ready_frames += 1
        busy = sc is not None and self.ready_frames < 20
        if (self.frames >= 120 and not busy) or self.frames >= 1200:
            self.root.update()
            target = self.capture_target if self.capture_target is not None else self.root
            ok = IS_WIN and capture_window(int(target.wm_frame(), 16), self.screenshot)
            if ok or self.frames >= 1260:
                if not ok and sys.stderr:
                    print("screenshot: picture was blank", file=sys.stderr)
                self.root.destroy()
                return
        self.root.after(16, self.run_screenshot_loop)


def show_without_focus(root):
    """Show the window without activating it, behind other windows (Windows)."""
    if not IS_WIN:
        root.deiconify()
        return
    u, _ = _win32()
    root.update_idletasks()
    hwnd = int(root.wm_frame(), 16)
    prev = u.GetForegroundWindow()
    u.SetWindowLongW(hwnd, -20, u.GetWindowLongW(hwnd, -20) | 0x08000000)  # WS_EX_NOACTIVATE
    root.deiconify()
    u.SetWindowPos(hwnd, 1, 0, 0, 0, 0, 0x0001 | 0x0002 | 0x0010)  # HWND_BOTTOM, NOSIZE|NOMOVE|NOACTIVATE
    if prev:
        u.SetForegroundWindow(prev)


def run_uninstall():
    """Started from the Start menu 'Uninstall' entry or Windows Settings > Apps."""
    import om_install
    root = tk.Tk()
    root.withdraw()
    exe = om_install.frozen_exe()
    if not exe or om_install.install_info(exe) is None:
        messagebox.showinfo(APP_NAME, "This copy was not installed by Setup (for example the portable copy).\n"
                                      "To remove it, just delete its folder.")
        root.destroy()
        return 0
    if not messagebox.askyesno(APP_NAME, f"Remove {APP_NAME} from this computer?"):
        root.destroy()
        return 0
    keep = not messagebox.askyesno(APP_NAME, "Also remove your settings and scan history?")
    try:
        set_shell_menu(False)
    except OSError:
        pass
    if not keep:
        shutil.rmtree(data_dir(), ignore_errors=True)
    om_install.uninstall(exe)
    messagebox.showinfo(APP_NAME, f"{APP_NAME} has been removed.")
    root.destroy()
    return 0


def main(argv):
    args = list(argv)
    if "--uninstall" in args:
        return run_uninstall()
    screenshot = tab = None
    if "--screenshot" in args:
        i = args.index("--screenshot")
        if i + 1 >= len(args):
            print("usage: main.py --screenshot <file.png> [--tab name] [folder]", file=sys.stderr)
            return 2
        screenshot = os.path.abspath(args[i + 1])
        del args[i:i + 2]
    if "--tab" in args:
        i = args.index("--tab")
        tab = args[i + 1] if i + 1 < len(args) else None
        del args[i:i + 2]
    options_page = None
    show_about = "--about" in args  # test aid: picture of the About window
    if show_about:
        args.remove("--about")
    if "--options" in args:  # test aid: picture of the Options window on page N
        i = args.index("--options")
        try:
            options_page = int(args[i + 1])
        except (IndexError, ValueError):
            options_page = 0
        del args[i:i + 2]
    start = args[0] if args else None
    if start:
        # Explorer passes drives as "C:\" which arrives as C:" - repair it
        start = start.strip().strip('"')
        if len(start) == 2 and start[1] == ":":
            start += "\\"
    if screenshot and not start:
        # a small, safe folder so the picture shows real results
        start = (os.path.join(os.environ.get("WINDIR", "C:\\Windows"), "Web") if getattr(sys, "frozen", False)
                 else sys.base_prefix)
    if IS_WIN:
        try:
            ctypes.windll.shcore.SetProcessDpiAwareness(1)
        except Exception:
            pass
    root = tk.Tk()
    if screenshot:
        root.withdraw()
    app = App(root, start, screenshot)
    if tab:
        names = [n.lower() for n in App.TAB_NAMES]
        if tab.lower() in names:
            app.nb.select(names.index(tab.lower()))
    if screenshot:
        show_without_focus(root)
        if options_page is not None:
            dlg = app.options(options_page)
            dlg.withdraw()
            show_without_focus(dlg)
            app.capture_target = dlg
        elif show_about:
            dlg = app.about()
            dlg.withdraw()
            show_without_focus(dlg)
            app.capture_target = dlg
        root.after(16, app.run_screenshot_loop)
    root.mainloop()
    return 0
