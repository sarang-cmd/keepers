use crate::decode::{self, Frame};
use crate::model::*;
use crate::theme::{self, Icon, Pal};
use crate::thumbs::{Msg, Thumbs};
use egui::{Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Swipe,
    Grid,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    Photos,
    Videos,
}

struct Token {
    gen: u64,
    idx: usize,
    res: Result<Frame, String>,
}

pub struct App {
    db: Db,
    folder: Option<PathBuf>,
    photos: Vec<Photo>,
    dims: Vec<Option<(u32, u32)>>,
    assign: Vec<String>,
    custom: Vec<String>,
    coll: String,
    filter: Filter,
    search: String,
    view: View,

    thumbs: Thumbs,
    tex: HashMap<usize, egui::TextureHandle>,
    tex_order: VecDeque<usize>,
    fail: HashMap<usize, String>,

    pv_gen: u64,
    pv_idx: Option<usize>,
    pv_tex: Option<egui::TextureHandle>,
    pv_tx: crossbeam_channel::Sender<Token>,
    pv_rx: crossbeam_channel::Receiver<Token>,
    pv_cache: HashMap<(usize, u32), egui::TextureHandle>,
    pv_order: VecDeque<(usize, u32)>,

    compare: Option<[Option<String>; 2]>,
    sel: Vec<usize>,
    undo: Vec<Vec<(usize, String)>>,

    settings_open: bool,
    new_coll: String,
    toast: Option<(String, f64)>,
    started_cache: bool,

    cache_bytes: u64,
    cache_files: u64,
    help_open: bool,
    path_input: String,
    show_path_input: bool,
}

fn dark_now(db: &Db) -> bool {
    match db.settings.theme.as_str() {
        "light" => false,
        "dark" => true,
        "auto" => {
            // follow the OS where the platform reports it, dark otherwise
            true
        }
        _ => true,
    }
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self::bare();
        app.init(&cc.egui_ctx);
        if let Some(first) = std::env::args().nth(1) {
            let p = PathBuf::from(first);
            if p.is_dir() {
                app.open_folder(&cc.egui_ctx, p);
            }
        }
        app
    }

    pub fn bare() -> Self {
        let db = Db::load();
        let (pv_tx, pv_rx) = crossbeam_channel::unbounded();
        let (cache_bytes, cache_files) = decode::cache_bytes();
        Self {
            db,
            folder: None,
            photos: Vec::new(),
            dims: Vec::new(),
            assign: Vec::new(),
            custom: Vec::new(),
            coll: INBOX.into(),
            filter: Filter::All,
            search: String::new(),
            view: View::Swipe,
            thumbs: Thumbs::empty(),
            tex: HashMap::new(),
            tex_order: VecDeque::new(),
            fail: HashMap::new(),
            pv_gen: 0,
            pv_idx: None,
            pv_tex: None,
            pv_tx,
            pv_rx,
            pv_cache: HashMap::new(),
            pv_order: VecDeque::new(),
            compare: None,
            sel: Vec::new(),
            undo: Vec::new(),
            settings_open: false,
            new_coll: String::new(),
            toast: None,
            started_cache: false,
            cache_bytes,
            cache_files,
            help_open: false,
            path_input: String::new(),
            show_path_input: false,
        }
    }

    pub fn init(&mut self, ctx: &egui::Context) {
        apply_visuals(ctx, dark_now(&self.db));
    }

    fn dark(&self) -> bool {
        dark_now(&self.db)
    }

    fn pal(&self) -> Pal {
        theme::pal(self.dark())
    }

    fn toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), 2.6));
    }

    fn open_folder(&mut self, ctx: &egui::Context, dir: PathBuf) {
        let photos = scan(&dir);
        if photos.is_empty() {
            self.toast("No photos in that folder");
            return;
        }
        let fm = self
            .db
            .folders
            .get(&folder_key(&dir))
            .cloned()
            .unwrap_or_default();
        self.assign = photos
            .iter()
            .map(|p| fm.assign.get(&p.name).cloned().unwrap_or_else(|| INBOX.into()))
            .collect();
        self.custom = fm.custom.clone();
        self.photos = photos;
        self.dims = vec![None; self.photos.len()];
        self.tex.clear();
        self.tex_order.clear();
        self.fail.clear();
        self.sel.clear();
        self.compare = None;
        self.undo.clear();
        self.pv_tex = None;
        self.pv_idx = None;
        self.folder = Some(dir);
        self.coll = INBOX.into();
        self.view = View::Swipe;
        self.start_cache(ctx);
    }

    fn open_files(&mut self, ctx: &egui::Context, files: Vec<PathBuf>) {
        if let Some(first) = files.first() {
            if let Some(dir) = first.parent() {
                self.open_folder(ctx, dir.to_path_buf());
            }
        }
    }

    fn start_cache(&mut self, _ctx: &egui::Context) {
        self.thumbs.stop();
        let paths: Vec<PathBuf> = self
            .photos
            .iter()
            .map(|p| p.path.clone())
            .collect();
        let workers = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .min(8)
            .max(2);
        self.thumbs = Thumbs::start(paths, workers);
        self.started_cache = true;
    }

    fn photos_in(&self, coll: &str) -> Vec<usize> {
        let q = self.search.to_lowercase();
        self.photos
            .iter()
            .enumerate()
            .filter(|(i, p)| {
                if self.assign[*i] != coll {
                    return false;
                }
                match self.filter {
                    Filter::Photos => p.kind == Kind::Photo,
                    Filter::Videos => p.kind == Kind::Video,
                    Filter::All => true,
                }
            })
            .filter(|(_, p)| q.is_empty() || p.name.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect()
    }

    fn counts(&self, coll: &str) -> usize {
        self.assign.iter().filter(|c| *c == coll).count()
    }

    fn sorted(&self) -> usize {
        self.assign.iter().filter(|c| *c != INBOX).count()
    }

    fn collections(&self) -> Vec<String> {
        let mut v: Vec<String> = DEFAULTS.iter().map(|s| s.to_string()).collect();
        for c in &self.custom {
            if !v.contains(c) {
                v.push(c.clone());
            }
        }
        v
    }

    fn move_to(&mut self, idx: usize, coll: &str) {
        let prev = self.assign[idx].clone();
        if prev == coll {
            return;
        }
        self.assign[idx] = coll.to_string();
        self.undo.push(vec![(idx, prev)]);
        self.persist();
    }

    fn move_many(&mut self, moves: Vec<(usize, String)>) {
        if moves.is_empty() {
            return;
        }
        let mut rec = Vec::new();
        for (i, c) in moves {
            rec.push((i, self.assign[i].clone()));
            self.assign[i] = c;
        }
        self.undo.push(rec);
        self.persist();
    }

    fn undo(&mut self) {
        if let Some(rec) = self.undo.pop() {
            for (i, c) in rec {
                self.assign[i] = c;
            }
            self.persist();
            self.toast("Undone");
        }
    }

    fn persist(&mut self) {
        if let Some(dir) = &self.folder {
            let fm = FolderMap {
                assign: self
                    .photos
                    .iter()
                    .enumerate()
                    .map(|(i, p)| (p.name.clone(), self.assign[i].clone()))
                    .collect(),
                custom: self.custom.clone(),
            };
            self.db.folders.insert(folder_key(dir), fm);
        }
        self.db.save();
    }

    fn ensure_texture(&mut self, ctx: &egui::Context, idx: usize) {
        if self.tex.contains_key(&idx) {
            if let Some(pos) = self.tex_order.iter().position(|i| *i == idx) {
                self.tex_order.remove(pos);
            }
            self.tex_order.push_back(idx);
            return;
        }
        if let Some(f) = self.dims.get(idx).copied().flatten() {
            let _ = f;
        }
        // texture arrives from the thumbnail channel, see drain()
        let _ = ctx;
    }

    fn drain_thumbs(&mut self, ctx: &egui::Context) {
        let mut done = Vec::new();
        for _ in 0..48 {
            match self.thumbs.rx.try_recv() {
                Ok(Msg::Done(idx, res)) => done.push((idx, res)),
                Ok(Msg::Finished) => continue,
                Err(_) => break,
            }
        }
        for (idx, res) in done {
            self.thumbs.done += 1;
            match res {
                Ok(frame) => {
                    self.thumbs.ok += 1;
                    self.dims[idx] = Some((frame.w, frame.h));
                    let img = egui::ColorImage::from_rgba_unmultiplied(
                        [frame.w as usize, frame.h as usize],
                        &frame.rgba,
                    );
                    let t = ctx.load_texture(
                        format!("th{idx}"),
                        img,
                        egui::TextureOptions::LINEAR,
                    );
                    self.tex.insert(idx, t);
                    self.tex_order.push_back(idx);
                    while self.tex_order.len() > 260 {
                        if let Some(old) = self.tex_order.pop_front() {
                            self.tex.remove(&old);
                        }
                    }
                }
                Err(e) => {
                    self.thumbs.fail += 1;
                    self.fail.insert(idx, e);
                }
            }
        }
        if self.thumbs.done >= self.thumbs.total && self.thumbs.ms.is_none() {
            self.thumbs.ms = Some(self.thumbs.started.elapsed().as_millis());
            if self.thumbs.total > 0 {
                self.toast(format!(
                    "Cached {} of {} photos for offline use",
                    self.thumbs.ok, self.thumbs.total
                ));
            }
        }
    }

    fn request_preview(&mut self, ctx: &egui::Context, idx: usize) {
        let max = self.db.settings.preview;
        if self.pv_idx == Some(idx) && self.pv_tex.is_some() {
            return;
        }
        if let Some(t) = self.pv_cache.get(&(idx, max)) {
            self.pv_tex = Some(t.clone());
            self.pv_idx = Some(idx);
            return;
        }
        self.pv_gen += 1;
        let gen = self.pv_gen;
        self.pv_idx = Some(idx);
        self.pv_tex = None;
        let path = self.photos[idx].path.clone();
        let tx = self.pv_tx.clone();
        std::thread::spawn(move || {
            let res = decode::decode(&path, max);
            let _ = tx.send(Token { gen, idx, res });
        });
        ctx.request_repaint();
    }

    fn drain_preview(&mut self, ctx: &egui::Context) {
        let mut got = None;
        while let Ok(t) = self.pv_rx.try_recv() {
            if t.gen == self.pv_gen {
                got = Some(t);
            }
        }
        if let Some(t) = got {
            match t.res {
                Ok(frame) => {
                    let img = egui::ColorImage::from_rgba_unmultiplied(
                        [frame.w as usize, frame.h as usize],
                        &frame.rgba,
                    );
                    let tex = ctx.load_texture("preview", img, egui::TextureOptions::LINEAR);
                    self.pv_tex = Some(tex.clone());
                    let key = (t.idx, self.db.settings.preview);
                    self.pv_cache.insert(key, tex);
                    self.pv_order.push_back(key);
                    while self.pv_order.len() > 5 {
                        if let Some(old) = self.pv_order.pop_front() {
                            self.pv_cache.remove(&old);
                        }
                    }
                }
                Err(e) => {
                    self.fail.insert(t.idx, e);
                }
            }
        }
    }

    fn thumb_rect_image(ui: &mut egui::Ui, rect: Rect, tex: &egui::TextureHandle, rounding: f32) {
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        let painter = ui.painter();
        let clip = painter.clip_rect();
        painter.with_clip_rect(clip.intersect(rect)).image(
            tex.id(),
            fit_rect(rect, tex.size_vec2()),
            uv,
            Color32::WHITE,
        );
        let _ = rounding;
    }
}

fn fit_rect(outer: Rect, size: Vec2) -> Rect {
    if size.x <= 0.0 || size.y <= 0.0 {
        return outer;
    }
    let s = (outer.width() / size.x).min(outer.height() / size.y);
    Rect::from_center_size(outer.center(), Vec2::new(size.x * s, size.y * s))
}

fn apply_visuals(ctx: &egui::Context, dark: bool) {
    let p = theme::pal(dark);
    let mut v = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.panel_fill = Color32::TRANSPARENT;
    v.window_fill = p.panel;
    v.faint_bg_color = p.panel_soft;
    v.extreme_bg_color = if dark {
        Color32::from_rgb(16, 20, 26)
    } else {
        Color32::from_rgb(238, 240, 246)
    };
    v.window_stroke = Stroke::new(1.0, p.line);
    v.override_text_color = Some(p.text);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.line);
    v.widgets.inactive.bg_fill = p.panel_soft;
    v.widgets.inactive.weak_bg_fill = p.panel_soft;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.muted);
    v.widgets.inactive.rounding = egui::Rounding::same(9.0);
    v.widgets.hovered.bg_fill = p.panel_soft.gamma_multiply(2.0);
    v.widgets.hovered.weak_bg_fill = p.panel_soft.gamma_multiply(2.0);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, p.text);
    v.widgets.hovered.rounding = egui::Rounding::same(9.0);
    v.widgets.active.bg_fill = p.accent;
    v.widgets.active.weak_bg_fill = p.accent;
    v.widgets.active.rounding = egui::Rounding::same(9.0);
    v.selection.bg_fill = p.accent.gamma_multiply(0.45);
    v.selection.stroke = Stroke::new(1.0, p.text);
    v.window_rounding = egui::Rounding::same(18.0);
    ctx.set_visuals(v);
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame(ctx);
    }
}

impl App {
    pub fn frame(&mut self, ctx: &egui::Context) {
        let p = self.pal();
        theme::gradient_bg(ctx, &p);
        self.drain_thumbs(ctx);
        self.drain_preview(ctx);

        if self.thumbs.running() {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }
        if self.toast.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(60));
        }

        self.handle_input(ctx);
        self.handle_drop(ctx);

        egui::SidePanel::left("side")
            .exact_width(286.0)
            .resizable(false)
            .frame(egui::Frame::none().inner_margin(egui::Margin::same(0.0)))
            .show(ctx, |ui| self.sidebar(ui, &p));

        egui::CentralPanel::default()
            .frame(egui::Frame::none().inner_margin(egui::Margin::same(0.0)))
            .show(ctx, |ui| self.main_panel(ui, &p));

        self.settings_window(ctx, &p);
        self.help_window(ctx, &p);
        self.toast_overlay(ctx, &p);
    }
}

impl App {
    fn sidebar(&mut self, ui: &mut egui::Ui, p: &Pal) {
        let w = ui.available_width();
        let rect = ui.max_rect();
        ui.painter().rect_filled(
            Rect::from_min_size(rect.min, Vec2::new(w, rect.height())),
            egui::Rounding::ZERO,
            p.panel,
        );
        ui.painter().line_segment(
            [Pos2::new(rect.right(), rect.top()), Pos2::new(rect.right(), rect.bottom())],
            Stroke::new(1.0, p.line),
        );
        let pad = 20.0;
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            ui.add_space(pad);
            brand(ui, p);
        });
        ui.add_space(20.0);

        // sorted progress
        let total = self.photos.len();
        let done = self.sorted();
        ui.horizontal(|ui| {
            ui.add_space(pad);
            let w = w - pad * 2.0;
            let (r, _) = ui.allocate_exact_size(Vec2::new(w, 6.0), Sense::hover());
            ui.painter().rect_filled(r, egui::Rounding::same(6.0), p.panel_soft);
            if total > 0 {
                let mut f = r;
                f.set_width((w * done as f32 / total as f32).max(3.0));
                ui.painter().rect_filled(f, egui::Rounding::same(6.0), p.accent);
            }
        });
        ui.horizontal(|ui| {
            ui.add_space(pad);
            ui.label(
                egui::RichText::new(format!("{done} of {total} sorted"))
                    .size(12.0)
                    .color(p.muted),
            );
        });

        // cache row
        if self.thumbs.total > 0 {
            ui.add_space(10.0);
            let running = self.thumbs.running();
            ui.horizontal(|ui| {
                ui.add_space(pad);
                let bw = w - pad * 2.0 - 20.0;
                let (r, _) = ui.allocate_exact_size(Vec2::new(bw, 6.0), Sense::hover());
                ui.painter().rect_filled(r, egui::Rounding::same(6.0), p.panel_soft);
                let frac = if self.thumbs.total == 0 {
                    0.0
                } else {
                    self.thumbs.done as f32 / self.thumbs.total as f32
                };
                let mut f = r;
                f.set_width((bw * frac).max(3.0));
                let c = if running {
                    Color32::from_rgb(110, 145, 255)
                } else {
                    p.green
                };
                ui.painter().rect_filled(f, egui::Rounding::same(6.0), c);
            });
            ui.horizontal(|ui| {
                ui.add_space(pad);
                let label = if running {
                    format!(
                        "Caching {} of {} . {:.1}/s",
                        self.thumbs.done.min(self.thumbs.total),
                        self.thumbs.total,
                        self.thumbs.rate()
                    )
                } else {
                    format!(
                        "Cached {} tiles in {:.1}s",
                        self.thumbs.ok,
                        self.thumbs.seconds()
                    )
                };
                ui.label(egui::RichText::new(label).size(11.0).color(p.muted));
                if running {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(pad);
                        if theme::chip(ui, p, "Stop", p.red, false).clicked() {
                            self.thumbs.stop();
                        }
                    });
                }
            });
        }

        ui.add_space(14.0);
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .max_height(ui.available_height() - 150.0)
            .show(ui, |ui| {
                let colls = self.collections();
                for name in colls {
                    let n = self.counts(&name);
                    let selected = self.coll == name;
                    ui.horizontal(|ui| {
                        ui.add_space(10.0);
                        let w = w - 20.0;
                        let (rect, resp) =
                            ui.allocate_exact_size(Vec2::new(w, 36.0), Sense::click());
                        if selected {
                            ui.painter().rect_filled(
                                rect,
                                egui::Rounding::same(11.0),
                                Color32::from_rgba_unmultiplied(255, 255, 255, if p.dark { 14 } else { 90 }),
                            );
                            let bar = Rect::from_min_size(
                                Pos2::new(rect.left(), rect.top() + 8.0),
                                Vec2::new(3.0, rect.height() - 16.0),
                            );
                            ui.painter().rect_filled(
                                bar,
                                egui::Rounding::same(3.0),
                                coll_color(&name, p.dark),
                            );
                        } else if resp.hovered() {
                            ui.painter().rect_filled(
                                rect,
                                egui::Rounding::same(11.0),
                                p.panel_soft,
                            );
                        }
                        let dot = Pos2::new(rect.left() + 18.0, rect.center().y);
                        ui.painter().circle_filled(dot, 5.0, coll_color(&name, p.dark));
                        ui.painter().text(
                            Pos2::new(rect.left() + 34.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            &name,
                            FontId::proportional(14.0),
                            if selected { p.text } else { p.muted },
                        );
                        ui.painter().text(
                            Pos2::new(rect.right() - 14.0, rect.center().y),
                            egui::Align2::RIGHT_CENTER,
                            n.to_string(),
                            FontId::proportional(12.5),
                            p.muted,
                        );
                        if resp.clicked() {
                            self.coll = name.clone();
                            self.sel.clear();
                            self.compare = None;
                            self.pv_idx = None;
                        }
                    });
                    ui.add_space(2.0);
                }
            });

        // footer
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(pad);
            let bw = (w - pad * 2.0 - 8.0) / 2.0;
            let btn = egui::Button::new(
                egui::RichText::new("Open folder")
                    .size(13.0)
                    .color(p.text),
            )
            .fill(p.panel_soft)
            .rounding(egui::Rounding::same(10.0))
            .stroke(Stroke::new(1.0, p.line))
            .min_size(Vec2::new(bw, 34.0));
            if ui.add(btn).clicked() {
                self.pick_folder(ctx_of(ui));
            }
            let btn2 = egui::Button::new(
                egui::RichText::new("Settings").size(13.0).color(p.text),
            )
            .fill(p.panel_soft)
            .rounding(egui::Rounding::same(10.0))
            .stroke(Stroke::new(1.0, p.line))
            .min_size(Vec2::new(bw, 34.0));
            if ui.add(btn2).clicked() {
                self.settings_open = true;
            }
        });
        ui.add_space(14.0);
    }

    fn main_panel(&mut self, ui: &mut egui::Ui, p: &Pal) {
        if self.photos.is_empty() {
            self.empty_state(ui, p);
            return;
        }
        self.header(ui, p);
        ui.add_space(10.0);
        let list = self.photos_in(&self.coll.clone());
        match self.view {
            View::Swipe => self.swipe_view(ui, p, &list),
            View::Grid => self.grid_view(ui, p, &list),
        }
        if self.compare.is_some() {
            self.compare_view(ui, p);
        }
    }

    fn empty_state(&mut self, ui: &mut egui::Ui, p: &Pal) {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.22);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(520.0, 300.0), Sense::hover());
            ui.painter().rect(
                rect,
                egui::Rounding::same(22.0),
                p.panel,
                Stroke::new(1.0, p.line),
            );
            ui.painter().text(
                Pos2::new(rect.center().x, rect.top() + 62.0),
                egui::Align2::CENTER_CENTER,
                "Keepers",
                FontId::proportional(44.0),
                p.accent,
            );
            ui.painter().text(
                Pos2::new(rect.center().x, rect.top() + 112.0),
                egui::Align2::CENTER_CENTER,
                "Sort a whole trip into collections. Files stay on your disk.",
                FontId::proportional(14.5),
                p.muted,
            );
            ui.painter().text(
                Pos2::new(rect.center().x, rect.top() + 134.0),
                egui::Align2::CENTER_CENTER,
                "HEIC, JPG, PNG and WebP paint straight from the system decoder.",
                FontId::proportional(14.5),
                p.muted,
            );
            let b1 = Rect::from_center_size(
                Pos2::new(rect.center().x - 78.0, rect.top() + 196.0),
                Vec2::new(140.0, 42.0),
            );
            let r1 = ui.interact(b1, ui.id().with("open1"), Sense::click());
            ui.painter()
                .rect_filled(b1, egui::Rounding::same(12.0), p.accent);
            ui.painter().text(
                b1.center(),
                egui::Align2::CENTER_CENTER,
                "Choose folder",
                FontId::proportional(14.5),
                Color32::from_rgb(24, 16, 12),
            );
            if r1.clicked() {
                self.pick_folder(ctx_of(ui));
            }
            let b2 = Rect::from_center_size(
                Pos2::new(rect.center().x + 78.0, rect.top() + 196.0),
                Vec2::new(140.0, 42.0),
            );
            let r2 = ui.interact(b2, ui.id().with("open2"), Sense::click());
            ui.painter().rect(
                b2,
                egui::Rounding::same(12.0),
                p.panel_soft,
                Stroke::new(1.0, p.line),
            );
            ui.painter().text(
                b2.center(),
                egui::Align2::CENTER_CENTER,
                "Choose files",
                FontId::proportional(14.5),
                p.text,
            );
            if r2.clicked() {
                self.pick_files(ctx_of(ui));
            }
            ui.add_space(200.0);
            ui.label(
                egui::RichText::new("or drop a folder anywhere in this window")
                    .size(12.5)
                    .color(p.muted),
            );
        });
    }

    fn header(&mut self, ui: &mut egui::Ui, p: &Pal) {
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.add_space(22.0);
            ui.label(
                egui::RichText::new(&self.coll)
                    .size(23.0)
                    .strong()
                    .color(p.text),
            );
            let n = self.counts(&self.coll);
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(format!("{n} items"))
                    .size(12.5)
                    .color(p.muted),
            );
            ui.add_space(22.0);

            // search
            let sr = egui::TextEdit::singleline(&mut self.search)
                .hint_text("Search names")
                .desired_width(190.0);
            ui.add(sr);
            ui.add_space(8.0);

            ui.add_space(8.0);
            egui::ComboBox::from_id_source("filter")
                .selected_text(match self.filter {
                    Filter::All => "Photos and videos",
                    Filter::Photos => "Photos only",
                    Filter::Videos => "Videos only",
                })
                .width(150.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.filter, Filter::All, "Photos and videos");
                    ui.selectable_value(&mut self.filter, Filter::Photos, "Photos only");
                    ui.selectable_value(&mut self.filter, Filter::Videos, "Videos only");
                });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(22.0);
                let sw = if self.view == View::Swipe { "Grid view" } else { "Swipe view" };
                if theme::ghost(ui, p, sw).clicked() {
                    self.view = if self.view == View::Swipe {
                        View::Grid
                    } else {
                        View::Swipe
                    };
                }
                if !self.sel.is_empty() {
                    if theme::accent(ui, p, &format!("Compare {}", self.sel.len())).clicked() {
                        self.start_compare();
                    }
                }
                if theme::ghost(ui, p, "Export").clicked() {
                    self.export();
                }
            });
        });
        ui.add_space(10.0);
    }

    fn start_compare(&mut self) {
        if self.sel.len() >= 2 {
            self.compare = Some([None, None]);
            self.pv_tex = None;
            self.pv_idx = None;
        } else {
            self.toast("Pick two photos in the grid first");
        }
    }

    fn swipe_view(&mut self, ui: &mut egui::Ui, p: &Pal, list: &[usize]) {
        if list.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(120.0);
                ui.label(
                    egui::RichText::new("Nothing in this collection")
                        .size(15.0)
                        .color(p.muted),
                );
            });
            return;
        }
        let pos = self
            .pv_idx
            .and_then(|i| list.iter().position(|x| *x == i))
            .unwrap_or(0);
        let idx = list[pos.min(list.len() - 1)];
        self.request_preview(ctx_of(ui), idx);

        let avail = ui.available_size();
        let ar = self
            .dims
            .get(idx)
            .copied()
            .flatten()
            .map(|(w, h)| w as f32 / h as f32)
            .unwrap_or(1.5);
        let card_w = (avail.x - 130.0).clamp(320.0, 1180.0);
        let card_h = ((avail.y - 250.0).max(200.0)).min(card_w / ar);
        let card_w = card_h * ar;

        ui.vertical_centered(|ui| {
            let (rect, resp) =
                ui.allocate_exact_size(Vec2::new(card_w, card_h), Sense::click());
            let painter = ui.painter();
            painter.rect(
                rect.expand(6.0),
                egui::Rounding::same(20.0),
                p.panel,
                Stroke::new(1.0, p.line),
            );
            if let Some(t) = self.tex.get(&idx).cloned() {
                painter.image(
                    t.id(),
                    rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            } else if self.fail.contains_key(&idx) {
                painter.rect_filled(rect, egui::Rounding::same(2.0), p.panel_soft);
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Could not decode this file",
                    FontId::proportional(14.0),
                    p.muted,
                );
            } else {
                painter.rect_filled(rect, egui::Rounding::same(2.0), p.panel_soft);
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Decoding",
                    FontId::proportional(14.0),
                    p.muted,
                );
            }
            guides(painter, rect, &self.db.settings.guides, p);
            // filename + counter
            let name_h = 30.0;
            let strip = Rect::from_min_size(
                Pos2::new(rect.left() + 12.0, rect.bottom() - name_h - 12.0),
                Vec2::new(rect.width() - 24.0, name_h),
            );
            painter.text(
                Pos2::new(strip.left(), strip.center().y),
                egui::Align2::LEFT_CENTER,
                &self.photos[idx].name,
                FontId::proportional(12.5),
                Color32::from_rgb(240, 244, 250),
            );
            painter.text(
                Pos2::new(strip.right(), strip.center().y),
                egui::Align2::RIGHT_CENTER,
                format!("{} of {}", pos + 1, list.len()),
                FontId::proportional(12.5),
                Color32::from_rgb(240, 244, 250),
            );
            // hover reject
            if resp.hovered() {
                let x = Rect::from_center_size(
                    Pos2::new(rect.right() - 22.0, rect.top() + 22.0),
                    Vec2::splat(30.0),
                );
                let xr = ui.interact(x, ui.id().with("qx"), Sense::click());
                painter.circle_filled(
                    x.center(),
                    15.0,
                    if xr.hovered() {
                        p.red.gamma_multiply(0.9)
                    } else {
                        Color32::from_black_alpha(150)
                    },
                );
                theme::draw_icon(&painter, x.center(), 6.0, Icon::X, Color32::WHITE, 2.0);
                if xr.clicked() {
                    self.move_to(idx, REJECTED);
                    self.pv_idx = None;
                }
            }
            if resp.clicked() && self.pv_tex.is_none() {
                self.request_preview(ctx_of(ui), idx);
            }
        });

        ui.add_space(16.0);
        ui.horizontal(|ui| {
            ui.add_space(ui.available_width() / 2.0 - 250.0);
            if theme::round_icon_button(ui, p, Icon::Reject, "Reject", p.red).clicked() {
                self.move_to(idx, REJECTED);
                self.pv_idx = None;
            }
            if theme::round_icon_button(ui, p, Icon::Later, "Later", p.gold).clicked() {
                self.move_to(idx, LATER);
                self.pv_idx = None;
            }
            if theme::round_icon_button(ui, p, Icon::Skip, "Skip", p.muted).clicked() {
                self.next_in(list, idx);
            }
            if theme::round_icon_button(ui, p, Icon::Keep, "Candidate", p.green).clicked() {
                self.move_to(idx, CANDIDATES);
                self.pv_idx = None;
            }
            if theme::round_icon_button(ui, p, Icon::Final, "Final cut", p.violet).clicked() {
                self.move_to(idx, FINAL);
                self.pv_idx = None;
            }
            if theme::round_icon_button(ui, p, Icon::Compare, "Compare", p.accent).clicked() {
                self.sel = vec![idx];
                if pos + 1 < list.len() {
                    self.sel.push(list[pos + 1]);
                }
                self.start_compare();
            }
            if theme::round_icon_button(ui, p, Icon::Undo, "Undo", p.muted).clicked() {
                self.undo();
            }
        });
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(ui.available_width() / 2.0 - 250.0);
            ui.label(egui::RichText::new("Move to:").size(12.0).color(p.muted));
            for (k, name) in [(2, CANDIDATES), (3, LATER), (4, FINAL), (5, REJECTED)] {
                let _ = coll_color(name, p.dark);
                let label = format!("{k}  {name}");
                if theme::chip(ui, p, &label, coll_color(name, p.dark), false).clicked() {
                    self.move_to(idx, name);
                    self.pv_idx = None;
                }
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_space(22.0);
            ui.label(
                egui::RichText::new(
                    "left reject   right candidate   space later   up final   down skip   z undo   c compare   / search   , settings",
                )
                .size(11.0)
                .color(p.muted),
            );
        });
    }

    fn next_in(&mut self, list: &[usize], idx: usize) {
        if let Some(pos) = list.iter().position(|x| *x == idx) {
            if pos + 1 < list.len() {
                self.pv_idx = Some(list[pos + 1]);
            }
        }
        self.pv_tex = None;
    }

    fn grid_view(&mut self, ui: &mut egui::Ui, p: &Pal, list: &[usize]) {
        let cell = self.db.settings.thumb as f32;
        let gap = 10.0;
        let mut clicked: Option<usize> = None;
        let mut reject: Option<usize> = None;
        let mut toggle: Option<usize> = None;
        let mut picked: Option<usize> = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(6.0);
                let width = ui.available_width() - 30.0;
                let cols = ((width + gap) / (cell + gap)).floor().max(1.0) as usize;
                let rows = (list.len() + cols - 1) / cols;
                for r in 0..rows {
                    ui.horizontal(|ui| {
                        ui.add_space(22.0);
                        for c in 0..cols {
                            let i = r * cols + c;
                            if i >= list.len() {
                                break;
                            }
                            let idx = list[i];
                            let (rect, resp) =
                                ui.allocate_exact_size(Vec2::splat(cell), Sense::click());
                            let painter = ui.painter();
                            painter.rect_filled(rect, egui::Rounding::same(12.0), p.panel_soft);
                            let ar = self
                                .dims
                                .get(idx)
                                .copied()
                                .flatten()
                                .map(|(w, h)| w as f32 / h as f32)
                                .unwrap_or(1.0);
                            let mut inner = Rect::from_center_size(
                                rect.center(),
                                Vec2::new(rect.width() - 8.0, rect.height() - 8.0),
                            );
                            if ar > inner.width() / inner.height() {
                                inner.set_height(inner.width() / ar);
                            } else {
                                inner.set_width(inner.height() * ar);
                            }
                            inner = inner.translate(Vec2::new(
                                0.0,
                                (rect.center().y - inner.center().y),
                            ));
                            if let Some(t) = self.tex.get(&idx) {
                                painter.with_clip_rect(rect).image(
                                    t.id(),
                                    inner,
                                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                    Color32::WHITE,
                                );
                            }
                            if self.sel.contains(&idx) {
                                painter.rect_stroke(
                                    rect,
                                    egui::Rounding::same(12.0),
                                    Stroke::new(2.0, p.accent),
                                );
                            }
                            if resp.hovered() {
                                painter.rect_stroke(
                                    rect,
                                    egui::Rounding::same(12.0),
                                    Stroke::new(1.0, p.line.gamma_multiply(2.0)),
                                );
                                let x = Rect::from_center_size(
                                    Pos2::new(rect.right() - 16.0, rect.top() + 16.0),
                                    Vec2::splat(24.0),
                                );
                                let xr = ui.interact(
                                    x,
                                    ui.id().with(("gx", idx)),
                                    Sense::click(),
                                );
                                ui.painter().circle_filled(
                                    x.center(),
                                    12.0,
                                    if xr.hovered() {
                                        p.red
                                    } else {
                                        Color32::from_black_alpha(160)
                                    },
                                );
                                theme::draw_icon(
                                    ui.painter(),
                                    x.center(),
                                    5.0,
                                    Icon::X,
                                    Color32::WHITE,
                                    1.8,
                                );
                                if xr.clicked() {
                                    reject = Some(idx);
                                }
                            }
                            if resp.clicked() {
                                clicked = Some(idx);
                            }
                            if resp.secondary_clicked() {
                                toggle = Some(idx);
                            }
                            let name = self.photos[idx].name.clone();
                            if resp.hovered() {
                                ui.painter().text(
                                    Pos2::new(rect.left() + 8.0, rect.bottom() - 12.0),
                                    egui::Align2::LEFT_CENTER,
                                    name,
                                    FontId::proportional(11.0),
                                    Color32::from_rgb(235, 240, 248),
                                );
                            }
                        }
                    });
                    ui.add_space(gap);
                }
                ui.add_space(10.0);
                let _ = &mut picked;
            });
        if let Some(i) = reject {
            self.move_to(i, REJECTED);
            self.sel.retain(|x| *x != i);
            self.toast(format!("Rejected {}", self.photos[i].name));
        }
        if let Some(i) = toggle {
            if self.sel.contains(&i) {
                self.sel.retain(|x| *x != i);
            } else {
                self.sel.push(i);
                if self.sel.len() > 2 {
                    self.sel.remove(0);
                }
            }
        }
        if let Some(i) = clicked {
            self.pv_idx = Some(i);
            self.view = View::Swipe;
        }
    }

    fn compare_view(&mut self, ui: &mut egui::Ui, p: &Pal) {
        if self.compare.is_none() {
            return;
        }
        let ctx = ui.ctx().clone();
        let screen = ctx.screen_rect().size();
        let win_w = (screen.x * 0.86).min(1180.0);
        let win_h = (screen.y * 0.82).min(760.0);
        let sel = self.sel.clone();
        let mut decisions = self.compare.clone().unwrap_or([None, None]);
        let mut close = false;
        egui::Window::new("compare")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .fixed_size(Vec2::new(win_w, win_h))
            .frame(theme::panel_frame(p, 20.0, 18.0))
            .show(&ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Compare").size(19.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::ghost(ui, p, "Close").clicked() {
                            close = true;
                        }
                    });
                });
                ui.add_space(10.0);
                let avail = ui.available_size();
                let pane_w = (avail.x - 14.0) / 2.0;
                let pane_h = avail.y - 10.0;
                ui.horizontal(|ui| {
                    for slot in 0..2 {
                        let idx = *sel.get(slot).unwrap_or(&usize::MAX);
                        let (rect, _) =
                            ui.allocate_exact_size(Vec2::new(pane_w, pane_h), Sense::hover());
                        let painter = ui.painter().clone();
                        painter.rect_filled(rect, egui::Rounding::same(14.0), p.panel_soft);
                        let img = Rect::from_min_size(
                            rect.min + Vec2::splat(10.0),
                            Vec2::new(rect.width() - 20.0, rect.height() - 98.0),
                        );
                        if idx != usize::MAX {
                            if let Some(t) = self.tex.get(&idx) {
                                painter.with_clip_rect(img).image(
                                    t.id(),
                                    fit_rect(img, t.size_vec2()),
                                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                    Color32::WHITE,
                                );
                            }
                            painter.text(
                                Pos2::new(img.left(), img.bottom() + 12.0),
                                egui::Align2::LEFT_TOP,
                                self.photos[idx].name.clone(),
                                FontId::proportional(12.0),
                                p.muted,
                            );
                            if let Some(choice) = &decisions[slot] {
                                painter.rect_filled(
                                    img,
                                    egui::Rounding::same(10.0),
                                    Color32::from_black_alpha(130),
                                );
                                painter.text(
                                    img.center(),
                                    egui::Align2::CENTER_CENTER,
                                    choice,
                                    FontId::proportional(18.0),
                                    coll_color(choice, p.dark),
                                );
                            }
                        }
                        if decisions[slot].is_some() {
                            continue;
                        }
                        let by = img.bottom() + 34.0;
                        let bw = (img.width() - 16.0) / 3.0;
                        let labels = [
                            ("Keep", CANDIDATES, p.green),
                            ("Later", LATER, p.gold),
                            ("Reject", REJECTED, p.red),
                        ];
                        let mut chosen: Option<&'static str> = None;
                        for (k, (label, coll, color)) in labels.iter().enumerate() {
                            let b = Rect::from_min_size(
                                Pos2::new(img.left() + k as f32 * (bw + 8.0), by),
                                Vec2::new(bw, 36.0),
                            );
                            let r = ui.interact(
                                b,
                                ui.id().with(("cmp", slot, k)),
                                Sense::click(),
                            );
                            ui.painter().rect(
                                b,
                                egui::Rounding::same(10.0),
                                if r.hovered() {
                                    color.gamma_multiply(0.32)
                                } else {
                                    p.panel
                                },
                                Stroke::new(1.0, color.gamma_multiply(0.6)),
                            );
                            ui.painter().text(
                                b.center(),
                                egui::Align2::CENTER_CENTER,
                                *label,
                                FontId::proportional(13.0),
                                color.gamma_multiply(0.95),
                            );
                            if r.clicked() {
                                chosen = Some(coll);
                            }
                        }
                        if let Some(coll) = chosen {
                            if idx != usize::MAX {
                                self.move_to(idx, coll);
                                decisions[slot] = Some(coll.to_string());
                                if let Some(c) = self.compare.as_mut() {
                                    c[slot] = Some(coll.to_string());
                                }
                            }
                        }
                    }
                });
            });
        if close || decisions.iter().all(|d| d.is_some()) {
            self.compare = None;
            self.sel.clear();
        }
    }

    fn settings_window(&mut self, ctx: &egui::Context, p: &Pal) {
        if !self.settings_open {
            return;
        }
        let mut open = true;
        let mut theme_changed = false;
        egui::Window::new("settings")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .fixed_size(Vec2::new(480.0, 520.0))
            .frame(theme::panel_frame(p, 20.0, 20.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Settings").size(20.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::ghost(ui, p, "Close").clicked() {
                            open = false;
                        }
                    });
                });
                ui.add_space(12.0);
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.label(egui::RichText::new("Theme").size(12.0).color(p.muted));
                ui.horizontal(|ui| {
                    for (t, label) in [("auto", "Auto"), ("light", "Light"), ("dark", "Dark")] {
                        let sel = self.db.settings.theme == t;
                        if theme::chip(ui, p, label, p.accent, sel).clicked() {
                            self.db.settings.theme = t.into();
                            theme_changed = true;
                        }
                    }
                });
                ui.add_space(14.0);
                ui.label(egui::RichText::new("Thumbnail size").size(12.0).color(p.muted));
                ui.add(
                    egui::Slider::new(&mut self.db.settings.thumb, 140..=320)
                        .suffix(" px")
                        .show_value(true),
                );
                ui.add_space(14.0);
                ui.label(egui::RichText::new("Guide lines").size(12.0).color(p.muted));
                ui.horizontal_wrapped(|ui| {
                    for (k, label) in [
                        ("off", "Off"),
                        ("thirds", "Thirds"),
                        ("golden", "Golden"),
                        ("cross", "Cross"),
                        ("diag", "Diagonals"),
                    ] {
                        let sel = self.db.settings.guides == k;
                        if theme::chip(ui, p, label, p.accent, sel).clicked() {
                            self.db.settings.guides = k.into();
                        }
                    }
                });
                ui.add_space(14.0);
                ui.label(egui::RichText::new("Preview size").size(12.0).color(p.muted));
                ui.horizontal(|ui| {
                    for v in [1200u32, 1600, 2048, 0] {
                        let sel = self.db.settings.preview == v;
                        let label = if v == 0 {
                            "Full".to_string()
                        } else {
                            format!("{v} px")
                        };
                        if theme::chip(ui, p, &label, p.accent, sel).clicked() {
                            self.db.settings.preview = v;
                            self.pv_idx = None;
                            self.pv_cache.clear();
                        }
                    }
                });
                ui.add_space(18.0);
                ui.separator();
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "Tile cache: {} in {} files",
                            human_mb(self.cache_bytes),
                            self.cache_files
                        ))
                        .size(13.0)
                        .color(p.text),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::ghost(ui, p, "Clear").clicked() {
                            let freed = decode::clear_cache();
                            let (b, n) = decode::cache_bytes();
                            self.cache_bytes = b;
                            self.cache_files = n;
                            self.toast(format!("Freed {}", human_mb(freed)));
                        }
                        if theme::ghost(ui, p, "Refresh").clicked() {
                            let (b, n) = decode::cache_bytes();
                            self.cache_bytes = b;
                            self.cache_files = n;
                        }
                    });
                });
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new(decode::codec_note())
                        .size(12.0)
                        .color(p.muted),
                );
                ui.add_space(18.0);
                ui.separator();
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if theme::ghost(ui, p, "Export sort map").clicked() {
                        self.export();
                    }
                    if theme::ghost(ui, p, "Restore sort map").clicked() {
                        self.import();
                    }
                });
                ui.add_space(14.0);
                ui.label(
                    egui::RichText::new(
                        "Keys: left reject, right candidate, space later, up final, down skip, z undo, c compare, / search, , settings",
                    )
                    .size(11.5)
                    .color(p.muted),
                );
                });
            });
        self.settings_open = open;
        if theme_changed {
            apply_visuals(ctx, self.dark());
            self.db.save();
        }
    }

    fn help_window(&mut self, _ctx: &egui::Context, _p: &Pal) {
        self.help_open = false;
    }

    fn toast_overlay(&mut self, ctx: &egui::Context, p: &Pal) {
        let Some((msg, left)) = self.toast.clone() else {
            return;
        };
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        let left = left - dt as f64;
        if left <= 0.0 {
            self.toast = None;
            return;
        }
        self.toast = Some((msg.clone(), left));
        let p2 = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("toast"));
        let painter = ctx.layer_painter(p2);
        let screen = ctx.screen_rect();
        let galley = painter.layout_no_wrap(
            msg,
            FontId::proportional(13.5),
            Color32::from_rgb(240, 244, 250),
        );
        let w = galley.size().x + 32.0;
        let rect = Rect::from_center_size(
            Pos2::new(screen.center().x, screen.bottom() - 34.0),
            Vec2::new(w, 38.0),
        );
        painter.rect(
            rect,
            egui::Rounding::same(19.0),
            Color32::from_rgba_unmultiplied(20, 24, 31, 240),
            Stroke::new(1.0, p.line),
        );
        painter.galley(
            Pos2::new(rect.center().x - galley.size().x / 2.0, rect.center().y - galley.size().y / 2.0),
            galley,
            Color32::WHITE,
        );
    }

    fn handle_input(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }
        let list = self.photos_in(&self.coll.clone());
        let cur = self
            .pv_idx
            .filter(|i| list.contains(i))
            .or_else(|| list.first().copied());
        let (left, right, space, up, down, undo, cmp, search, settings) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowLeft),
                i.key_pressed(egui::Key::ArrowRight),
                i.key_pressed(egui::Key::Space),
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::Z),
                i.key_pressed(egui::Key::C),
                i.key_pressed(egui::Key::Slash),
                i.key_pressed(egui::Key::Comma),
            )
        });
        if settings {
            self.settings_open = !self.settings_open;
        }
        let Some(idx) = cur else { return };
        if left || ctx.input(|i| i.key_pressed(egui::Key::X)) {
            self.move_to(idx, REJECTED);
            self.pv_idx = None;
        } else if right {
            self.move_to(idx, CANDIDATES);
            self.pv_idx = None;
        } else if space {
            self.move_to(idx, LATER);
            self.pv_idx = None;
        } else if up {
            self.move_to(idx, FINAL);
            self.pv_idx = None;
        } else if down {
            self.next_in(&list, idx);
        } else if undo {
            self.undo();
        } else if cmp {
            let pos = list.iter().position(|x| *x == idx).unwrap_or(0);
            self.sel = vec![idx];
            if pos + 1 < list.len() {
                self.sel.push(list[pos + 1]);
            }
            self.start_compare();
        } else if search {
            self.search.clear();
        }
    }

    fn handle_drop(&mut self, ctx: &egui::Context) {
        let files: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if files.is_empty() {
            return;
        }
        if let Some(dir) = files.iter().find(|p| p.is_dir()) {
            self.open_folder(ctx, dir.clone());
        } else {
            self.open_files(ctx, files);
        }
    }

    fn export(&mut self) {
        let mut out = String::from("{\n");
        let mut first = true;
        for (i, p) in self.photos.iter().enumerate() {
            if !first {
                out.push_str(",\n");
            }
            first = false;
            let name = p.name.replace('\\', "\\\\").replace('"', "\\\"");
            out.push_str(&format!(
                "  \"{}\": \"{}\"",
                name, self.assign[i]
            ));
        }
        out.push_str("\n}\n");
        let target = self
            .folder
            .clone()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("keepers-sortmap.json");
        match std::fs::write(&target, out) {
            Ok(_) => self.toast(format!("Saved {}", target.display())),
            Err(e) => self.toast(format!("Could not save: {e}")),
        }
    }

    fn import(&mut self) {
        let Some(dir) = self.folder.clone() else {
            self.toast("Open a folder first");
            return;
        };
        let src = dir.join("keepers-sortmap.json");
        let Ok(text) = std::fs::read_to_string(&src) else {
            self.toast("No keepers-sortmap.json in this folder");
            return;
        };
        let Ok(map) = serde_json::from_str::<std::collections::BTreeMap<String, String>>(&text)
        else {
            self.toast("Sort map is not readable");
            return;
        };
        let mut n = 0;
        for (i, p) in self.photos.iter().enumerate() {
            if let Some(c) = map.get(&p.name) {
                self.assign[i] = c.clone();
                n += 1;
            }
        }
        self.persist();
        self.toast(format!("Restored {n} decisions"));
    }

    fn pick_folder(&mut self, _ctx: &egui::Context) {
        #[cfg(windows)]
        {
            if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                self.open_folder(_ctx, dir);
            }
        }
        #[cfg(not(windows))]
        {
            self.show_path_input = true;
        }
    }

    fn pick_files(&mut self, _ctx: &egui::Context) {
        #[cfg(windows)]
        {
            if let Some(files) = rfd::FileDialog::new()
                .add_filter("Photos", &["heic", "heif", "jpg", "jpeg", "png", "webp", "avif"])
                .pick_files()
            {
                self.open_files(_ctx, files);
            }
        }
        #[cfg(not(windows))]
        {
            self.show_path_input = true;
        }
    }
}

fn brand(ui: &mut egui::Ui, p: &Pal) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(38.0, 38.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, egui::Rounding::same(12.0), p.accent);
    theme::draw_icon(
        ui.painter(),
        rect.center(),
        10.0,
        Icon::Keep,
        Color32::from_rgb(26, 18, 14),
        3.2,
    );
    ui.add_space(10.0);
    ui.label(
        egui::RichText::new("Keepers")
            .size(24.0)
            .strong()
            .color(p.text),
    );
}

fn ctx_of(ui: &egui::Ui) -> &egui::Context {
    ui.ctx()
}

fn guides(painter: &egui::Painter, rect: Rect, kind: &str, p: &Pal) {
    let c = Color32::from_rgba_unmultiplied(255, 255, 255, 90);
    let s = Stroke::new(1.0, c);
    match kind {
        "off" => {}
        "thirds" => {
            for i in 1..3 {
                let x = rect.left() + rect.width() * i as f32 / 3.0;
                let y = rect.top() + rect.height() * i as f32 / 3.0;
                painter.line_segment([Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())], s);
                painter.line_segment([Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)], s);
            }
        }
        "golden" => {
            let g = 0.381966;
            for f in [g, 1.0 - g] {
                let x = rect.left() + rect.width() * f;
                let y = rect.top() + rect.height() * f;
                painter.line_segment([Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())], s);
                painter.line_segment([Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)], s);
            }
        }
        "cross" => {
            painter.line_segment(
                [
                    Pos2::new(rect.center().x, rect.top()),
                    Pos2::new(rect.center().x, rect.bottom()),
                ],
                s,
            );
            painter.line_segment(
                [
                    Pos2::new(rect.left(), rect.center().y),
                    Pos2::new(rect.right(), rect.center().y),
                ],
                s,
            );
        }
        "diag" => {
            painter.line_segment([rect.left_top(), rect.right_bottom()], s);
            painter.line_segment([rect.right_top(), rect.left_bottom()], s);
        }
        _ => {}
    }
    let _ = p;
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Key, Modifiers, Pos2, RawInput, Rect};

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("keepers-test-{name}"));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn copy_fixtures(dir: &std::path::Path) {
        let src = std::path::Path::new("/home/user/fixtures");
        if !src.exists() {
            return;
        }
        for e in std::fs::read_dir(src).unwrap().flatten() {
            let name = e.file_name();
            let _ = std::fs::copy(e.path(), dir.join(name));
        }
    }

    fn make_images(dir: &std::path::Path, n: u32, w: u32, h: u32) {
        for i in 0..n {
            let mut img = image::RgbImage::new(w, h);
            for (x, y, px) in img.enumerate_pixels_mut() {
                let r = ((x * 255) / w) as u8;
                let g = ((y * 255) / h) as u8;
                let b = (i * 37) as u8;
                *px = image::Rgb([r, g, b]);
            }
            img.save(dir.join(format!("IMG_{:04}.JPG", 2140 + i))).unwrap();
        }
    }

    fn input(events_vec: Vec<Event>) -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(1440.0, 900.0),
            )),
            events: events_vec,
            ..Default::default()
        }
    }

    fn pump(app: &mut App, ctx: &egui::Context, frames: usize, sleep_ms: u64) {
        for _ in 0..frames {
            ctx.run(input(vec![]), |c| app.frame(c));
            if sleep_ms > 0 {
                std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
            }
        }
    }

    fn key(k: Key) -> Vec<Event> {
        vec![Event::Key {
            key: k,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::default(),
        }]
    }

    #[test]
    fn full_flow() {
        let data = scratch("data");
        let pics = scratch("pics");
        std::env::set_var("KEEPERS_DATA", &data);
        make_images(&pics, 6, 900, 600);

        let ctx = egui::Context::default();
        let mut app = App::bare();
        app.init(&ctx);
        app.open_folder(&ctx, pics.clone());
        assert_eq!(app.photos.len(), 6, "scan finds the six jpgs");

        // thumbnails finish on their own
        let mut spins = 0;
        while app.thumbs.done < app.thumbs.total && spins < 400 {
            pump(&mut app, &ctx, 1, 25);
            spins += 1;
        }
        assert_eq!(app.thumbs.ok, 6, "every photo decodes");
        assert_eq!(app.tex.len(), 6, "every photo gets a texture");
        assert_eq!(app.dims[0], Some((480, 320)), "thumbnail capped at 480");
        assert!(app.toast.as_ref().unwrap().0.contains("Cached 6 of 6"));

        // the preview is a separate, larger decode
        app.pv_idx = Some(0);
        pump(&mut app, &ctx, 1, 0);
        let mut spins = 0;
        while app.pv_tex.is_none() && spins < 200 {
            pump(&mut app, &ctx, 1, 20);
            spins += 1;
        }
        assert!(app.pv_tex.is_some(), "preview decodes at full preview size");

        // keyboard: right arrow files the photo as a candidate
        app.pv_idx = Some(0);
        ctx.run(input(key(Key::ArrowRight)), |c| app.frame(c));
        assert_eq!(app.assign[0], CANDIDATES);
        assert_eq!(app.counts(CANDIDATES), 1);

        // undo puts it back in the inbox
        ctx.run(input(key(Key::Z)), |c| app.frame(c));
        assert_eq!(app.assign[0], INBOX);

        // decisions survive a restart
        app.move_to(1, FINAL);
        app.move_to(2, REJECTED);
        let reloaded = Db::load();
        let fm = reloaded
            .folders
            .get(&folder_key(&pics))
            .expect("folder stored");
        assert_eq!(fm.assign.get("IMG_2141.JPG").map(String::as_str), Some(FINAL));
        assert_eq!(
            fm.assign.get("IMG_2142.JPG").map(String::as_str),
            Some(REJECTED)
        );

        // export writes a readable sort map, import restores it
        app.export();
        let map_path = pics.join("keepers-sortmap.json");
        let text = std::fs::read_to_string(&map_path).unwrap();
        let parsed: std::collections::BTreeMap<String, String> =
            serde_json::from_str(&text).unwrap();
        assert_eq!(parsed.get("IMG_2141.JPG").map(String::as_str), Some(FINAL));
        app.assign.iter_mut().for_each(|c| *c = INBOX.into());
        app.import();
        assert_eq!(app.assign[2], REJECTED);

        // compare opens, takes both decisions and closes
        app.sel = vec![0, 1];
        app.start_compare();
        assert!(app.compare.is_some());
        pump(&mut app, &ctx, 2, 0);
        if let Some(c) = app.compare.as_mut() {
            c[0] = Some(CANDIDATES.to_string());
            c[1] = Some(REJECTED.to_string());
        }
        pump(&mut app, &ctx, 2, 0);
        assert!(app.compare.is_none(), "compare closes when both sides chose");
        assert!(app.sel.is_empty());

        // filters and search narrow the list
        app.filter = Filter::Videos;
        assert!(app.photos_in(INBOX).is_empty(), "no videos in this folder");
        app.filter = Filter::All;
        app.search = "2143".into();
        assert_eq!(app.photos_in(INBOX).len(), 1);
        app.search.clear();

        // the app renders every view without panicking
        for v in [View::Swipe, View::Grid] {
            app.view = v;
            pump(&mut app, &ctx, 2, 0);
        }
        app.settings_open = true;
        pump(&mut app, &ctx, 2, 0);

        // the cache holds six tiles and clears to zero
        let (bytes, files) = decode::cache_bytes();
        assert!(files >= 6, "six tiles on disk, got {files}");
        assert!(bytes > 0);
        let freed = decode::clear_cache();
        assert!(freed > 0);
        assert_eq!(decode::cache_bytes().1, 0);
    }

    #[test]
    fn screenshots() {
        let data = scratch("data3");
        let pics = scratch("pics3");
        std::env::set_var("KEEPERS_DATA", &data);
        copy_fixtures(&pics);
        let out = PathBuf::from("/home/user/shots-rust");
        std::fs::create_dir_all(&out).unwrap();

        let ctx = egui::Context::default();
        ctx.style_mut(|s| s.animation_time = 0.0);
        let mut app = App::bare();
        app.init(&ctx);
        let mut shot = crate::shot::Shot::new(2880, 1800, 2.0);
        app.open_folder(&ctx, pics.clone());
        let mut spins = 0;
        while app.thumbs.done < app.thumbs.total && spins < 500 {
            shot.pump(&mut app, &ctx, 1, 12);
            spins += 1;
        }
        app.assign[1] = CANDIDATES.into();
        app.assign[2] = LATER.into();
        app.assign[3] = FINAL.into();
        app.assign[4] = REJECTED.into();
        app.pv_idx = Some(0);
        app.request_preview(&ctx, 0);
        let mut spins = 0;
        while app.pv_tex.is_none() && spins < 200 {
            shot.pump(&mut app, &ctx, 1, 15);
            spins += 1;
        }
        app.toast = Some(("Cached 8 of 8 photos for offline use".into(), 40.0));
        shot.pump(&mut app, &ctx, 3, 10);
        shot.save(&out.join("swipe.png"));

        app.toast = None;
        app.view = View::Grid;
        app.sel = vec![0, 1];
        shot.pump(&mut app, &ctx, 3, 10);
        shot.save(&out.join("grid.png"));

        app.view = View::Swipe;
        app.start_compare();
        shot.pump(&mut app, &ctx, 3, 10);
        shot.save(&out.join("compare.png"));

        app.compare = None;
        app.settings_open = true;
        shot.pump(&mut app, &ctx, 3, 10);
        shot.save(&out.join("settings.png"));

        app.settings_open = false;
        app.photos.clear();
        app.assign.clear();
        app.folder = None;
        shot.pump(&mut app, &ctx, 3, 10);
        shot.save(&out.join("empty.png"));
    }

    #[test]
    fn cache_is_parallel_and_warm_start_is_instant() {
        let data = scratch("data2");
        let pics = scratch("pics2");
        std::env::set_var("KEEPERS_DATA", &data);
        make_images(&pics, 24, 1600, 1067);
        decode::clear_cache();
        let ctx = egui::Context::default();

        let mut app = App::bare();
        app.init(&ctx);
        app.open_folder(&ctx, pics.clone());
        let t0 = std::time::Instant::now();
        while app.thumbs.done < app.thumbs.total {
            pump(&mut app, &ctx, 1, 10);
        }
        let cold = t0.elapsed();
        println!(
            "cold cache: 24 photos in {:.2}s with {} workers ({:.1}/s)",
            cold.as_secs_f64(),
            app.thumbs.workers,
            app.thumbs.rate()
        );
        assert_eq!(app.thumbs.ok, 24);

        let mut app2 = App::bare();
        app2.init(&ctx);
        let t1 = std::time::Instant::now();
        app2.open_folder(&ctx, pics.clone());
        let mut first = None;
        while app2.thumbs.done < app2.thumbs.total {
            pump(&mut app2, &ctx, 1, 2);
            if first.is_none() && !app2.tex.is_empty() {
                first = Some(t1.elapsed());
            }
        }
        println!(
            "warm cache: 24 photos in {:.2}s, first tile at {:.0} ms",
            t1.elapsed().as_secs_f64(),
            first.unwrap_or_default().as_secs_f64() * 1000.0
        );
        assert!(
            t1.elapsed() < cold,
            "warm pass beats the cold pass: {:?} vs {:?}",
            t1.elapsed(),
            cold
        );
    }
}
