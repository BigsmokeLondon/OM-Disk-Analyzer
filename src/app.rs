//! The main window: menus, toolbar, folder tree, drive list and status bar.
//! The result tabs and dialog windows are in `app/tabs.rs`.

mod tabs;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use egui::{
    pos2, vec2, Align, Align2, Button, Color32, Context, FontFamily, FontId, Key, Label, Layout, Rect,
    RichText, ScrollArea, Sense, Stroke, StrokeKind, TextureHandle, Ui, ViewportCommand,
};

use crate::draw::*;
use crate::fmt::{self, thousands, Unit};
use crate::scan::{self, Node, NodeId, Progress, ScanResult};
use crate::settings::{self, BoldMode, HistoryEntry, Mode, Settings, SortBy};
use crate::sys;
use crate::{Launch, APP_NAME, COMPANY, EDITION};
use tabs::{table, Cell, IconKind};

type Entry = (NodeId, bool);

/// One line of the folder tree: a folder, a folder's "[Files]" line, or one file.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Row {
    Node(Entry),
    File(NodeId, usize),
}

/// A file directly inside a folder, read when the folder is opened in the tree.
#[derive(Clone, Debug)]
struct FileItem {
    name: String,
    path: PathBuf,
    size: u64,
    alloc: u64,
    mtime: i64,
    atime: i64,
}

/// What a confirmed delete removes from the scanned tree.
#[derive(Clone, Copy, Debug)]
enum DelTarget {
    Folder(NodeId),
    File { folder: NodeId, size: u64, alloc: u64 },
}

#[derive(Clone, Debug)]
struct DeleteReq {
    path: PathBuf,
    is_dir: bool,
    size: u64,
    files: u64,
    target: Option<DelTarget>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Chart,
    Details,
    Extensions,
    Users,
    Age,
    Top,
    History,
    Treemap,
}

const TABS: [(Tab, &str, &str); 8] = [
    (Tab::Chart, "◕", "Chart"),
    (Tab::Details, "☰", "Details"),
    (Tab::Extensions, "⁂", "Extensions"),
    (Tab::Users, "👥", "Users"),
    (Tab::Age, "⌛", "Age of Files"),
    (Tab::Top, "⬇", "Top 100 Files"),
    (Tab::History, "🕘", "History"),
    (Tab::Treemap, "▦", "Treemap"),
];

pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

pub fn regular(size: f32) -> FontId {
    FontId::proportional(size)
}

const TREE_ROW: f32 = 20.0;

struct DetailRow {
    name: String,
    path: PathBuf,
    node: Option<NodeId>,
    size: u64,
    alloc: u64,
    files: u64,
    dirs: u64,
    mtime: i64,
    atime: i64,
}

struct Textures {
    icon: TextureHandle,
    badge: TextureHandle,
}

#[derive(Default)]
struct SearchState {
    open: bool,
    pattern: String,
    min_mb: String,
    stop: Option<Arc<AtomicBool>>,
    done: Option<Arc<AtomicBool>>,
    results: Arc<Mutex<Vec<(String, PathBuf, u64, i64)>>>,
    sel: Option<usize>,
}

pub struct App {
    st: Settings,
    path_text: String,
    vol: sys::VolumeInfo,
    drives: Vec<sys::DriveInfo>,
    drive_sel: Option<usize>,
    scanning: Option<Arc<Progress>>,
    nodes: Arc<Vec<Node>>,
    res: Option<ScanResult>,
    opened: HashSet<NodeId>,
    rows: Vec<Row>,
    tree_sel: Option<Row>,
    sel_file: Option<FileItem>,
    file_cache: RefCell<HashMap<NodeId, Arc<Vec<FileItem>>>>,
    pending_delete: Option<DeleteReq>,
    is_admin: bool,
    expand_after_scan: bool,
    rows_dirty: bool,
    sel: Option<Entry>,
    scroll_to_sel: bool,
    tree_scroll: Option<f32>,
    tree_offset: f32,
    tree_view_h: f32,
    tab: Tab,
    details: Vec<DetailRow>,
    details_key: Option<Entry>,
    details_sort: (usize, bool),
    details_sel: Option<usize>,
    users_key: Option<Entry>,
    users: Vec<(String, u64, u64)>,
    owners: RefCell<HashMap<PathBuf, String>>,
    history: Vec<HistoryEntry>,
    top_sel: Option<usize>,
    status: String,
    hover_status: Option<String>,
    title: String,
    tex: Textures,
    show_about: bool,
    update: crate::update::Checker,
    options: Option<Settings>,
    options_page: usize,
    explorer_menu: bool,
    show_errors: bool,
    search: SearchState,
    screenshot: Option<PathBuf>,
    frames: u32,
    ready_frames: u32,
    shot_sent: bool,
    shot_done: Arc<AtomicBool>,
}

pub fn load_texture(ctx: &Context, name: &str, bytes: &[u8]) -> TextureHandle {
    let img = image::load_from_memory(bytes).map(|i| i.to_rgba8()).unwrap_or_else(|_| image::RgbaImage::new(1, 1));
    let (w, h) = img.dimensions();
    let ci = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw());
    ctx.load_texture(name, ci, egui::TextureOptions::LINEAR)
}

pub fn setup_fonts(ctx: &Context) {
    let mut fonts = egui::FontDefinitions::default();
    let dir = PathBuf::from(std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into())).join("Fonts");
    let load = |f: &str| std::fs::read(dir.join(f)).ok();
    let mut prop = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    if let Some(b) = load("segoeui.ttf") {
        fonts.font_data.insert("segoe".into(), Arc::new(egui::FontData::from_owned(b)));
        prop.insert(0, "segoe".into());
    }
    if let Some(b) = load("seguisym.ttf") {
        fonts.font_data.insert("segoe_sym".into(), Arc::new(egui::FontData::from_owned(b)));
        prop.push("segoe_sym".into());
    }
    let mut boldf = prop.clone();
    if let Some(b) = load("segoeuib.ttf") {
        fonts.font_data.insert("segoe_bold".into(), Arc::new(egui::FontData::from_owned(b)));
        boldf.insert(0, "segoe_bold".into());
    }
    fonts.families.insert(FontFamily::Proportional, prop);
    fonts.families.insert(FontFamily::Name("bold".into()), boldf);
    ctx.set_fonts(fonts);
}

pub fn setup_style(ctx: &Context) {
    ctx.set_visuals(egui::Visuals::light());
    ctx.global_style_mut(|s| {
        use egui::TextStyle;
        s.text_styles.insert(TextStyle::Body, regular(14.0));
        s.text_styles.insert(TextStyle::Button, regular(14.0));
        s.text_styles.insert(TextStyle::Small, regular(11.5));
        s.text_styles.insert(TextStyle::Heading, bold(17.0));
        s.spacing.item_spacing = vec2(6.0, 3.0);
        s.spacing.button_padding = vec2(5.0, 2.0);
        s.interaction.selectable_labels = false;
        let v = &mut s.visuals;
        v.panel_fill = BG;
        v.window_fill = PANEL;
        v.extreme_bg_color = PANEL;
        v.faint_bg_color = Color32::from_rgb(0xF6, 0xF6, 0xF8);
        v.override_text_color = Some(TEXT);
        v.selection.bg_fill = SELECT;
        v.selection.stroke = Stroke::new(1.0, NAVY);
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
        v.widgets.inactive.weak_bg_fill = BG;
        v.widgets.inactive.bg_fill = BG;
        v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0xE5, 0xF1, 0xFB);
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x7E, 0xB4, 0xEA));
        v.widgets.active.weak_bg_fill = SELECT;
    });
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, launch: Launch) -> Self {
        setup_fonts(&cc.egui_ctx);
        setup_style(&cc.egui_ctx);
        let tex = Textures {
            icon: load_texture(&cc.egui_ctx, "om-icon", include_bytes!("../assets/icon_64.png")),
            badge: load_texture(&cc.egui_ctx, "om-badge", include_bytes!("../assets/badge_256.png")),
        };
        let mut st = Settings::load();
        if launch.show_files {
            st.show_files = true;
        }
        // the Details list starts sorted by what the tree shows (Allocated by default)
        let details_sort = (if st.mode == Mode::Alloc { 2 } else { 1 }, true);
        let tab = launch
            .tab
            .as_deref()
            .and_then(|t| TABS.iter().find(|(_, _, n)| n.to_lowercase().starts_with(&t.to_lowercase())).map(|x| x.0))
            .unwrap_or(Tab::Details);
        let mut app = Self {
            path_text: st.recent.first().cloned().unwrap_or_else(sys::default_root),
            st,
            vol: sys::VolumeInfo::default(),
            drives: sys::list_drives(),
            drive_sel: None,
            scanning: None,
            nodes: Arc::new(Vec::new()),
            res: None,
            opened: HashSet::new(),
            rows: Vec::new(),
            tree_sel: None,
            sel_file: None,
            file_cache: RefCell::new(HashMap::new()),
            pending_delete: None,
            is_admin: sys::is_admin(),
            expand_after_scan: false,
            rows_dirty: false,
            sel: None,
            scroll_to_sel: false,
            tree_scroll: None,
            tree_offset: 0.0,
            tree_view_h: 300.0,
            tab,
            details: Vec::new(),
            details_key: None,
            details_sort,
            details_sel: None,
            users_key: None,
            users: Vec::new(),
            owners: RefCell::new(HashMap::new()),
            history: settings::load_history(),
            top_sel: None,
            status: "Ready".into(),
            hover_status: None,
            title: String::new(),
            tex,
            show_about: launch.about,
            update: crate::update::Checker::new(),
            options: None,
            options_page: 0,
            explorer_menu: false,
            show_errors: false,
            search: SearchState { min_mb: "0".into(), ..Default::default() },
            screenshot: launch.screenshot,
            frames: 0,
            ready_frames: 0,
            shot_sent: false,
            shot_done: Arc::new(AtomicBool::new(false)),
        };
        if launch.options {
            app.open_options();
        }
        if let Some(p) = launch.start {
            app.start_scan(&p);
        }
        app.expand_after_scan = launch.show_files;
        if app.st.check_updates && app.screenshot.is_none() {
            app.update.start(false);
        }
        app
    }

    // ------------------------------------------------------------ values --
    fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id]
    }

    fn metric(&self, (id, f): Entry) -> u64 {
        let n = self.node(id);
        match self.st.mode {
            Mode::Alloc => {
                if f {
                    n.own_alloc
                } else {
                    n.alloc
                }
            }
            Mode::Files => {
                if f {
                    n.own_files
                } else {
                    n.files
                }
            }
            _ => {
                if f {
                    n.own_size
                } else {
                    n.size
                }
            }
        }
    }

    fn parent_of(&self, (id, f): Entry) -> Option<NodeId> {
        if f {
            Some(id)
        } else {
            self.node(id).parent
        }
    }

    fn metric_text(&self, e: Entry, parent_val: u64) -> String {
        let v = self.metric(e);
        match self.st.mode {
            Mode::Files => format!("{} Files", thousands(v)),
            Mode::Percent => {
                if self.parent_of(e).is_some() {
                    fmt::pct(v, parent_val)
                } else {
                    "100.0 %".into()
                }
            }
            _ => fmt::size(v, self.st.unit),
        }
    }

    fn val_text(&self, v: u64) -> String {
        if self.st.mode == Mode::Files {
            format!("{} Files", thousands(v))
        } else {
            fmt::size(v, self.st.unit)
        }
    }

    /// Files directly in a folder (read once, then kept until the folder changes).
    fn files_of(&self, id: NodeId) -> Arc<Vec<FileItem>> {
        if let Some(v) = self.file_cache.borrow().get(&id) {
            return v.clone();
        }
        let cl = self.res.as_ref().map(|r| r.cluster).unwrap_or(4096);
        let v: Vec<FileItem> = scan::files_in(&self.node(id).path)
            .into_iter()
            .map(|(name, path, size, mtime, atime)| {
                let alloc = scan::allocated(size, cl);
                FileItem { name, path, size, alloc, mtime, atime }
            })
            .collect();
        let v = Arc::new(v);
        self.file_cache.borrow_mut().insert(id, v.clone());
        v
    }

    fn file_metric(&self, f: &FileItem) -> u64 {
        match self.st.mode {
            Mode::Alloc => f.alloc,
            Mode::Files => 1,
            _ => f.size,
        }
    }

    fn row_metric(&self, r: Row) -> u64 {
        match r {
            Row::Node(e) => self.metric(e),
            Row::File(id, i) => self.files_of(id).get(i).map(|f| self.file_metric(f)).unwrap_or(0),
        }
    }

    /// Whether a folder opens in the tree. The original shows a folder's "[Files]" line only
    /// when the folder also has subfolders (setting AlwaysCreateFilesItem, default off).
    fn expandable(&self, id: NodeId) -> bool {
        let n = self.node(id);
        !n.children.is_empty() || (self.st.show_files && n.own_files > 0)
    }

    fn sorted_rows(&self, id: NodeId) -> Vec<Row> {
        let n = self.node(id);
        let mut v: Vec<Row> = n.children.iter().map(|&c| Row::Node((c, false))).collect();
        let files = if self.st.show_files { Some(self.files_of(id)) } else { None };
        if let Some(f) = &files {
            v.extend((0..f.len()).map(|i| Row::File(id, i)));
        } else if n.own_files > 0 && !n.children.is_empty() {
            v.push(Row::Node((id, true)));
        }
        let fi = |r: &Row| -> Option<FileItem> {
            match r {
                Row::File(_, i) => files.as_ref().and_then(|f| f.get(*i).cloned()),
                _ => None,
            }
        };
        match self.st.sort {
            SortBy::Name => v.sort_by_cached_key(|r| match r {
                Row::Node((c, false)) => self.node(*c).name.to_lowercase(),
                Row::Node(_) => String::new(),
                Row::File(..) => fi(r).map(|f| f.name.to_lowercase()).unwrap_or_default(),
            }),
            SortBy::Files => v.sort_by_cached_key(|r| {
                std::cmp::Reverse(match r {
                    Row::Node((c, f)) => {
                        if *f {
                            self.node(*c).own_files
                        } else {
                            self.node(*c).files
                        }
                    }
                    Row::File(..) => 1,
                })
            }),
            SortBy::Changed => v.sort_by_cached_key(|r| {
                std::cmp::Reverse(match r {
                    Row::Node((c, _)) => self.node(*c).mtime,
                    Row::File(..) => fi(r).map(|f| f.mtime).unwrap_or(0),
                })
            }),
            SortBy::Size => v.sort_by_cached_key(|r| std::cmp::Reverse(self.row_metric(*r))),
        }
        v
    }

    fn rebuild_rows(&mut self) {
        self.rows_dirty = false;
        let mut out = Vec::new();
        if !self.nodes.is_empty() {
            let mut stack = vec![Row::Node((0usize, false))];
            while let Some(r) = stack.pop() {
                out.push(r);
                if let Row::Node((id, false)) = r {
                    if self.opened.contains(&id) {
                        let mut ch = self.sorted_rows(id);
                        ch.reverse();
                        stack.extend(ch);
                    }
                }
            }
        }
        self.rows = out;
        if self.scroll_to_sel {
            self.scroll_to_sel = false;
            self.ensure_visible();
        }
    }

    fn ensure_visible(&mut self) {
        let Some(sel) = self.tree_sel else { return };
        let Some(i) = self.rows.iter().position(|&e| e == sel) else { return };
        let y = i as f32 * TREE_ROW;
        if y < self.tree_offset {
            self.tree_scroll = Some(y);
        } else if y + TREE_ROW > self.tree_offset + self.tree_view_h {
            self.tree_scroll = Some(y + TREE_ROW - self.tree_view_h);
        }
    }

    fn select(&mut self, e: Entry) {
        self.sel = Some(e);
        self.tree_sel = Some(Row::Node(e));
        self.sel_file = None;
        self.details_sel = None;
    }

    fn select_row(&mut self, r: Row) {
        match r {
            Row::Node(e) => self.select(e),
            Row::File(id, i) => {
                self.select((id, true));
                self.tree_sel = Some(r);
                self.sel_file = self.files_of(id).get(i).cloned();
            }
        }
    }

    fn reveal(&mut self, id: NodeId) {
        let mut p = self.node(id).parent;
        while let Some(x) = p {
            self.opened.insert(x);
            p = self.node(x).parent;
        }
        self.select((id, false));
        self.rows_dirty = true;
        self.scroll_to_sel = true;
    }

    fn toggle(&mut self, id: NodeId, state: Option<bool>) {
        if !self.expandable(id) {
            return;
        }
        let open = state.unwrap_or(!self.opened.contains(&id));
        if open {
            self.opened.insert(id);
        } else {
            self.opened.remove(&id);
        }
        self.rows_dirty = true;
    }

    fn expand(&mut self, levels: u16) {
        if self.nodes.is_empty() {
            return;
        }
        let start = self.sel.map(|e| e.0).unwrap_or(0);
        let d0 = self.node(start).depth;
        let mut todo = vec![start];
        while let Some(id) = todo.pop() {
            if self.node(id).depth - d0 < levels && self.expandable(id) {
                self.opened.insert(id);
                todo.extend(self.nodes[id].children.iter().take(500));
            }
        }
        self.rows_dirty = true;
    }

    fn collapse_all(&mut self) {
        self.opened.clear();
        if !self.nodes.is_empty() {
            self.opened.insert(0);
            self.select((0, false));
        }
        self.rows_dirty = true;
        self.tree_scroll = Some(0.0);
    }

    fn go_up(&mut self) {
        if let Some((id, f)) = self.sel {
            if f {
                self.reveal(id);
                return;
            }
            if let Some(p) = self.node(id).parent {
                self.reveal(p);
                return;
            }
            let path = self.node(id).path.clone();
            if let Some(parent) = path.parent() {
                let s = parent.to_string_lossy().into_owned();
                if !s.is_empty() {
                    self.start_scan(&s);
                }
            }
        }
    }

    fn move_sel(&mut self, d: i64) {
        if self.rows.is_empty() {
            return;
        }
        let i = self.tree_sel.and_then(|s| self.rows.iter().position(|&e| e == s)).unwrap_or(0) as i64;
        let j = (i + d).clamp(0, self.rows.len() as i64 - 1) as usize;
        self.select_row(self.rows[j]);
        self.ensure_visible();
    }

    // ------------------------------------------------------------ delete --
    /// Ask to delete the folder or file on a tree line.
    fn ask_delete_row(&mut self, r: Row) {
        match r {
            Row::Node((id, false)) => {
                let n = self.node(id);
                if n.parent.is_none() {
                    self.status = "The scanned folder itself can't be deleted from here; select something inside it.".into();
                    return;
                }
                self.pending_delete = Some(DeleteReq {
                    path: n.path.clone(),
                    is_dir: true,
                    size: n.size,
                    files: n.files,
                    target: Some(DelTarget::Folder(id)),
                });
            }
            Row::Node((_, true)) => {
                self.status = "Select a single file to delete it (View > Show files in tree, or the Details list).".into();
            }
            Row::File(id, i) => {
                if let Some(f) = self.files_of(id).get(i).cloned() {
                    self.ask_delete_file(id, &f);
                }
            }
        }
    }

    fn ask_delete_file(&mut self, folder: NodeId, f: &FileItem) {
        self.pending_delete = Some(DeleteReq {
            path: f.path.clone(),
            is_dir: false,
            size: f.size,
            files: 1,
            target: Some(DelTarget::File { folder, size: f.size, alloc: f.alloc }),
        });
    }

    /// Ask to delete a file found by path (Top 100, search): its folder is looked up in the tree.
    fn ask_delete_path(&mut self, path: &Path, size: u64) {
        let alloc = scan::allocated(size, self.res.as_ref().map(|r| r.cluster).unwrap_or(4096));
        let folder = path.parent().and_then(|p| self.nodes.iter().position(|n| n.path == p));
        self.pending_delete = Some(DeleteReq {
            path: path.to_path_buf(),
            is_dir: false,
            size,
            files: 1,
            target: folder.map(|folder| DelTarget::File { folder, size, alloc }),
        });
    }

    /// Take a deleted folder or file out of the scanned totals, so the tree stays right without a rescan.
    fn apply_delete(&mut self, t: DelTarget, path: &Path) {
        let nodes = Arc::make_mut(&mut self.nodes);
        let (start, size, alloc, files, dirs) = match t {
            DelTarget::Folder(c) => {
                let n = &nodes[c];
                let Some(p) = n.parent else { return };
                let v = (p, n.size, n.alloc, n.files, n.dirs + 1);
                nodes[p].children.retain(|&x| x != c);
                v
            }
            DelTarget::File { folder, size, alloc } => {
                let n = &mut nodes[folder];
                n.own_size = n.own_size.saturating_sub(size);
                n.own_alloc = n.own_alloc.saturating_sub(alloc);
                n.own_files = n.own_files.saturating_sub(1);
                (folder, size, alloc, 1, 0)
            }
        };
        let mut a = Some(start);
        while let Some(i) = a {
            let n = &mut nodes[i];
            n.size = n.size.saturating_sub(size);
            n.alloc = n.alloc.saturating_sub(alloc);
            n.files = n.files.saturating_sub(files);
            n.dirs = n.dirs.saturating_sub(dirs);
            a = n.parent;
        }
        if let Some(res) = &mut self.res {
            res.top_files.retain(|t| !t.path.starts_with(path));
        }
        self.file_cache.borrow_mut().remove(&start);
        self.details_key = None;
        self.users_key = None;
        self.rows_dirty = true;
        // keep the selection on something that still exists
        let mut inside = false;
        if let (Some((id, _)), DelTarget::Folder(c)) = (self.sel, &t) {
            let mut x = Some(id);
            while let Some(i) = x {
                inside |= i == *c;
                x = self.nodes[i].parent;
            }
        }
        if inside || matches!(self.tree_sel, Some(Row::File(..))) {
            self.select((start, false));
        }
    }

    fn is_big(&self, e: Entry, frac: f64, root_val: u64) -> bool {
        let limit = self.st.bold_value;
        match self.st.bold_mode {
            BoldMode::Megabytes => {
                let n = self.node(e.0);
                (if e.1 { n.own_size } else { n.size }) as f64 >= limit * 1048576.0
            }
            BoldMode::PercentOfParent => frac * 100.0 >= limit,
            BoldMode::PercentOfScan => root_val > 0 && self.metric(e) as f64 * 100.0 / root_val as f64 >= limit,
        }
    }

    fn owner(&self, path: &Path) -> String {
        if let Some(o) = self.owners.borrow().get(path) {
            return o.clone();
        }
        let o = sys::owner_of(path);
        self.owners.borrow_mut().insert(path.to_path_buf(), o.clone());
        o
    }

    // -------------------------------------------------------------- scan --
    fn start_scan(&mut self, raw: &str) {
        let mut p = raw.trim().trim_matches('"').to_string();
        if p.len() == 2 && p.ends_with(':') {
            p.push('\\');
        }
        let path = PathBuf::from(&p);
        if !path.is_dir() {
            self.status = format!("\"{p}\" is not a folder that can be opened.");
            return;
        }
        self.stop_scan();
        self.vol = sys::volume_info(&path);
        self.st.remember(&p);
        self.st.save();
        self.path_text = p.clone();
        self.nodes = Arc::new(Vec::new());
        self.res = None;
        self.rows.clear();
        self.sel = None;
        self.details_key = None;
        self.details.clear();
        self.users_key = None;
        self.owners.borrow_mut().clear();
        self.file_cache.borrow_mut().clear();
        self.tree_sel = None;
        self.sel_file = None;
        self.scanning = Some(scan::start(path, self.vol.cluster, self.st.scan_options()));
        self.status = format!("Scanning {p} ...");
    }

    fn stop_scan(&mut self) {
        if let Some(s) = &self.scanning {
            s.stop.store(true, Ordering::SeqCst);
        }
    }

    fn poll_scan(&mut self, ctx: &Context) {
        let Some(prog) = self.scanning.clone() else { return };
        if !prog.done.load(Ordering::SeqCst) {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            return;
        }
        self.scanning = None;
        let Some(mut res) = prog.result.lock().unwrap().take() else { return };
        self.nodes = Arc::new(std::mem::take(&mut res.nodes));
        let root = &self.nodes[0];
        self.status = format!(
            "{}: {} files in {} folders, {:.1} s{}",
            if res.cancelled { "Scan stopped" } else { "Scan complete" },
            thousands(root.files),
            thousands(root.dirs),
            res.elapsed,
            if res.error_count > 0 {
                format!(" - {} items could not be read (click to see them)", thousands(res.error_count))
            } else {
                String::new()
            }
        );
        if !res.cancelled {
            settings::add_history(HistoryEntry {
                date: res.finished_at as f64,
                path: root.path.to_string_lossy().into_owned(),
                size: root.size,
                alloc: root.alloc,
                files: root.files,
                dirs: root.dirs,
            });
            self.history = settings::load_history();
        }
        self.res = Some(res);
        self.opened = HashSet::from([0]);
        self.select((0, false));
        self.rows_dirty = true;
        self.tree_scroll = Some(0.0);
        self.drives = sys::list_drives();
        if std::mem::take(&mut self.expand_after_scan) {
            // test aid: open the biggest subfolder so its files show
            if let Some(&c) = self.nodes[0].children.iter().max_by_key(|&&c| self.nodes[c].size) {
                self.opened.insert(c);
                if let Some(&g) = self.nodes[c].children.iter().max_by_key(|&&g| self.nodes[g].size) {
                    self.opened.insert(g);
                }
            }
        }
    }

    // ------------------------------------------------------------- data --
    fn ensure_details(&mut self) {
        if self.details_key == self.sel {
            return;
        }
        self.details_key = self.sel;
        self.details.clear();
        let Some((id, f)) = self.sel else { return };
        let n = self.node(id);
        let mut rows = Vec::new();
        if !f {
            for &c in &n.children {
                let ch = self.node(c);
                rows.push(DetailRow {
                    name: ch.name.clone(),
                    path: ch.path.clone(),
                    node: Some(c),
                    size: ch.size,
                    alloc: ch.alloc,
                    files: ch.files,
                    dirs: ch.dirs,
                    mtime: ch.mtime,
                    atime: ch.atime,
                });
            }
        }
        if n.own_files > 0 {
            let cl = self.res.as_ref().map(|r| r.cluster).unwrap_or(4096);
            for (name, path, size, mtime, atime) in scan::files_in(&n.path) {
                rows.push(DetailRow {
                    name,
                    path,
                    node: None,
                    size,
                    alloc: scan::allocated(size, cl),
                    files: 1,
                    dirs: 0,
                    mtime,
                    atime,
                });
            }
        }
        self.details = rows;
        self.sort_details();
    }

    fn sort_details(&mut self) {
        let (col, desc) = self.details_sort;
        let d = &mut self.details;
        match col {
            0 => d.sort_by_key(|r| r.name.to_lowercase()),
            2 => d.sort_by_key(|r| r.alloc),
            3 => d.sort_by_key(|r| r.files),
            4 => d.sort_by_key(|r| r.dirs),
            6 => d.sort_by_key(|r| r.mtime),
            7 => d.sort_by_key(|r| r.atime),
            8 => {
                let owners = self.owners.borrow();
                d.sort_by_cached_key(|r| owners.get(&r.path).cloned().unwrap_or_default());
            }
            _ => d.sort_by_key(|r| r.size),
        }
        if desc {
            d.reverse();
        }
    }

    fn ensure_users(&mut self) {
        if self.users_key == self.sel {
            return;
        }
        self.users_key = self.sel;
        self.users.clear();
        let Some((id, f)) = self.sel else { return };
        let mut stats: HashMap<String, (u64, u64)> = HashMap::new();
        if let Some(res) = &self.res {
            if res.owner_enabled && id == 0 && !f {
                for (u, (s, c)) in &res.owner_stats {
                    stats.insert(u.clone(), (*s, *c));
                }
            }
        }
        if stats.is_empty() {
            let n = self.node(id);
            let unknown = |s: String| if s.is_empty() { "(unknown)".to_string() } else { s };
            if !f {
                for &c in &n.children {
                    let ch = self.node(c);
                    let r = stats.entry(unknown(self.owner(&ch.path))).or_insert((0, 0));
                    r.0 += ch.size;
                    r.1 += ch.files;
                }
            }
            for (_, path, size, _, _) in scan::files_in(&n.path) {
                let r = stats.entry(unknown(sys::owner_of(&path))).or_insert((0, 0));
                r.0 += size;
                r.1 += 1;
            }
        }
        let mut v: Vec<(String, u64, u64)> = stats.into_iter().map(|(k, (s, c))| (k, s, c)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        self.users = v;
    }

    /// The children of a folder as chart items (value, label, node, is "other"), biggest first, capped.
    fn chart_items(&self, id: NodeId, limit: usize) -> Vec<(u64, String, Option<NodeId>, bool)> {
        let n = self.node(id);
        let mut items: Vec<(u64, String, Option<NodeId>, bool)> = n
            .children
            .iter()
            .map(|&c| (self.metric((c, false)), self.node(c).name.clone(), Some(c), false))
            .filter(|t| t.0 > 0)
            .collect();
        let fv = self.metric((id, true));
        if fv > 0 {
            items.push((fv, "[Files]".into(), None, false));
        }
        items.sort_by(|a, b| b.0.cmp(&a.0));
        if items.len() > limit {
            let rest: u64 = items[limit - 1..].iter().map(|t| t.0).sum();
            let k = items.len() - limit + 1;
            items.truncate(limit - 1);
            items.push((rest, format!("Other ({k} items)"), None, true));
        }
        items
    }

    // ------------------------------------------------------------ actions --
    fn browse(&mut self) {
        if let Some(p) = rfd::FileDialog::new().set_title("Select a folder to scan").pick_folder() {
            self.start_scan(&p.to_string_lossy());
        }
    }

    fn export_csv(&mut self) {
        if self.nodes.is_empty() {
            self.status = "Scan a drive or folder first.".into();
            return;
        }
        let Some(p) = rfd::FileDialog::new()
            .add_filter("CSV file", &["csv"])
            .set_file_name("OM Disk Analyzer export.csv")
            .save_file()
        else {
            return;
        };
        match self.write_csv(&p) {
            Ok(n) => self.status = format!("Exported {} folders to {}", thousands(n as u64), p.display()),
            Err(e) => self.status = format!("Export failed: {e}"),
        }
    }

    fn write_csv(&self, p: &Path) -> std::io::Result<usize> {
        use std::io::Write;
        let mut f = std::io::BufWriter::new(std::fs::File::create(p)?);
        let q = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
        writeln!(f, "sep=,")?;
        writeln!(f, "\"Report by\",{}", q(&format!("{APP_NAME} {EDITION} - {COMPANY}")))?;
        writeln!(f, "Path,Name,Level,Size (Bytes),Allocated (Bytes),Files,Folders,% of Parent,Last Change,Last Access")?;
        let mut stack = vec![0usize];
        let mut count = 0;
        while let Some(id) = stack.pop() {
            let n = self.node(id);
            let parent = n.parent.map(|p| self.node(p).size).unwrap_or(n.size);
            writeln!(
                f,
                "{},{},{},{},{},{},{},{},{},{}",
                q(&n.path.to_string_lossy()),
                q(&n.name),
                n.depth,
                n.size,
                n.alloc,
                n.files,
                n.dirs,
                fmt::pct(n.size, parent).replace(" %", ""),
                fmt::date(n.mtime),
                fmt::date(n.atime)
            )?;
            count += 1;
            let mut ch = n.children.clone();
            ch.sort_by_key(|&c| self.node(c).size);
            stack.extend(ch);
        }
        f.flush()?;
        Ok(count)
    }

    fn open_options(&mut self) {
        self.options = Some(self.st.clone());
        self.explorer_menu = sys::explorer_menu_present();
    }

    fn copy_details(&self, ctx: &Context) {
        let u = self.st.unit;
        let mut s = String::from("Name\tSize\tAllocated\tFiles\tFolders\tLast Change\tLast Access\n");
        for r in &self.details {
            s.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                r.name,
                fmt::size(r.size, u),
                fmt::size(r.alloc, u),
                r.files,
                r.dirs,
                fmt::date(r.mtime),
                fmt::date(r.atime)
            ));
        }
        ctx.copy_text(s);
    }

    fn handle_keys(&mut self, ctx: &Context) {
        let typing = ctx.egui_wants_keyboard_input();
        let (f5, esc, ctrl_o, ctrl_f, ctrl_e, plus, minus) = ctx.input(|i| {
            let c = i.modifiers.command;
            (
                i.key_pressed(Key::F5),
                i.key_pressed(Key::Escape),
                c && i.key_pressed(Key::O),
                c && i.key_pressed(Key::F),
                c && i.key_pressed(Key::E),
                c && (i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals)),
                c && i.key_pressed(Key::Minus),
            )
        });
        if f5 {
            let p = self.path_text.clone();
            self.start_scan(&p);
        }
        if esc && self.scanning.is_some() {
            self.stop_scan();
        }
        if ctrl_o {
            self.browse();
        }
        if ctrl_f {
            self.search.open = true;
        }
        if ctrl_e {
            self.export_csv();
        }
        if plus {
            self.expand(2);
        }
        if minus {
            self.collapse_all();
        }
        if typing || self.rows.is_empty() {
            return;
        }
        let page = (self.tree_view_h / TREE_ROW).max(1.0) as i64;
        let keys = ctx.input(|i| {
            [
                Key::ArrowUp,
                Key::ArrowDown,
                Key::PageUp,
                Key::PageDown,
                Key::Home,
                Key::End,
                Key::ArrowLeft,
                Key::ArrowRight,
                Key::Enter,
                Key::Backspace,
                Key::Delete,
            ]
            .map(|k| i.key_pressed(k))
        });
        let moves = [-1, 1, -page, page, -1_000_000_000, 1_000_000_000];
        for (k, d) in moves.iter().enumerate() {
            if keys[k] {
                self.move_sel(*d);
            }
        }
        if let Some((id, f)) = self.sel {
            if keys[6] {
                if !f && self.opened.contains(&id) && id != 0 {
                    self.toggle(id, Some(false));
                } else if f {
                    self.select((id, false));
                } else if let Some(p) = self.node(id).parent {
                    self.select((p, false));
                    self.scroll_to_sel = true;
                    self.rows_dirty = true;
                }
            }
            if keys[7] && !f {
                self.toggle(id, Some(true));
            }
            if keys[8] && !f {
                self.toggle(id, None);
            }
        }
        if keys[9] {
            self.go_up();
        }
        if keys[10] {
            if let Some(r) = self.tree_sel {
                self.ask_delete_row(r);
            }
        }
    }

    fn handle_screenshot(&mut self, ctx: &Context) {
        let Some(path) = self.screenshot.clone() else { return };
        self.frames += 1;
        if self.scanning.is_none() && !self.nodes.is_empty() {
            self.ready_frames += 1;
        }
        let ready = self.frames >= 120 && (self.ready_frames >= 20 || self.frames >= 1200);
        if ready && !self.shot_sent {
            self.shot_sent = true;
            let done = self.shot_done.clone();
            let ctx2 = ctx.clone();
            ctx.send_viewport_cmd(ViewportCommand::Screenshot(egui::ScreenshotCallback::new(move |img| {
                let [w, h] = img.size;
                let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| [c.r(), c.g(), c.b(), 255]).collect();
                if let Some(d) = path.parent() {
                    let _ = std::fs::create_dir_all(d);
                }
                match image::RgbaImage::from_raw(w as u32, h as u32, bytes).map(|i| i.save(&path)) {
                    Some(Ok(())) => {}
                    Some(Err(e)) => eprintln!("screenshot: {e}"),
                    None => eprintln!("screenshot: bad image"),
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

    fn update_title(&mut self, ctx: &Context) {
        let mode = match self.st.mode {
            Mode::Size => "Size",
            Mode::Alloc => "Allocated",
            Mode::Files => "File Count",
            Mode::Percent => "Percent",
        };
        let root = sys::drive_root(Path::new(&self.path_text));
        let kind = if self.vol.kind.is_empty() { "Disk".to_string() } else { self.vol.kind.clone() };
        let admin = if self.is_admin { " (Administrator)" } else { "" };
        let t = format!("{APP_NAME} {EDITION}{admin} - {mode} - Drive: {kind} ({})  |  {COMPANY}", root.trim_end_matches('\\'));
        if t != self.title {
            ctx.send_viewport_cmd(ViewportCommand::Title(t.clone()));
            self.title = t;
        }
    }

    // ---------------------------------------------------------------- ui --
    fn menu_bar(&mut self, ui: &mut Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.add(Button::new("Select directory...").shortcut_text("Ctrl+O")).clicked() {
                    ui.close();
                    self.browse();
                }
                if ui.add(Button::new("Scan / Refresh").shortcut_text("F5")).clicked() {
                    ui.close();
                    let p = self.path_text.clone();
                    self.start_scan(&p);
                }
                if ui.add_enabled(self.scanning.is_some(), Button::new("Stop scan").shortcut_text("Esc")).clicked() {
                    ui.close();
                    self.stop_scan();
                }
                ui.separator();
                if ui.add(Button::new("Export to CSV...").shortcut_text("Ctrl+E")).clicked() {
                    ui.close();
                    self.export_csv();
                }
                ui.separator();
                if cfg!(windows) && !self.is_admin && ui.button("🛡 Restart as administrator").clicked() {
                    ui.close();
                    let ctx = ui.ctx().clone();
                    self.restart_as_admin(&ctx);
                }
                if ui.button("Exit").clicked() {
                    ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                }
            });
            ui.menu_button("Branch", |ui| {
                if ui.add(Button::new("Up one level").shortcut_text("Backspace")).clicked() {
                    ui.close();
                    self.go_up();
                }
                if ui.add(Button::new("Expand two levels").shortcut_text("Ctrl+Plus")).clicked() {
                    ui.close();
                    self.expand(2);
                }
                if ui.button("Expand all below").clicked() {
                    ui.close();
                    self.expand(64);
                }
                if ui.add(Button::new("Collapse all").shortcut_text("Ctrl+Minus")).clicked() {
                    ui.close();
                    self.collapse_all();
                }
                ui.separator();
                let path = self.sel.map(|e| self.node(e.0).path.clone());
                let has = path.is_some();
                if ui.add_enabled(has, Button::new("Open")).clicked() {
                    ui.close();
                    sys::open_path(path.as_ref().unwrap());
                }
                if ui.add_enabled(has, Button::new(sys::SHOW_IN_FM)).clicked() {
                    ui.close();
                    sys::show_in_explorer(path.as_ref().unwrap());
                }
                if ui.add_enabled(has, Button::new("Copy path")).clicked() {
                    ui.close();
                    ui.ctx().copy_text(path.as_ref().unwrap().display().to_string());
                }
                if ui.add_enabled(has, Button::new("Scan this branch")).clicked() {
                    ui.close();
                    self.start_scan(&path.unwrap().to_string_lossy());
                }
                ui.separator();
                if ui.add_enabled(self.tree_sel.is_some(), Button::new("🗑 Delete...").shortcut_text("Del")).clicked() {
                    ui.close();
                    if let Some(r) = self.tree_sel {
                        self.ask_delete_row(r);
                    }
                }
            });
            ui.menu_button("View", |ui| {
                for (t, icon, name) in TABS {
                    if ui.add(Button::new(format!("{icon}  {name}")).selected(self.tab == t)).clicked() {
                        self.tab = t;
                        ui.close();
                    }
                }
                ui.separator();
                ui.label(RichText::new("Show in tree").color(MUTED));
                for (m, name) in [
                    (Mode::Size, "Size"),
                    (Mode::Alloc, "Allocated space"),
                    (Mode::Files, "File count"),
                    (Mode::Percent, "Percent"),
                ] {
                    if ui.add(Button::new(name).selected(self.st.mode == m)).clicked() {
                        self.set_mode(m);
                        ui.close();
                    }
                }
                ui.separator();
                ui.label(RichText::new("Unit").color(MUTED));
                for u in [Unit::Kb, Unit::Mb, Unit::Gb, Unit::Auto] {
                    if ui.add(Button::new(u.label()).selected(self.st.unit == u)).clicked() {
                        self.st.unit = u;
                        self.st.save();
                        ui.close();
                    }
                }
                ui.separator();
                if ui.checkbox(&mut self.st.show_drives, "Drive list").changed() {
                    self.st.save();
                }
                if ui.checkbox(&mut self.st.show_files, "Show files in tree").changed() {
                    self.st.save();
                    self.rows_dirty = true;
                }
            });
            ui.menu_button("Sort", |ui| {
                for (s, name) in [
                    (SortBy::Size, "By size"),
                    (SortBy::Name, "By name"),
                    (SortBy::Files, "By file count"),
                    (SortBy::Changed, "By last change"),
                ] {
                    if ui.add(Button::new(name).selected(self.st.sort == s)).clicked() {
                        self.set_sort(s);
                        ui.close();
                    }
                }
            });
            ui.menu_button("File Search", |ui| {
                if ui.add(Button::new("Search files by name...").shortcut_text("Ctrl+F")).clicked() {
                    self.search.open = true;
                    ui.close();
                }
                if ui.button("Largest files").clicked() {
                    self.tab = Tab::Top;
                    ui.close();
                }
                if ui.button("Files by age").clicked() {
                    self.tab = Tab::Age;
                    ui.close();
                }
            });
            ui.menu_button("Tools", |ui| {
                if ui.button("Export to CSV...").clicked() {
                    ui.close();
                    self.export_csv();
                }
                if ui.button("Clear scan history").clicked() {
                    settings::clear_history();
                    self.history.clear();
                    ui.close();
                }
                let errs = self.res.as_ref().map(|r| r.error_count).unwrap_or(0);
                if ui.add_enabled(errs > 0, Button::new(format!("Show scan errors ({})", thousands(errs)))).clicked() {
                    self.show_errors = true;
                    ui.close();
                }
                ui.separator();
                if cfg!(windows) && ui.button("Explorer context menu...").clicked() {
                    self.open_options();
                    self.options_page = 3;
                    ui.close();
                }
                if ui.button("Options...").clicked() {
                    self.open_options();
                    ui.close();
                }
            });
            ui.menu_button("Help", |ui| {
                if ui.button("Check for updates...").clicked() {
                    self.update.start(true);
                    ui.close();
                }
                if ui.button(format!("About {APP_NAME}")).clicked() {
                    self.show_about = true;
                    ui.close();
                }
            });
        });
    }

    fn set_mode(&mut self, m: Mode) {
        self.st.mode = m;
        self.st.save();
        self.rows_dirty = true;
    }

    fn set_sort(&mut self, s: SortBy) {
        self.st.sort = s;
        self.st.save();
        self.rows_dirty = true;
    }

    fn toolbar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            let edit = ui.add(
                egui::TextEdit::singleline(&mut self.path_text).desired_width(300.0).hint_text("Folder or drive to scan"),
            );
            if edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                let p = self.path_text.clone();
                self.start_scan(&p);
            }
            ui.menu_button("⏷", |ui| {
                ui.label(RichText::new("Drives").color(MUTED));
                for d in self.drives.clone() {
                    if ui.button(format!("{}  {}", d.root, d.label)).clicked() {
                        ui.close();
                        self.start_scan(&d.root);
                    }
                }
                if !self.st.recent.is_empty() {
                    ui.separator();
                    ui.label(RichText::new("Recent").color(MUTED));
                    for p in self.st.recent.clone() {
                        if ui.button(&p).clicked() {
                            ui.close();
                            self.start_scan(&p);
                        }
                    }
                }
            })
            .response
            .on_hover_text("Drives and recent folders");
            ui.separator();
            if ui.button("⟳ Scan").on_hover_text("Scan / refresh (F5)").clicked() {
                let p = self.path_text.clone();
                self.start_scan(&p);
            }
            if ui.add_enabled(self.scanning.is_some(), Button::new("■")).on_hover_text("Stop scan (Esc)").clicked() {
                self.stop_scan();
            }
            if ui.button("📂").on_hover_text("Select directory (Ctrl+O)").clicked() {
                self.browse();
            }
            if ui.button("⇧").on_hover_text("Up one level (Backspace)").clicked() {
                self.go_up();
            }
            ui.separator();
            if ui.button("⊞").on_hover_text("Expand two levels (Ctrl+Plus)").clicked() {
                self.expand(2);
            }
            if ui.button("⊟").on_hover_text("Collapse all (Ctrl+Minus)").clicked() {
                self.collapse_all();
            }
            if ui.button("🔍").on_hover_text("Search files (Ctrl+F)").clicked() {
                self.search.open = true;
            }
            if ui.button("💾").on_hover_text("Export to CSV (Ctrl+E)").clicked() {
                self.export_csv();
            }
            if ui.button("⚙").on_hover_text("Options").clicked() {
                self.open_options();
            }
            ui.separator();
            for (m, label, tip) in [
                (Mode::Files, "000", "Show file count"),
                (Mode::Size, "Size", "Show size"),
                (Mode::Alloc, "Alloc", "Show allocated space"),
                (Mode::Percent, "%", "Show percent of parent folder"),
            ] {
                if ui.add(Button::new(label).selected(self.st.mode == m)).on_hover_text(tip).clicked() {
                    self.set_mode(m);
                }
            }
            ui.separator();
            if ui.add(Button::new("A↓Z").selected(self.st.sort == SortBy::Name)).on_hover_text("Sort by name").clicked() {
                self.set_sort(SortBy::Name);
            }
            if ui.add(Button::new("9↓1").selected(self.st.sort == SortBy::Size)).on_hover_text("Sort by size").clicked() {
                self.set_sort(SortBy::Size);
            }
            ui.separator();
            for u in [Unit::Kb, Unit::Mb, Unit::Gb, Unit::Auto] {
                if ui.add(Button::new(RichText::new(u.label()).font(bold(12.5))).selected(self.st.unit == u)).clicked() {
                    self.st.unit = u;
                    self.st.save();
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(6.0);
                ui.label(RichText::new("INC.").font(bold(16.0)).color(RUST));
                ui.label(RichText::new("O.M. INDUSTRIES").font(bold(16.0)).color(NAVY));
                ui.add(egui::Image::new(egui::load::SizedTexture::new(self.tex.icon.id(), vec2(28.0, 28.0))));
            });
        });
    }

    fn info_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let small = regular(12.5);
            let b = bold(12.5);
            let Some((id, f)) = self.sel else {
                let t = if self.scanning.is_some() { "Scanning..." } else { "No folder scanned yet" };
                ui.label(RichText::new(t).font(small));
                return;
            };
            let n = self.node(id);
            let u = self.st.unit;
            if let Some(fi) = self.sel_file.clone() {
                ui.label(RichText::new(fi.path.display().to_string()).font(small.clone()));
                let owner = self.owner(&fi.path);
                for (k, v) in [
                    ("Size:", fmt::size(fi.size, u)),
                    ("Allocated:", fmt::size(fi.alloc, u)),
                    ("Last Change:", fmt::date(fi.mtime)),
                    ("Last Access:", fmt::date(fi.atime)),
                    ("Owner:", if owner.is_empty() { "-".into() } else { owner }),
                ] {
                    ui.add_space(14.0);
                    ui.label(RichText::new(k).font(b.clone()));
                    ui.label(RichText::new(v).font(small.clone()));
                }
                return;
            }
            let mut path = n.path.to_string_lossy().into_owned();
            if f {
                path.push_str("\\[Files]");
            }
            ui.label(RichText::new(path).font(small.clone()));
            let owner = self.owner(&n.path);
            let parts = [
                ("Size:", fmt::size(if f { n.own_size } else { n.size }, u)),
                ("Allocated:", fmt::size(if f { n.own_alloc } else { n.alloc }, u)),
                ("Files:", thousands(if f { n.own_files } else { n.files })),
                ("Folders:", thousands(if f { 0 } else { n.dirs })),
                ("Last Change:", fmt::date(n.mtime)),
                ("Last Access:", fmt::date(n.atime)),
                ("Owner:", if owner.is_empty() { "-".into() } else { owner }),
            ];
            for (k, v) in parts {
                ui.add_space(14.0);
                ui.label(RichText::new(k).font(b.clone()));
                ui.label(RichText::new(v).font(small.clone()));
            }
        });
    }

    fn status_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let (total, free) = sys::disk_space(Path::new(&self.path_text));
            let seg = |ui: &mut Ui, text: String, w: f32| {
                ui.add_sized(vec2(w, 18.0), Label::new(text).truncate());
                ui.separator();
            };
            let free_text = if total > 0 {
                format!("Free Space: {}  (of {})", fmt::short(free), fmt::short(total))
            } else {
                String::new()
            };
            seg(ui, free_text, 250.0);
            let files = self.sel.map(|(id, f)| if f { self.node(id).own_files } else { self.node(id).files }).unwrap_or(0);
            seg(ui, format!("{}  Files", thousands(files)), 120.0);
            let fs = if self.vol.file_system.is_empty() { String::new() } else { format!(" ({})", self.vol.file_system) };
            seg(ui, format!("{} Bytes per Cluster{}", thousands(self.vol.cluster), fs), 240.0);
            let msg = if let Some(p) = &self.scanning {
                format!(
                    "Scanning: {} files, {} folders, {} - {}",
                    thousands(p.files.load(Ordering::Relaxed)),
                    thousands(p.dirs.load(Ordering::Relaxed)),
                    fmt::size(p.bytes.load(Ordering::Relaxed), self.st.unit),
                    p.current.lock().map(|c| c.clone()).unwrap_or_default()
                )
            } else {
                self.hover_status.clone().unwrap_or_else(|| self.status.clone())
            };
            let has_errors = self.res.as_ref().map(|r| r.error_count > 0).unwrap_or(false);
            let icon = self.tex.icon.id();
            let mut show_errors = false;
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(format!("© {COMPANY}"));
                ui.add(egui::Image::new(egui::load::SizedTexture::new(icon, vec2(18.0, 18.0))));
                ui.separator();
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    let r = ui.add(Label::new(msg).truncate().sense(Sense::click()));
                    if has_errors && r.clicked() {
                        show_errors = true;
                    }
                });
            });
            if show_errors {
                self.show_errors = true;
            }
        });
    }

    fn empty_message(&self, ui: &mut Ui, msg: &str) {
        let r = ui.available_rect_before_wrap();
        let p = ui.painter();
        let side = (r.width().min(r.height()) * 0.45).clamp(64.0, 170.0);
        let mut ty = r.center().y;
        if r.height() > side + 70.0 {
            let img = Rect::from_center_size(pos2(r.center().x, r.center().y - side / 2.0 - 10.0), vec2(side, side));
            p.image(self.tex.badge.id(), img, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            ty = r.center().y + 6.0;
        }
        let galley = p.layout(msg.to_string(), regular(14.0), MUTED, (r.width() - 40.0).max(60.0));
        let gw = galley.size().x;
        p.galley(pos2(r.center().x - gw / 2.0, ty), galley, MUTED);
        ui.allocate_rect(r, Sense::hover());
    }

    fn tree_ui(&mut self, ui: &mut Ui) {
        if self.nodes.is_empty() {
            let msg = if let Some(p) = &self.scanning {
                format!(
                    "Scanning... {} files in {} folders ({})",
                    thousands(p.files.load(Ordering::Relaxed)),
                    thousands(p.dirs.load(Ordering::Relaxed)),
                    fmt::size(p.bytes.load(Ordering::Relaxed), self.st.unit)
                )
            } else {
                "Choose a drive below or type a folder above, then press Scan (F5).".into()
            };
            self.empty_message(ui, &msg);
            return;
        }
        enum Act {
            Select(Row),
            Toggle(NodeId),
            Open(PathBuf),
            Explore(PathBuf),
            Copy(String),
            Expand(Entry),
            Rescan(String),
            Delete(Row),
        }
        let mut acts: Vec<Act> = Vec::new();
        let mut sa = ScrollArea::vertical().id_salt("tree").auto_shrink([false, false]);
        if let Some(off) = self.tree_scroll.take() {
            sa = sa.vertical_scroll_offset(off);
        }
        let g0 = rgb(self.st.grad_start);
        let g1 = rgb(self.st.grad_end);
        let root_val = self.metric((0, false));
        let out = sa.show_rows(ui, TREE_ROW, self.rows.len(), |ui, range| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let w = ui.available_width();
            let valw = ui.painter().layout_no_wrap("00,000,000.0 MB".into(), bold(14.0), TEXT).size().x;
            for i in range {
                let row = self.rows[i];
                let (id, f, file) = match row {
                    Row::Node((id, f)) => (id, f, None),
                    Row::File(id, k) => (id, true, self.files_of(id).get(k).cloned()),
                };
                let n = self.node(id);
                let (rect, resp) = ui.allocate_exact_size(vec2(w, TREE_ROW), Sense::click());
                let p = ui.painter();
                let cy = rect.center().y;
                let depth = n.depth as f32 + if f { 1.0 } else { 0.0 };
                let x0 = rect.left() + 4.0 + depth * 16.0;
                if self.tree_sel == Some(row) {
                    p.rect_filled(rect.shrink(0.5), 0.0, Color32::from_rgb(0xE5, 0xE5, 0xEA));
                    p.rect_stroke(
                        rect.shrink(0.5),
                        0.0,
                        Stroke::new(1.0, Color32::from_rgb(0x9A, 0x9A, 0xA5)),
                        StrokeKind::Inside,
                    );
                }
                let expandable = !f && self.expandable(id);
                if expandable {
                    expander(p, pos2(x0 + 5.0, cy), self.opened.contains(&id));
                }
                let ix = x0 + 13.0;
                if f {
                    file_icon(p, ix, cy, 1.0);
                } else if n.parent.is_none() {
                    drive_icon(p, ix, cy, 1.0);
                } else {
                    folder_icon(p, ix, cy, 1.0, n.error.is_some());
                }
                let bx = ix + 20.0;
                let parent = if f { Some(id) } else { n.parent };
                let val = self.row_metric(row);
                let pval = parent.map(|pp| self.metric((pp, false))).unwrap_or(val);
                let frac = if parent.is_none() {
                    1.0
                } else if pval > 0 {
                    val as f64 / pval as f64
                } else {
                    0.0
                };
                let full = (rect.right() - bx - 4.0).max(1.0);
                let bw = full * frac as f32;
                if bw >= 1.0 {
                    let br = Rect::from_min_size(pos2(bx, rect.top() + 2.0), vec2(bw, TREE_ROW - 4.0));
                    if self.st.gradient {
                        gradient_rect(p, br, mix(g0, g1, 0.4 * frac as f32), mix(g0, g1, frac as f32));
                    } else {
                        p.rect_filled(br, 0.0, mix(g0, g1, 0.35));
                    }
                }
                let (value_text, name, big, path) = match &file {
                    Some(fi) => {
                        let t = match self.st.mode {
                            Mode::Files => "1 Files".to_string(),
                            Mode::Percent => fmt::pct(val, pval),
                            _ => fmt::size(val, self.st.unit),
                        };
                        let big = self.is_big_value(fi.size, frac, val, root_val);
                        (t, fi.name.clone(), big, fi.path.clone())
                    }
                    None => {
                        let e = (id, f);
                        let big = parent.is_none() || self.is_big(e, frac, root_val);
                        let name = if f { "[Files]".to_string() } else { n.name.clone() };
                        (self.metric_text(e, pval), name, big, n.path.clone())
                    }
                };
                let font = if big { bold(14.0) } else { regular(14.0) };
                p.text(pos2(bx + valw, cy), Align2::RIGHT_CENTER, value_text, font.clone(), TEXT);
                let color = if n.error.is_some() && !f {
                    ERROR
                } else if n.compressed && !f && self.st.compressed_color {
                    COMPRESSED
                } else if big && parent.is_some() {
                    BIG_TEXT
                } else {
                    TEXT
                };
                p.text(pos2(bx + valw + 8.0, cy), Align2::LEFT_CENTER, name, font, color);

                if resp.clicked() {
                    if let Some(pos) = resp.interact_pointer_pos() {
                        if expandable && pos.x >= x0 - 2.0 && pos.x <= x0 + 13.0 {
                            acts.push(Act::Toggle(id));
                        }
                    }
                    acts.push(Act::Select(row));
                }
                if resp.double_clicked() {
                    if file.is_some() {
                        acts.push(Act::Open(path.clone()));
                    } else if !f {
                        acts.push(Act::Toggle(id));
                    }
                }
                if resp.secondary_clicked() {
                    acts.push(Act::Select(row));
                }
                let deletable = file.is_some() || (!f && n.parent.is_some());
                resp.context_menu(|ui| {
                    if ui.button("Open").clicked() {
                        acts.push(Act::Open(path.clone()));
                        ui.close();
                    }
                    if ui.button(sys::SHOW_IN_FM).clicked() {
                        acts.push(Act::Explore(path.clone()));
                        ui.close();
                    }
                    if ui.button("Copy path").clicked() {
                        acts.push(Act::Copy(path.display().to_string()));
                        ui.close();
                    }
                    if !f {
                        ui.separator();
                        if ui.button("Expand two levels").clicked() {
                            acts.push(Act::Expand((id, false)));
                            ui.close();
                        }
                        if ui.button("Scan this branch").clicked() {
                            acts.push(Act::Rescan(n.path.to_string_lossy().into_owned()));
                            ui.close();
                        }
                    }
                    if deletable {
                        ui.separator();
                        if ui.add(Button::new("🗑 Delete...").shortcut_text("Del")).clicked() {
                            acts.push(Act::Delete(row));
                            ui.close();
                        }
                    }
                });
            }
        });
        self.tree_offset = out.state.clamped_offset().y;
        self.tree_view_h = out.inner_rect.height();
        for a in acts {
            match a {
                Act::Select(r) => self.select_row(r),
                Act::Toggle(id) => self.toggle(id, None),
                Act::Open(p) => sys::open_path(&p),
                Act::Explore(p) => sys::show_in_explorer(&p),
                Act::Copy(s) => ui.ctx().copy_text(s),
                Act::Expand(e) => {
                    self.select(e);
                    self.expand(2);
                }
                Act::Rescan(p) => self.start_scan(&p),
                Act::Delete(r) => self.ask_delete_row(r),
            }
        }
    }

    fn is_big_value(&self, size: u64, frac: f64, val: u64, root_val: u64) -> bool {
        let limit = self.st.bold_value;
        match self.st.bold_mode {
            BoldMode::Megabytes => size as f64 >= limit * 1048576.0,
            BoldMode::PercentOfParent => frac * 100.0 >= limit,
            BoldMode::PercentOfScan => root_val > 0 && val as f64 * 100.0 / root_val as f64 >= limit,
        }
    }

    /// The confirmation window for deleting a folder or file.
    fn delete_window(&mut self, ctx: &Context) {
        let Some(req) = self.pending_delete.clone() else { return };
        let mut close = false;
        let mut action: Option<bool> = None; // Some(true) = recycle bin, Some(false) = permanently
        let mut restart = false;
        egui::Window::new(if req.is_dir { "Delete folder" } else { "Delete file" })
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_max_width(520.0);
                ui.label(RichText::new(req.path.display().to_string()).font(bold(14.0)).color(NAVY));
                ui.label(if req.is_dir {
                    format!("{} · {} files inside", fmt::size(req.size, self.st.unit), thousands(req.files))
                } else {
                    fmt::size(req.size, self.st.unit)
                });
                ui.add_space(6.0);
                if self.is_admin || !cfg!(windows) {
                    if self.is_admin {
                        ui.label(RichText::new("Running as administrator: protected items can be deleted too. Take care.").color(RUST));
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.add(Button::new(format!("Move to {}", sys::TRASH_NAME)).min_size(vec2(150.0, 26.0))).clicked() {
                            action = Some(true);
                        }
                        if ui.add(Button::new(RichText::new("Delete permanently").color(ERROR)).min_size(vec2(150.0, 26.0))).clicked() {
                            action = Some(false);
                        }
                        if ui.add(Button::new("Cancel").min_size(vec2(80.0, 26.0))).clicked() {
                            close = true;
                        }
                    });
                } else {
                    ui.label("Deleting from OM Disk Analyzer needs administrator mode.");
                    ui.label(RichText::new("Restart as administrator, then right-click the item again and choose Delete.").color(MUTED));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.add(Button::new("🛡 Restart as administrator").min_size(vec2(190.0, 26.0))).clicked() {
                            restart = true;
                        }
                        if ui.add(Button::new("Cancel").min_size(vec2(80.0, 26.0))).clicked() {
                            close = true;
                        }
                    });
                }
            });
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            close = true;
        }
        if restart {
            self.restart_as_admin(ctx);
            close = true;
        }
        if let Some(recycle) = action {
            close = true;
            match sys::delete_path(&req.path, req.is_dir, recycle) {
                Ok(()) => {
                    if let Some(t) = req.target {
                        self.apply_delete(t, &req.path);
                    }
                    self.status = format!(
                        "{} {}",
                        if recycle { format!("Moved to the {}:", sys::TRASH_NAME) } else { "Deleted:".to_string() },
                        req.path.display()
                    );
                }
                Err(e) => self.status = format!("Could not delete {}: {e}", req.path.display()),
            }
        }
        if close {
            self.pending_delete = None;
        }
    }

    fn restart_as_admin(&mut self, ctx: &Context) {
        let current = self.nodes.first().map(|n| n.path.display().to_string()).unwrap_or_else(|| self.path_text.clone());
        match sys::restart_as_admin(&current) {
            Ok(()) => ctx.send_viewport_cmd(ViewportCommand::Close),
            Err(e) => self.status = format!("Could not restart as administrator: {e}"),
        }
    }

    fn drives_ui(&mut self, ui: &mut Ui) {
        let drives = self.drives.clone();
        let out = table(
            ui,
            "drives",
            &[("Name", 150.0, false), ("Size", 70.0, true), ("Free", 70.0, true), ("% Free", 90.0, false)],
            drives.len(),
            self.drive_sel,
            None,
            &["Scan", sys::OPEN_IN_FM],
            |i| {
                let d = &drives[i];
                let name = if d.label.is_empty() {
                    d.root.clone()
                } else {
                    format!("{} ({})", d.label, d.root.trim_end_matches('\\'))
                };
                let frac = if d.total > 0 { d.free as f32 / d.total as f32 } else { 0.0 };
                vec![
                    Cell::Icon(IconKind::Drive, name, TEXT),
                    Cell::R(fmt::short(d.total)),
                    Cell::R(fmt::short(d.free)),
                    Cell::Bar(frac, format!("{:.0} %", frac * 100.0), FREE_BAR),
                ]
            },
        );
        if let Some(i) = out.clicked {
            self.drive_sel = Some(i);
            self.status = format!("Double-click {} to scan it.", drives[i].root);
        }
        if let Some(i) = out.double {
            self.start_scan(&drives[i].root);
        }
        if let Some((i, k)) = out.menu {
            if k == 0 {
                self.start_scan(&drives[i].root);
            } else {
                sys::open_path(Path::new(&drives[i].root));
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = &ui.ctx().clone();
        self.poll_scan(ctx);
        self.handle_keys(ctx);
        if self.rows_dirty {
            self.rebuild_rows();
        }
        self.ensure_details();
        self.update_title(ctx);
        self.hover_status = None;

        egui::Panel::top("menu")
            .frame(egui::Frame::new().fill(PANEL).inner_margin(egui::Margin::symmetric(4, 2)))
            .show(ui, |ui| self.menu_bar(ui));
        egui::Panel::top("toolbar")
            .frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin::symmetric(4, 3)))
            .show(ui, |ui| self.toolbar(ui));
        egui::Panel::top("info")
            .frame(egui::Frame::new().fill(PANEL).stroke(Stroke::new(1.0, LINE)).inner_margin(egui::Margin::symmetric(6, 2)))
            .show(ui, |ui| self.info_bar(ui));
        egui::Panel::bottom("status")
            .frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin::symmetric(4, 2)))
            .show(ui, |ui| self.status_bar(ui));
        let width = ctx.content_rect().width();
        egui::Panel::left("left")
            .resizable(true)
            .default_size(width * 0.3)
            .size_range(260.0..=(width * 0.75).max(300.0))
            .frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin::symmetric(3, 3)))
            .show(ui, |ui| {
                if self.st.show_drives {
                    egui::Panel::bottom("drives")
                        .resizable(true)
                        .default_size(150.0)
                        .size_range(60.0..=400.0)
                        .frame(egui::Frame::new().fill(PANEL).stroke(Stroke::new(1.0, LINE)).inner_margin(3.0))
                        .show(ui, |ui| self.drives_ui(ui));
                }
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(PANEL).stroke(Stroke::new(1.0, LINE)).inner_margin(2.0))
                    .show(ui, |ui| self.tree_ui(ui));
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin::symmetric(4, 3)))
            .show(ui, |ui| self.central_ui(ui));

        self.about_window(ctx);
        self.update_window(ctx);
        self.options_window(ctx);
        self.search_window(ctx);
        self.errors_window(ctx);
        self.delete_window(ctx);
        self.handle_screenshot(ctx);
    }
}
