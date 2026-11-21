//! The result tabs (Chart, Details, Extensions, Users, Age, Top 100, History, Treemap),
//! the list helper they share, and the About, Options, Search and Errors windows.

use super::*;
use egui_extras::{Column, TableBuilder};

#[derive(Clone, Copy)]
pub enum IconKind {
    Folder,
    File,
    Drive,
    User,
}

pub enum Cell {
    L(String),
    R(String),
    Bar(f32, String, Color32),
    Icon(IconKind, String, Color32),
}

#[derive(Default)]
pub struct TableOut {
    pub clicked: Option<usize>,
    pub double: Option<usize>,
    pub sort: Option<usize>,
    pub menu: Option<(usize, usize)>,
}

const ROW_H: f32 = 20.0;

fn text_fits(p: &egui::Painter, text: &str, size: f32, width: f32) -> bool {
    p.layout_no_wrap(text.to_string(), regular(size), TEXT).size().x <= width
}

fn paint_cell(ui: &mut Ui, c: Cell) {
    match c {
        Cell::L(s) => {
            ui.add(Label::new(s).truncate());
        }
        Cell::R(s) => {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add(Label::new(s).truncate());
            });
        }
        Cell::Bar(frac, s, color) => {
            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H - 1.0), Sense::hover());
            pct_bar(ui.painter(), r, frac, color, &s, regular(13.0));
        }
        Cell::Icon(k, s, color) => {
            let (r, _) = ui.allocate_exact_size(vec2(20.0, ROW_H), Sense::hover());
            let p = ui.painter();
            let cy = r.center().y;
            match k {
                IconKind::Folder => folder_icon(p, r.left() + 1.0, cy, 1.0, false),
                IconKind::File => file_icon(p, r.left() + 2.0, cy, 0.9),
                IconKind::Drive => drive_icon(p, r.left(), cy, 1.0),
                IconKind::User => {
                    p.circle_filled(pos2(r.left() + 8.0, cy - 3.0), 3.5, NAVY);
                    p.rect_filled(Rect::from_min_size(pos2(r.left() + 2.5, cy + 1.5), vec2(11.0, 5.5)), 3.0, NAVY);
                }
            }
            ui.add(Label::new(RichText::new(s).color(color)).truncate());
        }
    }
}

/// A striped, resizable list with sortable headers, row clicks and a right-click menu.
#[allow(clippy::too_many_arguments)]
pub fn table(
    ui: &mut Ui,
    id: &str,
    cols: &[(&str, f32, bool)],
    n: usize,
    selected: Option<usize>,
    sort: Option<(usize, bool)>,
    menu: &[&str],
    cell: impl Fn(usize) -> Vec<Cell>,
) -> TableOut {
    let mut out = TableOut::default();
    ui.push_id(id, |ui| {
        let mut tb = TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .sense(Sense::click())
            .cell_layout(Layout::left_to_right(Align::Center))
            .auto_shrink([false, false])
            .min_scrolled_height(0.0);
        for (i, (_, w, _)) in cols.iter().enumerate() {
            tb = tb.column(if i + 1 == cols.len() {
                Column::remainder().at_least(40.0).clip(true)
            } else {
                Column::initial(*w).at_least(28.0).clip(true)
            });
        }
        let out_ref = &mut out;
        tb.header(22.0, |mut h| {
            for (i, (t, _, right)) in cols.iter().enumerate() {
                h.col(|ui| {
                    let mut s = t.to_string();
                    if let Some((c, desc)) = sort {
                        if c == i {
                            s.push_str(if desc { " ▾" } else { " ▴" });
                        }
                    }
                    let layout = if *right { Layout::right_to_left(Align::Center) } else { Layout::left_to_right(Align::Center) };
                    ui.with_layout(layout, |ui| {
                        if ui.add(Label::new(s).sense(Sense::click()).truncate()).clicked() {
                            out_ref.sort = Some(i);
                        }
                    });
                });
            }
        })
        .body(|body| {
            body.rows(ROW_H, n, |mut row| {
                let i = row.index();
                row.set_selected(selected == Some(i));
                for c in cell(i) {
                    row.col(|ui| paint_cell(ui, c));
                }
                let r = row.response();
                if r.clicked() {
                    out_ref.clicked = Some(i);
                }
                if r.double_clicked() {
                    out_ref.double = Some(i);
                }
                if !menu.is_empty() {
                    r.context_menu(|ui| {
                        for (k, m) in menu.iter().enumerate() {
                            if ui.button(*m).clicked() {
                                out_ref.menu = Some((i, k));
                                ui.close();
                            }
                        }
                    });
                }
            });
        });
    });
    out
}

impl App {
    fn ask_delete_detail(&mut self, i: usize) {
        let Some(r) = self.details.get(i) else { return };
        match r.node {
            Some(id) => self.ask_delete_row(Row::Node((id, false))),
            None => {
                let Some((folder, _)) = self.sel else { return };
                let f = FileItem { name: r.name.clone(), path: r.path.clone(), size: r.size, alloc: r.alloc, mtime: r.mtime, atime: r.atime };
                self.ask_delete_file(folder, &f);
            }
        }
    }

    fn tabs_ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (t, icon, name) in TABS {
                let sel = self.tab == t;
                let b = Button::new(RichText::new(format!("{icon} {name}")).size(14.5))
                    .fill(if sel { PANEL } else { Color32::from_rgb(0xE8, 0xE8, 0xE8) })
                    .stroke(Stroke::new(1.0, if sel { Color32::from_rgb(0xA0, 0xA0, 0xA0) } else { LINE }))
                    .corner_radius(egui::CornerRadius { nw: 3, ne: 3, sw: 0, se: 0 })
                    .min_size(vec2(0.0, 26.0));
                if ui.add(b).clicked() {
                    self.tab = t;
                }
            }
        });
    }

    pub(super) fn central_ui(&mut self, ui: &mut Ui) {
        self.tabs_ui(ui);
        egui::Frame::new().fill(PANEL).stroke(Stroke::new(1.0, LINE)).inner_margin(4.0).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            let ctx = ui.ctx().clone();
            match self.tab {
                Tab::Details => {
                    ui.horizontal(|ui| {
                        if ui.small_button("⎘ Copy").on_hover_text("Copy the list to the clipboard").clicked() {
                            self.copy_details(&ctx);
                            self.status = format!("Copied {} rows to the clipboard", self.details.len());
                        }
                        if ui.small_button("💾 Export").clicked() {
                            self.export_csv();
                        }
                        if ui.small_button(format!("📂 {}", sys::FILE_MANAGER)).clicked() {
                            if let Some((id, _)) = self.sel {
                                sys::open_path(&self.node(id).path.clone());
                            }
                        }
                        if ui.small_button("⟳ Rescan").clicked() {
                            if let Some((id, _)) = self.sel {
                                let p = self.node(id).path.to_string_lossy().into_owned();
                                self.start_scan(&p);
                            }
                        }
                    });
                    ui.separator();
                    self.details_ui(ui);
                }
                Tab::Chart => self.chart_ui(ui),
                Tab::Extensions => self.ext_ui(ui),
                Tab::Users => self.users_ui(ui),
                Tab::Age => self.age_ui(ui),
                Tab::Top => self.top_ui(ui),
                Tab::History => self.history_ui(ui),
                Tab::Treemap => self.treemap_ui(ui),
            }
        });
    }

    fn details_ui(&mut self, ui: &mut Ui) {
        if self.sel.is_none() {
            self.empty_message(ui, "Scan a drive or folder to see its contents here.");
            return;
        }
        let u = self.st.unit;
        let total = self
            .sel
            .map(|e| {
                let n = self.node(e.0);
                if e.1 {
                    n.own_size
                } else {
                    n.size
                }
            })
            .unwrap_or(0);
        let cols = [
            ("Name", 200.0, false),
            ("Size", 90.0, true),
            ("Allocated", 90.0, true),
            ("Files", 65.0, true),
            ("Folders", 60.0, true),
            ("% of Parent", 95.0, false),
            ("Last Change", 90.0, false),
            ("Last Access", 90.0, false),
            ("Owner", 100.0, false),
        ];
        let rows = &self.details;
        let out = table(
            ui,
            "details",
            &cols,
            rows.len(),
            self.details_sel,
            Some(self.details_sort),
            &["Open", sys::SHOW_IN_FM, "Copy path", "🗑 Delete..."],
            |i| {
                let r = &rows[i];
                let frac = if total > 0 { r.size as f32 / total as f32 } else { 0.0 };
                vec![
                    Cell::Icon(if r.node.is_some() { IconKind::Folder } else { IconKind::File }, r.name.clone(), TEXT),
                    Cell::R(fmt::size(r.size, u)),
                    Cell::R(fmt::size(r.alloc, u)),
                    Cell::R(thousands(r.files)),
                    Cell::R(thousands(r.dirs)),
                    Cell::Bar(frac, fmt::pct(r.size, total), PCT_BAR),
                    Cell::L(fmt::date(r.mtime)),
                    Cell::L(fmt::date(r.atime)),
                    Cell::L(if i < 400 { self.owner(&r.path) } else { String::new() }),
                ]
            },
        );
        if let Some(c) = out.sort {
            // names and owners sort A-Z first, numbers and dates biggest/newest first
            self.details_sort = if self.details_sort.0 == c { (c, !self.details_sort.1) } else { (c, c != 0 && c != 8) };
            self.sort_details();
        }
        if let Some(i) = out.clicked {
            self.details_sel = Some(i);
        }
        if let Some(i) = out.double {
            match self.details[i].node {
                Some(id) => self.reveal(id),
                None => sys::open_path(&self.details[i].path.clone()),
            }
        }
        if let Some((i, k)) = out.menu {
            let p = self.details[i].path.clone();
            match k {
                0 => sys::open_path(&p),
                1 => sys::show_in_explorer(&p),
                2 => ui.ctx().copy_text(p.display().to_string()),
                _ => self.ask_delete_detail(i),
            }
        }
        if out.clicked.is_none() && ui.input(|i| i.key_pressed(Key::Delete)) && !ui.ctx().egui_wants_keyboard_input() {
            // the tree handles Delete for its own selection; a selected list row takes priority
            if let Some(i) = self.details_sel {
                self.ask_delete_detail(i);
            }
        }
    }

    fn whole_scan_note(&self, ui: &mut Ui, what: &str) {
        if let Some(n) = self.nodes.first() {
            ui.label(RichText::new(format!("{what} for the whole scan of {}", n.path.display())).font(bold(13.0)).color(NAVY));
        }
    }

    fn ext_ui(&mut self, ui: &mut Ui) {
        let Some(res) = &self.res else {
            self.empty_message(ui, "Scan a drive or folder to see which file types use the space.");
            return;
        };
        if !res.ext_enabled {
            self.empty_message(ui, "Statistics on file extensions are turned off in Options (Scan page).");
            return;
        }
        self.whole_scan_note(ui, "File types");
        let mut v: Vec<(&String, &(u64, u64, u64))> = res.ext_stats.iter().collect();
        v.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
        let total = self.nodes[0].size;
        let u = self.st.unit;
        table(
            ui,
            "ext",
            &[
                ("Extension", 110.0, false),
                ("Description", 150.0, false),
                ("Size", 95.0, true),
                ("Allocated", 95.0, true),
                ("% of Total", 110.0, false),
                ("Files", 80.0, true),
            ],
            v.len(),
            None,
            None,
            &[],
            |i| {
                let (ext, &(s, c, a)) = v[i];
                vec![
                    Cell::Icon(IconKind::File, if ext.is_empty() { "(none)".into() } else { format!(".{ext}") }, TEXT),
                    Cell::L(fmt::ext_kind(ext)),
                    Cell::R(fmt::size(s, u)),
                    Cell::R(fmt::size(a, u)),
                    Cell::Bar(if total > 0 { s as f32 / total as f32 } else { 0.0 }, fmt::pct(s, total), lighten(PALETTE[i % PALETTE.len()], 0.35)),
                    Cell::R(thousands(c)),
                ]
            },
        );
    }

    fn users_ui(&mut self, ui: &mut Ui) {
        let Some((id, f)) = self.sel else {
            self.empty_message(ui, "Scan a drive or folder to see who owns the space.");
            return;
        };
        self.ensure_users();
        let exact = self.res.as_ref().map(|r| r.owner_enabled).unwrap_or(false) && id == 0 && !f;
        let n = self.node(id);
        let note = if exact {
            format!("Owners of every file in {}", n.path.display())
        } else {
            format!("Owners of the folders and files in {} (turn on owner statistics in Options to count every file)", n.path.display())
        };
        ui.label(RichText::new(note).font(bold(13.0)).color(NAVY));
        let total: u64 = self.users.iter().map(|u| u.1).sum();
        let u = self.st.unit;
        let users = &self.users;
        table(
            ui,
            "users",
            &[("User", 220.0, false), ("Size", 100.0, true), ("% of Total", 130.0, false), ("Files", 90.0, true)],
            users.len(),
            None,
            None,
            &[],
            |i| {
                let (name, s, c) = &users[i];
                vec![
                    Cell::Icon(IconKind::User, name.clone(), TEXT),
                    Cell::R(fmt::size(*s, u)),
                    Cell::Bar(if total > 0 { *s as f32 / total as f32 } else { 0.0 }, fmt::pct(*s, total), PCT_BAR),
                    Cell::R(thousands(*c)),
                ]
            },
        );
    }

    fn age_ui(&mut self, ui: &mut Ui) {
        let Some(res) = &self.res else {
            self.empty_message(ui, "Scan a drive or folder to see how old its files are.");
            return;
        };
        let basis = res.age_basis.map(|b| b.label()).unwrap_or("Date");
        self.whole_scan_note(ui, &format!("Age of files by {}", basis.to_lowercase()));
        let total: u64 = res.age_stats.iter().map(|a| a.0).sum();
        let u = self.st.unit;
        let h = (ui.available_height() * 0.45).clamp(80.0, 260.0);
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
        let p = ui.painter();
        let n = res.age_stats.len().max(1);
        let maxv = res.age_stats.iter().map(|a| a.0).max().unwrap_or(0).max(1);
        let slot = (r.width() - 20.0) / n as f32;
        for (i, ((_, label), (s, _))) in res.age_buckets.iter().zip(&res.age_stats).enumerate() {
            let x = r.left() + 10.0 + slot * i as f32;
            let bh = (r.height() - 40.0) * (*s as f32 / maxv as f32);
            let br = Rect::from_min_max(pos2(x + slot * 0.18, r.bottom() - 22.0 - bh), pos2(x + slot * 0.82, r.bottom() - 22.0));
            let c = PALETTE[i % PALETTE.len()];
            gradient_rect(p, br, lighten(c, 0.2), c);
            p.text(pos2(x + slot / 2.0, br.top() - 2.0), Align2::CENTER_BOTTOM, fmt::pct(*s, total), regular(12.0), MUTED);
            p.text(pos2(x + slot / 2.0, r.bottom() - 10.0), Align2::CENTER_CENTER, label, regular(12.0), TEXT);
        }
        p.line_segment([pos2(r.left() + 6.0, r.bottom() - 22.0), pos2(r.right() - 6.0, r.bottom() - 22.0)], Stroke::new(1.0, LINE));
        let rows: Vec<(String, u64, u64)> =
            res.age_buckets.iter().zip(&res.age_stats).map(|((_, l), (s, c))| (l.clone(), *s, *c)).collect();
        table(
            ui,
            "age",
            &[(basis, 180.0, false), ("Size", 100.0, true), ("% of Total", 130.0, false), ("Files", 90.0, true)],
            rows.len(),
            None,
            None,
            &[],
            |i| {
                let (l, s, c) = &rows[i];
                vec![
                    Cell::L(l.clone()),
                    Cell::R(fmt::size(*s, u)),
                    Cell::Bar(if total > 0 { *s as f32 / total as f32 } else { 0.0 }, fmt::pct(*s, total), PCT_BAR),
                    Cell::R(thousands(*c)),
                ]
            },
        );
    }

    fn top_ui(&mut self, ui: &mut Ui) {
        let Some(res) = &self.res else {
            self.empty_message(ui, "Scan a drive or folder to list its 100 largest files.");
            return;
        };
        self.whole_scan_note(ui, "The 100 largest files");
        let u = self.st.unit;
        let tops = &res.top_files;
        let out = table(
            ui,
            "top",
            &[
                ("Name", 220.0, false),
                ("Size", 95.0, true),
                ("Allocated", 95.0, true),
                ("Folder", 330.0, false),
                ("Last Change", 130.0, false),
                ("Last Access", 100.0, false),
            ],
            tops.len(),
            self.top_sel,
            None,
            &["Open", sys::SHOW_IN_FM, "Copy path", "🗑 Delete..."],
            |i| {
                let t = &tops[i];
                vec![
                    Cell::Icon(IconKind::File, t.path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), TEXT),
                    Cell::R(fmt::size(t.size, u)),
                    Cell::R(fmt::size(t.alloc, u)),
                    Cell::L(t.path.parent().map(|p| p.display().to_string()).unwrap_or_default()),
                    Cell::L(fmt::date_time(t.mtime)),
                    Cell::L(fmt::date(t.atime)),
                ]
            },
        );
        let paths: Vec<PathBuf> = res.top_files.iter().map(|t| t.path.clone()).collect();
        if let Some(i) = out.clicked {
            self.top_sel = Some(i);
        }
        if let Some(i) = out.double {
            sys::show_in_explorer(&paths[i]);
        }
        if let Some((i, k)) = out.menu {
            match k {
                0 => sys::open_path(&paths[i]),
                1 => sys::show_in_explorer(&paths[i]),
                2 => ui.ctx().copy_text(paths[i].display().to_string()),
                _ => {
                    let size = self.res.as_ref().map(|r| r.top_files[i].size).unwrap_or(0);
                    self.ask_delete_path(&paths[i].clone(), size);
                }
            }
        }
    }

    fn history_ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Earlier scans, newest first").font(bold(13.0)).color(NAVY));
            if ui.small_button("Clear history").clicked() {
                settings::clear_history();
                self.history.clear();
            }
        });
        if self.history.is_empty() {
            self.empty_message(ui, "No scans saved yet. Every finished scan is added here, so you can see how folders grow.");
            return;
        }
        let u = self.st.unit;
        let mut prev: HashMap<&str, u64> = HashMap::new();
        let mut rows: Vec<[String; 7]> = Vec::new();
        for h in &self.history {
            let diff = match prev.get(h.path.as_str()) {
                None => String::new(),
                Some(&p) => format!("{}{}", if h.size >= p { "+" } else { "-" }, fmt::size(h.size.abs_diff(p), u)),
            };
            prev.insert(&h.path, h.size);
            rows.push([
                fmt::date_time(h.date as i64),
                h.path.clone(),
                fmt::size(h.size, u),
                fmt::size(h.alloc, u),
                thousands(h.files),
                thousands(h.dirs),
                diff,
            ]);
        }
        rows.reverse();
        let out = table(
            ui,
            "history",
            &[
                ("Date", 130.0, false),
                ("Path", 300.0, false),
                ("Size", 95.0, true),
                ("Allocated", 95.0, true),
                ("Files", 80.0, true),
                ("Folders", 70.0, true),
                ("Change", 100.0, true),
            ],
            rows.len(),
            None,
            None,
            &["Scan again"],
            |i| {
                let r = &rows[i];
                vec![
                    Cell::L(r[0].clone()),
                    Cell::L(r[1].clone()),
                    Cell::R(r[2].clone()),
                    Cell::R(r[3].clone()),
                    Cell::R(r[4].clone()),
                    Cell::R(r[5].clone()),
                    Cell::R(r[6].clone()),
                ]
            },
        );
        if let Some(i) = out.double.or(out.menu.map(|m| m.0)) {
            let p = rows[i][1].clone();
            self.start_scan(&p);
        }
    }

    fn chart_ui(&mut self, ui: &mut Ui) {
        let Some((id, f)) = self.sel else {
            self.empty_message(ui, "Scan a drive or folder to see what takes up space.");
            return;
        };
        let r = ui.available_rect_before_wrap();
        ui.allocate_rect(r, Sense::hover());
        let p = ui.painter_at(r);
        let n = self.node(id);
        let total = self.metric((id, f));
        let title = n.path.display().to_string() + if f { "\\[Files]" } else { "" };
        p.text(r.left_top() + vec2(10.0, 14.0), Align2::LEFT_CENTER, title, bold(16.0), NAVY);
        let sub = format!(
            "{} · {} files · {} folders",
            self.val_text(total),
            thousands(if f { n.own_files } else { n.files }),
            thousands(if f { 0 } else { n.dirs })
        );
        p.text(r.left_top() + vec2(10.0, 36.0), Align2::LEFT_CENTER, sub, regular(13.0), MUTED);
        let items = if f { vec![(total, "[Files]".to_string(), None, false)] } else { self.chart_items(id, 12) };
        if items.is_empty() || total == 0 {
            p.text(r.center(), Align2::CENTER_CENTER, "This folder is empty.", regular(14.0), MUTED);
            return;
        }
        let legend_w = (r.width() * 0.42).min(380.0);
        let rad = ((r.width() - legend_w - 60.0) / 2.0).min((r.height() - 80.0) / 2.0).max(40.0);
        let c = pos2(r.left() + 24.0 + rad, r.top() + 56.0 + (r.height() - 56.0) / 2.0);
        let hover = ui.input(|i| i.pointer.hover_pos());
        let clicked = ui.input(|i| i.pointer.primary_clicked());
        // find the hovered slice or legend row first, so it can be drawn raised
        let mut hit: Option<usize> = None;
        let mut a = 0.0f32;
        for (i, (v, ..)) in items.iter().enumerate() {
            let span = std::f32::consts::TAU * (*v as f32 / total as f32);
            if let Some(h) = hover {
                let d = h - c;
                let dist = d.length();
                if dist <= rad && dist >= rad * 0.55 {
                    let mut ang = d.x.atan2(-d.y);
                    if ang < 0.0 {
                        ang += std::f32::consts::TAU;
                    }
                    if ang >= a && ang < a + span {
                        hit = Some(i);
                    }
                }
                let ly = r.top() + 70.0 + i as f32 * 26.0;
                let lx = c.x + rad + 30.0;
                if Rect::from_min_max(pos2(lx - 4.0, ly - 11.0), pos2(r.right() - 6.0, ly + 11.0)).contains(h) {
                    hit = Some(i);
                }
            }
            a += span;
        }
        let mut a = 0.0f32;
        for (i, (v, name, node, other)) in items.iter().enumerate() {
            let color = if *other {
                OTHER
            } else if node.is_none() {
                FILES
            } else {
                PALETTE[i % PALETTE.len()]
            };
            let span = std::f32::consts::TAU * (*v as f32 / total as f32);
            let grow = if hit == Some(i) { 5.0 } else { 0.0 };
            ring_slice(&p, c, rad + grow, rad * 0.55, a, a + span, color);
            a += span;
            let ly = r.top() + 70.0 + i as f32 * 26.0;
            let lx = c.x + rad + 30.0;
            if ly < r.bottom() - 10.0 {
                if hit == Some(i) {
                    p.rect_filled(Rect::from_min_max(pos2(lx - 4.0, ly - 11.0), pos2(r.right() - 6.0, ly + 11.0)), 2.0, Color32::from_rgb(0xF1, 0xF4, 0xF8));
                }
                p.rect_filled(Rect::from_center_size(pos2(lx + 6.0, ly), vec2(12.0, 12.0)), 2.0, color);
                let mut label = name.clone();
                if label.chars().count() > 30 {
                    label = label.chars().take(28).collect::<String>() + "…";
                }
                p.text(pos2(lx + 20.0, ly), Align2::LEFT_CENTER, label, regular(13.5), TEXT);
                let right = format!("{}   {:.1} %", self.val_text(*v), 100.0 * *v as f64 / total as f64);
                p.text(pos2(r.right() - 12.0, ly), Align2::RIGHT_CENTER, right, regular(13.0), MUTED);
            }
        }
        p.text(c - vec2(0.0, 10.0), Align2::CENTER_CENTER, self.val_text(total), bold(17.0), NAVY);
        p.text(c + vec2(0.0, 14.0), Align2::CENTER_CENTER, "total", regular(12.5), MUTED);
        if let Some(i) = hit {
            self.hover_status = Some(format!("{}: {} ({})", items[i].1, self.val_text(items[i].0), fmt::pct(items[i].0, total)));
            if clicked {
                if let Some(nid) = items[i].2 {
                    self.reveal(nid);
                }
            }
        }
    }

    fn treemap_ui(&mut self, ui: &mut Ui) {
        let Some((id, _)) = self.sel else {
            self.empty_message(ui, "Scan a drive or folder to see its treemap.");
            return;
        };
        let r = ui.available_rect_before_wrap();
        ui.allocate_rect(r, Sense::hover());
        let p = ui.painter_at(r);
        let n = self.node(id);
        let head = format!("{}   ({})", n.path.display(), self.val_text(self.metric((id, false))));
        p.text(r.left_top() + vec2(6.0, 10.0), Align2::LEFT_CENTER, head, bold(13.0), NAVY);
        p.text(r.right_top() + vec2(-6.0, 10.0), Align2::RIGHT_CENTER, "click a box to open it", regular(11.5), MUTED);
        let area = Rect::from_min_max(pos2(r.left() + 2.0, r.top() + 22.0), r.max - vec2(2.0, 2.0));
        let hover = ui.input(|i| i.pointer.hover_pos());
        let clicked = ui.input(|i| i.pointer.primary_clicked());
        let mut target: Option<(NodeId, String)> = None;
        let items = self.chart_items(id, 60);
        let rects = squarify(&items.iter().map(|t| t.0 as f64).collect::<Vec<_>>(), area);
        for (i, ((_, name, node, other), rr)) in items.iter().zip(rects).enumerate() {
            if !rr.is_positive() {
                continue;
            }
            let color = if *other {
                OTHER
            } else if node.is_none() {
                FILES
            } else {
                PALETTE[i % PALETTE.len()]
            };
            p.rect_filled(rr, 0.0, color);
            p.rect_stroke(rr, 0.0, Stroke::new(2.0, PANEL), StrokeKind::Inside);
            let inner = node.filter(|&c| !self.node(c).children.is_empty() && rr.width() > 70.0 && rr.height() > 50.0);
            if let Some(c) = inner {
                p.with_clip_rect(rr.shrink(2.0)).text(rr.left_top() + vec2(5.0, 9.0), Align2::LEFT_CENTER, name, bold(12.0), Color32::WHITE);
                let sub = Rect::from_min_max(rr.min + vec2(3.0, 18.0), rr.max - vec2(3.0, 3.0));
                let sitems = self.chart_items(c, 30);
                let srects = squarify(&sitems.iter().map(|t| t.0 as f64).collect::<Vec<_>>(), sub);
                for (j, ((_, sname, snode, sother), sr)) in sitems.iter().zip(srects).enumerate() {
                    if !sr.is_positive() {
                        continue;
                    }
                    let sc = if snode.is_some() { lighten(color, 0.25 + 0.5 * (j % 3) as f32 / 3.0) } else { lighten(color, 0.8) };
                    p.rect_filled(sr, 0.0, sc);
                    p.rect_stroke(sr, 0.0, Stroke::new(1.0, PANEL), StrokeKind::Inside);
                    if sr.height() > 18.0 && text_fits(&p, sname, 11.5, sr.width() - 6.0) {
                        p.text(sr.center(), Align2::CENTER_CENTER, sname, regular(11.5), TEXT);
                    }
                    if hover.map(|h| sr.contains(h)).unwrap_or(false) {
                        let label = if snode.is_some() {
                            String::new()
                        } else if *sother {
                            sname.clone()
                        } else {
                            "[Files]".into()
                        };
                        target = Some((snode.unwrap_or(c), label));
                    }
                }
            } else if rr.height() > 18.0 && text_fits(&p, name, 12.0, rr.width() - 6.0) {
                p.text(rr.center(), Align2::CENTER_CENTER, name, regular(12.0), Color32::WHITE);
            }
            if target.is_none() && hover.map(|h| rr.contains(h)).unwrap_or(false) {
                target = Some(match node {
                    Some(c) => (*c, String::new()),
                    None => (id, name.clone()),
                });
            }
        }
        if let Some((t, label)) = target {
            let tn = self.node(t);
            self.hover_status = Some(if label.is_empty() {
                format!("{}   {}", tn.path.display(), self.val_text(self.metric((t, false))))
            } else {
                format!("{label} in {}", tn.path.display())
            });
            if clicked && t != id {
                self.reveal(t);
            }
        }
    }

    // ------------------------------------------------------------ windows --
    pub(super) fn about_window(&mut self, ctx: &Context) {
        if !self.show_about {
            return;
        }
        let mut open = true;
        let mut ok = false;
        egui::Window::new(format!("About {APP_NAME}"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .frame(egui::Frame::window(&ctx.global_style()).fill(CREAM))
            .open(&mut open)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(self.tex.badge.id(), vec2(170.0, 170.0))));
                    ui.label(RichText::new(format!("{APP_NAME} {EDITION}")).font(bold(19.0)).color(NAVY));
                    ui.label(RichText::new(format!("Version {}", crate::setup::short_version())).font(bold(13.0)).color(RUST));
                    let (r, _) = ui.allocate_exact_size(vec2(260.0, 10.0), Sense::hover());
                    ui.painter().line_segment([pos2(r.left(), r.center().y), pos2(r.right(), r.center().y)], Stroke::new(2.0, GOLD));
                    ui.label(
                        RichText::new(
                            "See what is using space on your drives: folder sizes,\ncharts, file types, owners, largest and oldest files,\nand scan history.",
                        )
                        .color(NAVY),
                    );
                    ui.label(RichText::new("Designed for OM Industries Inc · Built with Rust").color(MUTED).size(12.0));
                    ui.add_space(6.0);
                    ui.label(RichText::new(format!("© {} {COMPANY}. All rights reserved.", chrono::Local::now().format("%Y"))).color(MUTED));
                    ui.add_space(8.0);
                    if ui.add(Button::new("OK").min_size(vec2(90.0, 24.0))).clicked() {
                        ok = true;
                    }
                    ui.add_space(4.0);
                });
            });
        if !open || ok {
            self.show_about = false;
        }
    }

    pub(super) fn update_window(&mut self, ctx: &Context) {
        self.update.poll();
        if !self.update.show {
            return;
        }
        let mut open = true;
        let mut close = false;
        let mut download: Option<String> = None;
        egui::Window::new("Check for updates")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .open(&mut open)
            .show(ctx, |ui| {
                ui.add_space(6.0);
                if self.update.checking {
                    ui.label("Checking for a newer version...");
                } else {
                    match &self.update.outcome {
                        Some(crate::update::Outcome::Newer(r)) => {
                            ui.label(RichText::new(format!("Version {} is available.", r.tag.trim_start_matches(['v', 'V']))).font(bold(15.0)).color(NAVY));
                            ui.label(format!("You have version {}.", crate::setup::short_version()));
                            ui.add_space(8.0);
                            if ui.add(Button::new("Open the download page").min_size(vec2(170.0, 26.0))).clicked() {
                                download = Some(r.url.clone());
                            }
                        }
                        Some(crate::update::Outcome::Current) => {
                            ui.label(format!("You have the latest version ({}).", crate::setup::short_version()));
                        }
                        Some(crate::update::Outcome::Failed(m)) => {
                            ui.label("Could not check for updates.");
                            ui.label(RichText::new(m.as_str()).color(MUTED));
                        }
                        None => {}
                    }
                }
                ui.add_space(8.0);
                if ui.add(Button::new("Close").min_size(vec2(90.0, 24.0))).clicked() {
                    close = true;
                }
                ui.add_space(4.0);
            });
        if let Some(u) = download {
            sys::open_url(&u);
        }
        if !open || close {
            self.update.show = false;
        }
    }

    pub(super) fn options_window(&mut self, ctx: &Context) {
        let Some(mut d) = self.options.take() else { return };
        let mut open = true;
        let mut result: Option<bool> = None;
        let mut apply = false;
        egui::Window::new("Options")
            .collapsible(false)
            .resizable(false)
            .default_width(520.0)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    for (i, name) in ["Appearance", "Scan", "Age of Files", "Explorer menu"].iter().enumerate() {
                        if ui.add(Button::new(*name).selected(self.options_page == i)).clicked() {
                            self.options_page = i;
                        }
                    }
                });
                ui.separator();
                ui.set_min_height(240.0);
                match self.options_page {
                    0 => {
                        ui.checkbox(&mut d.gradient, "Draw the size bars with a colour gradient");
                        ui.horizontal(|ui| {
                            ui.label("Bar colours from");
                            ui.color_edit_button_srgb(&mut d.grad_start);
                            ui.label("to");
                            ui.color_edit_button_srgb(&mut d.grad_end);
                            if ui.button("Default").clicked() {
                                let def = Settings::default();
                                d.grad_start = def.grad_start;
                                d.grad_end = def.grad_end;
                            }
                        });
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.label("Show folders in bold when they reach");
                            ui.add(egui::DragValue::new(&mut d.bold_value).range(0.0..=1_000_000.0).speed(0.5));
                            let label = |m: BoldMode| match m {
                                BoldMode::PercentOfScan => "% of the scanned folder",
                                BoldMode::PercentOfParent => "% of the parent folder",
                                BoldMode::Megabytes => "MB",
                            };
                            egui::ComboBox::from_id_salt("bold_mode").selected_text(label(d.bold_mode)).show_ui(ui, |ui| {
                                for m in [BoldMode::PercentOfScan, BoldMode::PercentOfParent, BoldMode::Megabytes] {
                                    ui.selectable_value(&mut d.bold_mode, m, label(m));
                                }
                            });
                        });
                        ui.checkbox(&mut d.compressed_color, "Show compressed folders in blue");
                        ui.checkbox(&mut d.show_drives, "Show the drive list below the tree");
                        ui.checkbox(&mut d.show_files, "Show files in the tree (not only folders)");
                        if cfg!(windows) {
                            ui.checkbox(&mut d.start_as_admin, "Start as administrator when my account is an administrator")
                                .on_hover_text("Windows asks for permission each time. Needed to delete protected files and folders.");
                        }
                        ui.checkbox(&mut d.check_updates, "Check for a newer version when the program starts")
                            .on_hover_text("Asks GitHub for the latest version number. Nothing else is sent.");
                    }
                    1 => {
                        ui.checkbox(&mut d.follow_links, "Follow symbolic links and junctions");
                        ui.checkbox(&mut d.ext_stats, "Collect statistics on file extensions");
                        ui.checkbox(&mut d.owner_stats, "Collect owner statistics for every file (slower)");
                        ui.add_space(6.0);
                        ui.label("Skip folders matching (separate with ;  wildcards * and ? allowed):");
                        ui.add(egui::TextEdit::singleline(&mut d.exclude_dirs).desired_width(480.0).hint_text("e.g. node_modules; *\\Temp"));
                        ui.label("Only count files matching (empty = all files):");
                        ui.add(egui::TextEdit::singleline(&mut d.include_files).desired_width(480.0).hint_text("e.g. *.jpg; *.png"));
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.label("Scan priority");
                            let label = |p: i32| match p {
                                -15 => "Idle",
                                -1 => "Lower priority",
                                1 => "Higher priority",
                                _ => "Normal priority",
                            };
                            egui::ComboBox::from_id_salt("prio").selected_text(label(d.priority)).show_ui(ui, |ui| {
                                for p in [-15, -1, 0, 1] {
                                    ui.selectable_value(&mut d.priority, p, label(p));
                                }
                            });
                        });
                    }
                    2 => {
                        ui.horizontal(|ui| {
                            ui.label("Group files by");
                            egui::ComboBox::from_id_salt("age_basis").selected_text(d.age_basis.label()).show_ui(ui, |ui| {
                                for b in [scan::AgeBasis::LastAccess, scan::AgeBasis::LastChange, scan::AgeBasis::Created] {
                                    ui.selectable_value(&mut d.age_basis, b, b.label());
                                }
                            });
                        });
                        ui.label("Age limits in days (separate with commas):");
                        ui.add(egui::TextEdit::singleline(&mut d.age_days).desired_width(300.0));
                        let b = scan::make_buckets(&d.age_days_list());
                        ui.label(RichText::new(b.iter().map(|x| x.1.clone()).collect::<Vec<_>>().join("  ·  ")).color(MUTED));
                        ui.label(RichText::new("Takes effect with the next scan.").color(MUTED));
                    }
                    _ => {
                        ui.label("Add \"Scan with OM Disk Analyzer\" to the right-click menu of folders and drives in File Explorer (for your user only).");
                        ui.add_space(6.0);
                        let mut on = self.explorer_menu;
                        if ui.checkbox(&mut on, "Show in File Explorer's right-click menu").changed() {
                            match sys::set_explorer_menu(on) {
                                Ok(()) => {
                                    self.explorer_menu = on;
                                    self.status = if on { "Added to File Explorer's menu".into() } else { "Removed from File Explorer's menu".into() };
                                }
                                Err(e) => self.status = format!("Could not change File Explorer's menu: {e}"),
                            }
                        }
                        ui.label(RichText::new("On Windows 11 it appears under \"Show more options\".").color(MUTED));
                    }
                }
                ui.separator();
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.add(Button::new("Cancel").min_size(vec2(80.0, 24.0))).clicked() {
                        result = Some(false);
                    }
                    if ui.add(Button::new("Apply").min_size(vec2(80.0, 24.0))).clicked() {
                        apply = true;
                    }
                    if ui.add(Button::new("OK").min_size(vec2(80.0, 24.0))).clicked() {
                        result = Some(true);
                    }
                });
            });
        if apply || result == Some(true) {
            let recent = std::mem::take(&mut self.st.recent);
            self.st = d.clone();
            self.st.recent = recent;
            self.st.save();
            self.rows_dirty = true;
        }
        if open && result.is_none() {
            self.options = Some(d);
        }
    }

    pub(super) fn errors_window(&mut self, ctx: &Context) {
        if !self.show_errors {
            return;
        }
        let Some(res) = &self.res else { return };
        let mut open = true;
        egui::Window::new("Items that could not be read").open(&mut open).default_size(vec2(700.0, 360.0)).show(ctx, |ui| {
            ui.label(format!("{} items could not be read (the first 1,000 are listed).", thousands(res.error_count)));
            let errs = &res.errors;
            table(ui, "errors", &[("Path", 420.0, false), ("Error", 200.0, false)], errs.len(), None, None, &[], |i| {
                vec![Cell::L(errs[i].0.clone()), Cell::L(errs[i].1.clone())]
            });
        });
        self.show_errors = open;
    }

    pub(super) fn search_window(&mut self, ctx: &Context) {
        if !self.search.open {
            return;
        }
        let mut open = true;
        let mut start = false;
        let mut stop = false;
        let running = self.search.done.as_ref().map(|d| !d.load(Ordering::Relaxed)).unwrap_or(false);
        if running {
            ctx.request_repaint_after(std::time::Duration::from_millis(150));
        }
        let results = self.search.results.lock().unwrap().clone();
        let mut out = TableOut::default();
        let u = self.st.unit;
        egui::Window::new(format!("File Search - {APP_NAME}")).open(&mut open).default_size(vec2(820.0, 420.0)).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Find files named");
                let r = ui.add(egui::TextEdit::singleline(&mut self.search.pattern).desired_width(220.0).hint_text("e.g. *.iso or report"));
                if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    start = true;
                }
                ui.label("at least");
                ui.add(egui::TextEdit::singleline(&mut self.search.min_mb).desired_width(50.0));
                ui.label("MB");
                if ui.add_enabled(!running && !self.nodes.is_empty(), Button::new("Search")).clicked() {
                    start = true;
                }
                if ui.add_enabled(running, Button::new("Stop")).clicked() {
                    stop = true;
                }
            });
            let note = if self.nodes.is_empty() {
                "Scan a drive or folder first; the search looks inside the scanned folders.".to_string()
            } else if running {
                format!("Searching... {} found", results.len())
            } else {
                format!("{} files found in {}", thousands(results.len() as u64), self.nodes[0].path.display())
            };
            ui.label(RichText::new(note).color(MUTED));
            out = table(
                ui,
                "search",
                &[("Name", 220.0, false), ("Size", 95.0, true), ("Folder", 360.0, false), ("Last Change", 120.0, false)],
                results.len(),
                self.search.sel,
                None,
                &["Open", sys::SHOW_IN_FM, "Copy path"],
                |i| {
                    let (name, path, size, mt) = &results[i];
                    vec![
                        Cell::Icon(IconKind::File, name.clone(), TEXT),
                        Cell::R(fmt::size(*size, u)),
                        Cell::L(path.parent().map(|p| p.display().to_string()).unwrap_or_default()),
                        Cell::L(fmt::date_time(*mt)),
                    ]
                },
            );
        });
        if let Some(i) = out.clicked {
            self.search.sel = Some(i);
        }
        if let Some(i) = out.double {
            sys::show_in_explorer(&results[i].1);
        }
        if let Some((i, k)) = out.menu {
            let p = &results[i].1;
            match k {
                0 => sys::open_path(p),
                1 => sys::show_in_explorer(p),
                _ => ctx.copy_text(p.display().to_string()),
            }
        }
        if stop || !open {
            if let Some(s) = &self.search.stop {
                s.store(true, Ordering::SeqCst);
            }
        }
        if start && !self.nodes.is_empty() && !self.search.pattern.trim().is_empty() {
            if let Some(s) = &self.search.stop {
                s.store(true, Ordering::SeqCst);
            }
            let stop = Arc::new(AtomicBool::new(false));
            let done = Arc::new(AtomicBool::new(false));
            let results = Arc::new(Mutex::new(Vec::new()));
            let nodes = self.nodes.clone();
            let pattern = self.search.pattern.trim().to_string();
            let min = (self.search.min_mb.trim().replace(',', ".").parse::<f64>().unwrap_or(0.0) * 1048576.0) as u64;
            let (s2, d2, r2) = (stop.clone(), done.clone(), results.clone());
            std::thread::spawn(move || {
                scan::search(&nodes, &pattern, min, &s2, &r2);
                r2.lock().unwrap().sort_by(|a, b| b.2.cmp(&a.2));
                d2.store(true, Ordering::SeqCst);
            });
            self.search.stop = Some(stop);
            self.search.done = Some(done);
            self.search.results = results;
            self.search.sel = None;
        }
        self.search.open = open;
    }
}
