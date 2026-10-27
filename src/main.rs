//! GUI explorer for redb files, laid out like a classic desktop database client:
//! menu + ribbon toolbar, object tree on the left, tabbed data grid in the middle,
//! object properties on the right.
#![windows_subsystem = "windows"]

mod db;
mod icons;
mod titlebar;

use db::{Db, Rows, MAX_ROWS};
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontFamily, FontId, Layout, Margin, Rect,
    RichText, Sense, Stroke, StrokeKind, Vec2,
};
use egui_extras::{Column, TableBuilder};
use icons::Icon;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const PAGE_SIZE: usize = 1000;
/// eframe storage key of the session restored at startup (see `App::session`).
const SESSION_KEY: &str = "session";

mod theme {
    use eframe::egui::Color32;
    pub const TEXT: Color32 = Color32::from_rgb(0x1f, 0x1f, 0x1f);
    pub const TEXT_WEAK: Color32 = Color32::from_rgb(0x70, 0x70, 0x70);
    pub const CHROME: Color32 = Color32::from_rgb(0xf5, 0xf5, 0xf5);
    pub const PANEL: Color32 = Color32::WHITE;
    pub const BORDER: Color32 = Color32::from_rgb(0xdd, 0xdd, 0xdd);
    pub const GRID_LINE: Color32 = Color32::from_rgb(0xe4, 0xe4, 0xe4);
    pub const ACCENT: Color32 = Color32::from_rgb(0x1a, 0x73, 0xe8);
    pub const HOVER: Color32 = Color32::from_rgb(0xe9, 0xf2, 0xfc);
    pub const TREE_SELECTED: Color32 = Color32::from_rgb(0xcc, 0xe4, 0xf7);
    pub const TREE_SELECTED_BORDER: Color32 = Color32::from_rgb(0x6c, 0xa9, 0xe8);
    pub const TOOL_ACTIVE: Color32 = Color32::from_rgb(0xdc, 0xea, 0xf9);
    pub const ROW_SELECTED: Color32 = Color32::from_rgb(0xee, 0xee, 0xee);
    pub const CELL_SELECTED: Color32 = Color32::from_rgb(0xd4, 0xd4, 0xd4);
}

fn main() -> eframe::Result {
    let paths: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1360.0, 820.0])
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).expect("valid icon")),
        ..Default::default()
    };
    eframe::run_native(
        "redb explorer",
        options,
        Box::new(move |cc| {
            let has_icon_font = setup_style(&cc.egui_ctx);
            let mut app = App {
                show_toolbar: true,
                show_info: true,
                show_tree: true,
                custom_chrome: titlebar::install(cc),
                has_icon_font,
                ..App::default()
            };
            if let Some(session) = cc.storage.and_then(|s| s.get_string(SESSION_KEY)) {
                app.restore_session(&session);
            }
            for p in paths {
                app.open_file(p);
            }
            Ok(Box::new(app))
        }),
    )
}

fn bold() -> FontFamily {
    FontFamily::Name("bold".into())
}

/// Windows' own caption-button glyph font (Segoe Fluent Icons / MDL2 Assets).
fn caption_icons() -> FontFamily {
    FontFamily::Name("caption_icons".into())
}

/// Segoe UI / Consolas when available (Windows), light theme with flat widgets.
/// Returns whether the Windows caption icon font could be loaded.
fn setup_style(ctx: &egui::Context) -> bool {
    let mut fonts = egui::FontDefinitions::default();
    let fonts_dir = std::env::var("WINDIR").map(|w| PathBuf::from(w).join("Fonts")).unwrap_or_default();
    let mut load = |name: &str, file: &str| -> bool {
        match std::fs::read(fonts_dir.join(file)) {
            Ok(bytes) => {
                fonts.font_data.insert(name.into(), Arc::new(egui::FontData::from_owned(bytes)));
                true
            }
            Err(_) => false,
        }
    };
    let has_ui = load("ui", "segoeui.ttf");
    let has_bold = load("ui_bold", "segoeuib.ttf");
    let has_mono = load("mono", "consola.ttf");
    let defaults = fonts.families[&FontFamily::Proportional].clone();
    if has_ui {
        fonts.families.get_mut(&FontFamily::Proportional).unwrap().insert(0, "ui".into());
    }
    if has_mono {
        fonts.families.get_mut(&FontFamily::Monospace).unwrap().insert(0, "mono".into());
    }
    let mut bold_family = defaults;
    if has_ui {
        bold_family.insert(0, "ui".into());
    }
    if has_bold {
        bold_family.insert(0, "ui_bold".into());
    }
    fonts.families.insert(bold(), bold_family);
    // Windows 11 ships Segoe Fluent Icons, Windows 10 Segoe MDL2 Assets (same code points).
    let has_icons = load("caption_icons", "SegoeIcons.ttf") || load("caption_icons", "segmdl2.ttf");
    if has_icons {
        fonts.families.insert(caption_icons(), vec!["caption_icons".into()]);
    }
    ctx.set_fonts(fonts);

    ctx.set_theme(egui::Theme::Light);
    ctx.global_style_mut(|style| {
        use egui::TextStyle::*;
        style.text_styles = [
            (Small, FontId::proportional(11.5)),
            (Body, FontId::proportional(13.0)),
            (Button, FontId::proportional(13.0)),
            (Heading, FontId::new(17.0, bold())),
            (Monospace, FontId::monospace(12.5)),
        ]
        .into();
        style.spacing.item_spacing = Vec2::new(6.0, 4.0);
        style.spacing.button_padding = Vec2::new(8.0, 3.0);
        style.spacing.interact_size.y = 22.0;

        let v = &mut style.visuals;
        v.panel_fill = theme::PANEL;
        v.window_fill = theme::PANEL;
        v.extreme_bg_color = Color32::WHITE;
        v.faint_bg_color = Color32::from_rgb(0xfa, 0xfa, 0xfa);
        v.selection.bg_fill = theme::TREE_SELECTED;
        v.selection.stroke = Stroke::new(1.0, theme::TEXT);
        v.hyperlink_color = theme::ACCENT;
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, theme::GRID_LINE);
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, theme::TEXT);
        v.widgets.inactive.fg_stroke = Stroke::new(1.0, theme::TEXT);
        v.widgets.inactive.weak_bg_fill = Color32::from_rgb(0xf0, 0xf0, 0xf0);
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, theme::BORDER);
        v.widgets.hovered.weak_bg_fill = theme::HOVER;
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, theme::TREE_SELECTED_BORDER);
        v.widgets.hovered.fg_stroke = Stroke::new(1.0, theme::TEXT);
        v.widgets.active.weak_bg_fill = theme::TREE_SELECTED;
        v.widgets.active.bg_stroke = Stroke::new(1.0, theme::ACCENT);
        v.widgets.active.fg_stroke = Stroke::new(1.0, theme::TEXT);
        for w in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active] {
            w.corner_radius = CornerRadius::same(3);
        }
    });
    has_icons
}

/// One open table in the central tab bar.
struct Tab {
    db_path: PathBuf,
    table: String,
    rows: Rows,
    truncated: bool,
    error: Option<String>,
    filter: String,
    visible: Vec<usize>,
    page: usize,
    selected_row: Option<usize>,
    selected_col: usize,
}

impl Tab {
    fn new(db_path: PathBuf, table: String) -> Self {
        Tab {
            db_path,
            table,
            rows: Vec::new(),
            truncated: false,
            error: None,
            filter: String::new(),
            visible: Vec::new(),
            page: 0,
            selected_row: None,
            selected_col: 1,
        }
    }

    fn load(&mut self, db: &Db) {
        let result = db
            .table(&self.table)
            .ok_or_else(|| format!("Table « {} » introuvable.", self.table))
            .and_then(|t| db.read(t));
        match result {
            Ok((rows, truncated)) => {
                self.rows = rows;
                self.truncated = truncated;
                self.error = None;
            }
            Err(e) => {
                self.rows.clear();
                self.truncated = false;
                self.error = Some(e);
            }
        }
        self.apply_filter();
    }

    fn apply_filter(&mut self) {
        let needle = self.filter.to_lowercase();
        self.visible = (0..self.rows.len())
            .filter(|&i| {
                let (k, v) = &self.rows[i];
                needle.is_empty() || k.to_lowercase().contains(&needle) || v.to_lowercase().contains(&needle)
            })
            .collect();
        self.page = self.page.min(self.page_count() - 1);
        if self.selected_row.is_some_and(|r| !self.visible.contains(&r)) {
            self.selected_row = None;
        }
    }

    fn page_count(&self) -> usize {
        self.visible.len().div_ceil(PAGE_SIZE).max(1)
    }

    fn page_range(&self) -> std::ops::Range<usize> {
        let start = self.page * PAGE_SIZE;
        start.min(self.visible.len())..(start + PAGE_SIZE).min(self.visible.len())
    }
}

/// UI events are collected while drawing and applied afterwards, to keep borrows simple.
enum Action {
    PickFile,
    Reload(PathBuf),
    CloseDb(PathBuf),
    OpenTab(PathBuf, String),
    CloseTab(usize),
    ActivateTab(usize),
    ShowObjects,
    SelectDb(PathBuf),
    SelectObject(String),
    Toggle(String, bool),
    Export,
}

#[derive(Default)]
struct App {
    dbs: Vec<Db>,
    tabs: Vec<Tab>,
    active: Option<usize>,
    /// The "Objets" tab (table list of `current_db`) is shown instead of a table tab.
    show_objects: bool,
    current_db: Option<PathBuf>,
    objects_selected: Option<String>,
    objects_filter: String,
    expanded: HashMap<String, bool>,
    tree_search: String,
    show_toolbar: bool,
    show_tree: bool,
    show_info: bool,
    /// The native caption is replaced by our title bar (see `titlebar`).
    custom_chrome: bool,
    has_icon_font: bool,
    error: Option<String>,
}

impl App {
    /// The table tab currently displayed (None while the Objets tab is shown).
    fn active_tab(&self) -> Option<&Tab> {
        self.active.filter(|_| !self.show_objects).and_then(|i| self.tabs.get(i))
    }

    fn db(&self, path: &Path) -> Option<&Db> {
        self.dbs.iter().find(|d| d.path == path)
    }

    /// The database listed by the Objets tab.
    fn objects_db(&self) -> Option<&Db> {
        self.current_db.as_deref().and_then(|p| self.db(p)).or(self.dbs.first())
    }

    /// The database the toolbar actions (reload, close) apply to.
    fn focused_db(&self) -> Option<PathBuf> {
        match self.active_tab() {
            Some(tab) => Some(tab.db_path.clone()),
            None => self.objects_db().map(|d| d.path.clone()),
        }
    }

    fn is_open(&self, key: &str, default: bool) -> bool {
        self.expanded.get(key).copied().unwrap_or(default)
    }

    fn open_file(&mut self, path: PathBuf) {
        if self.db(&path).is_some() {
            self.reload(&path);
            return;
        }
        match Db::open(&path) {
            Ok(db) => {
                self.dbs.push(db);
                self.error = None;
                self.current_db = Some(path);
                self.objects_selected = None;
                self.show_objects = true;
            }
            Err(e) => self.error = Some(format!("Impossible d'ouvrir {} : {e}", path.display())),
        }
    }

    fn reload(&mut self, path: &Path) {
        let Some(idx) = self.dbs.iter().position(|d| d.path == path) else { return };
        match Db::open(path) {
            Ok(db) => {
                self.dbs[idx] = db;
                self.error = None;
                for tab in self.tabs.iter_mut().filter(|t| t.db_path == path) {
                    tab.load(&self.dbs[idx]);
                }
            }
            Err(e) => self.error = Some(format!("Rechargement impossible : {e}")),
        }
    }

    fn close_db(&mut self, path: &Path) {
        let active = self.active.and_then(|i| self.tabs.get(i)).map(|t| (t.db_path.clone(), t.table.clone()));
        if self.current_db.as_deref() == Some(path) {
            self.current_db = None;
            self.objects_selected = None;
        }
        self.dbs.retain(|d| d.path != path);
        self.tabs.retain(|t| t.db_path != path);
        self.active = active
            .and_then(|(p, n)| self.tabs.iter().position(|t| t.db_path == p && t.table == n))
            .or(if self.tabs.is_empty() { None } else { Some(0) });
    }

    fn open_tab(&mut self, path: PathBuf, table: String) {
        self.show_objects = false;
        self.current_db = Some(path.clone());
        self.objects_selected = Some(table.clone());
        if let Some(i) = self.tabs.iter().position(|t| t.db_path == path && t.table == table) {
            self.active = Some(i);
            return;
        }
        let mut tab = Tab::new(path, table);
        if let Some(db) = self.db(&tab.db_path) {
            tab.load(db);
        }
        self.tabs.push(tab);
        self.active = Some(self.tabs.len() - 1);
    }

    fn close_tab(&mut self, i: usize) {
        self.tabs.remove(i);
        self.active = match self.active {
            _ if self.tabs.is_empty() => None,
            Some(a) if a > i => Some(a - 1),
            Some(a) => Some(a.min(self.tabs.len() - 1)),
            None => None,
        };
    }

    fn export_csv(&mut self) {
        let Some(tab) = self.active_tab() else { return };
        let Some(path) = rfd::FileDialog::new()
            .set_file_name(format!("{}.csv", tab.table))
            .add_filter("CSV", &["csv"])
            .save_file()
        else {
            return;
        };
        let mut out = String::from("key,value\n");
        for &i in &tab.visible {
            let (k, v) = &tab.rows[i];
            out.push_str(&format!("{},{}\n", csv_field(k), csv_field(v)));
        }
        if let Err(e) = std::fs::write(&path, out) {
            self.error = Some(format!("Export impossible : {e}"));
        }
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::PickFile => {
                if let Some(files) = rfd::FileDialog::new()
                    .add_filter("redb", &["redb"])
                    .add_filter("Tous les fichiers", &["*"])
                    .pick_files()
                {
                    for f in files {
                        self.open_file(f);
                    }
                }
            }
            Action::Reload(p) => self.reload(&p),
            Action::CloseDb(p) => self.close_db(&p),
            Action::OpenTab(p, t) => self.open_tab(p, t),
            Action::CloseTab(i) => self.close_tab(i),
            Action::ActivateTab(i) => {
                self.active = Some(i);
                self.show_objects = false;
                self.current_db = Some(self.tabs[i].db_path.clone());
                self.objects_selected = Some(self.tabs[i].table.clone());
            }
            Action::ShowObjects => self.show_objects = true,
            Action::SelectDb(p) => {
                self.current_db = Some(p);
                self.objects_selected = None;
                self.show_objects = true;
            }
            Action::SelectObject(name) => self.objects_selected = Some(name),
            Action::Toggle(key, open) => {
                self.expanded.insert(key, open);
            }
            Action::Export => self.export_csv(),
        }
    }

    /// Open databases, tabs and panel layout as tab-separated lines, read back by `restore_session`.
    fn session(&self) -> String {
        let flag = |b: bool| if b { "1" } else { "0" };
        let mut out = format!("panels\t{}\t{}\t{}\n", flag(self.show_toolbar), flag(self.show_tree), flag(self.show_info));
        for db in &self.dbs {
            out.push_str(&format!("db\t{}\n", db.path.display()));
        }
        for tab in &self.tabs {
            out.push_str(&format!("tab\t{}\t{}\t{}\n", tab.db_path.display(), tab.table, tab.filter));
        }
        if let Some(tab) = self.active.and_then(|i| self.tabs.get(i)) {
            out.push_str(&format!("active\t{}\t{}\n", tab.db_path.display(), tab.table));
        }
        if let Some(p) = &self.current_db {
            out.push_str(&format!("current\t{}\n", p.display()));
        }
        out.push_str(&format!("objects\t{}\n", flag(self.show_objects)));
        out
    }

    /// Databases that can no longer be opened are dropped silently, along with their tabs.
    fn restore_session(&mut self, session: &str) {
        let mut active = None;
        for line in session.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            match f.as_slice() {
                ["panels", toolbar, tree, info] => {
                    self.show_toolbar = *toolbar == "1";
                    self.show_tree = *tree == "1";
                    self.show_info = *info == "1";
                }
                ["db", path] => {
                    if let Ok(db) = Db::open(Path::new(path)) {
                        self.dbs.push(db);
                    }
                }
                ["tab", path, table, filter] => {
                    let Some(db) = self.db(Path::new(path)) else { continue };
                    if db.table(table).is_none() {
                        continue;
                    }
                    let mut tab = Tab::new(PathBuf::from(path), table.to_string());
                    tab.filter = filter.to_string();
                    tab.load(db);
                    self.tabs.push(tab);
                }
                ["active", path, table] => {
                    active = self.tabs.iter().position(|t| t.db_path == Path::new(path) && t.table == *table);
                }
                ["current", path] if self.db(Path::new(path)).is_some() => self.current_db = Some(PathBuf::from(path)),
                ["objects", shown] => self.show_objects = *shown == "1",
                _ => {}
            }
        }
        self.active = active.or(if self.tabs.is_empty() { None } else { Some(0) });
        if self.active.is_none() {
            self.show_objects = true;
        }
        if let Some(tab) = self.active_tab() {
            self.objects_selected = Some(tab.table.clone());
        }
    }
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(SESSION_KEY, self.session());
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let mut actions = Vec::new();
        let ctx = ui.ctx().clone();

        // Files dropped on the window, and keyboard shortcuts.
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        for p in dropped {
            self.open_file(p);
        }
        let focused_db = self.focused_db();
        let shown_tab = self.active.filter(|_| !self.show_objects);
        ctx.input_mut(|i| {
            if i.consume_key(egui::Modifiers::COMMAND, egui::Key::O) {
                actions.push(Action::PickFile);
            }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::F5) {
                if let Some(p) = &focused_db {
                    actions.push(Action::Reload(p.clone()));
                }
            }
            if i.consume_key(egui::Modifiers::COMMAND, egui::Key::W) {
                if let Some(a) = shown_tab {
                    actions.push(Action::CloseTab(a));
                }
            }
        });

        self.menu_bar(ui, &mut actions);
        if self.show_toolbar {
            self.toolbar(ui, &mut actions);
        }
        self.status_bar(ui);
        if self.show_tree {
            self.tree(ui, &mut actions);
        }
        if self.show_info {
            self.info_panel(ui);
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::PANEL))
            .show(ui, |ui| self.central(ui, &mut actions));

        for a in actions {
            self.apply(a);
        }
    }
}

// ---------------------------------------------------------------------------
// Panels

/// Width of a caption button (minimize / maximize / close), as in Windows 11.
const CAPTION_BUTTON_W: f32 = 46.0;

/// Caption glyphs drawn with lines when the Windows icon font is unavailable.
fn caption_glyph_fallback(painter: &egui::Painter, c: egui::Pos2, glyph: char, color: Color32) {
    let st = Stroke::new(1.0, color);
    let d = 5.0;
    match glyph {
        '\u{E921}' => {
            painter.hline((c.x - d)..=(c.x + d), c.y, st);
        }
        '\u{E8BB}' => {
            painter.line_segment([c + Vec2::new(-d, -d), c + Vec2::new(d, d)], st);
            painter.line_segment([c + Vec2::new(-d, d), c + Vec2::new(d, -d)], st);
        }
        _ => {
            painter.rect_stroke(Rect::from_center_size(c, Vec2::splat(2.0 * d)), 0.0, st, StrokeKind::Inside);
        }
    }
}

/// Bottom strip of the tree and of the grid: same fixed height so both line up.
fn footer_panel(id: &str) -> egui::Panel {
    egui::Panel::bottom(id.to_owned())
        .exact_size(34.0)
        .frame(egui::Frame::new().fill(theme::PANEL).inner_margin(Margin::symmetric(8, 0)))
}

fn chrome_frame() -> egui::Frame {
    egui::Frame::new().fill(theme::CHROME).inner_margin(Margin::symmetric(6, 2))
}

impl App {
    /// Title bar, VS Code style: app icon and menus on the left, caption buttons on the right. Without custom chrome it is just the menu row under
    /// the native title bar.
    fn menu_bar(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let active_db = self.focused_db();
        let height = if self.custom_chrome { 34.0 } else { 28.0 };
        egui::Panel::top("menu")
            .exact_size(height)
            .frame(egui::Frame::new().fill(theme::CHROME))
            .show_separator_line(false)
            .show(ui, |ui| {
                let bar = ui.max_rect();
                let mut interactive = Vec::new();
                let menus = ui
                    .with_layout(Layout::left_to_right(Align::Center), |ui| {
                        ui.add_space(10.0);
                        if self.custom_chrome {
                            icon(ui, Icon::Database { open: true }, 16.0);
                            ui.add_space(2.0);
                        }
                        self.menus(ui, actions, &active_db)
                    })
                    .inner;
                interactive.push(menus);

                // Layout toggles (toolbar, objects panel, info panel), just left of the caption buttons.
                let captions_w = if self.custom_chrome { 3.0 * CAPTION_BUTTON_W } else { 0.0 };
                let toggles_right = bar.right() - captions_w - 8.0;
                let toggles = [
                    (0, "Afficher / masquer la barre d'outils"),
                    (1, "Afficher / masquer le panneau des objets"),
                    (2, "Afficher / masquer le panneau d'informations"),
                ];
                for (i, tip) in toggles {
                    let rect = Rect::from_center_size(
                        egui::pos2(toggles_right - (2.5 - i as f32) * 28.0, bar.center().y),
                        Vec2::splat(26.0),
                    );
                    let flag = match i {
                        0 => &mut self.show_toolbar,
                        1 => &mut self.show_tree,
                        _ => &mut self.show_info,
                    };
                    let on = *flag;
                    let kind = match i {
                        0 => Icon::PanelTop { on },
                        1 => Icon::PanelLeft { on },
                        _ => Icon::PanelRight { on },
                    };
                    let resp = ui.interact(rect, ui.scope_id().with(("layout_toggle", i)), Sense::click()).on_hover_text(tip);
                    if resp.hovered() {
                        ui.painter().rect_filled(rect, 4.0, Color32::from_black_alpha(18));
                    }
                    icons::paint(ui.painter(), rect.shrink(5.0), kind);
                    if resp.clicked() {
                        *flag = !*flag;
                    }
                    interactive.push(rect);
                }

                if self.custom_chrome {
                    let max_rect = self.caption_buttons(ui, bar, &mut interactive);
                    titlebar::publish(ui.ctx(), bar, max_rect, &interactive);
                }
            });
    }

    /// Minimize / maximize / close, drawn with Windows' own glyphs. Returns the maximize
    /// button rect, whose input is handled by Windows (snap layouts), see `titlebar`.
    fn caption_buttons(&self, ui: &mut egui::Ui, bar: Rect, interactive: &mut Vec<Rect>) -> Rect {
        let maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
        let button = |i: f32| {
            Rect::from_min_max(
                egui::pos2(bar.right() - (i + 1.0) * CAPTION_BUTTON_W, bar.top()),
                egui::pos2(bar.right() - i * CAPTION_BUTTON_W, bar.bottom()),
            )
        };
        let (close_rect, max_rect, min_rect) = (button(0.0), button(1.0), button(2.0));

        let min = ui.interact(min_rect, ui.scope_id().with("caption_min"), Sense::click());
        let close = ui.interact(close_rect, ui.scope_id().with("caption_close"), Sense::click());
        let (max_hover, max_pressed) = titlebar::max_button_state();
        interactive.push(min_rect);
        interactive.push(close_rect);

        let painter = ui.painter();
        let neutral = |hovered: bool, pressed: bool| match (hovered, pressed) {
            (_, true) => Color32::from_black_alpha(10),
            (true, _) => Color32::from_black_alpha(16),
            _ => Color32::TRANSPARENT,
        };
        painter.rect_filled(min_rect, 0.0, neutral(min.hovered(), min.is_pointer_button_down_on()));
        painter.rect_filled(max_rect, 0.0, neutral(max_hover, max_pressed));
        let close_bg = if close.is_pointer_button_down_on() {
            Color32::from_rgb(0xc8, 0x40, 0x31)
        } else if close.hovered() {
            Color32::from_rgb(0xc4, 0x2b, 0x1c)
        } else {
            Color32::TRANSPARENT
        };
        painter.rect_filled(close_rect, 0.0, close_bg);

        let close_fg = if close.hovered() { Color32::WHITE } else { theme::TEXT };
        let glyphs = [
            (min_rect, '\u{E921}', theme::TEXT),
            (max_rect, if maximized { '\u{E923}' } else { '\u{E922}' }, theme::TEXT),
            (close_rect, '\u{E8BB}', close_fg),
        ];
        for (rect, glyph, color) in glyphs {
            if self.has_icon_font {
                painter.text(rect.center(), Align2::CENTER_CENTER, glyph, FontId::new(10.0, caption_icons()), color);
            } else {
                caption_glyph_fallback(painter, rect.center(), glyph, color);
            }
        }

        if min.clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
        if close.clicked() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        max_rect
    }

    fn menus(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>, active_db: &Option<PathBuf>) -> Rect {
        let active_db = active_db.clone();
        egui::MenuBar::new()
            .ui(ui, |ui| {
                ui.menu_button("Fichier", |ui| {
                    if ui.add(egui::Button::new("Ouvrir…").shortcut_text("Ctrl+O")).clicked() {
                        actions.push(Action::PickFile);
                    }
                    if ui.add_enabled(active_db.is_some(), egui::Button::new("Fermer le fichier")).clicked() {
                        actions.push(Action::CloseDb(active_db.clone().unwrap()));
                    }
                    ui.separator();
                    if ui.button("Quitter").clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Affichage", |ui| {
                    ui.checkbox(&mut self.show_toolbar, "Barre d'outils");
                    ui.checkbox(&mut self.show_tree, "Panneau des objets");
                    ui.checkbox(&mut self.show_info, "Panneau d'informations");
                });
                ui.menu_button("Table", |ui| {
                    let has_tab = self.active_tab().is_some();
                    if ui.add_enabled(active_db.is_some(), egui::Button::new("Recharger").shortcut_text("F5")).clicked() {
                        actions.push(Action::Reload(active_db.clone().unwrap()));
                    }
                    if ui.add_enabled(has_tab, egui::Button::new("Exporter en CSV…")).clicked() {
                        actions.push(Action::Export);
                    }
                    if ui.add_enabled(has_tab, egui::Button::new("Fermer l'onglet").shortcut_text("Ctrl+W")).clicked() {
                        actions.push(Action::CloseTab(self.active.unwrap()));
                    }
                });
                ui.menu_button("Aide", |ui| {
                    ui.label("redb explorer — lecture seule");
                    ui.weak("Glisse un fichier .redb dans la fenêtre pour l'ouvrir.");
                });
                // Only the space the menu buttons use (the bar's own rect spans the full width).
                let used = ui.min_rect();
                Rect::from_min_max(used.min, egui::pos2(ui.cursor().left(), used.max.y))
            })
            .inner
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let frame = chrome_frame().inner_margin(Margin { left: 4, right: 8, top: 2, bottom: 4 });
        egui::Panel::top("toolbar").frame(frame).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                let active_db = self.focused_db();
                if tool_button(ui, Icon::Open, "Ouvrir", true, false).on_hover_text("Ouvrir un fichier redb (Ctrl+O)").clicked() {
                    actions.push(Action::PickFile);
                }
                if tool_button(ui, Icon::Table, "Table", active_db.is_some(), active_db.is_some())
                    .on_hover_text("Liste des tables")
                    .clicked()
                {
                    actions.push(Action::ShowObjects);
                }
                if tool_button(ui, Icon::Reload, "Recharger", active_db.is_some(), false).on_hover_text("F5").clicked() {
                    actions.push(Action::Reload(active_db.clone().unwrap()));
                }
                if tool_button(ui, Icon::Export, "Exporter", self.active_tab().is_some(), false)
                    .on_hover_text("Exporter les lignes filtrées en CSV")
                    .clicked()
                {
                    actions.push(Action::Export);
                }
                if tool_button(ui, Icon::CloseDb, "Fermer", active_db.is_some(), false)
                    .on_hover_text("Fermer le fichier courant")
                    .clicked()
                {
                    actions.push(Action::CloseDb(active_db.unwrap()));
                }
            });
        });
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        egui::Panel::bottom("status").frame(chrome_frame().inner_margin(Margin::symmetric(8, 2))).show(ui, |ui| {
            ui.horizontal(|ui| {
                match self.active_tab() {
                    Some(tab) => {
                        let mut query = format!("SCAN `{}`", tab.table);
                        if !tab.filter.is_empty() {
                            query.push_str(&format!(" WHERE key|value LIKE '%{}%'", tab.filter));
                        }
                        query.push_str(&format!(" LIMIT {},{}", tab.page_range().start, PAGE_SIZE));
                        ui.label(RichText::new(query).size(12.5));
                    }
                    None => match self.objects_db() {
                        Some(db) => {
                            ui.label(format!("{} tables", db.tables.len()));
                            ui.separator();
                            icon(ui, Icon::Folder, 14.0);
                            ui.label("Mes fichiers");
                            icon(ui, Icon::Database { open: true }, 14.0);
                            ui.label(db.file_name());
                        }
                        None => {
                            ui.label(RichText::new("Prêt").color(theme::TEXT_WEAK));
                        }
                    },
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new(format!("{} fichier(s)", self.dbs.len())).color(theme::TEXT_WEAK));
                    if let Some(tab) = self.active_tab() {
                        ui.separator();
                        let record = tab
                            .selected_row
                            .and_then(|s| tab.visible.iter().position(|&v| v == s))
                            .map_or(0, |p| p + 1);
                        ui.label(format!(
                            "Ligne {} sur {} dans la page {}",
                            record,
                            tab.visible.len(),
                            tab.page + 1
                        ));
                    }
                });
            });
        });
    }

    fn tree(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        egui::Panel::left("tree")
            .resizable(true)
            .default_size(280.0)
            .frame(egui::Frame::new().fill(theme::PANEL))
            .show(ui, |ui| {
                footer_panel("tree_search").show(ui, |ui| {
                        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                            icon(ui, Icon::Search, 14.0);
                            ui.add(
                                egui::TextEdit::singleline(&mut self.tree_search)
                                    .hint_text("Rechercher")
                                    .frame(egui::Frame::NONE)
                                    .desired_width(f32::INFINITY),
                            );
                        });
                    });
                egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                    ui.add_space(4.0);
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let needle = self.tree_search.to_lowercase();
                    let current_db = self.objects_db().map(|d| d.path.clone());

                    let root_open = self.is_open("root", true);
                    if tree_row(ui, 0, Some(root_open), Icon::Folder, "Mes fichiers", false).toggled {
                        actions.push(Action::Toggle("root".into(), !root_open));
                    }
                    if !root_open {
                        return;
                    }
                    for db in &self.dbs {
                        let db_key = format!("db:{}", db.path.display());
                        let db_open = self.is_open(&db_key, true);
                        let is_current = current_db.as_ref() == Some(&db.path);
                        let db_selected = is_current && self.objects_selected.is_none();
                        let row = tree_row(ui, 1, Some(db_open), Icon::Database { open: true }, &db.file_name(), db_selected);
                        if row.toggled {
                            actions.push(Action::Toggle(db_key, !db_open));
                        } else if row.response.clicked() {
                            actions.push(Action::SelectDb(db.path.clone()));
                        }
                        row.response.on_hover_text(db.path.display().to_string()).context_menu(|ui| {
                            if ui.button("Recharger").clicked() {
                                actions.push(Action::Reload(db.path.clone()));
                            }
                            if ui.button("Fermer le fichier").clicked() {
                                actions.push(Action::CloseDb(db.path.clone()));
                            }
                        });
                        if !db_open {
                            continue;
                        }
                        let tables_key = format!("tables:{}", db.path.display());
                        let tables_open = self.is_open(&tables_key, true);
                        let row = tree_row(ui, 2, Some(tables_open), Icon::Table, "Tables", false);
                        if row.toggled {
                            actions.push(Action::Toggle(tables_key, !tables_open));
                        } else if row.response.clicked() {
                            actions.push(Action::SelectDb(db.path.clone()));
                        }
                        if !tables_open {
                            continue;
                        }
                        for t in db.tables.iter().filter(|t| t.name.to_lowercase().contains(&needle)) {
                            let t_key = format!("t:{}:{}", db.path.display(), t.name);
                            let t_open = self.is_open(&t_key, false);
                            let selected = is_current && self.objects_selected.as_ref() == Some(&t.name);
                            let row = tree_row(ui, 3, Some(t_open), Icon::Table, &t.name, selected);
                            // Like a desktop DB client: click selects, double-click opens the data.
                            if row.response.double_clicked() {
                                actions.push(Action::OpenTab(db.path.clone(), t.name.clone()));
                            } else if row.toggled {
                                actions.push(Action::Toggle(t_key, !t_open));
                            } else if row.response.clicked() {
                                actions.push(Action::SelectDb(db.path.clone()));
                                actions.push(Action::SelectObject(t.name.clone()));
                            }
                            if t_open {
                                if t.multimap {
                                    tree_row(ui, 4, None, Icon::Field, "multimap", false);
                                } else {
                                    tree_row(ui, 4, None, Icon::Key, &format!("key  ·  {}", t.key_type), false);
                                    tree_row(ui, 4, None, Icon::Field, &format!("value  ·  {}", t.value_type), false);
                                }
                            }
                        }
                    }
                });
            });
    }

    fn info_panel(&self, ui: &mut egui::Ui) {
        egui::Panel::right("info")
            .resizable(true)
            .default_size(300.0)
            .frame(egui::Frame::new().fill(theme::PANEL).inner_margin(Margin::symmetric(16, 8)))
            .show(ui, |ui| {
                egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                    ui.vertical_centered(|ui| icon(ui, Icon::Info, 18.0));
                    ui.add_space(14.0);
                    // Table tab: that table. Objets tab: the selected table, or the database itself.
                    let tab = self.active_tab();
                    let db = match tab {
                        Some(t) => self.db(&t.db_path),
                        None => self.objects_db(),
                    };
                    let Some(db) = db else {
                        ui.label(RichText::new("Aucun objet sélectionné").color(theme::TEXT_WEAK));
                        return;
                    };
                    let table_name = tab.map(|t| &t.table).or(self.objects_selected.as_ref());
                    let info = table_name.and_then(|n| db.table(n));

                    let (big_icon, title, kind) = match info {
                        Some(t) => (Icon::Table, t.name.clone(), if t.multimap { "Table multimap" } else { "Table" }),
                        None => (Icon::Database { open: true }, db.file_name(), "Base de données redb"),
                    };
                    ui.horizontal(|ui| {
                        icon(ui, big_icon, 60.0);
                        ui.add_space(4.0);
                        ui.vertical(|ui| {
                            ui.add_space(6.0);
                            ui.label(RichText::new(title).family(bold()).size(14.0));
                            ui.label(RichText::new(kind).color(theme::TEXT_WEAK));
                        });
                    });
                    ui.add_space(14.0);

                    if info.is_some() {
                        ui.horizontal(|ui| {
                            icon(ui, Icon::Database { open: true }, 16.0);
                            ui.label(db.file_name());
                        });
                        ui.add_space(12.0);
                    } else {
                        property(ui, "Tables", &db.tables.len().to_string());
                        let rows: u64 = db.tables.iter().map(|t| t.len).sum();
                        property(ui, "Lignes (toutes tables)", &rows.to_string());
                    }
                    if let Some(t) = info {
                        property(ui, "Lignes", &t.len.to_string());
                        property(ui, "Type de clé", &type_label(&t.key_type));
                        property(ui, "Type de valeur", &type_label(&t.value_type));
                        if let Some(s) = &t.stats {
                            property(ui, "Hauteur de l'arbre", &s.tree_height.to_string());
                            property(ui, "Pages feuilles / branches", &format!("{} / {}", s.leaf_pages, s.branch_pages));
                            property(ui, "Données stockées", &human_bytes(s.stored_bytes));
                            property(ui, "Métadonnées", &human_bytes(s.metadata_bytes));
                            property(ui, "Espace fragmenté", &human_bytes(s.fragmented_bytes));
                        }
                    }
                    property(ui, "Taille du fichier", &human_bytes(db.file_size));
                    property(ui, "Emplacement", &db.path.display().to_string());

                    if let Some((tab, r)) = tab.and_then(|t| t.selected_row.map(|r| (t, r))) {
                        let (k, v) = &tab.rows[r];
                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(6.0);
                        ui.label(RichText::new("Ligne sélectionnée").family(bold()));
                        ui.add_space(6.0);
                        for (label, text) in [("Clé", k), ("Valeur", v)] {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(label).color(theme::TEXT_WEAK));
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if icon_button(ui, Icon::Copy, 16.0, false).on_hover_text("Copier").clicked() {
                                        ui.ctx().copy_text(text.clone());
                                    }
                                });
                            });
                            ui.add(
                                egui::TextEdit::multiline(&mut text.as_str())
                                    .code_editor()
                                    .desired_rows(1)
                                    .desired_width(f32::INFINITY),
                            );
                            ui.add_space(8.0);
                        }
                    }
                });
            });
    }

    /// The "Objets" tab: the tables of the current database, one per line.
    fn objects_view(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let selected = self.objects_selected.clone();
        let db_path = self.objects_db().map(|d| d.path.clone());

        egui::Panel::top("objects_bar")
            .frame(egui::Frame::new().fill(theme::PANEL).inner_margin(Margin::symmetric(8, 4)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let target = db_path.clone().zip(selected.clone());
                    if flat_button_enabled(ui, Icon::Table, "Ouvrir la table", target.is_some()).clicked() {
                        let (p, t) = target.clone().unwrap();
                        actions.push(Action::OpenTab(p, t));
                    }
                    if flat_button_enabled(ui, Icon::Reload, "Recharger", db_path.is_some()).clicked() {
                        actions.push(Action::Reload(db_path.clone().unwrap()));
                    }
                    if flat_button_enabled(ui, Icon::Export, "Exporter la table", target.is_some()).clicked() {
                        let (p, t) = target.unwrap();
                        actions.push(Action::OpenTab(p, t));
                        actions.push(Action::Export);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.objects_filter)
                                .hint_text("Rechercher")
                                .desired_width(180.0),
                        );
                        icon(ui, Icon::Search, 14.0);
                    });
                });
            });

        footer_panel("objects_footer").show(ui, |ui| {
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                if let Some(db) = self.objects_db() {
                    ui.label(RichText::new(format!("{} tables", db.tables.len())).color(theme::TEXT_WEAK));
                }
            });
        });

        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::PANEL)).show(ui, |ui| {
            let top = ui.max_rect().top();
            ui.painter().hline(ui.max_rect().x_range(), top, Stroke::new(1.0, theme::BORDER));
            let Some(db) = self.objects_db() else {
                ui.add_space(12.0);
                if let Some(e) = &self.error {
                    ui.colored_label(ui.visuals().error_fg_color, e);
                }
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new("Ouvre un fichier .redb (Ctrl+O) ou glisse-le ici").color(theme::TEXT_WEAK));
                });
                return;
            };
            if let Some(e) = &self.error {
                ui.colored_label(ui.visuals().error_fg_color, e);
            }
            let needle = self.objects_filter.to_lowercase();
            let tables: Vec<_> = db.tables.iter().filter(|t| t.name.to_lowercase().contains(&needle)).collect();

            ui.spacing_mut().item_spacing = Vec2::ZERO;
            let right = Layout::right_to_left(Align::Center);
            TableBuilder::new(ui)
                .sense(Sense::click())
                .cell_layout(Layout::left_to_right(Align::Center))
                .column(Column::initial(240.0).at_least(120.0).clip(true))
                .column(Column::exact(70.0))
                .column(Column::exact(130.0))
                .column(Column::exact(120.0).clip(true))
                .column(Column::exact(120.0).clip(true))
                .column(Column::exact(70.0))
                .column(Column::remainder())
                .header(26.0, |mut header| {
                    for (title, numeric) in [
                        ("Nom", false),
                        ("Lignes", true),
                        ("Données stockées", true),
                        ("Type de clé", false),
                        ("Type de valeur", false),
                        ("Hauteur", true),
                        ("", false),
                    ] {
                        header.col(|ui| {
                            header_cell_bg(ui);
                            let text = RichText::new(title).color(theme::TEXT);
                            if numeric {
                                ui.with_layout(right, |ui| {
                                    ui.add_space(8.0);
                                    ui.label(text);
                                });
                            } else {
                                ui.add_space(8.0);
                                ui.label(text);
                            }
                        });
                    }
                })
                .body(|body| {
                    body.rows(22.0, tables.len(), |mut row| {
                        let t = tables[row.index()];
                        let is_selected = selected.as_ref() == Some(&t.name);
                        let stats = t.stats.as_ref();
                        row.col(|ui| {
                            ui.add_space(4.0);
                            let (rect, _) = ui.allocate_exact_size(Vec2::new(20.0, 16.0), Sense::hover());
                            icons::paint(ui.painter(), rect.translate(Vec2::new(-2.0, 0.0)), Icon::Table);
                            let galley_w = ui.fonts_mut(|f| {
                                f.layout_no_wrap(t.name.clone(), FontId::proportional(13.0), theme::TEXT).size().x
                            });
                            if is_selected {
                                let r = Rect::from_min_size(
                                    egui::pos2(ui.cursor().left() - 3.0, ui.max_rect().top() + 1.0),
                                    Vec2::new(galley_w + 8.0, ui.max_rect().height() - 2.0),
                                );
                                ui.painter().rect_filled(r, 2.0, theme::TREE_SELECTED);
                            }
                            ui.add(egui::Label::new(RichText::new(&t.name).color(theme::TEXT)).truncate().selectable(false));
                        });
                        let cells = [
                            (if t.multimap { "—".into() } else { t.len.to_string() }, true),
                            (stats.map_or("—".into(), |s| short_bytes(s.stored_bytes)), true),
                            (if t.multimap { "multimap".into() } else { type_label(&t.key_type) }, false),
                            (if t.multimap { "multimap".into() } else { type_label(&t.value_type) }, false),
                            (stats.map_or("—".into(), |s| s.tree_height.to_string()), true),
                        ];
                        for (text, numeric) in cells {
                            row.col(|ui| cell_text(ui, &text, numeric));
                        }
                        row.col(|_| {});
                        let resp = row.response();
                        if resp.double_clicked() {
                            actions.push(Action::OpenTab(db.path.clone(), t.name.clone()));
                        } else if resp.clicked() {
                            actions.push(Action::SelectObject(t.name.clone()));
                        }
                    });
                });
        });
    }

    fn central(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        // Tab bar
        egui::Panel::top("tabs")
            .frame(egui::Frame::new().fill(theme::CHROME).inner_margin(Margin { left: 6, right: 6, top: 4, bottom: 0 }))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let objects_shown = self.active_tab().is_none();
                    if doc_tab(ui, None, "Objets", objects_shown).response.clicked() {
                        actions.push(Action::ShowObjects);
                    }
                    for (i, tab) in self.tabs.iter().enumerate() {
                        let file = self.db(&tab.db_path).map(|d| d.file_name()).unwrap_or_default();
                        let shown = !objects_shown && self.active == Some(i);
                        let r = doc_tab(ui, Some(Icon::Table), &format!("{} @ {}", tab.table, file), shown);
                        if r.close_clicked || r.response.middle_clicked() {
                            actions.push(Action::CloseTab(i));
                        } else if r.response.clicked() {
                            actions.push(Action::ActivateTab(i));
                        }
                    }
                });
            });

        let Some(active) = self.active.filter(|_| !self.show_objects) else {
            self.objects_view(ui, actions);
            return;
        };
        let (key_type, value_type) = self
            .db(&self.tabs[active].db_path)
            .and_then(|d| d.table(&self.tabs[active].table))
            .map(|t| (t.key_type.clone(), t.value_type.clone()))
            .unwrap_or_default();
        let error = self.error.clone();
        let tab = &mut self.tabs[active];

        // Object toolbar
        egui::Panel::top("objbar")
            .frame(egui::Frame::new().fill(theme::PANEL).inner_margin(Margin::symmetric(8, 4)))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    icon(ui, Icon::Filter, 16.0);
                    ui.label("Filtre");
                    let filter = ui.add(
                        egui::TextEdit::singleline(&mut tab.filter)
                            .hint_text("clé ou valeur contient…")
                            .desired_width(260.0),
                    );
                    if filter.changed() {
                        tab.page = 0;
                        tab.apply_filter();
                    }
                    ui.add_space(8.0);
                    if flat_button(ui, Icon::Reload, "Recharger").clicked() {
                        actions.push(Action::Reload(tab.db_path.clone()));
                    }
                    if flat_button(ui, Icon::Export, "Exporter").clicked() {
                        actions.push(Action::Export);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if tab.truncated {
                            ui.colored_label(ui.visuals().warn_fg_color, format!("limité à {MAX_ROWS} lignes"));
                        }
                        ui.label(
                            RichText::new(format!("{} / {} lignes", tab.visible.len(), tab.rows.len()))
                                .color(theme::TEXT_WEAK),
                        );
                    });
                });
            });

        // Grid footer: pager
        footer_panel("gridbar").show(ui, |ui| {
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    if icon_button(ui, Icon::Reload, 16.0, false).on_hover_text("Recharger (F5)").clicked() {
                        actions.push(Action::Reload(tab.db_path.clone()));
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        let last = tab.page_count() - 1;
                        if icon_button_enabled(ui, Icon::Last, tab.page < last).clicked() {
                            tab.page = last;
                        }
                        if icon_button_enabled(ui, Icon::Next, tab.page < last).clicked() {
                            tab.page += 1;
                        }
                        egui::Frame::new()
                            .inner_margin(Margin::symmetric(8, 1))
                            .show(ui, |ui| ui.label((tab.page + 1).to_string()));
                        if icon_button_enabled(ui, Icon::Prev, tab.page > 0).clicked() {
                            tab.page -= 1;
                        }
                        if icon_button_enabled(ui, Icon::First, tab.page > 0).clicked() {
                            tab.page = 0;
                        }
                    });
                });
            });

        // Grid
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::PANEL)).show(ui, |ui| {
            let top = ui.max_rect().top();
            ui.painter().hline(ui.max_rect().x_range(), top, Stroke::new(1.0, theme::BORDER));
            if let Some(e) = tab.error.as_ref().or(error.as_ref()) {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    ui.colored_label(ui.visuals().error_fg_color, e);
                });
            }
            let range = tab.page_range();
            let mut clicked = None;
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            TableBuilder::new(ui)
                .resizable(true)
                .sense(Sense::click())
                .cell_layout(Layout::left_to_right(Align::Center))
                .column(Column::exact(16.0).resizable(false))
                .column(Column::initial(320.0).at_least(60.0).clip(true))
                .column(Column::remainder().at_least(100.0).clip(true))
                .header(28.0, |mut header| {
                    header.col(header_cell_bg);
                    header.col(|ui| column_header(ui, "KEY", &key_type));
                    header.col(|ui| column_header(ui, "VALUE", &value_type));
                })
                .body(|body| {
                    body.rows(24.0, range.len(), |mut row| {
                        let idx = tab.visible[range.start + row.index()];
                        let (k, v) = &tab.rows[idx];
                        let row_selected = tab.selected_row == Some(idx);
                        row.col(|ui| {
                            cell_bg(ui, row_selected, row_selected);
                            if row_selected {
                                let c = ui.max_rect().center();
                                ui.painter().add(egui::Shape::convex_polygon(
                                    vec![c + Vec2::new(-3.0, -4.5), c + Vec2::new(3.0, 0.0), c + Vec2::new(-3.0, 4.5)],
                                    theme::TEXT,
                                    Stroke::NONE,
                                ));
                            }
                        });
                        for (col, text, numeric) in
                            [(1, k, db::is_numeric(&key_type)), (2, v, db::is_numeric(&value_type))]
                        {
                            let (_, resp) = row.col(|ui| {
                                cell_bg(ui, row_selected, row_selected && tab.selected_col == col);
                                cell_text(ui, text, numeric);
                            });
                            if resp.clicked() {
                                clicked = Some((idx, col));
                            }
                        }
                    });
                });
            if let Some((i, col)) = clicked {
                tab.selected_row = Some(i);
                tab.selected_col = col;
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Widgets

fn icon(ui: &mut egui::Ui, icon: Icon, size: f32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    icons::paint(ui.painter(), rect, icon);
    resp
}

/// Large icon over a label, like a ribbon toolbar.
fn tool_button(ui: &mut egui::Ui, icon: Icon, label: &str, enabled: bool, active: bool) -> egui::Response {
    let font = FontId::proportional(13.0);
    let text_w = ui.fonts_mut(|f| f.layout_no_wrap(label.into(), font.clone(), theme::TEXT).size().x);
    let size = Vec2::new((text_w + 16.0).max(58.0), 62.0);
    let (rect, resp) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    let painter = ui.painter();
    if active {
        painter.rect_filled(rect, 3.0, theme::TOOL_ACTIVE);
    } else if enabled && resp.hovered() {
        painter.rect_filled(rect, 3.0, theme::HOVER);
    }
    let icon_rect = Rect::from_center_size(rect.center_top() + Vec2::new(0.0, 22.0), Vec2::splat(34.0));
    if enabled {
        icons::paint(painter, icon_rect, icon);
    } else {
        let mut faded = painter.clone();
        faded.multiply_opacity(0.35);
        icons::paint(&faded, icon_rect, icon);
    }
    let color = if enabled { theme::TEXT } else { theme::TEXT_WEAK };
    painter.text(rect.center_bottom() - Vec2::new(0.0, 11.0), Align2::CENTER_CENTER, label, font, color);
    resp
}

/// Small icon + text button without frame (object toolbar).
fn flat_button(ui: &mut egui::Ui, icon: Icon, label: &str) -> egui::Response {
    flat_button_enabled(ui, icon, label, true)
}

fn flat_button_enabled(ui: &mut egui::Ui, icon: Icon, label: &str, enabled: bool) -> egui::Response {
    let font = FontId::proportional(13.0);
    let color = if enabled { theme::TEXT } else { theme::TEXT_WEAK };
    let galley = ui.fonts_mut(|f| f.layout_no_wrap(label.into(), font, color));
    let size = Vec2::new(galley.size().x + 30.0, 24.0);
    let (rect, resp) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    if enabled && resp.hovered() {
        ui.painter().rect_filled(rect, 3.0, theme::HOVER);
    }
    let mut painter = ui.painter().clone();
    if !enabled {
        painter.multiply_opacity(0.4);
    }
    let icon_rect = Rect::from_center_size(rect.left_center() + Vec2::new(13.0, 0.0), Vec2::splat(16.0));
    icons::paint(&painter, icon_rect, icon);
    painter.galley(rect.left_center() + Vec2::new(24.0, -galley.size().y / 2.0), galley, color);
    resp
}

fn icon_button(ui: &mut egui::Ui, icon: Icon, size: f32, active: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size + 8.0), Sense::click());
    if active {
        ui.painter().rect_filled(rect, 3.0, theme::TOOL_ACTIVE);
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, 3.0, theme::HOVER);
    }
    icons::paint(ui.painter(), rect.shrink(4.0), icon);
    resp
}

fn icon_button_enabled(ui: &mut egui::Ui, icon: Icon, enabled: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(22.0), if enabled { Sense::click() } else { Sense::hover() });
    if enabled && resp.hovered() {
        ui.painter().rect_filled(rect, 3.0, theme::HOVER);
    }
    let mut painter = ui.painter().clone();
    if !enabled {
        painter.multiply_opacity(0.3);
    }
    icons::paint(&painter, rect.shrink(4.0), icon);
    resp
}

struct TreeRow {
    response: egui::Response,
    toggled: bool,
}

/// One line of the object tree: chevron, icon, label; full-width selection highlight.
fn tree_row(ui: &mut egui::Ui, depth: usize, expanded: Option<bool>, icon: Icon, label: &str, selected: bool) -> TreeRow {
    let height = 22.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click());
    let painter = ui.painter();
    let indent = rect.left() + 8.0 + depth as f32 * 16.0;
    let chevron = Rect::from_min_size(egui::pos2(indent, rect.top()), Vec2::new(14.0, height));
    let galley = ui.fonts_mut(|f| f.layout_no_wrap(label.into(), FontId::proportional(13.0), theme::TEXT));
    let text_pos = egui::pos2(indent + 38.0, rect.center().y - galley.size().y / 2.0);
    if selected {
        let end = text_pos.x + galley.size().x;
        let sel = Rect::from_min_max(egui::pos2(indent + 15.0, rect.top() + 1.0), egui::pos2(end + 6.0, rect.bottom() - 1.0));
        painter.rect_filled(sel, 2.0, theme::TREE_SELECTED);
        painter.rect_stroke(sel, 2.0, Stroke::new(1.0, theme::TREE_SELECTED_BORDER), StrokeKind::Inside);
    } else if response.hovered() {
        painter.rect_filled(rect, 0.0, theme::HOVER);
    }
    painter.galley(text_pos, galley, theme::TEXT);
    if let Some(open) = expanded {
        let c = chevron.center();
        let st = Stroke::new(1.2, theme::TEXT_WEAK);
        let pts = if open {
            vec![c + Vec2::new(-3.5, -1.5), c + Vec2::new(0.0, 2.0), c + Vec2::new(3.5, -1.5)]
        } else {
            vec![c + Vec2::new(-1.5, -3.5), c + Vec2::new(2.0, 0.0), c + Vec2::new(-1.5, 3.5)]
        };
        painter.add(egui::Shape::line(pts, st));
    }
    icons::paint(painter, Rect::from_center_size(egui::pos2(indent + 26.0, rect.center().y), Vec2::splat(16.0)), icon);

    let on_chevron = response.interact_pointer_pos().is_some_and(|p| chevron.contains(p));
    let toggled = expanded.is_some() && ((response.clicked() && on_chevron) || response.double_clicked());
    TreeRow { response, toggled }
}

struct DocTab {
    response: egui::Response,
    close_clicked: bool,
}

/// A document tab: white with a border when active, close cross on hover.
fn doc_tab(ui: &mut egui::Ui, icon: Option<Icon>, label: &str, active: bool) -> DocTab {
    let galley = ui.fonts_mut(|f| f.layout_no_wrap(label.into(), FontId::proportional(13.0), theme::TEXT));
    let icon_w = if icon.is_some() { 20.0 } else { 0.0 };
    let close_w = if icon.is_some() { 22.0 } else { 0.0 };
    let size = Vec2::new(galley.size().x + icon_w + close_w + 20.0, 28.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter();
    if active {
        painter.rect_filled(rect, CornerRadius { nw: 3, ne: 3, sw: 0, se: 0 }, theme::PANEL);
        let st = Stroke::new(1.0, theme::BORDER);
        painter.line_segment([rect.left_bottom(), rect.left_top()], st);
        painter.line_segment([rect.left_top(), rect.right_top()], st);
        painter.line_segment([rect.right_top(), rect.right_bottom()], st);
    } else if response.hovered() {
        painter.rect_filled(rect, CornerRadius { nw: 3, ne: 3, sw: 0, se: 0 }, theme::HOVER);
    }
    let mut x = rect.left() + 10.0;
    if let Some(i) = icon {
        icons::paint(painter, Rect::from_center_size(egui::pos2(x + 8.0, rect.center().y), Vec2::splat(16.0)), i);
        x += icon_w;
    }
    let color = if icon.is_none() { theme::TEXT_WEAK } else { theme::TEXT };
    painter.galley(egui::pos2(x, rect.center().y - galley.size().y / 2.0), galley, color);

    let mut close_clicked = false;
    if icon.is_some() && (active || response.hovered()) {
        let c = egui::pos2(rect.right() - 16.0, rect.center().y);
        let close = Rect::from_center_size(c, Vec2::splat(16.0));
        let over = ui.ctx().pointer_hover_pos().is_some_and(|p| close.contains(p));
        if over {
            painter.rect_filled(close, 2.0, theme::BORDER);
        }
        let st = Stroke::new(1.2, theme::TEXT_WEAK);
        painter.line_segment([c + Vec2::new(-3.5, -3.5), c + Vec2::new(3.5, 3.5)], st);
        painter.line_segment([c + Vec2::new(-3.5, 3.5), c + Vec2::new(3.5, -3.5)], st);
        close_clicked = response.clicked() && response.interact_pointer_pos().is_some_and(|p| close.contains(p));
    }
    DocTab { response, close_clicked }
}

fn header_cell_bg(ui: &mut egui::Ui) {
    let rect = ui.max_rect();
    ui.painter().rect_filled(rect, 0.0, theme::PANEL);
    ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, theme::BORDER));
}

fn column_header(ui: &mut egui::Ui, name: &str, type_name: &str) {
    header_cell_bg(ui);
    ui.add_space(8.0);
    ui.label(RichText::new(name).family(bold()).size(12.5));
    ui.add_space(8.0);
    ui.label(RichText::new(type_tag(type_name)).size(10.5).color(theme::TEXT_WEAK));
    ui.add_space(4.0);
    ui.label(RichText::new(type_name).size(12.0).color(theme::TEXT_WEAK));
}

fn cell_bg(ui: &mut egui::Ui, row_selected: bool, cell_selected: bool) {
    let rect = ui.max_rect();
    let fill = if cell_selected {
        theme::CELL_SELECTED
    } else if row_selected {
        theme::ROW_SELECTED
    } else {
        theme::PANEL
    };
    ui.painter().rect_filled(rect, 0.0, fill);
    ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, Stroke::new(1.0, theme::GRID_LINE));
}

fn cell_text(ui: &mut egui::Ui, text: &str, right_align: bool) {
    let label = egui::Label::new(RichText::new(text).color(theme::TEXT)).truncate().selectable(false);
    if right_align {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(6.0);
            ui.add(label);
        });
    } else {
        ui.add_space(6.0);
        ui.add(label);
    }
}

fn property(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.label(RichText::new(label).color(theme::TEXT_WEAK));
    ui.add_space(2.0);
    ui.label(value);
    ui.add_space(10.0);
}

// ---------------------------------------------------------------------------
// Formatting

fn type_tag(type_name: &str) -> &'static str {
    match type_name {
        t if db::is_numeric(t) => "#",
        "&str" | "String" | "char" => "ABC",
        "&[u8]" => "0x",
        "bool" => "☑",
        _ => "◆",
    }
}

fn type_label(type_name: &str) -> String {
    format!("{} {type_name}", type_tag(type_name))
}

fn human_bytes(n: u64) -> String {
    let (value, unit) = match n {
        n if n >= 1 << 30 => (n as f64 / (1u64 << 30) as f64, "Go"),
        n if n >= 1 << 20 => (n as f64 / (1u64 << 20) as f64, "Mo"),
        n if n >= 1 << 10 => (n as f64 / 1024.0, "Ko"),
        n => return format!("{n} octets"),
    };
    format!("{} {unit} ({n})", format!("{value:.2}").replace('.', ","))
}

/// Compact size for list columns, rounded up to the kilobyte.
fn short_bytes(n: u64) -> String {
    format!("{} Ko", n.div_ceil(1024))
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}
