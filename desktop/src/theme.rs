//! Design tokens and drawing helpers that mirror the look of keepers.html:
//! soft neumorphic surfaces, warm accent, Bricolage Grotesque.

use egui::{Color32, FontFamily, FontId, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};

pub const SIDEBAR: f32 = 272.0;

#[derive(Clone)]
pub struct Pal {
    pub dark: bool,
    pub bg: Color32,
    pub hi: Color32,
    pub lo: Color32,
    pub hi_s: Color32,
    pub lo_s: Color32,
    pub tx: Color32,
    pub mu: Color32,
    pub ac: Color32,
    pub ac2: Color32,
    pub cool: Color32,
    pub line: Color32,
    pub media: Color32,
    pub ok: Color32,
    pub bad: Color32,
}

pub fn pal(dark: bool) -> Pal {
    if dark {
        Pal {
            dark,
            bg: Color32::from_rgb(0x20, 0x24, 0x2c),
            hi: Color32::from_rgb(0x2b, 0x30, 0x3a),
            lo: Color32::from_rgb(0x14, 0x17, 0x1c),
            hi_s: Color32::from_rgb(0x2b, 0x30, 0x3a),
            lo_s: Color32::from_rgb(0x14, 0x17, 0x1c),
            tx: Color32::from_rgb(0xee, 0xf1, 0xf7),
            mu: Color32::from_rgb(0x9a, 0xa3, 0xb4),
            ac: Color32::from_rgb(0xff, 0x8f, 0x52),
            ac2: Color32::from_rgb(0xff, 0xc3, 0x9a),
            cool: Color32::from_rgb(0x81, 0x8c, 0xff),
            line: Color32::from_rgba_unmultiplied(255, 255, 255, 31),
            media: Color32::from_rgb(0x0d, 0x0e, 0x11),
            ok: Color32::from_rgb(0x2e, 0xc0, 0x7e),
            bad: Color32::from_rgb(0xf0, 0x6a, 0x6a),
        }
    } else {
        Pal {
            dark,
            bg: Color32::from_rgb(0xdd, 0xe3, 0xee),
            hi: Color32::from_rgb(0xff, 0xff, 0xff),
            lo: Color32::from_rgb(0xad, 0xb8, 0xcb),
            hi_s: Color32::from_rgb(0xff, 0xff, 0xff),
            lo_s: Color32::from_rgb(0xb8, 0xc2, 0xd4),
            tx: Color32::from_rgb(0x1c, 0x23, 0x30),
            mu: Color32::from_rgb(0x59, 0x63, 0x77),
            ac: Color32::from_rgb(0xe8, 0x55, 0x1d),
            ac2: Color32::from_rgb(0xff, 0x9a, 0x5c),
            cool: Color32::from_rgb(0x5f, 0x6b, 0xff),
            line: Color32::from_rgba_unmultiplied(255, 255, 255, 204),
            media: Color32::from_rgb(0x0d, 0x0e, 0x11),
            ok: Color32::from_rgb(0x12, 0x86, 0x5a),
            bad: Color32::from_rgb(0xc9, 0x3a, 0x3a),
        }
    }
}

pub fn regular(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semi".into()))
}
pub fn bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut f = egui::FontDefinitions::default();
    f.font_data.insert(
        "bg".into(),
        egui::FontData::from_static(include_bytes!("../assets/Bricolage-Regular.ttf")),
    );
    f.font_data.insert(
        "bgsemi".into(),
        egui::FontData::from_static(include_bytes!("../assets/Bricolage-SemiBold.ttf")),
    );
    f.font_data.insert(
        "bgbold".into(),
        egui::FontData::from_static(include_bytes!("../assets/Bricolage-Bold.ttf")),
    );
    f.families
        .insert(egui::FontFamily::Proportional, vec!["bg".to_owned()]);
    f.families
        .insert(egui::FontFamily::Name("semi".into()), vec!["bgsemi".to_owned()]);
    f.families
        .insert(egui::FontFamily::Name("bold".into()), vec!["bgbold".to_owned()]);
    ctx.set_fonts(f);
}

pub fn apply(ctx: &egui::Context, dark: bool) {
    install_fonts(ctx);
    let p = pal(dark);
    let mut v = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.panel_fill = Color32::TRANSPARENT;
    v.window_fill = p.bg;
    v.extreme_bg_color = p.bg;
    v.faint_bg_color = p.bg;
    v.window_stroke = Stroke::NONE;
    v.window_rounding = egui::Rounding::same(30.0);
    v.window_shadow = egui::epaint::Shadow::NONE;
    v.popup_shadow = egui::epaint::Shadow::NONE;
    v.override_text_color = Some(p.tx);
    v.widgets.noninteractive.bg_fill = p.bg;
    v.widgets.noninteractive.bg_stroke = Stroke::new(0.0, Color32::TRANSPARENT);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.tx);
    v.widgets.noninteractive.rounding = egui::Rounding::same(14.0);
    v.widgets.inactive.bg_fill = p.bg;
    v.widgets.inactive.weak_bg_fill = p.bg;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.tx);
    v.widgets.inactive.bg_stroke = Stroke::new(0.0, Color32::TRANSPARENT);
    v.widgets.inactive.rounding = egui::Rounding::same(14.0);
    v.widgets.hovered.bg_fill = p.bg;
    v.widgets.hovered.weak_bg_fill = p.bg;
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, p.ac);
    v.widgets.hovered.bg_stroke = Stroke::new(0.0, Color32::TRANSPARENT);
    v.widgets.hovered.rounding = egui::Rounding::same(14.0);
    v.widgets.active.bg_fill = p.bg;
    v.widgets.active.weak_bg_fill = p.bg;
    v.widgets.active.fg_stroke = Stroke::new(1.0, p.ac);
    v.widgets.active.rounding = egui::Rounding::same(14.0);
    v.selection.bg_fill = p.ac.gamma_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0, p.tx);
    v.slider_trailing_fill = true;
    let mut style = (*ctx.style()).clone();
    style.visuals = v;
    style.spacing.item_spacing = Vec2::new(10.0, 10.0);
    style.spacing.button_padding = Vec2::new(14.0, 9.0);
    style.spacing.slider_width = 190.0;
    style.text_styles = [
        (egui::TextStyle::Body, regular(14.0)),
        (egui::TextStyle::Button, regular(14.0)),
        (egui::TextStyle::Small, regular(12.0)),
        (egui::TextStyle::Heading, semibold(34.0)),
        (egui::TextStyle::Monospace, FontId::monospace(13.0)),
    ]
    .into();
    ctx.set_style(style);
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()))
}

/// A raised panel in the same colour family as the page, lighter than the page.
pub fn surface(p: &Pal, painter: &egui::Painter, rect: Rect, rounding: f32, lift: f32) {
    raised(p, painter, rect, rounding, true);
    let fill = if p.dark {
        mix(p.bg, p.hi, lift)
    } else {
        mix(p.bg, p.hi, lift)
    };
    painter.rect_filled(rect, egui::Rounding::same(rounding), fill);
}

/// Button drawn into a fixed rect, optional icon on the left.
pub fn button_rect(
    ui: &mut Ui,
    p: &Pal,
    rect: Rect,
    label: &str,
    icon: Option<Icon>,
    font: FontId,
) -> Response {
    let resp = ui.interact(rect, ui.id().with(("btn", label)), Sense::click());
    let painter = ui.painter();
    if resp.is_pointer_button_down_on() {
        painter.rect_filled(rect, egui::Rounding::same(14.0), p.bg);
        inset(p, painter, rect, 14.0);
    } else {
        raised(p, painter, rect, 14.0, false);
        painter.rect_filled(rect, egui::Rounding::same(14.0), p.bg);
    }
    let color = if resp.hovered() { p.ac } else { p.tx };
    let galley = painter.layout_no_wrap(label.to_owned(), font.clone(), color);
    let tx = match icon {
        Some(_) => rect.left() + 16.0 + 22.0,
        None => rect.center().x - galley.size().x / 2.0,
    };
    if let Some(ic) = icon {
        draw_icon(
            painter,
            Pos2::new(rect.left() + 22.0, rect.center().y),
            8.0,
            ic,
            color,
            1.7,
        );
    }
    painter.galley(
        Pos2::new(tx, rect.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
    resp
}

fn tint(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

/// Soft offset shadow, built from stacked rounded rects so it needs no blur shader.
pub fn soft(
    painter: &egui::Painter,
    rect: Rect,
    rounding: f32,
    off: Vec2,
    blur: f32,
    color: Color32,
) {
    let n = 10;
    for i in 0..n {
        let t = i as f32 / (n - 1) as f32; // 0 outermost
        let grow = blur * (1.0 - t) * 0.5;
        let a = (15.0f32 * (1.0 - t).powf(1.4)) as u8;
        if a == 0 {
            continue;
        }
        painter.rect_filled(
            rect.translate(off).expand(grow),
            egui::Rounding::same(rounding + grow),
            tint(color, a),
        );
    }
}

/// The 8px / 20px raised look from the page.
pub fn raised(p: &Pal, painter: &egui::Painter, rect: Rect, rounding: f32, big: bool) {
    let (d, b) = if big { (8.0, 20.0) } else { (4.0, 10.0) };
    soft(painter, rect, rounding, Vec2::new(d, d), b, p.lo_s);
    soft(painter, rect, rounding, Vec2::new(-d, -d), b, p.hi_s);
}

/// Pressed surface: inset shading on a flat fill.
pub fn inset(p: &Pal, painter: &egui::Painter, rect: Rect, rounding: f32) {
    painter.rect_filled(rect, egui::Rounding::same(rounding), p.bg);
    let n = 5;
    for i in 0..n {
        let t = i as f32 / n as f32;
        let r = rect.shrink(i as f32 * 0.9);
        painter.rect_stroke(
            r,
            egui::Rounding::same((rounding - i as f32 * 0.9).max(2.0)),
            Stroke::new(1.3, tint(p.lo, (34.0 * (1.0 - t)) as u8)),
        );
    }
    for i in 0..3 {
        let t = i as f32 / 3.0;
        let r = rect.shrink(4.6 + i as f32 * 1.1);
        painter.rect_stroke(
            r,
            egui::Rounding::same((rounding - 4.0).max(2.0)),
            Stroke::new(1.1, tint(p.hi, (52.0 * (1.0 - t)) as u8)),
        );
    }
}

pub fn dot(p: &Pal, painter: &egui::Painter, c: Pos2, color: Color32, on: bool) {
    if on {
        painter.circle_filled(c, 7.5, tint(color, 40));
        painter.circle_filled(c, 5.5, color);
    } else {
        painter.circle_filled(c, 5.5, tint(color, 150));
    }
    let _ = p;
}

/// Plain raised button.
pub fn button(ui: &mut Ui, p: &Pal, label: &str, primary: bool) -> Response {
    let font = if primary { semibold(14.0) } else { regular(14.0) };
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        font.clone(),
        if primary { p.ac } else { p.tx },
    );
    let pad = Vec2::new(14.0, 9.0);
    let size = galley.size() + pad * 2.0;
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter();
    if resp.is_pointer_button_down_on() {
        painter.rect_filled(rect, egui::Rounding::same(14.0), p.bg);
        inset(p, painter, rect, 14.0);
    } else {
        raised(p, painter, rect, 14.0, false);
        painter.rect_filled(rect, egui::Rounding::same(14.0), p.bg);
    }
    let color = if primary {
        p.ac
    } else if resp.hovered() {
        p.ac
    } else {
        p.tx
    };
    let g = ui.painter().layout_no_wrap(label.to_owned(), font, color);
    ui.painter().galley(
        rect.center() - g.size() / 2.0,
        g,
        color,
    );
    resp
}

/// Square icon button, used for the small toolbar corners.
pub fn icon_button(ui: &mut Ui, p: &Pal, icon: Icon, color: Color32, size: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let painter = ui.painter();
    raised(p, painter, rect, size * 0.4, false);
    painter.rect_filled(rect, egui::Rounding::same(size * 0.4), p.bg);
    let c = if resp.hovered() { color } else { p.mu };
    draw_icon(painter, rect.center(), size * 0.24, icon, c, 1.8);
    resp
}

/// The 80px round action button with an icon and a caption.
pub fn round_button(
    ui: &mut Ui,
    p: &Pal,
    icon: Icon,
    caption: &str,
    color: Color32,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(82.0), Sense::click());
    let painter = ui.painter();
    let pressed = resp.is_pointer_button_down_on();
    if pressed {
        painter.rect_filled(rect, egui::Rounding::same(41.0), p.bg);
        inset(p, painter, rect, 41.0);
    } else {
        raised(p, painter, rect, 41.0, true);
        painter.rect_filled(rect, egui::Rounding::same(41.0), p.bg);
    }
    let hovered = resp.hovered();
    let ic = if hovered { color } else { color };
    draw_icon(
        ui.painter(),
        Pos2::new(rect.center().x, rect.center().y - 9.0),
        13.0,
        icon,
        ic,
        1.9,
    );
    ui.painter().text(
        Pos2::new(rect.center().x, rect.bottom() - 22.0),
        egui::Align2::CENTER_CENTER,
        caption,
        regular(11.0),
        p.mu,
    );
    resp
}

/// Small pill, used for the move-to row and chips.
pub fn pill(ui: &mut Ui, p: &Pal, label: &str, active: bool) -> Response {
    let font = if active { semibold(12.0) } else { regular(12.0) };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font.clone(), p.tx);
    let pad = Vec2::new(14.0, 6.0);
    let size = galley.size() + pad * 2.0;
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter();
    raised(p, painter, rect, size.y / 2.0, false);
    painter.rect_filled(rect, egui::Rounding::same(size.y / 2.0), p.bg);
    let color = if active || resp.hovered() { p.ac } else { p.tx };
    let g = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font, color);
    ui.painter()
        .galley(rect.center() - g.size() / 2.0, g, color);
    resp
}

pub fn kbd(ui: &mut Ui, p: &Pal, key: &str) {
    let font = regular(11.0);
    let galley = ui.painter().layout_no_wrap(key.to_owned(), font, p.tx);
    let size = galley.size() + Vec2::new(14.0, 3.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();
    raised(p, painter, rect, 7.0, false);
    painter.rect_filled(rect, egui::Rounding::same(7.0), p.bg);
    painter.galley(rect.center() - galley.size() / 2.0, galley, p.tx);
}

pub fn progress(
    p: &Pal,
    painter: &egui::Painter,
    rect: Rect,
    frac: f32,
    gradient: bool,
) {
    let r = rect.height() / 2.0;
    inset(p, painter, rect, r);
    let inner = rect.shrink(3.0);
    if frac > 0.0 {
        let mut f = inner;
        f.set_width((inner.width() * frac).max(6.0));
        let a = if gradient {
            Color32::from_rgb(0xff, 0xa2, 0x67)
        } else {
            p.ac
        };
        let mut m = egui::Mesh::default();
        m.colored_vertex(f.left_top(), a);
        m.colored_vertex(f.right_top(), p.ac);
        m.colored_vertex(f.right_bottom(), p.ac);
        m.colored_vertex(f.left_bottom(), a);
        m.add_triangle(0, 1, 2);
        m.add_triangle(0, 2, 3);
        painter.add(egui::Shape::mesh(m));
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Reject,
    Later,
    Skip,
    Keep,
    Final,
    Compare,
    Undo,
    Grid,
    Swipe,
    Gear,
    Folder,
    X,
    Download,
    Plus,
    Chevron,
}

pub fn draw_icon(painter: &egui::Painter, c: Pos2, s: f32, icon: Icon, color: Color32, w: f32) {
    let stroke = Stroke::new(w, color);
    match icon {
        Icon::Reject | Icon::X => {
            let k = s * 0.78;
            painter.line_segment([c + Vec2::new(-k, -k), c + Vec2::new(k, k)], stroke);
            painter.line_segment([c + Vec2::new(k, -k), c + Vec2::new(-k, k)], stroke);
        }
        Icon::Later => {
            painter.circle_stroke(c, s * 0.85, stroke);
            painter.line_segment([c, c + Vec2::new(0.0, -s * 0.5)], stroke);
            painter.line_segment([c, c + Vec2::new(s * 0.4, s * 0.08)], stroke);
        }
        Icon::Skip => {
            painter.line_segment(
                [c + Vec2::new(-s * 0.6, -s * 0.55), c + Vec2::new(0.0, 0.0)],
                stroke,
            );
            painter.line_segment(
                [c + Vec2::new(s * 0.6, -s * 0.55), c + Vec2::new(0.0, 0.0)],
                stroke,
            );
            painter.line_segment(
                [
                    c + Vec2::new(0.0, -s * 0.2),
                    c + Vec2::new(0.0, s * 0.8),
                ],
                stroke,
            );
        }
        Icon::Keep => {
            painter.line_segment(
                [
                    c + Vec2::new(-s * 0.85, 0.0),
                    c + Vec2::new(-s * 0.15, s * 0.68),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    c + Vec2::new(-s * 0.15, s * 0.68),
                    c + Vec2::new(s * 0.85, -s * 0.65),
                ],
                stroke,
            );
        }
        Icon::Final => {
            let mut pts = Vec::new();
            for i in 0..10 {
                let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                let rr = if i % 2 == 0 { s } else { s * 0.44 };
                pts.push(c + Vec2::new(a.cos() * rr, a.sin() * rr));
            }
            painter.add(egui::Shape::closed_line(pts, stroke));
        }
        Icon::Compare => {
            painter.line_segment(
                [
                    c + Vec2::new(-s * 0.85, -s * 0.32),
                    c + Vec2::new(s * 0.45, -s * 0.32),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    c + Vec2::new(s * 0.1, -s * 0.65),
                    c + Vec2::new(s * 0.45, -s * 0.32),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    c + Vec2::new(s * 0.85, s * 0.32),
                    c + Vec2::new(-s * 0.45, s * 0.32),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    c + Vec2::new(-s * 0.1, s * 0.65),
                    c + Vec2::new(-s * 0.45, s * 0.32),
                ],
                stroke,
            );
        }
        Icon::Undo => {
            painter.add(egui::Shape::line(
                vec![
                    c + Vec2::new(s * 0.7, -s * 0.5),
                    c + Vec2::new(-s * 0.3, -s * 0.5),
                    c + Vec2::new(-s * 0.3, s * 0.35),
                    c + Vec2::new(s * 0.75, s * 0.35),
                ],
                stroke,
            ));
            painter.line_segment(
                [
                    c + Vec2::new(-s * 0.65, -s * 0.8),
                    c + Vec2::new(-s * 0.3, -s * 0.5),
                ],
                stroke,
            );
        }
        Icon::Grid => {
            let u = s * 0.62;
            for (dx, dy) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                painter.rect_stroke(
                    Rect::from_center_size(
                        c + Vec2::new(dx * u * 0.62, dy * u * 0.62),
                        Vec2::splat(u * 0.95),
                    ),
                    egui::Rounding::same(2.0),
                    stroke,
                );
            }
        }
        Icon::Swipe => {
            painter.rect_stroke(
                Rect::from_center_size(c, Vec2::new(s * 1.9, s * 1.4)),
                egui::Rounding::same(3.0),
                stroke,
            );
            painter.line_segment(
                [c + Vec2::new(-s * 0.45, 0.0), c + Vec2::new(s * 0.45, 0.0)],
                stroke,
            );
        }
        Icon::Gear => {
            let n = 8;
            for i in 0..n {
                let a = i as f32 * std::f32::consts::TAU / n as f32;
                painter.circle_filled(
                    c + Vec2::new(a.cos(), a.sin()) * (s * 0.9),
                    s * 0.26,
                    color,
                );
            }
            painter.circle_stroke(c, s * 0.72, Stroke::new(w, color));
            painter.circle_stroke(c, s * 0.3, Stroke::new(w, color));
        }
        Icon::Folder => {
            painter.rect_stroke(
                Rect::from_center_size(c + Vec2::new(0.0, s * 0.08), Vec2::new(s * 1.9, s * 1.25)),
                egui::Rounding::same(3.0),
                stroke,
            );
            painter.line_segment(
                [
                    c + Vec2::new(-s * 0.95, -s * 0.5),
                    c + Vec2::new(-s * 0.25, -s * 0.5),
                ],
                stroke,
            );
        }
        Icon::Download => {
            painter.line_segment(
                [c + Vec2::new(0.0, -s * 0.8), c + Vec2::new(0.0, s * 0.35)],
                stroke,
            );
            painter.line_segment(
                [
                    c + Vec2::new(-s * 0.45, -s * 0.1),
                    c + Vec2::new(0.0, s * 0.35),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    c + Vec2::new(s * 0.45, -s * 0.1),
                    c + Vec2::new(0.0, s * 0.35),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    c + Vec2::new(-s * 0.8, s * 0.8),
                    c + Vec2::new(s * 0.8, s * 0.8),
                ],
                stroke,
            );
        }
        Icon::Plus => {
            painter.line_segment(
                [c + Vec2::new(-s * 0.75, 0.0), c + Vec2::new(s * 0.75, 0.0)],
                stroke,
            );
            painter.line_segment(
                [c + Vec2::new(0.0, -s * 0.75), c + Vec2::new(0.0, s * 0.75)],
                stroke,
            );
        }
        Icon::Chevron => {
            painter.line_segment(
                [c + Vec2::new(-s * 0.5, -s * 0.25), c + Vec2::new(0.0, s * 0.3)],
                stroke,
            );
            painter.line_segment(
                [c + Vec2::new(s * 0.5, -s * 0.25), c + Vec2::new(0.0, s * 0.3)],
                stroke,
            );
        }
    }
}

/// Paints the four corner wedges of `rect` in `color`, so content inside it
/// reads as clipped to a rounded rect.
pub fn round_mask(painter: &egui::Painter, rect: Rect, r: f32, color: Color32) {
    let r = r.min(rect.width() * 0.5).min(rect.height() * 0.5);
    if r <= 0.0 {
        return;
    }
    let corners = [
        (rect.left_top(), 1.0f32, 1.0f32),
        (rect.right_top(), -1.0, 1.0),
        (rect.right_bottom(), -1.0, -1.0),
        (rect.left_bottom(), 1.0, -1.0),
    ];
    let steps = 8;
    for (c, sx, sy) in corners {
        let center = Pos2::new(c.x + sx * r, c.y + sy * r);
        let mut pts = vec![c];
        for i in 0..=steps {
            let a = std::f32::consts::FRAC_PI_2 * (i as f32 / steps as f32);
            let (sin, cos) = a.sin_cos();
            pts.push(Pos2::new(
                center.x - sx * r * cos,
                center.y - sy * r * sin,
            ));
        }
        painter.add(egui::Shape::Path(egui::epaint::PathShape {
            points: pts,
            closed: true,
            fill: color,
            stroke: egui::epaint::PathStroke::NONE,
        }));
    }
}

pub fn guides(painter: &egui::Painter, rect: Rect, kind: &str) {
    let shadow = Stroke::new(2.2, Color32::from_rgba_unmultiplied(0, 0, 0, 90));
    let line = Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 210));
    let mut segs: Vec<[Pos2; 2]> = Vec::new();
    match kind {
        "thirds" => {
            for i in 1..3 {
                let f = i as f32 / 3.0;
                segs.push([
                    Pos2::new(rect.left() + rect.width() * f, rect.top()),
                    Pos2::new(rect.left() + rect.width() * f, rect.bottom()),
                ]);
                segs.push([
                    Pos2::new(rect.left(), rect.top() + rect.height() * f),
                    Pos2::new(rect.right(), rect.top() + rect.height() * f),
                ]);
            }
        }
        "golden" => {
            for f in [0.381966f32, 0.618034] {
                segs.push([
                    Pos2::new(rect.left() + rect.width() * f, rect.top()),
                    Pos2::new(rect.left() + rect.width() * f, rect.bottom()),
                ]);
                segs.push([
                    Pos2::new(rect.left(), rect.top() + rect.height() * f),
                    Pos2::new(rect.right(), rect.top() + rect.height() * f),
                ]);
            }
        }
        "cross" => {
            segs.push([
                Pos2::new(rect.center().x, rect.top()),
                Pos2::new(rect.center().x, rect.bottom()),
            ]);
            segs.push([
                Pos2::new(rect.left(), rect.center().y),
                Pos2::new(rect.right(), rect.center().y),
            ]);
        }
        "diag" => {
            segs.push([rect.left_top(), rect.right_bottom()]);
            segs.push([rect.right_top(), rect.left_bottom()]);
        }
        _ => {}
    }
    let painter = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    for s in &segs {
        painter.line_segment(*s, shadow);
    }
    for s in &segs {
        painter.line_segment(*s, line);
    }
}
