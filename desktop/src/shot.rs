//! Small software renderer for egui frames. Used by the test suite to capture
//! screenshots of the real interface without a window or a GPU.
#![allow(dead_code)]

use egui::{ClippedPrimitive, Color32, Context, TextureId};
use std::collections::HashMap;
use std::path::Path;

pub struct TexStore {
    map: HashMap<TextureId, egui::ColorImage>,
}

impl TexStore {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn apply(&mut self, delta: &egui::TexturesDelta) {
        for (id, d) in &delta.set {
            let img = match &d.image {
                egui::ImageData::Color(c) => c.as_ref().clone(),
                egui::ImageData::Font(f) => {
                    let mut ci = egui::ColorImage {
                        size: [f.size[0], f.size[1]],
                        pixels: Vec::with_capacity(f.size[0] * f.size[1]),
                    };
                    for px in f.srgba_pixels(None) {
                        ci.pixels.push(px);
                    }
                    ci
                }
            };
            match d.pos {
                None => {
                    self.map.insert(*id, img);
                }
                Some([ox, oy]) => {
                    if let Some(dst) = self.map.get_mut(id) {
                        for y in 0..img.size[1] {
                            for x in 0..img.size[0] {
                                let dx = ox + x;
                                let dy = oy + y;
                                if dx < dst.size[0] && dy < dst.size[1] {
                                    dst.pixels[dy * dst.size[0] + dx] =
                                        img.pixels[y * img.size[0] + x];
                                }
                            }
                        }
                    } else {
                        self.map.insert(*id, img);
                    }
                }
            }
        }
        for id in &delta.free {
            self.map.remove(id);
        }
    }
}

fn sample(tex: &egui::ColorImage, u: f32, v: f32) -> Color32 {
    if tex.size[0] == 0 || tex.size[1] == 0 {
        return Color32::WHITE;
    }
    let x = (u * tex.size[0] as f32).clamp(0.0, tex.size[0] as f32 - 1.0);
    let y = (v * tex.size[1] as f32).clamp(0.0, tex.size[1] as f32 - 1.0);
    // bilinear
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(tex.size[0] - 1);
    let y1 = (y0 + 1).min(tex.size[1] - 1);
    let fx = x - x0 as f32;
    let fy = y - y0 as f32;
    let p = |xx: usize, yy: usize| tex.pixels[yy * tex.size[0] + xx];
    let mix = |a: Color32, b: Color32, t: f32| {
        Color32::from_rgba_premultiplied(
            (a.r() as f32 * (1.0 - t) + b.r() as f32 * t) as u8,
            (a.g() as f32 * (1.0 - t) + b.g() as f32 * t) as u8,
            (a.b() as f32 * (1.0 - t) + b.b() as f32 * t) as u8,
            (a.a() as f32 * (1.0 - t) + b.a() as f32 * t) as u8,
        )
    };
    let top = mix(p(x0, y0), p(x1, y0), fx);
    let bot = mix(p(x0, y1), p(x1, y1), fx);
    mix(top, bot, fy)
}

/// Rasterize the frame into an RGB buffer.
pub fn render(
    prims: &[ClippedPrimitive],
    tex: &TexStore,
    w: usize,
    h: usize,
    ppp: f32,
) -> Vec<u8> {
    let mut buf = vec![0u8; w * h * 3];
    // start on the darkest background colour so nothing is transparent
    for px in buf.chunks_mut(3) {
        px[0] = 9;
        px[1] = 12;
        px[2] = 16;
    }
    for prim in prims {
        let egui::epaint::Primitive::Mesh(mesh) = &prim.primitive else {
            continue;
        };
        let Some(texture) = tex.map.get(&mesh.texture_id) else {
            continue;
        };
        let clip = prim.clip_rect;
        let cx0 = (clip.min.x * ppp).floor().max(0.0) as i32;
        let cy0 = (clip.min.y * ppp).floor().max(0.0) as i32;
        let cx1 = (clip.max.x * ppp).ceil().min(w as f32) as i32;
        let cy1 = (clip.max.y * ppp).ceil().min(h as f32) as i32;
        for tri in mesh.indices.chunks(3) {
            if tri.len() < 3 {
                continue;
            }
            let v: Vec<_> = tri.iter().map(|i| mesh.vertices[*i as usize]).collect();
            let xs = [
                v[0].pos.x * ppp,
                v[1].pos.x * ppp,
                v[2].pos.x * ppp,
            ];
            let ys = [
                v[0].pos.y * ppp,
                v[1].pos.y * ppp,
                v[2].pos.y * ppp,
            ];
            let minx = xs.iter().cloned().fold(f32::INFINITY, f32::min).floor().max(cx0 as f32) as i32;
            let maxx = xs.iter().cloned().fold(f32::NEG_INFINITY, f32::max).ceil().min(cx1 as f32) as i32;
            let miny = ys.iter().cloned().fold(f32::INFINITY, f32::min).floor().max(cy0 as f32) as i32;
            let maxy = ys.iter().cloned().fold(f32::NEG_INFINITY, f32::max).ceil().min(cy1 as f32) as i32;
            let area = (xs[1] - xs[0]) * (ys[2] - ys[0]) - (xs[2] - xs[0]) * (ys[1] - ys[0]);
            if area.abs() < 1e-6 {
                continue;
            }
            for py in miny..maxy {
                for px in minx..maxx {
                    let fx = px as f32 + 0.5;
                    let fy = py as f32 + 0.5;
                    let w0 = ((xs[1] - fx) * (ys[2] - fy) - (xs[2] - fx) * (ys[1] - fy)) / area;
                    let w1 = ((xs[2] - fx) * (ys[0] - fy) - (xs[0] - fx) * (ys[2] - fy)) / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < -0.012 || w1 < -0.012 || w2 < -0.012 {
                        continue;
                    }
                    let u = w0 * v[0].uv.x + w1 * v[1].uv.x + w2 * v[2].uv.x;
                    let vv = w0 * v[0].uv.y + w1 * v[1].uv.y + w2 * v[2].uv.y;
                    let t = sample(texture, u, vv);
                    let cr = w0 * v[0].color.r() as f32
                        + w1 * v[1].color.r() as f32
                        + w2 * v[2].color.r() as f32;
                    let cg = w0 * v[0].color.g() as f32
                        + w1 * v[1].color.g() as f32
                        + w2 * v[2].color.g() as f32;
                    let cb = w0 * v[0].color.b() as f32
                        + w1 * v[1].color.b() as f32
                        + w2 * v[2].color.b() as f32;
                    let ca = w0 * v[0].color.a() as f32
                        + w1 * v[1].color.a() as f32
                        + w2 * v[2].color.a() as f32;
                    // premultiplied source over destination
                    let sr = cr * t.r() as f32 / 255.0;
                    let sg = cg * t.g() as f32 / 255.0;
                    let sb = cb * t.b() as f32 / 255.0;
                    let sa = ca * t.a() as f32 / 255.0 / 255.0;
                    let o = (py as usize * w + px as usize) * 3;
                    for (k, s) in [sr, sg, sb].iter().enumerate() {
                        let dst = buf[o + k] as f32;
                        let out = s + dst * (1.0 - sa);
                        buf[o + k] = out.clamp(0.0, 255.0) as u8;
                    }
                }
            }
        }
    }
    buf
}

pub fn save_rgb(buf: &[u8], w: u32, h: u32, path: &Path) {
    let img = image::RgbImage::from_raw(w, h, buf.to_vec()).expect("buffer size");
    img.save(path).expect("write png");
}

/// Keeps the texture atlas alive across frames so user textures survive.
pub struct Shot {
    pub store: TexStore,
    pub prims: Vec<ClippedPrimitive>,
    w: u32,
    h: u32,
    ppp: f32,
}

impl Shot {
    pub fn new(w: u32, h: u32, ppp: f32) -> Self {
        Self {
            store: TexStore::new(),
            prims: Vec::new(),
            w,
            h,
            ppp,
        }
    }

    pub fn pump(&mut self, app: &mut crate::app::App, ctx: &Context, frames: usize, sleep_ms: u64) {
        self.pump_events(app, ctx, frames, sleep_ms, Vec::new());
    }

    pub fn pump_events(
        &mut self,
        app: &mut crate::app::App,
        ctx: &Context,
        frames: usize,
        sleep_ms: u64,
        events: Vec<egui::Event>,
    ) {
        ctx.set_pixels_per_point(self.ppp);
        for f in 0..frames {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(self.w as f32 / self.ppp, self.h as f32 / self.ppp),
                )),
                events: if f == 0 { events.clone() } else { Vec::new() },
                time: Some(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64()),
                ..Default::default()
            };
            let full = ctx.run(input, |c| app.frame(c));
            self.store.apply(&full.textures_delta);
            self.prims = ctx.tessellate(full.shapes, full.pixels_per_point);
            if sleep_ms > 0 {
                std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
            }
        }
    }

    pub fn save(&self, path: &Path) {
        let buf = render(&self.prims, &self.store, self.w as usize, self.h as usize, self.ppp);
        save_rgb(&buf, self.w, self.h, path);
    }
}
