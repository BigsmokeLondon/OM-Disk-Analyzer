"""OM Disk Analyzer Professional - a disk space analyzer by OM Industries Inc."""
import csv
import fnmatch
import heapq
import json
import os
import queue
import shutil
import string
import struct
import subprocess
import sys
import threading
import time
import zlib
import tkinter as tk
import tkinter.font as tkfont
from tkinter import ttk, filedialog, messagebox

APP_NAME = "OM Disk Analyzer"
EDITION = "Professional"
COMPANY = "OM Industries Inc"
VERSION = "1.2"
IS_WIN = sys.platform == "win32"

if IS_WIN:
    import ctypes
    from ctypes import wintypes

# OM Industries brand colours (sampled from the logo pack)
NAVY = "#1B2A3A"
ORANGE = "#B5472A"   # rust
GOLD = "#DCA93F"
CREAM = "#F2E8D0"
SAND = "#E6D6B0"
BG = "#F0F0F0"
PANEL = "#FFFFFF"
TEXT = "#1B1B1B"
MUTED = "#6B7280"
SELECT = "#CCE4F7"
LINE = "#C8C8C8"
BAR_START = (0xFF, 0xFF, 0x70)
BAR_END = (0xFF, 0x7A, 0x00)
PALETTE = ["#1B2A3A", "#B5472A", "#DCA93F", "#3E6E8E", "#8A3F2E", "#6F8F5A",
           "#C98B2B", "#5B4A6E", "#2E8C8C", "#9C6B3F", "#4F5D75", "#D07A55"]
OTHER_COLOR = "#B9B6AB"
FILES_COLOR = "#E6D6B0"

ASSETS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "assets")
_images = {}


def brand_image(kind, px):
    """The OM logo ("icon", "badge" or "horizontal") at the stored size closest to px, or None."""
    try:
        sizes = sorted(int(f[len(kind) + 1:-4]) for f in os.listdir(ASSETS)
                       if f.startswith(kind + "_") and f.endswith(".png"))
    except (OSError, ValueError):
        return None
    if not sizes:
        return None
    pick = min(sizes, key=lambda s: (abs(s - px), -s))
    key = (kind, pick)
    if key not in _images:
        try:
            _images[key] = tk.PhotoImage(file=os.path.join(ASSETS, f"{kind}_{pick}.png"))
        except tk.TclError:
            return None
    return _images[key]

UNIT_FACTORS = {"KB": 1024, "MB": 1024 ** 2, "GB": 1024 ** 3}
MODES = {"size": "Size", "alloc": "Allocated", "files": "File count", "percent": "Percent"}
AGE_BUCKETS = [(30, "Less than 1 month"), (90, "1 - 3 months"), (180, "3 - 6 months"),
               (365, "6 - 12 months"), (730, "1 - 2 years"), (1825, "2 - 5 years"),
               (float("inf"), "More than 5 years")]
EXT_KINDS = {
    "Image": "jpg jpeg png gif bmp tif tiff webp heic raw svg ico psd",
    "Video": "mp4 mkv avi mov wmv flv webm m4v mpg mpeg",
    "Audio": "mp3 wav flac aac ogg m4a wma",
    "Document": "pdf doc docx xls xlsx ppt pptx odt ods txt rtf md csv",
    "Archive": "zip rar 7z gz tar bz2 xz cab iso",
    "Program": "exe dll msi sys bat cmd ps1 com pyd",
    "Source code": "py pyc js ts html css json xml c h cpp hpp rs go java cs",
    "Database": "db sqlite mdb accdb ldf mdf",
}
EXT_TO_KIND = {e: k for k, v in EXT_KINDS.items() for e in v.split()}


def fmt_size(n, unit="MB"):
    if unit == "Bytes":
        return f"{n:,} Bytes"
    if unit in UNIT_FACTORS:
        return f"{n / UNIT_FACTORS[unit]:,.1f} {unit}"
    v = float(n)
    for u in ("Bytes", "KB", "MB", "GB", "TB"):
        if v < 1024 or u == "TB":
            return f"{int(v):,} Bytes" if u == "Bytes" else f"{v:,.1f} {u}"
        v /= 1024


def fmt_short(n):
    v = float(n)
    for u in ("B", "KB", "MB", "GB", "TB", "PB"):
        if v < 1000 or u == "PB":
            return f"{v:.3g} {u}" if v >= 1 else f"0 {u}"
        v /= 1024


def pct_text(part, whole):
    return f"{100.0 * part / whole:.1f} %" if whole else "0.0 %"


def fmt_date(t):
    return time.strftime("%d/%m/%Y", time.localtime(t)) if t else ""


def fmt_time(t):
    return time.strftime("%d/%m/%Y %H:%M", time.localtime(t)) if t else ""


def mix(c1, c2, f):
    f = max(0.0, min(1.0, f))
    return "#%02x%02x%02x" % tuple(int(a + (b - a) * f) for a, b in zip(c1, c2))


def hex_rgb(value, default):
    try:
        v = value.lstrip("#")
        return tuple(int(v[i:i + 2], 16) for i in (0, 2, 4))
    except (AttributeError, ValueError, IndexError):
        return default


def lighten(hex_color, f):
    r, g, b = (int(hex_color[i:i + 2], 16) for i in (1, 3, 5))
    return mix((r, g, b), (255, 255, 255), f)


def volume_info(path):
    """(cluster bytes, file system, label, kind) for the drive holding path."""
    drive = os.path.splitdrive(os.path.abspath(path))[0] + "\\"
    if not IS_WIN:
        try:
            cluster = os.statvfs(path).f_frsize or 4096
        except (OSError, AttributeError):
            cluster = 4096
        return cluster, "", "", "Disk"
    k32 = ctypes.windll.kernel32
    spc, bps, fr, tot = (wintypes.DWORD() for _ in range(4))
    cluster = 4096
    if k32.GetDiskFreeSpaceW(drive, ctypes.byref(spc), ctypes.byref(bps), ctypes.byref(fr), ctypes.byref(tot)):
        cluster = spc.value * bps.value or 4096
    label = ctypes.create_unicode_buffer(261)
    fs = ctypes.create_unicode_buffer(261)
    k32.GetVolumeInformationW(drive, label, 261, None, None, None, fs, 261)
    kind = {2: "Removable Disk", 3: "Local Disk", 4: "Network Drive", 5: "CD Drive", 6: "RAM Disk"}.get(
        k32.GetDriveTypeW(drive), "Disk")
    return cluster, fs.value, label.value, kind


_owner_cache = {}


def get_owner(path, cache=True):
    if not IS_WIN:
        try:
            import pwd
            return pwd.getpwuid(os.lstat(path).st_uid).pw_name
        except (ImportError, KeyError, OSError):
            return ""
    if cache and path in _owner_cache:
        return _owner_cache[path]
    adv = ctypes.windll.advapi32
    adv.GetNamedSecurityInfoW.argtypes = [ctypes.c_wchar_p, ctypes.c_int, ctypes.c_uint,
                                          ctypes.POINTER(ctypes.c_void_p), ctypes.c_void_p, ctypes.c_void_p,
                                          ctypes.c_void_p, ctypes.POINTER(ctypes.c_void_p)]
    adv.LookupAccountSidW.argtypes = [ctypes.c_wchar_p, ctypes.c_void_p, ctypes.c_wchar_p,
                                      ctypes.POINTER(wintypes.DWORD), ctypes.c_wchar_p,
                                      ctypes.POINTER(wintypes.DWORD), ctypes.POINTER(wintypes.DWORD)]
    ctypes.windll.kernel32.LocalFree.argtypes = [ctypes.c_void_p]
    sid, sd = ctypes.c_void_p(), ctypes.c_void_p()
    name = ""
    if adv.GetNamedSecurityInfoW(path, 1, 1, ctypes.byref(sid), None, None, None, ctypes.byref(sd)) == 0:
        nb, db = ctypes.create_unicode_buffer(256), ctypes.create_unicode_buffer(256)
        n, d, use = wintypes.DWORD(256), wintypes.DWORD(256), wintypes.DWORD()
        if adv.LookupAccountSidW(None, sid, nb, ctypes.byref(n), db, ctypes.byref(d), ctypes.byref(use)):
            name = nb.value
        ctypes.windll.kernel32.LocalFree(sd)
    if cache:
        _owner_cache[path] = name
    return name


def list_drives():
    if not IS_WIN:
        vols = ["/"]
        if sys.platform == "darwin" and os.path.isdir("/Volumes"):
            for name in sorted(os.listdir("/Volumes")):
                p = os.path.join("/Volumes", name)
                if os.path.isdir(p) and not os.path.islink(p):  # the start-up disk appears as a link to /
                    vols.append(p)
        return vols
    mask = ctypes.windll.kernel32.GetLogicalDrives()
    return [f"{c}:\\" for i, c in enumerate(string.ascii_uppercase) if mask >> i & 1]


class Node:
    __slots__ = ("name", "path", "parent", "children", "size", "own_size", "alloc", "own_alloc", "files",
                 "own_files", "dirs", "mtime", "atime", "error", "depth", "compressed")

    def __init__(self, name, path, parent):
        self.name, self.path, self.parent = name, path, parent
        self.children = []
        self.size = self.own_size = self.alloc = self.own_alloc = 0
        self.files = self.own_files = self.dirs = 0
        self.mtime = self.atime = 0.0
        self.error = None
        self.compressed = False
        self.depth = parent.depth + 1 if parent else 0


def make_buckets(days):
    """[(limit_days, label), ...] from a list of day thresholds."""
    vals = sorted({int(d) for d in days if str(d).strip() and int(d) > 0})

    def human(d):
        if d % 365 == 0:
            n = d // 365
            return f"{n} year" + ("s" if n > 1 else "")
        if d % 30 == 0:
            n = d // 30
            return f"{n} month" + ("s" if n > 1 else "")
        return f"{d} days"
    out, prev = [], None
    for d in vals:
        out.append((d, f"Less than {human(d)}" if prev is None else f"{human(prev)} - {human(d)}"))
        prev = d
    out.append((float("inf"), f"More than {human(prev)}" if prev else "All files"))
    return out


PRIORITY_VALUES = {"Idle": -15, "Lower priority": -1, "Normal priority": 0, "Higher priority": 1}


class Scanner(threading.Thread):
    TOP_N = 200

    def __init__(self, path, cluster, opts=None):
        super().__init__(daemon=True)
        o = opts or {}
        self.path, self.cluster = path, cluster
        self.follow = bool(o.get("follow_links", False))
        self.want_ext = bool(o.get("ext_stats", True))
        self.want_owner = bool(o.get("owner_stats", False))
        self.exclude = [x.strip().lower() for x in o.get("exclude_dirs", []) if x.strip()]
        self.include = [x.strip().lower() for x in o.get("include_files", []) if x.strip()]
        self.age_basis = o.get("age_basis", "Last change")
        self.priority = o.get("thread_priority", "Lower priority")
        self.buckets = make_buckets(o.get("age_days", [30, 90, 180, 365, 730, 1825]))
        self.stop_evt = threading.Event()
        self.files = self.dirs = self.bytes = self.errors = 0
        self.current = path
        self.done = self.cancelled = False
        self.root = None
        self.ext_stats = {}
        self.age_stats = [[0, 0] for _ in self.buckets]
        self.owner_stats = {}
        self.error_list = []
        self.top_files = []
        self.elapsed = 0.0
        self.finished_at = 0.0

    def _excluded(self, name, path):
        n, p = name.lower(), path.lower()
        return any(fnmatch.fnmatch(n, x) or fnmatch.fnmatch(p, x) for x in self.exclude)

    def _error(self, path, msg):
        self.errors += 1
        if len(self.error_list) < 1000:
            self.error_list.append((path, msg))

    def run(self):
        if IS_WIN:
            try:
                k32 = ctypes.windll.kernel32
                k32.GetCurrentThread.restype = ctypes.c_void_p
                k32.SetThreadPriority.argtypes = [ctypes.c_void_p, ctypes.c_int]
                k32.SetThreadPriority(k32.GetCurrentThread(), PRIORITY_VALUES.get(self.priority, 0))
            except Exception:
                pass
        t0 = now = time.time()
        cl = self.cluster
        root = Node(self.path, self.path, None)
        try:
            st = os.stat(self.path)
            root.mtime, root.atime = st.st_mtime, st.st_atime
        except OSError:
            pass
        visited = {os.path.normcase(os.path.realpath(self.path))}
        order, stack = [root], [root]
        ext_stats, age_stats, top, buckets = self.ext_stats, self.age_stats, self.top_files, self.buckets
        basis = self.age_basis
        while stack:
            if self.stop_evt.is_set():
                self.cancelled = True
                break
            node = stack.pop()
            self.current = node.path
            try:
                with os.scandir(node.path) as it:
                    for e in it:
                        try:
                            link = e.is_symlink() or e.is_junction()
                            if link and not self.follow:
                                continue
                            if e.is_dir(follow_symlinks=link):
                                if link:
                                    real = os.path.normcase(os.path.realpath(e.path))
                                    if real in visited:
                                        continue
                                    visited.add(real)
                                if self.exclude and self._excluded(e.name, e.path):
                                    continue
                                child = Node(e.name, e.path, node)
                                try:
                                    st = e.stat(follow_symlinks=False)
                                    child.mtime, child.atime = st.st_mtime, st.st_atime
                                    child.compressed = bool(getattr(st, "st_file_attributes", 0) & 0x800)
                                except OSError:
                                    pass
                                node.children.append(child)
                                stack.append(child)
                                order.append(child)
                                self.dirs += 1
                            elif e.is_file(follow_symlinks=link):
                                if self.include and not any(fnmatch.fnmatch(e.name.lower(), x) for x in self.include):
                                    continue
                                st = e.stat(follow_symlinks=link)
                                sz, mt = st.st_size, st.st_mtime
                                al = -(-sz // cl) * cl
                                node.own_size += sz
                                node.own_alloc += al
                                node.own_files += 1
                                if mt > node.mtime:
                                    node.mtime = mt
                                if st.st_atime > node.atime:
                                    node.atime = st.st_atime
                                if self.want_ext:
                                    dot = e.name.rfind(".")
                                    ext = e.name[dot + 1:].lower() if dot > 0 else ""
                                    rec = ext_stats.get(ext)
                                    if rec is None:
                                        ext_stats[ext] = [sz, 1, al]
                                    else:
                                        rec[0] += sz
                                        rec[1] += 1
                                        rec[2] += al
                                if self.want_owner:
                                    rec = self.owner_stats.setdefault(get_owner(e.path, cache=False) or "(unknown)",
                                                                      [0, 0])
                                    rec[0] += sz
                                    rec[1] += 1
                                if basis == "Last access":
                                    t = st.st_atime
                                elif basis == "Creation date":
                                    t = getattr(st, "st_birthtime", st.st_ctime)
                                else:
                                    t = mt
                                days = (now - t) / 86400
                                for i, (limit, _) in enumerate(buckets):
                                    if days < limit:
                                        age_stats[i][0] += sz
                                        age_stats[i][1] += 1
                                        break
                                item = (sz, e.path, mt)
                                if len(top) < self.TOP_N:
                                    heapq.heappush(top, item)
                                elif sz > top[0][0]:
                                    heapq.heapreplace(top, item)
                                self.files += 1
                                self.bytes += sz
                        except OSError as ex:
                            self._error(e.path, ex.strerror or str(ex))
            except OSError as ex:
                node.error = ex.strerror or str(ex)
                self._error(node.path, node.error)
        for n in reversed(order):
            n.size += n.own_size
            n.alloc += n.own_alloc
            n.files += n.own_files
            p = n.parent
            if p is not None:
                p.size += n.size
                p.alloc += n.alloc
                p.files += n.files
                p.dirs += n.dirs + 1
                p.mtime = max(p.mtime, n.mtime)
                p.atime = max(p.atime, n.atime)
        self.top_files = sorted(top, reverse=True)
        self.root = root
        self.elapsed = time.time() - t0
        self.finished_at = time.time()
        self.done = True


def squarify(values, x, y, w, h):
    values = [(v, p) for v, p in values if v > 0]
    total = sum(v for v, _ in values)
    out = []
    if total <= 0 or w < 1 or h < 1:
        return out
    scale = w * h / total
    items = [(v * scale, p) for v, p in values]

    def worst(row, side):
        s = sum(a for a, _ in row)
        return max(side * side * max(a for a, _ in row) / (s * s), (s * s) / (side * side * min(a for a, _ in row)))

    while items:
        side = min(w, h)
        if side <= 0:
            break
        row, i = [items[0]], 1
        while i < len(items) and worst(row + [items[i]], side) <= worst(row, side):
            row.append(items[i])
            i += 1
        items = items[i:]
        s = sum(a for a, _ in row)
        if w >= h:
            cw, yy = s / h, y
            for a, p in row:
                out.append((x, yy, cw, a / cw, p))
                yy += a / cw
            x, w = x + cw, w - cw
        else:
            rh, xx = s / w, x
            for a, p in row:
                out.append((xx, y, a / rh, rh, p))
                xx += a / rh
            y, h = y + rh, h - rh
    return out


def _win32():
    u, g = ctypes.windll.user32, ctypes.windll.gdi32
    H = ctypes.c_void_p
    u.GetWindowDC.restype = H
    u.GetWindowDC.argtypes = [H]
    u.ReleaseDC.argtypes = [H, H]
    u.PrintWindow.argtypes = [H, H, ctypes.c_uint]
    u.GetWindowRect.argtypes = [H, ctypes.POINTER(wintypes.RECT)]
    u.SetWindowPos.argtypes = [H, H, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_uint]
    u.GetForegroundWindow.restype = H
    u.SetForegroundWindow.argtypes = [H]
    u.GetWindowLongW.argtypes = [H, ctypes.c_int]
    u.SetWindowLongW.argtypes = [H, ctypes.c_int, ctypes.c_long]
    g.CreateCompatibleDC.restype = H
    g.CreateCompatibleDC.argtypes = [H]
    g.CreateCompatibleBitmap.restype = H
    g.CreateCompatibleBitmap.argtypes = [H, ctypes.c_int, ctypes.c_int]
    g.SelectObject.restype = H
    g.SelectObject.argtypes = [H, H]
    g.DeleteObject.argtypes = [H]
    g.DeleteDC.argtypes = [H]
    g.GetDIBits.argtypes = [H, H, ctypes.c_uint, ctypes.c_uint, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_uint]
    return u, g


def png_bytes(w, h, rgb):
    def chunk(tag, data):
        return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
    stride = w * 3
    raw = b"".join(b"\x00" + bytes(rgb[r * stride:(r + 1) * stride]) for r in range(h))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw, 6)) + chunk(b"IEND", b""))


def write_png(path, w, h, rgb):
    os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
    with open(path, "wb") as f:
        f.write(png_bytes(w, h, rgb))


def capture_window(hwnd, path):
    """Render the window (even when covered) with PrintWindow and save a PNG. False if blank."""
    u, g = _win32()
    rect = wintypes.RECT()
    u.GetWindowRect(hwnd, ctypes.byref(rect))
    w, h = rect.right - rect.left, rect.bottom - rect.top
    vis = wintypes.RECT()
    try:
        ctypes.windll.dwmapi.DwmGetWindowAttribute(ctypes.c_void_p(hwnd), 9, ctypes.byref(vis), ctypes.sizeof(vis))
        ox, oy = max(0, vis.left - rect.left), max(0, vis.top - rect.top)
        cw, ch = min(w - ox, vis.right - vis.left), min(h - oy, vis.bottom - vis.top)
        if cw <= 0 or ch <= 0:
            raise OSError
    except Exception:
        ox, oy, cw, ch = 0, 0, w, h
    hdc_win = u.GetWindowDC(hwnd)
    hdc = g.CreateCompatibleDC(hdc_win)
    bmp = g.CreateCompatibleBitmap(hdc_win, w, h)
    old = g.SelectObject(hdc, bmp)
    u.PrintWindow(hwnd, hdc, 2)

    class BIH(ctypes.Structure):
        _fields_ = [("biSize", ctypes.c_uint32), ("biWidth", ctypes.c_int32), ("biHeight", ctypes.c_int32),
                    ("biPlanes", ctypes.c_uint16), ("biBitCount", ctypes.c_uint16),
                    ("biCompression", ctypes.c_uint32), ("biSizeImage", ctypes.c_uint32),
                    ("biXPelsPerMeter", ctypes.c_int32), ("biYPelsPerMeter", ctypes.c_int32),
                    ("biClrUsed", ctypes.c_uint32), ("biClrImportant", ctypes.c_uint32)]
    bmi = BIH(ctypes.sizeof(BIH), w, -h, 1, 32)
    buf = ctypes.create_string_buffer(w * h * 4)
    g.GetDIBits(hdc, bmp, 0, h, buf, ctypes.byref(bmi), 0)
    g.SelectObject(hdc, old)
    g.DeleteObject(bmp)
    g.DeleteDC(hdc)
    u.ReleaseDC(hwnd, hdc_win)
    raw = buf.raw
    rgb = bytearray(cw * ch * 3)
    for r in range(ch):
        src = raw[((r + oy) * w + ox) * 4:((r + oy) * w + ox + cw) * 4]
        row = bytearray(cw * 3)
        row[0::3], row[1::3], row[2::3] = src[2::4], src[1::4], src[0::4]
        rgb[r * cw * 3:(r + 1) * cw * 3] = row
    if len(set(rgb[::997])) <= 1:
        return False
    write_png(path, cw, ch, rgb)
    return True


class SizeTree(tk.Frame):
    """Folder tree where every row carries a bar showing its share of the parent folder."""

    def __init__(self, master, app):
        super().__init__(master, bg=PANEL)
        self.app = app
        k = app.k
        self.rh = int(22 * k)
        self.indent = int(18 * k)
        self.font = tkfont.Font(family="Segoe UI", size=11)
        self.bold = tkfont.Font(family="Segoe UI", size=11, weight="bold")
        self.c = tk.Canvas(self, bg=PANEL, highlightthickness=0, takefocus=1)
        self.vs = ttk.Scrollbar(self, orient="vertical", command=self._yview)
        self.vs.pack(side="right", fill="y")
        self.c.pack(side="left", fill="both", expand=True)
        self.root_node = None
        self.opened = set()
        self.rows = []
        self.top = 0
        self.sel = None
        self.hint = "Choose a drive below or type a folder above, then press Scan (F5)."
        self.c.bind("<Configure>", lambda e: self.draw())
        self.c.bind("<Button-1>", self._click)
        self.c.bind("<Double-1>", self._double)
        self.c.bind("<Button-2>" if sys.platform == "darwin" else "<Button-3>", self._right)
        self.c.bind("<MouseWheel>", lambda e: (self._hide_tip(e), self.scroll(-3 if e.delta > 0 else 3)))
        self._tip, self._tip_job, self._tip_row = None, None, None
        self.c.bind("<Motion>", self._motion)
        self.c.bind("<Leave>", self._hide_tip)
        self.c.bind("<ButtonPress>", self._hide_tip, add="+")
        for key, fn in (("<Up>", lambda: self.move(-1)), ("<Down>", lambda: self.move(1)),
                        ("<Prior>", lambda: self.move(-self.page())), ("<Next>", lambda: self.move(self.page())),
                        ("<Home>", lambda: self.move(-10 ** 9)), ("<End>", lambda: self.move(10 ** 9)),
                        ("<Left>", self._key_left), ("<Right>", self._key_right),
                        ("<Return>", lambda: self.toggle(self.sel[0]) if self.sel else None)):
            self.c.bind(key, lambda e, f=fn: f())

    def set_root(self, node):
        self.root_node = node
        self.opened = {node}
        self.top = 0
        self.refresh()
        self.select((node, False))

    def clear(self, hint):
        self.root_node, self.rows, self.sel, self.hint = None, [], None, hint
        self.draw()

    def refresh(self):
        out = []
        if self.root_node:
            stack = [(self.root_node, False)]
            while stack:
                n, f = stack.pop()
                out.append((n, f))
                if not f and n in self.opened:
                    stack.extend(reversed(self.app.sorted_entries(n)))
        self.rows = out
        self.top = max(0, min(self.top, len(out) - self.page()))
        self.draw()

    def page(self):
        return max(1, self.c.winfo_height() // self.rh)

    def toggle(self, node, state=None):
        if not (node.children or node.own_files):
            return
        if state is None:
            state = node not in self.opened
        if state:
            self.opened.add(node)
        else:
            self.opened.discard(node)
        self.refresh()

    def expand(self, start, levels):
        todo = [start]
        while todo:
            n = todo.pop()
            if n.depth - start.depth < levels and (n.children or n.own_files):
                self.opened.add(n)
                todo.extend(n.children[:300])
        self.reveal(start)

    def collapse_all(self):
        self.opened = {self.root_node} if self.root_node else set()
        self.refresh()
        if self.root_node:
            self.select((self.root_node, False))

    def reveal(self, node, files_row=False):
        p = node if files_row else node.parent
        while p is not None:
            self.opened.add(p)
            p = p.parent
        self.refresh()
        self.select((node, files_row))

    def select(self, entry):
        self.sel = entry
        if entry in self.rows:
            i = self.rows.index(entry)
            if i < self.top:
                self.top = i
            elif i >= self.top + self.page():
                self.top = i - self.page() + 1
        self.draw()
        self.app.on_tree_select(entry)

    def move(self, d):
        if not self.rows:
            return
        i = self.rows.index(self.sel) if self.sel in self.rows else 0
        self.select(self.rows[max(0, min(len(self.rows) - 1, i + d))])

    def _key_left(self):
        if not self.sel:
            return
        n, f = self.sel
        if not f and n in self.opened and n is not self.root_node:
            self.toggle(n, False)
        elif f:
            self.select((n, False))
        elif n.parent:
            self.select((n.parent, False))

    def _key_right(self):
        if self.sel and not self.sel[1]:
            self.toggle(self.sel[0], True)

    def _row_at(self, y):
        i = self.top + int(y // self.rh)
        return i if 0 <= i < len(self.rows) else None

    def _click(self, e):
        self.c.focus_set()
        i = self._row_at(e.y)
        if i is None:
            return
        entry = self.rows[i]
        n, f = entry
        x0 = 4 + (n.depth + f) * self.indent
        if not f and x0 - 2 <= e.x <= x0 + self.indent:
            self.toggle(n)
        self.select(entry)

    def _double(self, e):
        i = self._row_at(e.y)
        if i is not None and not self.rows[i][1]:
            self.toggle(self.rows[i][0])

    def _right(self, e):
        i = self._row_at(e.y)
        if i is not None:
            self.select(self.rows[i])
            self.app.tree_menu(e, *self.rows[i])

    def scroll(self, d):
        self.top = max(0, min(self.top + d, len(self.rows) - self.page()))
        self.draw()

    def _yview(self, *a):
        if a[0] == "moveto":
            self.top = int(float(a[1]) * len(self.rows))
        elif a[0] == "scroll":
            self.top += int(a[1]) * (self.page() if a[2] == "pages" else 1)
        self.scroll(0)

    def draw(self):
        c, app, k = self.c, self.app, self.app.k
        c.delete("all")
        W, H = c.winfo_width(), c.winfo_height()
        if not self.rows:
            badge = brand_image("badge", min(int(200 * k), max(96, int(min(W, H) * 0.45))))
            ty = H / 2
            if badge is not None and H > badge.height() + 80:
                c.create_image(W / 2, H / 2 - 24 * k, image=badge, anchor="s")
                ty = H / 2 + 2 * k
            c.create_text(W / 2, ty, text=self.hint, fill=MUTED, font=("Segoe UI", 10), width=max(50, W - 40),
                          anchor="n" if badge is not None else "center")
            self.vs.set(0, 1)
            return
        rh = self.rh
        valw = self.bold.measure("00,000,000.0 MB")
        st = app.settings
        g0, g1 = hex_rgb(st.get("grad_start"), BAR_START), hex_rgb(st.get("grad_end"), BAR_END)
        root_val = app.metric(self.root_node, False)
        for i in range(self.top, min(len(self.rows), self.top + H // rh + 1)):
            n, f = self.rows[i]
            y = (i - self.top) * rh
            cy = y + rh / 2
            depth = n.depth + (1 if f else 0)
            x0 = 4 + depth * self.indent
            if (n, f) == self.sel:
                c.create_rectangle(1, y, W - 2, y + rh - 1, fill="#E5E5EA", outline="#9A9AA5")
            if not f and (n.children or n.own_files):
                b = int(4 * k)
                ex, ey = x0 + int(5 * k), cy
                c.create_rectangle(ex - b, ey - b, ex + b, ey + b, outline="#8A8A8A", fill=PANEL)
                c.create_line(ex - b + 2, ey, ex + b - 1, ey, fill="#333")
                if n not in self.opened:
                    c.create_line(ex, ey - b + 2, ex, ey + b - 1, fill="#333")
            ix = x0 + int(14 * k)
            self._icon(ix, cy, n, f)
            bx = ix + int(22 * k)
            parent = n if f else n.parent
            val = app.metric(n, f)
            pval = app.metric(parent, False) if parent else val
            frac = val / pval if pval else (1.0 if parent is None else 0.0)
            full = max(1, W - bx - 4)
            bw = full * frac
            if bw >= 1:
                if st.get("gradient", True):
                    step = max(3, full / 90)
                    sx = 0.0
                    while sx < bw:
                        ex2 = min(bw, sx + step)
                        c.create_rectangle(bx + sx, y + 2, bx + ex2 + 1, y + rh - 2, outline="",
                                           fill=mix(g0, g1, (sx / full) * 0.6 + frac * 0.4))
                        sx = ex2
                else:
                    c.create_rectangle(bx, y + 2, bx + bw, y + rh - 2, outline="", fill=mix(g0, g1, 0.35))
            big = parent is None or self._is_big(n, f, frac, root_val)
            fnt = self.bold if big else self.font
            c.create_text(bx + valw, cy, text=app.metric_text(n, f, pval), anchor="e", font=fnt, fill=TEXT)
            name = "[Files]" if f else n.name
            if n.error and not f:
                color = "#B00020"
            elif n.compressed and not f and st.get("compressed_color", True):
                color = "#0050D0"
            else:
                color = "#1A3D8F" if big and parent is not None else TEXT
            c.create_text(bx + valw + int(10 * k), cy, text=name, anchor="w", font=fnt, fill=color)
        n = len(self.rows)
        self.vs.set(self.top / n, min(1.0, (self.top + self.page()) / n))

    def _is_big(self, n, f, frac, root_val):
        st = self.app.settings
        try:
            limit = float(st.get("bold_value", 15))
        except (TypeError, ValueError):
            limit = 15.0
        mode = st.get("bold_mode", "% of the scanned folder")
        if mode == "MB":
            return (n.own_size if f else n.size) >= limit * 1024 * 1024
        if mode == "% of the parent folder":
            return frac * 100 >= limit
        val = self.app.metric(n, f)
        return root_val > 0 and val * 100.0 / root_val >= limit

    # ------------------------------------------------------------ tooltips --
    def _motion(self, e):
        if not self.app.settings.get("long_tips", True):
            return
        i = self._row_at(e.y)
        if i == self._tip_row:
            return
        self._hide_tip()
        self._tip_row = i
        if i is not None:
            self._tip_job = self.after(600, lambda: self._show_tip(i, e.x_root, e.y_root))

    def _show_tip(self, i, x, y):
        self._tip_job = None
        if i >= len(self.rows):
            return
        n, f = self.rows[i]
        a = self.app
        u = a.unit.get()
        lines = [n.path + ("\\[Files]" if f else ""),
                 f"Size: {fmt_size(n.own_size if f else n.size, u)}     Allocated: "
                 f"{fmt_size(n.own_alloc if f else n.alloc, u)}",
                 f"Files: {n.own_files if f else n.files:,}     Folders: {0 if f else n.dirs:,}",
                 f"Last change: {a.fdate(n.mtime)}     Last access: {a.fdate(n.atime)}"]
        if n.parent is not None or f:
            parent = n if f else n.parent
            lines.append(f"{pct_text(a.metric(n, f), a.metric(parent))} of {parent.name}")
        if n.compressed and not f:
            lines.append("Compressed folder")
        if n.error and not f:
            lines.append(f"Could not be read: {n.error}")
        tw = tk.Toplevel(self)
        tw.wm_overrideredirect(True)
        tw.geometry(f"+{x + 16}+{y + 18}")
        tk.Label(tw, text="\n".join(lines), justify="left", bg="#FFFFE1", relief="solid", bd=1,
                 font=("Segoe UI", 9), padx=6, pady=4).pack()
        self._tip = tw

    def _hide_tip(self, _e=None):
        if self._tip_job:
            self.after_cancel(self._tip_job)
            self._tip_job = None
        if self._tip is not None:
            self._tip.destroy()
            self._tip = None
        if _e is not None:
            self._tip_row = None

    def _icon(self, x, cy, n, f):
        c, s = self.c, self.app.k
        if f:
            c.create_polygon(x + 2 * s, cy - 8 * s, x + 10 * s, cy - 8 * s, x + 14 * s, cy - 4 * s, x + 14 * s,
                             cy + 8 * s, x + 2 * s, cy + 8 * s, fill=PANEL, outline="#777")
        elif n.parent is None:
            c.create_rectangle(x, cy - 5 * s, x + 16 * s, cy + 5 * s, fill="#D7DCE3", outline="#59606B")
            c.create_rectangle(x + 11 * s, cy - 1 * s, x + 14 * s, cy + 2 * s, fill="#2BB673", outline="")
        else:
            c.create_rectangle(x, cy - 6 * s, x + 7 * s, cy - 3 * s, fill="#E8A317", outline="")
            c.create_rectangle(x, cy - 4 * s, x + 16 * s, cy + 7 * s, fill="#FFC83D", outline="#D69A1C")
            if n.error:
                c.create_text(x + 8 * s, cy + 1 * s, text="!", fill="#B00020", font=("Segoe UI", 8, "bold"))
