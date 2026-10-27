//! Flat, colourful vector icons painted with egui shapes (no image assets).
//! Each icon is designed on a 16×16 grid and scaled to the target rect.

use eframe::egui::{
    epaint::CubicBezierShape, Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke,
    StrokeKind, Vec2,
};

const BLUE: Color32 = Color32::from_rgb(0x2f, 0x80, 0xe0);
const BLUE_BODY: Color32 = Color32::from_rgb(0xd6, 0xe9, 0xfc);
const BLUE_LINE: Color32 = Color32::from_rgb(0x8c, 0xbc, 0xf0);
const GREEN: Color32 = Color32::from_rgb(0x2e, 0xa4, 0x4f);
const GREEN_TOP: Color32 = Color32::from_rgb(0x86, 0xd6, 0x9c);
const GRAY: Color32 = Color32::from_rgb(0x8a, 0x90, 0x99);
const GRAY_TOP: Color32 = Color32::from_rgb(0xc6, 0xca, 0xd0);
const FOLDER_BACK: Color32 = Color32::from_rgb(0xe3, 0xa0, 0x08);
const FOLDER_FRONT: Color32 = Color32::from_rgb(0xf8, 0xc8, 0x3c);
const GOLD: Color32 = Color32::from_rgb(0xd9, 0x9a, 0x0e);
const RED: Color32 = Color32::from_rgb(0xe0, 0x3e, 0x3e);
const DARK: Color32 = Color32::from_rgb(0x5f, 0x66, 0x70);
/// VS Code light theme icon foreground.
const LAYOUT_FG: Color32 = Color32::from_rgb(0x42, 0x42, 0x42);

#[derive(Clone, Copy, PartialEq)]
pub enum Icon {
    Folder,
    Database { open: bool },
    Table,
    Key,
    Field,
    Open,
    Reload,
    Export,
    CloseDb,
    Info,
    Filter,
    Search,
    Copy,
    First,
    Prev,
    Next,
    Last,
    /// Layout toggles, drawn like VS Code's codicons (layout-sidebar-left, ...):
    /// the panel area is filled when the panel is shown, outlined when hidden.
    PanelTop { on: bool },
    PanelLeft { on: bool },
    PanelRight { on: bool },
}

/// Maps 16×16 design coordinates into `rect`.
struct Grid {
    min: Pos2,
    s: f32,
}

impl Grid {
    fn new(rect: Rect) -> Self {
        let size = rect.width().min(rect.height());
        Grid { min: rect.center() - Vec2::splat(size / 2.0), s: size / 16.0 }
    }
    fn p(&self, x: f32, y: f32) -> Pos2 {
        self.min + Vec2::new(x, y) * self.s
    }
    fn r(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
        Rect::from_min_max(self.p(x0, y0), self.p(x1, y1))
    }
    fn w(&self, w: f32) -> f32 {
        w * self.s
    }
}

pub fn paint(p: &Painter, rect: Rect, icon: Icon) {
    let g = Grid::new(rect);
    match icon {
        Icon::Folder => folder(p, &g),
        Icon::Database { open } => database(p, &g, if open { (GREEN, GREEN_TOP) } else { (GRAY, GRAY_TOP) }),
        Icon::Table => table(p, &g, false),
        Icon::Field => table(p, &g, true),
        Icon::Key => key(p, &g),
        Icon::Open => {
            folder(p, &g);
            plus_badge(p, &g);
        }
        Icon::Reload => reload(p, &g),
        Icon::Export => {
            table(p, &Grid { min: g.min, s: g.s * 0.85 }, false);
            arrow_out(p, &g);
        }
        Icon::CloseDb => {
            database(p, &g, (GRAY, GRAY_TOP));
            cross_badge(p, &g);
        }
        Icon::Info => {
            p.circle_stroke(g.p(8.0, 8.0), g.w(6.6), Stroke::new(g.w(1.3), BLUE));
            p.circle_filled(g.p(8.0, 4.9), g.w(1.0), BLUE);
            p.line_segment([g.p(8.0, 7.0), g.p(8.0, 11.8)], Stroke::new(g.w(1.6), BLUE));
        }
        Icon::Filter => {
            let pts = vec![g.p(2.0, 3.0), g.p(14.0, 3.0), g.p(9.5, 8.5), g.p(9.5, 13.5), g.p(6.5, 12.0), g.p(6.5, 8.5)];
            p.add(Shape::convex_polygon(pts[..3].iter().copied().chain([pts[5]]).collect(), BLUE_BODY, Stroke::NONE));
            p.add(Shape::closed_line(pts, Stroke::new(g.w(1.1), BLUE)));
        }
        Icon::Search => {
            p.circle_stroke(g.p(6.8, 6.8), g.w(4.3), Stroke::new(g.w(1.4), DARK));
            p.line_segment([g.p(10.0, 10.0), g.p(14.0, 14.0)], Stroke::new(g.w(1.8), DARK));
        }
        Icon::Copy => {
            let st = Stroke::new(g.w(1.1), DARK);
            p.rect_stroke(g.r(2.5, 2.5, 10.0, 11.0), CornerRadius::same(1), st, StrokeKind::Middle);
            p.rect_filled(g.r(6.0, 5.0, 13.5, 13.5), CornerRadius::same(1), Color32::WHITE);
            p.rect_stroke(g.r(6.0, 5.0, 13.5, 13.5), CornerRadius::same(1), st, StrokeKind::Middle);
        }
        Icon::First | Icon::Prev | Icon::Next | Icon::Last => pager(p, &g, icon),
        Icon::PanelTop { on } | Icon::PanelLeft { on } | Icon::PanelRight { on } => {
            let color = LAYOUT_FG;
            let st = Stroke::new(g.w(1.0), color);
            // Outer frame 1.5..14.5 × 2.5..13.5, split by one line like the codicons.
            let (area, divider) = match icon {
                Icon::PanelTop { .. } => (g.r(1.5, 2.5, 14.5, 6.5), [g.p(1.5, 6.5), g.p(14.5, 6.5)]),
                Icon::PanelLeft { .. } => (g.r(1.5, 2.5, 6.5, 13.5), [g.p(6.5, 2.5), g.p(6.5, 13.5)]),
                _ => (g.r(9.5, 2.5, 14.5, 13.5), [g.p(9.5, 2.5), g.p(9.5, 13.5)]),
            };
            if on {
                p.rect_filled(area, CornerRadius::same(1), color);
            }
            p.line_segment(divider, st);
            p.rect_stroke(g.r(1.5, 2.5, 14.5, 13.5), CornerRadius::same(1), st, StrokeKind::Middle);
        }
    }
}

fn table(p: &Painter, g: &Grid, highlight_column: bool) {
    let body = g.r(1.5, 2.5, 14.5, 13.5);
    p.rect_filled(body, CornerRadius::same(2), BLUE_BODY);
    if highlight_column {
        p.rect_filled(g.r(6.0, 5.5, 10.0, 13.5), CornerRadius::ZERO, BLUE_LINE);
    }
    let line = Stroke::new(g.w(0.8), BLUE_LINE);
    for y in [8.2, 10.9] {
        p.line_segment([g.p(1.5, y), g.p(14.5, y)], line);
    }
    for x in [6.0, 10.0] {
        p.line_segment([g.p(x, 5.5), g.p(x, 13.5)], line);
    }
    p.rect_filled(g.r(1.5, 2.5, 14.5, 5.5), CornerRadius { nw: 2, ne: 2, sw: 0, se: 0 }, BLUE);
    p.rect_stroke(body, CornerRadius::same(2), Stroke::new(g.w(0.9), BLUE), StrokeKind::Inside);
}

fn database(p: &Painter, g: &Grid, (body, top): (Color32, Color32)) {
    let r = Vec2::new(g.w(5.0), g.w(1.9));
    p.rect_filled(g.r(3.0, 3.6, 13.0, 12.4), CornerRadius::ZERO, body);
    p.add(Shape::ellipse_filled(g.p(8.0, 12.4), r, body));
    // Subtle band lines give the cylinder its "stacked disks" look.
    for y in [6.6, 9.5] {
        p.add(CubicBezierShape::from_points_stroke(
            [g.p(3.0, y), g.p(4.5, y + 2.3), g.p(11.5, y + 2.3), g.p(13.0, y)],
            false,
            Color32::TRANSPARENT,
            Stroke::new(g.w(0.7), top),
        ));
    }
    p.add(Shape::ellipse_filled(g.p(8.0, 3.6), r, top));
}

fn folder(p: &Painter, g: &Grid) {
    p.rect_filled(g.r(1.5, 2.8, 7.0, 6.0), CornerRadius::same(1), FOLDER_BACK);
    p.rect_filled(g.r(1.5, 4.2, 14.5, 13.2), CornerRadius::same(1), FOLDER_BACK);
    p.rect_filled(g.r(1.5, 5.6, 14.5, 13.2), CornerRadius::same(1), FOLDER_FRONT);
}

fn key(p: &Painter, g: &Grid) {
    let st = Stroke::new(g.w(1.6), GOLD);
    p.circle_stroke(g.p(5.0, 8.0), g.w(2.7), st);
    p.line_segment([g.p(7.7, 8.0), g.p(14.0, 8.0)], st);
    p.line_segment([g.p(12.0, 8.0), g.p(12.0, 10.6)], st);
    p.line_segment([g.p(14.0, 8.0), g.p(14.0, 10.6)], st);
}

fn plus_badge(p: &Painter, g: &Grid) {
    let c = g.p(12.2, 12.2);
    p.circle_filled(c, g.w(3.6), Color32::WHITE);
    p.circle_filled(c, g.w(3.0), GREEN);
    let st = Stroke::new(g.w(1.2), Color32::WHITE);
    p.line_segment([c - Vec2::new(g.w(1.7), 0.0), c + Vec2::new(g.w(1.7), 0.0)], st);
    p.line_segment([c - Vec2::new(0.0, g.w(1.7)), c + Vec2::new(0.0, g.w(1.7))], st);
}

fn cross_badge(p: &Painter, g: &Grid) {
    let c = g.p(12.2, 12.2);
    p.circle_filled(c, g.w(3.6), Color32::WHITE);
    p.circle_filled(c, g.w(3.0), RED);
    let st = Stroke::new(g.w(1.2), Color32::WHITE);
    let d = g.w(1.3);
    p.line_segment([c + Vec2::new(-d, -d), c + Vec2::new(d, d)], st);
    p.line_segment([c + Vec2::new(-d, d), c + Vec2::new(d, -d)], st);
}

fn arrow_out(p: &Painter, g: &Grid) {
    let st = Stroke::new(g.w(1.6), GREEN);
    p.circle_filled(g.p(12.0, 11.5), g.w(4.0), Color32::WHITE);
    p.line_segment([g.p(9.2, 14.2), g.p(14.3, 9.1)], st);
    p.line_segment([g.p(10.8, 9.1), g.p(14.3, 9.1)], st);
    p.line_segment([g.p(14.3, 9.1), g.p(14.3, 12.6)], st);
}

fn reload(p: &Painter, g: &Grid) {
    let st = Stroke::new(g.w(1.7), BLUE);
    let c = g.p(8.0, 8.0);
    let radius = g.w(5.2);
    let arc = |from: f32, to: f32| -> Vec<Pos2> {
        (0..=16)
            .map(|i| {
                let a = (from + (to - from) * i as f32 / 16.0).to_radians();
                c + Vec2::new(a.cos(), a.sin()) * radius
            })
            .collect()
    };
    p.add(Shape::line(arc(200.0, 340.0), st));
    p.add(Shape::line(arc(20.0, 160.0), st));
    let head = |tip: Pos2, dir: Vec2| {
        let n = Vec2::new(-dir.y, dir.x);
        p.add(Shape::convex_polygon(
            vec![tip + dir * g.w(2.2), tip + n * g.w(2.0), tip - n * g.w(2.0)],
            BLUE,
            Stroke::NONE,
        ));
    };
    let a = 340f32.to_radians();
    head(c + Vec2::new(a.cos(), a.sin()) * radius, Vec2::new(-a.sin(), a.cos()));
    let a = 160f32.to_radians();
    head(c + Vec2::new(a.cos(), a.sin()) * radius, Vec2::new(-a.sin(), a.cos()));
}

fn pager(p: &Painter, g: &Grid, icon: Icon) {
    let st = Stroke::new(g.w(1.4), DARK);
    let (left, bar) = match icon {
        Icon::First => (true, true),
        Icon::Prev => (true, false),
        Icon::Next => (false, false),
        _ => (false, true),
    };
    let (tip, back, bar_x) = if left { (5.5, 10.0, 4.0) } else { (10.5, 6.0, 12.0) };
    p.add(Shape::line(vec![g.p(back, 3.5), g.p(tip, 8.0), g.p(back, 12.5)], st));
    if bar {
        p.line_segment([g.p(bar_x, 3.5), g.p(bar_x, 12.5)], st);
    }
}
