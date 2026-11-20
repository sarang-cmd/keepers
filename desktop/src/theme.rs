use egui::{Color32, FontId, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};

#[derive(Clone)]
pub struct Pal {
    pub dark: bool,
    pub bg_top: Color32,
    pub bg_bottom: Color32,
    pub panel: Color32,
    pub panel_soft: Color32,
    pub line: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_soft: Color32,
    pub green: Color32,
    pub gold: Color32,
    pub violet: Color32,
    pub red: Color32,
}

pub fn pal(dark: bool) -> Pal {
    if dark {
        Pal {
            dark,
            bg_top: Color32::from_rgb(13, 17, 23),
            bg_bottom: Color32::from_rgb(9, 12, 16),
            panel: Color32::from_rgba_unmultiplied(23, 28, 36, 252),
            panel_soft: Color32::from_rgba_unmultiplied(255, 255, 255, 10),
            line: Color32::from_rgba_unmultiplied(255, 255, 255, 18),
            text: Color32::from_rgb(230, 237, 243),
            muted: Color32::from_rgb(139, 148, 158),
            accent: Color32::from_rgb(255, 122, 69),
            accent_soft: Color32::from_rgb(255, 159, 107),
            green: Color32::from_rgb(74, 222, 128),
            gold: Color32::from_rgb(234, 179, 8),
            violet: Color32::from_rgb(167, 139, 250),
            red: Color32::from_rgb(248, 113, 113),
        }
    } else {
        Pal {
            dark,
            bg_top: Color32::from_rgb(247, 248, 252),
            bg_bottom: Color32::from_rgb(233, 236, 244),
            panel: Color32::from_rgba_unmultiplied(255, 255, 255, 252),
            panel_soft: Color32::from_rgba_unmultiplied(13, 17, 23, 10),
            line: Color32::from_rgba_unmultiplied(13, 17, 23, 22),
            text: Color32::from_rgb(16, 22, 30),
            muted: Color32::from_rgb(91, 100, 114),
            accent: Color32::from_rgb(226, 92, 36),
            accent_soft: Color32::from_rgb(255, 138, 84),
            green: Color32::from_rgb(22, 163, 74),
            gold: Color32::from_rgb(161, 98, 7),
            violet: Color32::from_rgb(124, 58, 237),
            red: Color32::from_rgb(220, 38, 38),
        }
    }
}

pub fn gradient_bg(ctx: &egui::Context, p: &Pal) {
    let screen: Rect = ctx.screen_rect();
    let painter = ctx.layer_painter(egui::LayerId::background());
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(screen.left_top(), p.bg_top);
    mesh.colored_vertex(screen.right_top(), p.bg_top);
    mesh.colored_vertex(screen.right_bottom(), p.bg_bottom);
    mesh.colored_vertex(screen.left_bottom(), p.bg_bottom);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(mesh));

    // a hairline of warm colour along the bottom edge, no banding
    let bar = Rect::from_min_size(
        Pos2::new(screen.left(), screen.bottom() - 2.0),
        Vec2::new(screen.width(), 2.0),
    );
    let mut m = egui::Mesh::default();
    m.colored_vertex(bar.left_top(), Color32::from_rgba_unmultiplied(p.accent.r(), p.accent.g(), p.accent.b(), 0));
    m.colored_vertex(bar.right_top(), Color32::from_rgba_unmultiplied(p.accent.r(), p.accent.g(), p.accent.b(), 0));
    m.colored_vertex(bar.right_bottom(), Color32::from_rgba_unmultiplied(p.accent.r(), p.accent.g(), p.accent.b(), 90));
    m.colored_vertex(bar.left_bottom(), Color32::from_rgba_unmultiplied(p.accent.r(), p.accent.g(), p.accent.b(), 90));
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(m));
}

pub fn panel_frame(p: &Pal, rounding: f32, pad: f32) -> egui::Frame {
    egui::Frame::none()
        .fill(p.panel)
        .rounding(egui::Rounding::same(rounding))
        .stroke(Stroke::new(1.0, p.line))
        .inner_margin(egui::Margin::same(pad))
}

pub fn ghost(ui: &mut Ui, p: &Pal, text: &str) -> Response {
    let btn = egui::Button::new(egui::RichText::new(text).color(p.text).size(13.5))
        .fill(p.panel_soft)
        .rounding(egui::Rounding::same(10.0))
        .stroke(Stroke::new(1.0, p.line));
    ui.add(btn)
}

pub fn accent(ui: &mut Ui, p: &Pal, text: &str) -> Response {
    let btn = egui::Button::new(
        egui::RichText::new(text)
            .color(Color32::from_rgb(24, 16, 12))
            .size(13.5)
            .strong(),
    )
    .fill(p.accent)
    .rounding(egui::Rounding::same(10.0))
    .stroke(Stroke::NONE);
    ui.add(btn)
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
}

pub fn round_icon_button(ui: &mut Ui, p: &Pal, icon: Icon, caption: &str, color: Color32) -> Response {
    let r = 26.0;
    let caption_h = 16.0;
    let size = Vec2::new(r * 2.0 + 18.0, r * 2.0 + caption_h);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let hover = resp.hovered();
    let painter = ui.painter();
    let c = Pos2::new(rect.center().x, rect.top() + r + 2.0);
    let fill = if hover {
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 46)
    } else {
        p.panel_soft
    };
    painter.circle_filled(c, r, fill);
    painter.circle_stroke(
        c,
        r,
        Stroke::new(1.0, if hover { color.gamma_multiply(0.9) } else { p.line }),
    );
    draw_icon(painter, c, r * 0.62, icon, color, 2.0);
    painter.text(
        Pos2::new(c.x, rect.bottom() - 6.0),
        egui::Align2::CENTER_CENTER,
        caption,
        FontId::proportional(11.0),
        if hover { p.text } else { p.muted },
    );
    resp
}

pub fn draw_icon(
    painter: &egui::Painter,
    c: Pos2,
    s: f32,
    icon: Icon,
    color: Color32,
    w: f32,
) {
    let stroke = Stroke::new(w, color);
    match icon {
        Icon::Reject | Icon::X => {
            painter.line_segment([c + Vec2::new(-s, -s) * 0.8, c + Vec2::new(s, s) * 0.8], stroke);
            painter.line_segment([c + Vec2::new(s, -s) * 0.8, c + Vec2::new(-s, s) * 0.8], stroke);
        }
        Icon::Later => {
            painter.circle_stroke(c, s * 0.9, stroke);
            painter.line_segment([c, c + Vec2::new(0.0, -s * 0.55)], stroke);
            painter.line_segment([c, c + Vec2::new(s * 0.42, s * 0.1)], stroke);
        }
        Icon::Skip => {
            painter.line_segment([c + Vec2::new(-s * 0.7, -s * 0.6), c + Vec2::new(0.0, 0.0)], stroke);
            painter.line_segment([c + Vec2::new(s * 0.7, -s * 0.6), c + Vec2::new(0.0, 0.0)], stroke);
            painter.line_segment([c + Vec2::new(0.0, -s * 0.6), c + Vec2::new(0.0, s * 0.6)], stroke);
        }
        Icon::Keep => {
            painter.line_segment([c + Vec2::new(-s * 0.85, 0.0), c + Vec2::new(-s * 0.15, s * 0.7)], stroke);
            painter.line_segment([c + Vec2::new(-s * 0.15, s * 0.7), c + Vec2::new(s * 0.85, -s * 0.7)], stroke);
        }
        Icon::Final => {
            let mut pts = Vec::new();
            for i in 0..10 {
                let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                let rr = if i % 2 == 0 { s } else { s * 0.45 };
                pts.push(c + Vec2::new(a.cos() * rr, a.sin() * rr));
            }
            painter.add(egui::Shape::closed_line(pts, stroke));
        }
        Icon::Compare => {
            painter.line_segment([c + Vec2::new(-s * 0.9, -s * 0.35), c + Vec2::new(s * 0.5, -s * 0.35)], stroke);
            painter.line_segment([c + Vec2::new(s * 0.1, -s * 0.7), c + Vec2::new(s * 0.5, -s * 0.35)], stroke);
            painter.line_segment([c + Vec2::new(s * 0.9, s * 0.35), c + Vec2::new(-s * 0.5, s * 0.35)], stroke);
            painter.line_segment([c + Vec2::new(-s * 0.1, s * 0.7), c + Vec2::new(-s * 0.5, s * 0.35)], stroke);
        }
        Icon::Undo => {
            painter.circle_stroke(c + Vec2::new(0.0, s * 0.1), s * 0.75, stroke);
            painter.line_segment([c + Vec2::new(-s * 0.75, -s * 0.5), c + Vec2::new(-s * 0.75, s * 0.1)], stroke);
            painter.line_segment([c + Vec2::new(-s * 0.75, -s * 0.5), c + Vec2::new(-s * 0.1, -s * 0.5)], stroke);
            painter.rect_filled(
                Rect::from_min_max(c + Vec2::new(-s, -s * 0.9), c + Vec2::new(s * 0.9, s * 0.9)),
                0.0,
                Color32::TRANSPARENT,
            );
        }
        Icon::Grid => {
            let u = s * 0.75;
            for (dx, dy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                painter.rect_stroke(
                    Rect::from_center_size(c + Vec2::new(dx * u * 0.62, dy * u * 0.62), Vec2::splat(u * 0.9)),
                    egui::Rounding::same(1.0),
                    stroke,
                );
            }
        }
        Icon::Swipe => {
            painter.rect_stroke(
                Rect::from_center_size(c, Vec2::new(s * 1.8, s * 1.35)),
                egui::Rounding::same(2.0),
                stroke,
            );
            painter.line_segment([c + Vec2::new(-s * 0.5, 0.0), c + Vec2::new(s * 0.5, 0.0)], stroke);
        }
        Icon::Gear => {
            let n = 8;
            for i in 0..n {
                let a = i as f32 * std::f32::consts::TAU / n as f32;
                painter.circle_filled(c + Vec2::new(a.cos(), a.sin()) * (s * 0.85), s * 0.24, color);
            }
            painter.circle_stroke(c, s * 0.72, Stroke::new(w, color));
            painter.circle_filled(c, s * 0.24, color);
        }
        Icon::Folder => {
            painter.rect_stroke(
                Rect::from_center_size(c + Vec2::new(0.0, s * 0.1), Vec2::new(s * 1.8, s * 1.2)),
                egui::Rounding::same(2.0),
                stroke,
            );
            painter.line_segment([c + Vec2::new(-s * 0.9, -s * 0.5), c + Vec2::new(-s * 0.2, -s * 0.5)], stroke);
        }
    }
}

pub fn chip(ui: &mut Ui, p: &Pal, text: &str, color: Color32, selected: bool) -> Response {
    let t = egui::RichText::new(text)
        .size(12.5)
        .color(if selected { color } else { p.text.gamma_multiply(0.72) });
    let btn = egui::Button::new(t)
        .fill(if selected {
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 34)
        } else {
            Color32::TRANSPARENT
        })
        .rounding(egui::Rounding::same(9.0))
        .stroke(Stroke::new(1.0, if selected { color.gamma_multiply(0.6) } else { p.line }));
    ui.add(btn)
}
