use crate::decode::{self, Frame};
use crate::model::*;
use crate::theme::{self, Icon, Pal};
use crate::thumbs::{Msg, Thumbs};
use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};
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
    filter_open: bool,

    cache_bytes: u64,
    cache_files: u64,

    bg: Option<egui::TextureHandle>,
    dots: Option<egui::TextureHandle>,
    bg_key: Option<(bool, usize, usize)>,
    show_path_input: bool,
}

fn dark_now(db: &Db) -> bool {
    match db.settings.theme.as_str() {
        "light" => false,
        "dark" => true,
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
            filter_open: false,
            cache_bytes,
            cache_files,
            bg: None,
            dots: None,
            bg_key: None,
            show_path_input: false,
        }
    }

    pub fn init(&mut self, ctx: &egui::Context) {
        theme::apply(ctx, dark_now(&self.db));
        self.dots = None;
    }

    fn dark(&self) -> bool {
        dark_now(&self.db)
    }

    fn pal(&self) -> Pal {
        theme::pal(self.dark())
    }

    fn toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), 2.8));
    }

    // ---------- data ----------

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
            .map(|p| {
                fm.assign
                    .get(&p.name)
                    .cloned()
                    .unwrap_or_else(|| INBOX.into())
            })
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
        let paths: Vec<PathBuf> = self.photos.iter().map(|p| p.path.clone()).collect();
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
                    let t = ctx.load_texture(format!("t{idx}"), img, egui::TextureOptions::LINEAR);
                    self.tex.insert(idx, t);
                    self.tex_order.push_back(idx);
                    while self.tex_order.len() > 300 {
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

    fn pick_folder(&mut self, ctx: &egui::Context) {
        #[cfg(windows)]
        {
            if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                self.open_folder(ctx, dir);
            }
        }
        #[cfg(not(windows))]
        {
            let _ = ctx;
            self.show_path_input = true;
            self.toast("Type a folder path and press Enter");
        }
    }

    fn pick_files(&mut self, ctx: &egui::Context) {
        #[cfg(windows)]
        {
            if let Some(files) = rfd::FileDialog::new()
                .add_filter(
                    "Photos",
                    &["heic", "heif", "jpg", "jpeg", "png", "webp", "avif"],
                )
                .pick_files()
            {
                self.open_files(ctx, files);
            }
        }
        #[cfg(not(windows))]
        {
            self.pick_folder(ctx);
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
            out.push_str(&format!("  \"{}\": \"{}\"", name, self.assign[i]));
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

    fn start_compare(&mut self) {
        if self.sel.len() >= 2 {
            self.compare = Some([None, None]);
            self.pv_tex = None;
            self.pv_idx = None;
        } else {
            self.toast("Pick two photos in the grid first");
        }
    }

    // ---------- page ----------

    fn page(&mut self, ctx: &egui::Context, p: &Pal) {
        let screen = ctx.screen_rect();
        let w = ((screen.width() / 3.0).ceil() as usize).max(2);
        let h = ((screen.height() / 3.0).ceil() as usize).max(2);
        let key = (self.dark(), w, h);
        if self.bg_key != Some(key) {
            let img = page_image(w, h, key.0);
            let opts = egui::TextureOptions {
                magnification: egui::TextureFilter::Linear,
                minification: egui::TextureFilter::Linear,
                wrap_mode: egui::TextureWrapMode::ClampToEdge,
                mipmap_mode: None,
            };
            self.bg = Some(ctx.load_texture("page", img, opts));
            self.bg_key = Some(key);
        }
        if self.dots.is_none() {
            self.dots = Some(dot_texture(ctx));
        }
        let painter = ctx.layer_painter(egui::LayerId::background());
        painter.rect_filled(screen, egui::Rounding::ZERO, p.bg);
        if let Some(t) = &self.bg {
            painter.image(
                t.id(),
                screen,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        if let Some(t) = &self.dots {
            let mut m = egui::Mesh::default();
            m.texture_id = t.id();
            let uv = Rect::from_min_max(
                Pos2::ZERO,
                Pos2::new(screen.width() / 24.0, screen.height() / 24.0),
            );
            m.add_rect_with_uv(screen, uv, Color32::WHITE);
            for v in m.vertices.iter_mut() {
                let f = 1.0 - ((v.pos.y - screen.top()) / (screen.height() * 0.78)).clamp(0.0, 1.0);
                v.color = Color32::from_white_alpha((f * 92.0) as u8);
            }
            painter.add(egui::Shape::mesh(m));
        }
    }

    // ---------- shell ----------

    pub fn frame_inner(&mut self, ctx: &egui::Context) {
        let p = self.pal();
        self.page(ctx, &p);
        self.drain_thumbs(ctx);
        self.drain_preview(ctx);
        if self.thumbs.running() || self.toast.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(80));
        }
        self.handle_input(ctx);
        self.handle_drop(ctx);

        egui::SidePanel::left("side")
            .exact_width(theme::SIDEBAR)
            .resizable(false)
            .frame(egui::Frame::none())
            .show(ctx, |ui| self.sidebar(ui, &p));

        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| self.main_panel(ui, &p));

        self.settings_window(ctx, &p);
        self.toast_overlay(ctx, &p);
    }

    fn sidebar(&mut self, ui: &mut egui::Ui, p: &Pal) {
        let full = ui.max_rect();
        let pad = 20.0;
        let x = full.left() + pad;
        let w = theme::SIDEBAR - pad * 2.0;
        let mut y = full.top() + 26.0;

        // brand
        let mark = Rect::from_min_size(Pos2::new(x, y), Vec2::splat(30.0));
        {
            let painter = ui.painter();
            painter.rect_filled(mark, egui::Rounding::same(9.0), p.ac);
            theme::draw_icon(
                painter,
                mark.center(),
                7.0,
                Icon::Keep,
                Color32::from_rgb(0x2b, 0x14, 0x08),
                2.6,
            );
            painter.text(
                Pos2::new(mark.right() + 10.0, mark.center().y),
                egui::Align2::LEFT_CENTER,
                "Keepers",
                theme::bold(22.0),
                p.tx,
            );
        }
        y = mark.bottom() + 26.0;

        // sorted progress
        let total = self.photos.len();
        let done = self.sorted();
        let bar = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w - 8.0, 12.0));
        let frac = if total == 0 {
            0.0
        } else {
            done as f32 / total as f32
        };
        theme::progress(p, ui.painter(), bar, frac, true);
        y = bar.bottom() + 10.0;
        ui.painter().text(
            Pos2::new(x + 2.0, y),
            egui::Align2::LEFT_TOP,
            format!("{done} of {total} sorted"),
            theme::regular(12.0),
            p.mu,
        );
        y += 30.0;

        // tile cache
        if self.thumbs.total > 0 {
            let running = self.thumbs.running();
            let bar = Rect::from_min_size(Pos2::new(x, y), Vec2::new(104.0, 8.0));
            let f = (self.thumbs.done as f32 / self.thumbs.total.max(1) as f32).clamp(0.0, 1.0);
            theme::progress(p, ui.painter(), bar, f, false);
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
            ui.painter().text(
                Pos2::new(bar.right() + 10.0, bar.center().y),
                egui::Align2::LEFT_CENTER,
                label,
                theme::regular(11.5),
                p.mu,
            );
            if running {
                let stop = Rect::from_min_size(
                    Pos2::new(x + w - 44.0, bar.center().y - 11.0),
                    Vec2::new(44.0, 22.0),
                );
                let r = ui.interact(stop, ui.id().with("stopcache"), Sense::click());
                theme::raised(p, ui.painter(), stop, 11.0, false);
                ui.painter()
                    .rect_filled(stop, egui::Rounding::same(11.0), p.bg);
                ui.painter().text(
                    stop.center(),
                    egui::Align2::CENTER_CENTER,
                    "Stop",
                    theme::regular(11.0),
                    p.mu,
                );
                if r.clicked() {
                    self.thumbs.stop();
                }
            }
            y = bar.bottom() + 20.0;
        }

        // collections
        let colls = self.collections();
        let nav_h = colls.len() as f32 * 46.0 + 20.0;
        let nav = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, nav_h));
        theme::surface(p, ui.painter(), nav, 24.0, if p.dark { 0.8 } else { 0.5 });
        let mut ry = nav.top() + 10.0;
        for name in colls {
            let n = self.counts(&name);
            let selected = self.coll == name;
            let row = Rect::from_min_size(
                Pos2::new(nav.left() + 10.0, ry),
                Vec2::new(nav.width() - 20.0, 42.0),
            );
            let resp = ui.interact(row, ui.id().with(("col", &name)), Sense::click());
            let painter = ui.painter();
            if selected {
                theme::raised(p, painter, row, 16.0, false);
                painter.rect_filled(
                    row,
                    egui::Rounding::same(16.0),
                    theme::mix(p.bg, p.hi, if p.dark { 0.85 } else { 0.95 }),
                );
                let arc = Rect::from_min_size(
                    Pos2::new(row.left() + 6.0, row.top() + 9.0),
                    Vec2::new(3.0, row.height() - 18.0),
                );
                painter.rect_filled(arc, egui::Rounding::same(3.0), p.ac);
            }
            let color = coll_color(&name, p.dark);
            theme::dot(
                p,
                painter,
                Pos2::new(row.left() + 26.0, row.center().y),
                color,
                selected,
            );
            let font = if selected {
                theme::semibold(14.0)
            } else {
                theme::regular(14.0)
            };
            let tc = if selected {
                p.tx
            } else if resp.hovered() {
                p.ac
            } else {
                p.tx.gamma_multiply(0.84)
            };
            painter.text(
                Pos2::new(row.left() + 44.0, row.center().y),
                egui::Align2::LEFT_CENTER,
                &name,
                font,
                tc,
            );
            painter.text(
                Pos2::new(row.right() - 14.0, row.center().y),
                egui::Align2::RIGHT_CENTER,
                n.to_string(),
                theme::regular(12.5),
                p.mu,
            );
            if resp.clicked() {
                self.coll = name.clone();
                self.sel.clear();
                self.compare = None;
                self.pv_idx = None;
            }
            ry += 46.0;
        }
        y = nav.bottom() + 16.0;

        // new collection
        let input = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w - 52.0, 42.0));
        theme::inset(p, ui.painter(), input, 14.0);
        let inner = input.shrink2(Vec2::new(14.0, 0.0));
        let mut child = ui.child_ui(
            inner,
            egui::Layout::left_to_right(egui::Align::Center),
            None,
        );
        let te = child.add(
            egui::TextEdit::singleline(&mut self.new_coll)
                .hint_text("New collection")
                .desired_width(inner.width())
                .frame(false),
        );
        let commit = (te.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
            || theme::button_rect(
                ui,
                p,
                Rect::from_min_size(Pos2::new(x + w - 42.0, y), Vec2::splat(42.0)),
                "",
                Some(Icon::Plus),
                theme::regular(14.0),
            )
            .clicked();
        if commit {
            let name = self.new_coll.trim().to_string();
            if !name.is_empty() && !self.collections().contains(&name) {
                self.custom.push(name);
                self.persist();
                self.new_coll.clear();
            }
        }

        // footer
        let bh = 42.0;
        let fy = full.bottom() - 22.0 - bh * 2.0 - 10.0;
        if theme::button_rect(
            ui,
            p,
            Rect::from_min_size(Pos2::new(x, fy), Vec2::new(w, bh)),
            "Open another folder",
            Some(Icon::Folder),
            theme::regular(13.5),
        )
        .clicked()
        {
            self.pick_folder(ui.ctx());
        }
        if theme::button_rect(
            ui,
            p,
            Rect::from_min_size(Pos2::new(x, fy + bh + 10.0), Vec2::new(w, bh)),
            "Settings",
            Some(Icon::Gear),
            theme::regular(13.5),
        )
        .clicked()
        {
            self.settings_open = true;
        }
    }

    fn main_panel(&mut self, ui: &mut egui::Ui, p: &Pal) {
        let pad = 34.0;
        ui.add_space(30.0);
        self.top_bar(ui, p);
        ui.add_space(26.0);
        if self.photos.is_empty() {
            self.empty_state(ui, p);
            return;
        }
        let list = self.photos_in(&self.coll.clone());
        if self.compare.is_some() {
            self.compare_view(ui, p);
            return;
        }
        match self.view {
            View::Swipe => self.swipe_view(ui, p, &list),
            View::Grid => self.grid_view(ui, p, &list),
        }
        let _ = pad;
    }

    fn top_bar(&mut self, ui: &mut egui::Ui, p: &Pal) {
        let pad = 34.0;
        let avail = ui.available_width();
        let bar = Rect::from_min_size(
            Pos2::new(ui.cursor().min.x + pad, ui.cursor().min.y),
            Vec2::new(avail - pad * 2.0, 76.0),
        );
        theme::surface(p, ui.painter(), bar, 28.0, if p.dark { 0.75 } else { 0.55 });
        ui.allocate_rect(bar, Sense::hover());
        let cy = bar.center().y;

        // title group
        let color = coll_color(&self.coll, p.dark);
        ui.painter()
            .circle_filled(Pos2::new(bar.left() + 26.0, cy), 6.0, color);
        let title = ui.painter().layout_no_wrap(
            self.coll.clone(),
            theme::semibold(34.0),
            p.tx,
        );
        ui.painter().galley(
            Pos2::new(bar.left() + 44.0, cy - title.size().y / 2.0),
            title.clone(),
            p.tx,
        );
        ui.painter().text(
            Pos2::new(bar.left() + 52.0 + title.size().x, cy),
            egui::Align2::LEFT_CENTER,
            format!("{} items", self.counts(&self.coll)),
            theme::regular(14.0),
            p.mu,
        );

        // right cluster
        let mut rx = bar.right() - 18.0;
        let gear = Rect::from_min_size(Pos2::new(rx - 44.0, cy - 22.0), Vec2::splat(44.0));
        let gr = ui.interact(gear, ui.id().with("gear"), Sense::click());
        theme::inset(p, ui.painter(), gear, 14.0);
        theme::draw_icon(
            ui.painter(),
            gear.center(),
            9.0,
            Icon::Gear,
            if gr.hovered() { p.ac } else { p.mu },
            1.7,
        );
        if gr.clicked() {
            self.settings_open = true;
        }
        rx = gear.left() - 10.0;
        let dl = Rect::from_min_size(Pos2::new(rx - 44.0, cy - 22.0), Vec2::splat(44.0));
        if ui
            .interact(dl, ui.id().with("export"), Sense::click())
            .on_hover_text("Export sort map")
            .clicked()
        {
            self.export();
        }
        theme::inset(p, ui.painter(), dl, 14.0);
        theme::draw_icon(ui.painter(), dl.center(), 9.0, Icon::Download, p.mu, 1.7);
        rx = dl.left() - 10.0;

        // segmented control
        let seg_w = 150.0;
        let seg = Rect::from_min_size(Pos2::new(rx - seg_w, cy - 21.0), Vec2::new(seg_w, 42.0));
        theme::inset(p, ui.painter(), seg, 16.0);
        for (i, (label, view)) in [("Swipe", View::Swipe), ("Grid", View::Grid)]
            .into_iter()
            .enumerate()
        {
            let cell = Rect::from_min_size(
                Pos2::new(seg.left() + 4.0 + i as f32 * 71.0, seg.top() + 4.0),
                Vec2::new(71.0, 34.0),
            );
            let resp = ui.interact(cell, ui.id().with(("seg", i)), Sense::click());
            let on = self.view == view && self.compare.is_none();
            if on {
                theme::raised(p, ui.painter(), cell, 12.0, false);
                ui.painter()
                    .rect_filled(cell, egui::Rounding::same(12.0), p.bg);
            }
            ui.painter().text(
                cell.center(),
                egui::Align2::CENTER_CENTER,
                label,
                if on {
                    theme::semibold(13.0)
                } else {
                    theme::regular(13.0)
                },
                if on { p.tx } else { p.mu },
            );
            if resp.clicked() {
                self.view = view;
                self.compare = None;
            }
        }
        rx = seg.left() - 10.0;

        // filter select
        let sel_w = 172.0;
        let sel = Rect::from_min_size(Pos2::new(rx - sel_w, cy - 21.0), Vec2::new(sel_w, 42.0));
        let resp = ui.interact(sel, ui.id().with("filtersel"), Sense::click());
        theme::inset(p, ui.painter(), sel, 14.0);
        let label = match self.filter {
            Filter::All => "Photos and videos",
            Filter::Photos => "Photos only",
            Filter::Videos => "Videos only",
        };
        ui.painter().text(
            Pos2::new(sel.left() + 16.0, cy),
            egui::Align2::LEFT_CENTER,
            label,
            theme::regular(13.0),
            p.tx,
        );
        theme::draw_icon(
            ui.painter(),
            Pos2::new(sel.right() - 18.0, cy),
            5.0,
            Icon::Chevron,
            p.mu,
            1.6,
        );
        if resp.clicked() {
            self.filter_open = !self.filter_open;
        }
        if self.filter_open {
            let mut chosen = None;
            egui::Area::new(egui::Id::new("filterpop"))
                .order(egui::Order::Foreground)
                .fixed_pos(Pos2::new(sel.left(), sel.bottom() + 8.0))
                .show(ui.ctx(), |ui| {
                    egui::Frame::none()
                        .fill(p.bg)
                        .rounding(egui::Rounding::same(16.0))
                        .inner_margin(egui::Margin::same(8.0))
                        .show(ui, |ui| {
                            ui.set_width(156.0);
                            for (f, name) in [
                                (Filter::All, "Photos and videos"),
                                (Filter::Photos, "Photos only"),
                                (Filter::Videos, "Videos only"),
                            ] {
                                let (r, resp) = ui
                                    .allocate_exact_size(Vec2::new(156.0, 32.0), Sense::click());
                                if resp.hovered() {
                                    ui.painter().rect_filled(
                                        r,
                                        egui::Rounding::same(10.0),
                                        theme::mix(p.bg, p.hi, 0.08),
                                    );
                                }
                                ui.painter().text(
                                    Pos2::new(r.left() + 8.0, r.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    name,
                                    theme::regular(13.0),
                                    if self.filter == f { p.ac } else { p.tx },
                                );
                                if resp.clicked() {
                                    chosen = Some(f);
                                }
                            }
                        });
                });
            if let Some(f) = chosen {
                self.filter = f;
                self.filter_open = false;
            }
        }
        rx = sel.left() - 10.0;

        // search
        let sr = Rect::from_min_size(Pos2::new(rx - 214.0, cy - 21.0), Vec2::new(214.0, 42.0));
        theme::inset(p, ui.painter(), sr, 14.0);
        let inner = sr.shrink2(Vec2::new(16.0, 0.0));
        let mut child = ui.child_ui(
            inner,
            egui::Layout::left_to_right(egui::Align::Center),
            None,
        );
        child.add(
            egui::TextEdit::singleline(&mut self.search)
                .hint_text("Search names")
                .desired_width(inner.width())
                .frame(false),
        );

        if !self.sel.is_empty() {
            let cmp = Rect::from_min_size(
                Pos2::new(seg.right() + 10.0, cy - 20.0),
                Vec2::new(128.0, 40.0),
            );
            if cmp.left() < bar.right() - 300.0
                && theme::button_rect(
                    ui,
                    p,
                    cmp,
                    &format!("Compare {}", self.sel.len()),
                    None,
                    theme::semibold(13.0),
                )
                .clicked()
            {
                self.start_compare();
            }
        }
    }

    fn swipe_view(&mut self, ui: &mut egui::Ui, p: &Pal, list: &[usize]) {
        if list.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(90.0);
                ui.label(
                    egui::RichText::new("Nothing in this collection")
                        .font(theme::regular(15.0))
                        .color(p.mu),
                );
            });
            return;
        }
        let pos = self
            .pv_idx
            .and_then(|i| list.iter().position(|x| *x == i))
            .unwrap_or(0);
        let idx = list[pos.min(list.len() - 1)];
        self.request_preview(ui.ctx(), idx);

        let screen_h = ui.ctx().screen_rect().height();
        let avail = ui.available_size();
        let ar = self
            .dims
            .get(idx)
            .copied()
            .flatten()
            .map(|(w, h)| w as f32 / h as f32)
            .unwrap_or(1.5);
        let max_w = (avail.x - 190.0).min(1000.0).max(200.0);
        let max_h = (screen_h * 0.6).min(avail.y - 356.0).max(160.0);
        let mut w = max_w;
        let mut h = w / ar;
        if h > max_h {
            h = max_h;
            w = h * ar;
        }
        let media = Vec2::new(w, h);
        let card = media + Vec2::splat(28.0);

        ui.horizontal(|ui| {
            let full = ui.available_width();
            ui.add_space(((full - card.x) / 2.0).max(0.0));
            let (crect, _) = ui.allocate_exact_size(card, Sense::hover());
            let painter = ui.painter();
            theme::raised(p, painter, crect, 38.0, true);
            painter.rect_filled(
                crect,
                egui::Rounding::same(38.0),
                theme::mix(p.bg, p.hi, if p.dark { 0.62 } else { 0.75 }),
            );
            let m = Rect::from_min_size(crect.min + Vec2::splat(14.0), media);
            painter.rect_filled(m, egui::Rounding::same(26.0), p.media);
            let clip = ui.painter().with_clip_rect(m);
            if let Some(t) = self.pv_tex.clone().or_else(|| self.tex.get(&idx).cloned()) {
                clip.image(
                    t.id(),
                    fit_rect(m, t.size_vec2()),
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            } else if self.fail.contains_key(&idx) {
                clip.text(
                    m.center(),
                    egui::Align2::CENTER_CENTER,
                    "Could not decode this file",
                    theme::regular(13.0),
                    Color32::from_rgb(0x9a, 0xa3, 0xb4),
                );
            }
            theme::guides(&clip, m, &self.db.settings.guides);
            // caption
            let cap_h = 78.0;
            let cap = Rect::from_min_size(
                Pos2::new(m.left(), m.bottom() - cap_h),
                Vec2::new(m.width(), cap_h),
            );
            let mut mesh = egui::Mesh::default();
            let clear = Color32::from_rgba_unmultiplied(0, 0, 0, 0);
            let dark = Color32::from_rgba_unmultiplied(0, 0, 0, 210);
            mesh.colored_vertex(cap.left_top(), clear);
            mesh.colored_vertex(cap.right_top(), clear);
            mesh.colored_vertex(cap.right_bottom(), dark);
            mesh.colored_vertex(cap.left_bottom(), dark);
            mesh.add_triangle(0, 1, 2);
            mesh.add_triangle(0, 2, 3);
            clip.add(egui::Shape::mesh(mesh));
            clip.text(
                Pos2::new(m.left() + 18.0, m.bottom() - 20.0),
                egui::Align2::LEFT_CENTER,
                &self.photos[idx].name,
                theme::regular(12.0),
                Color32::WHITE,
            );
            clip.text(
                Pos2::new(m.right() - 18.0, m.bottom() - 20.0),
                egui::Align2::RIGHT_CENTER,
                format!("{} of {}", pos + 1, list.len()),
                theme::regular(12.0),
                Color32::from_rgba_unmultiplied(255, 255, 255, 220),
            );
            theme::round_mask(
                &clip,
                m,
                26.0,
                theme::mix(p.bg, p.hi, if p.dark { 0.62 } else { 0.75 }),
            );
            // hover reject
            let hover_rect = ui.interact(crect, ui.id().with("cardhover"), Sense::click());
            if hover_rect.hovered() {
                let xr = Rect::from_center_size(
                    Pos2::new(m.right() - 18.0, m.top() + 18.0),
                    Vec2::splat(30.0),
                );
                let resp = ui.interact(xr, ui.id().with("kill"), Sense::click());
                ui.painter().rect_filled(
                    xr,
                    egui::Rounding::same(10.0),
                    if resp.hovered() {
                        p.bad
                    } else {
                        Color32::from_rgba_unmultiplied(0, 0, 0, 140)
                    },
                );
                theme::draw_icon(
                    ui.painter(),
                    xr.center(),
                    6.0,
                    Icon::X,
                    Color32::WHITE,
                    1.9,
                );
                if resp.clicked() {
                    self.move_to(idx, REJECTED);
                    self.pv_idx = None;
                }
            }
        });

        ui.add_space(34.0);
        let n = list.len();
        ui.horizontal(|ui| {
            let total_w = 82.0 * 7.0 + 20.0 * 6.0;
            ui.add_space(((ui.available_width() - total_w) / 2.0).max(0.0));
            if theme::round_button(ui, p, Icon::Reject, "Reject", p.bad).clicked() {
                self.move_to(idx, REJECTED);
                self.pv_idx = None;
            }
            if theme::round_button(ui, p, Icon::Later, "Later", p.ac).clicked() {
                self.move_to(idx, LATER);
                self.pv_idx = None;
            }
            if theme::round_button(ui, p, Icon::Skip, "Skip", p.mu).clicked() {
                self.next_in(list, idx);
            }
            if theme::round_button(ui, p, Icon::Keep, "Candidate", p.ok).clicked() {
                self.move_to(idx, CANDIDATES);
                self.pv_idx = None;
            }
            if theme::round_button(ui, p, Icon::Final, "Final", p.cool).clicked() {
                self.move_to(idx, FINAL);
                self.pv_idx = None;
            }
            if theme::round_button(ui, p, Icon::Compare, "Compare", p.ac).clicked() {
                self.sel = vec![idx];
                if pos + 1 < n {
                    self.sel.push(list[pos + 1]);
                }
                self.start_compare();
            }
            if theme::round_button(ui, p, Icon::Undo, "Undo", p.mu).clicked() {
                self.undo();
            }
        });

        ui.add_space(28.0);
        ui.horizontal(|ui| {
            let labels = [
                ("2", CANDIDATES),
                ("3", LATER),
                ("4", FINAL),
                ("5", REJECTED),
            ];
            let mut total_w = 60.0;
            for (k, c) in labels.iter() {
                total_w += 26.0 + k.len() as f32 * 7.0 + c.len() as f32 * 7.0;
            }
            ui.add_space(((ui.available_width() - total_w) / 2.0).max(0.0));
            ui.label(
                egui::RichText::new("Move to:")
                    .font(theme::regular(12.0))
                    .color(p.mu),
            );
            for (k, c) in labels.iter() {
                if theme::pill(ui, p, &format!("{k}  {c}"), false).clicked() {
                    self.move_to(idx, c);
                    self.pv_idx = None;
                }
            }
        });

        ui.add_space(22.0);
        ui.horizontal(|ui| {
            let keys = [
                ("left", "reject"),
                ("right", "candidate"),
                ("space", "later"),
                ("up", "final"),
                ("down", "skip"),
                ("z", "undo"),
                ("c", "compare"),
                ("/", "search"),
                (",", "settings"),
            ];
            let mut total_w = 0.0;
            for (k, v) in keys.iter() {
                total_w += k.len() as f32 * 6.4 + 20.0 + v.len() as f32 * 6.2 + 18.0;
            }
            ui.add_space(((ui.available_width() - total_w) / 2.0).max(0.0));
            for (k, v) in keys.iter() {
                theme::kbd(ui, p, k);
                ui.label(
                    egui::RichText::new(*v)
                        .font(theme::regular(12.0))
                        .color(p.mu),
                );
            }
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
        let gap = 22.0;
        let pad = 34.0;
        let mut clicked = None;
        let mut kill = None;
        let mut toggle = None;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(4.0);
                let width = ui.available_width() - pad * 2.0;
                let cols = (((width + gap) / (cell + gap)).floor() as usize).max(1);
                let rows = (list.len() + cols - 1) / cols;
                for r in 0..rows {
                    ui.horizontal(|ui| {
                        ui.add_space(pad);
                        for c in 0..cols {
                            let i = r * cols + c;
                            if i >= list.len() {
                                break;
                            }
                            let idx = list[i];
                            let (rect, resp) =
                                ui.allocate_exact_size(Vec2::splat(cell), Sense::click());
                            let painter = ui.painter();
                            theme::inset(p, painter, rect, 22.0);
                            let m = rect.shrink(8.0);
                            painter.rect_filled(m, egui::Rounding::same(15.0), p.media);
                            if let Some(t) = self.tex.get(&idx) {
                                let clip = painter.with_clip_rect(m);
                                clip.image(
                                    t.id(),
                                    cover_rect(m, t.size_vec2()),
                                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                    Color32::WHITE,
                                );
                                theme::round_mask(&clip, m, 15.0, p.bg);
                            }
                            if self.sel.contains(&idx) {
                                painter.rect_stroke(
                                    rect,
                                    egui::Rounding::same(22.0),
                                    Stroke::new(3.0, p.ac),
                                );
                            }
                            if resp.hovered() {
                                let xr = Rect::from_center_size(
                                    Pos2::new(rect.right() - 22.0, rect.top() + 22.0),
                                    Vec2::new(28.0, 26.0),
                                );
                                let kr = ui.interact(xr, ui.id().with(("kill", idx)), Sense::click());
                                theme::raised(p, ui.painter(), xr, 10.0, false);
                                ui.painter().rect_filled(
                                    xr,
                                    egui::Rounding::same(10.0),
                                    if kr.hovered() { p.bad } else { p.bg },
                                );
                                theme::draw_icon(
                                    ui.painter(),
                                    xr.center(),
                                    5.5,
                                    Icon::X,
                                    if kr.hovered() { Color32::WHITE } else { p.bad },
                                    1.8,
                                );
                                if kr.clicked() {
                                    kill = Some(idx);
                                }
                                let tr = Rect::from_center_size(
                                    Pos2::new(rect.left() + 26.0, rect.top() + 22.0),
                                    Vec2::splat(26.0),
                                );
                                let sr = ui.interact(tr, ui.id().with(("pick", idx)), Sense::click());
                                theme::raised(p, ui.painter(), tr, 13.0, false);
                                ui.painter().rect_filled(tr, egui::Rounding::same(13.0), p.bg);
                                theme::draw_icon(
                                    ui.painter(),
                                    tr.center(),
                                    5.0,
                                    Icon::Keep,
                                    if self.sel.contains(&idx) { p.ac } else { p.mu },
                                    1.8,
                                );
                                if sr.clicked() {
                                    toggle = Some(idx);
                                }
                            }
                            if resp.clicked() {
                                clicked = Some(idx);
                            }
                        }
                    });
                    ui.add_space(gap);
                }
                ui.add_space(20.0);
            });
        if let Some(i) = kill {
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
            self.compare = None;
        }
    }

    fn compare_view(&mut self, ui: &mut egui::Ui, p: &Pal) {
        if self.compare.is_none() {
            return;
        }
        let pad = 34.0;
        let sel = self.sel.clone();
        let mut decisions = self.compare.clone().unwrap_or([None, None]);
        ui.horizontal(|ui| {
            ui.add_space(pad);
            ui.label(
                egui::RichText::new("Compare")
                    .font(theme::semibold(24.0))
                    .color(p.tx),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(pad);
                if theme::button(ui, p, "Close", false).clicked() {
                    self.compare = None;
                    self.sel.clear();
                }
            });
        });
        ui.add_space(16.0);
        let avail = ui.available_size();
        let pane_w = ((avail.x - pad * 2.0 - 18.0) / 2.0).min(540.0);
        let shot_h = (ui.ctx().screen_rect().height() * 0.56).min(540.0);
        ui.horizontal(|ui| {
            ui.add_space(((avail.x - (pane_w * 2.0 + 18.0)) / 2.0).max(pad - 20.0));
            for slot in 0..2 {
                let idx = *sel.get(slot).unwrap_or(&usize::MAX);
                let (rect, _) = ui.allocate_exact_size(
                    Vec2::new(pane_w, shot_h + 78.0),
                    Sense::hover(),
                );
                let painter = ui.painter();
                theme::raised(p, painter, rect, 26.0, true);
                painter.rect_filled(rect, egui::Rounding::same(26.0), p.bg);
                let shot = Rect::from_min_size(
                    rect.min + Vec2::splat(12.0),
                    Vec2::new(rect.width() - 24.0, shot_h - 12.0 * 2.0),
                );
                painter.rect_filled(shot, egui::Rounding::same(18.0), p.media);
                if idx != usize::MAX {
                    if let Some(t) = self.tex.get(&idx) {
                        let clip = ui.painter().with_clip_rect(shot);
                        clip.image(
                            t.id(),
                            cover_rect(shot, t.size_vec2()),
                            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                            Color32::WHITE,
                        );
                        theme::round_mask(&clip, shot, 18.0, p.bg);
                    }
                    let cap = Rect::from_min_size(
                        Pos2::new(shot.left(), shot.bottom() - 44.0),
                        Vec2::new(shot.width(), 44.0),
                    );
                    let mut mesh = egui::Mesh::default();
                    mesh.colored_vertex(
                        cap.left_top(),
                        Color32::from_rgba_unmultiplied(0, 0, 0, 0),
                    );
                    mesh.colored_vertex(
                        cap.right_top(),
                        Color32::from_rgba_unmultiplied(0, 0, 0, 0),
                    );
                    mesh.colored_vertex(
                        cap.right_bottom(),
                        Color32::from_rgba_unmultiplied(0, 0, 0, 200),
                    );
                    mesh.colored_vertex(
                        cap.left_bottom(),
                        Color32::from_rgba_unmultiplied(0, 0, 0, 200),
                    );
                    mesh.add_triangle(0, 1, 2);
                    mesh.add_triangle(0, 2, 3);
                    ui.painter().with_clip_rect(shot).add(egui::Shape::mesh(mesh));
                    ui.painter().text(
                        Pos2::new(shot.left() + 14.0, shot.bottom() - 16.0),
                        egui::Align2::LEFT_CENTER,
                        self.photos[idx].name.clone(),
                        theme::regular(12.0),
                        Color32::WHITE,
                    );
                    if let Some(choice) = &decisions[slot] {
                        ui.painter().rect_filled(
                            shot,
                            egui::Rounding::same(18.0),
                            Color32::from_rgba_unmultiplied(0, 0, 0, 150),
                        );
                        ui.painter().text(
                            shot.center(),
                            egui::Align2::CENTER_CENTER,
                            choice,
                            theme::semibold(20.0),
                            coll_color(choice, p.dark),
                        );
                    }
                }
                if decisions[slot].is_none() && idx != usize::MAX {
                    let by = shot.bottom() + 16.0;
                    let bw = (shot.width() - 16.0) / 3.0;
                    let mut chosen: Option<&'static str> = None;
                    for (k, (label, coll, color)) in [
                        ("Keep", CANDIDATES, p.ok),
                        ("Later", LATER, p.ac),
                        ("Reject", REJECTED, p.bad),
                    ]
                    .iter()
                    .enumerate()
                    {
                        let b = Rect::from_min_size(
                            Pos2::new(shot.left() + k as f32 * (bw + 8.0), by),
                            Vec2::new(bw, 40.0),
                        );
                        let r = ui.interact(b, ui.id().with(("cmp", slot, k)), Sense::click());
                        theme::raised(p, ui.painter(), b, 12.0, false);
                        ui.painter().rect_filled(b, egui::Rounding::same(12.0), p.bg);
                        ui.painter().text(
                            b.center(),
                            egui::Align2::CENTER_CENTER,
                            *label,
                            theme::regular(12.5),
                            *color,
                        );
                        if r.clicked() {
                            chosen = Some(coll);
                        }
                    }
                    if let Some(coll) = chosen {
                        self.move_to(idx, coll);
                        decisions[slot] = Some(coll.to_string());
                        if let Some(c) = self.compare.as_mut() {
                            c[slot] = Some(coll.to_string());
                        }
                    }
                }
            }
        });
        if decisions.iter().all(|d| d.is_some()) {
            self.compare = None;
            self.sel.clear();
        }
    }

    fn empty_state(&mut self, ui: &mut egui::Ui, p: &Pal) {
        ui.vertical_centered(|ui| {
            ui.add_space((ui.available_height() * 0.16).max(30.0));
            ui.label(
                egui::RichText::new("Keepers")
                    .font(theme::bold(58.0))
                    .color(p.ac),
            );
            ui.add_space(10.0);
            ui.label(
                egui::RichText::new(
                    "Sort a whole trip into collections, then build your final cut.",
                )
                .font(theme::regular(15.0))
                .color(p.mu),
            );
            ui.label(
                egui::RichText::new("Files are read straight from your disk and never uploaded.")
                    .font(theme::regular(15.0))
                    .color(p.mu),
            );
            ui.add_space(26.0);
            ui.horizontal(|ui| {
                let total = 172.0 * 2.0 + 12.0;
                ui.add_space(((ui.available_width() - total) / 2.0).max(0.0));
                if theme::button(ui, p, "Choose folder", true).clicked() {
                    self.pick_folder(ui.ctx());
                }
                if theme::button(ui, p, "Choose files", false).clicked() {
                    self.pick_files(ui.ctx());
                }
            });
            ui.add_space(16.0);
            if theme::button(ui, p, "Restore a sort map", true).clicked() {
                self.import();
            }
            ui.add_space(20.0);
            ui.label(
                egui::RichText::new("or drop a folder anywhere in this window")
                    .font(theme::regular(12.0))
                    .color(p.mu),
            );
            if self.show_path_input {
                ui.add_space(10.0);
                let mut path = String::new();
                let r = ui.add(
                    egui::TextEdit::singleline(&mut path)
                        .hint_text("C:\\Users\\you\\Pictures")
                        .desired_width(360.0),
                );
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let dir = PathBuf::from(path.clone());
                    if dir.is_dir() {
                        self.show_path_input = false;
                        self.open_folder(ui.ctx(), dir);
                    } else {
                        self.toast("That path is not a folder");
                    }
                }
            }
        });
    }

    fn settings_window(&mut self, ctx: &egui::Context, p: &Pal) {
        if !self.settings_open {
            return;
        }
        let mut open = true;
        let mut theme_changed = false;
        let screen = ctx.screen_rect().size();
        egui::Window::new("settings")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .fixed_size(Vec2::new(470.0, (screen.y - 100.0).min(620.0)))
            .frame(
                egui::Frame::none()
                    .fill(p.bg)
                    .rounding(egui::Rounding::same(30.0))
                    .inner_margin(egui::Margin::same(24.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Settings")
                            .font(theme::semibold(22.0))
                            .color(p.tx),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if theme::icon_button(ui, p, Icon::X, p.bad, 34.0).clicked() {
                            open = false;
                        }
                    });
                });
                ui.add_space(16.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("Theme")
                                .font(theme::regular(12.0))
                                .color(p.mu),
                        );
                        ui.horizontal(|ui| {
                            for (t, label) in
                                [("auto", "Auto"), ("light", "Light"), ("dark", "Dark")]
                            {
                                let sel = self.db.settings.theme == t;
                                if theme::pill(ui, p, label, sel).clicked() {
                                    self.db.settings.theme = t.into();
                                    theme_changed = true;
                                }
                            }
                        });
                        ui.add_space(14.0);
                        ui.label(
                            egui::RichText::new("Thumbnail size")
                                .font(theme::regular(12.0))
                                .color(p.mu),
                        );
                        ui.add(
                            egui::Slider::new(&mut self.db.settings.thumb, 140..=320)
                                .suffix(" px"),
                        );
                        ui.add_space(14.0);
                        ui.label(
                            egui::RichText::new("Guide lines")
                                .font(theme::regular(12.0))
                                .color(p.mu),
                        );
                        ui.horizontal_wrapped(|ui| {
                            for (k, label) in [
                                ("off", "Off"),
                                ("thirds", "Thirds"),
                                ("golden", "Golden"),
                                ("cross", "Cross"),
                                ("diag", "Diagonals"),
                            ] {
                                let sel = self.db.settings.guides == k;
                                if theme::pill(ui, p, label, sel).clicked() {
                                    self.db.settings.guides = k.into();
                                }
                            }
                        });
                        ui.add_space(14.0);
                        ui.label(
                            egui::RichText::new("Preview size")
                                .font(theme::regular(12.0))
                                .color(p.mu),
                        );
                        ui.horizontal(|ui| {
                            for v in [1200u32, 1600, 2048, 0] {
                                let sel = self.db.settings.preview == v;
                                let label = if v == 0 {
                                    "Full".to_string()
                                } else {
                                    format!("{v} px")
                                };
                                if theme::pill(ui, p, &label, sel).clicked() {
                                    self.db.settings.preview = v;
                                    self.pv_idx = None;
                                    self.pv_cache.clear();
                                }
                            }
                        });
                        ui.add_space(18.0);
                        ui.separator();
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "Tile cache: {} in {} files",
                                human_mb(self.cache_bytes),
                                self.cache_files
                            ))
                            .font(theme::regular(13.0))
                            .color(p.tx),
                        );
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if theme::button(ui, p, "Refresh", false).clicked() {
                                let (b, n) = decode::cache_bytes();
                                self.cache_bytes = b;
                                self.cache_files = n;
                            }
                            if theme::button(ui, p, "Clear", false).clicked() {
                                let freed = decode::clear_cache();
                                let (b, n) = decode::cache_bytes();
                                self.cache_bytes = b;
                                self.cache_files = n;
                                self.toast(format!("Freed {}", human_mb(freed)));
                            }
                        });
                        ui.add_space(10.0);
                        ui.label(
                            egui::RichText::new(decode::codec_note())
                                .font(theme::regular(12.0))
                                .color(p.mu),
                        );
                        ui.add_space(16.0);
                        ui.separator();
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if theme::button(ui, p, "Export sort map", false).clicked() {
                                self.export();
                            }
                            if theme::button(ui, p, "Restore sort map", false).clicked() {
                                self.import();
                            }
                        });
                        ui.add_space(12.0);
                        ui.label(
                            egui::RichText::new(
                                "Keys: left reject, right candidate, space later, up final, down skip, z undo, c compare, / search, , settings",
                            )
                            .font(theme::regular(11.5))
                            .color(p.mu),
                        );
                    });
            });
        self.settings_open = open;
        if theme_changed {
            theme::apply(ctx, self.dark());
                self.dots = None;
            self.db.save();
        }
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
        let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("toast"));
        let painter = ctx.layer_painter(layer);
        let screen = ctx.screen_rect();
        let galley = painter.layout_no_wrap(msg, theme::regular(13.5), p.tx);
        let size = galley.size() + Vec2::new(32.0, 22.0);
        let rect = Rect::from_center_size(
            Pos2::new(screen.center().x, screen.bottom() - 40.0),
            size,
        );
        theme::raised(p, &painter, rect, size.y / 2.0, true);
        painter.rect_filled(rect, egui::Rounding::same(size.y / 2.0), p.bg);
        painter.galley(
            rect.center() - galley.size() / 2.0,
            galley,
            p.tx,
        );
    }

    // ---------- input ----------

    fn handle_input(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }
        let list = self.photos_in(&self.coll.clone());
        let cur = self
            .pv_idx
            .filter(|i| list.contains(i))
            .or_else(|| list.first().copied());
        let (left, right, space, up, down, undo, cmp, search, settings, esc) = ctx.input(|i| {
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
                i.key_pressed(egui::Key::Escape),
            )
        });
        if settings {
            self.settings_open = !self.settings_open;
        }
        if esc {
            self.compare = None;
            self.settings_open = false;
            self.filter_open = false;
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
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.frame_inner(ctx);
    }
}

impl App {
    pub fn frame(&mut self, ctx: &egui::Context) {
        self.frame_inner(ctx);
    }
}

fn fit_rect(outer: Rect, size: Vec2) -> Rect {
    if size.x <= 0.0 || size.y <= 0.0 {
        return outer;
    }
    let s = (outer.width() / size.x).min(outer.height() / size.y);
    Rect::from_center_size(outer.center(), size * s)
}

fn cover_rect(outer: Rect, size: Vec2) -> Rect {
    if size.x <= 0.0 || size.y <= 0.0 {
        return outer;
    }
    let s = (outer.width() / size.x).max(outer.height() / size.y);
    Rect::from_center_size(outer.center(), size * s)
}

fn grad_specs(dark: bool) -> [((f32, f32, f32, f32), Color32, u8); 3] {
    if dark {
        [
            ((0.05, -0.12, 1500.0, 1150.0), Color32::from_rgb(0x3d, 0x46, 0x56), 0xa8),
            ((1.02, -0.02, 1300.0, 1000.0), Color32::from_rgb(0x6a, 0x3a, 0x24), 0x96),
            ((0.46, 1.22, 1250.0, 1100.0), Color32::from_rgb(0x2c, 0x37, 0x6c), 0x9c),
        ]
    } else {
        [
            ((0.06, -0.10, 1600.0, 1200.0), Color32::from_rgb(0xff, 0xff, 0xff), 0xc8),
            ((1.02, 0.02, 1250.0, 950.0), Color32::from_rgb(0xff, 0xd8, 0xc4), 0xae),
            ((0.46, 1.22, 1250.0, 1150.0), Color32::from_rgb(0xcc, 0xd7, 0xff), 0xa6),
        ]
    }
}

/// The whole page wash in one image: three soft blobs over the base colour.
fn page_image(w: usize, h: usize, dark: bool) -> egui::ColorImage {
    let specs = grad_specs(dark);
    let mut img = egui::ColorImage::new([w, h], Color32::TRANSPARENT);
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0.0f32; 3];
            for ((fx, fy, rx, ry), color, alpha) in specs.iter() {
                let cx = w as f32 * fx;
                let cy = h as f32 * fy;
                let dx = (x as f32 + 0.5 - cx) / (rx * 0.5);
                let dy = (y as f32 + 0.5 - cy) / (ry * 0.5);
                let r = (dx * dx + dy * dy).sqrt();
                let f = (1.0 - r / 1.15).clamp(0.0, 1.0).powf(1.6);
                let a = (*alpha as f32 / 255.0) * f * 0.62;
                if a <= 0.0 {
                    continue;
                }
                for c in 0..3 {
                    let src = [color.r(), color.g(), color.b()][c] as f32;
                    acc[c] = src * a + acc[c] * (1.0 - a);
                }
            }
            let mut hsh: u32 =
                (x as u32).wrapping_mul(73856093) ^ (y as u32).wrapping_mul(19349663);
            hsh ^= hsh >> 13;
            let noise = (((hsh >> 16) & 7) as f32 - 3.5) * 0.8;
            let px = Color32::from_rgba_unmultiplied(
                (acc[0] + noise).clamp(0.0, 255.0) as u8,
                (acc[1] + noise).clamp(0.0, 255.0) as u8,
                (acc[2] + noise).clamp(0.0, 255.0) as u8,
                if acc[0] + acc[1] + acc[2] > 0.5 { 255 } else { 0 },
            );
            img.pixels[y * w + x] = px;
        }
    }
    img
}

fn dot_texture(ctx: &egui::Context) -> egui::TextureHandle {
    let n = 24usize;
    let mut img = egui::ColorImage::new([n, n], Color32::TRANSPARENT);
    let dot = Color32::from_rgba_unmultiplied(0x46, 0x56, 0x78, 0x4a);
    for (dx, dy) in [(0i32, 0i32), (1, 0), (0, 1), (1, 1)] {
        let x = 11 + dx;
        let y = 11 + dy;
        img.pixels[(y as usize) * n + x as usize] = dot;
    }
    let opts = egui::TextureOptions {
        magnification: egui::TextureFilter::Linear,
        minification: egui::TextureFilter::Linear,
        wrap_mode: egui::TextureWrapMode::Repeat,
        mipmap_mode: None,
    };
    ctx.load_texture("dots", img, opts)
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
            let _ = std::fs::copy(e.path(), dir.join(e.file_name()));
        }
    }

    fn make_images(dir: &std::path::Path, n: u32, w: u32, h: u32) {
        for i in 0..n {
            let mut img = image::RgbImage::new(w, h);
            for (x, y, px) in img.enumerate_pixels_mut() {
                *px = image::Rgb([
                    ((x * 255) / w) as u8,
                    ((y * 255) / h) as u8,
                    (i * 37) as u8,
                ]);
            }
            img.save(dir.join(format!("IMG_{:04}.JPG", 2140 + i))).unwrap();
        }
    }

    fn input(events_vec: Vec<Event>) -> RawInput {
        RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1440.0, 900.0))),
            events: events_vec,
            ..Default::default()
        }
    }

    fn pump(app: &mut App, ctx: &egui::Context, frames: usize, sleep_ms: u64) {
        for _ in 0..frames {
            let _ = ctx.run(input(vec![]), |c| app.frame(c));
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
        assert_eq!(app.photos.len(), 6);

        let mut spins = 0;
        while app.thumbs.done < app.thumbs.total && spins < 500 {
            pump(&mut app, &ctx, 1, 25);
            spins += 1;
        }
        assert_eq!(app.thumbs.ok, 6);
        assert_eq!(app.tex.len(), 6);
        assert_eq!(app.dims[0], Some((480, 320)));
        assert!(app.toast.as_ref().unwrap().0.contains("Cached 6 of 6"));

        app.pv_idx = Some(0);
        pump(&mut app, &ctx, 1, 0);
        let mut spins = 0;
        while app.pv_tex.is_none() && spins < 300 {
            pump(&mut app, &ctx, 1, 20);
            spins += 1;
        }
        assert!(app.pv_tex.is_some());

        app.pv_idx = Some(0);
        let _ = ctx.run(input(key(Key::ArrowRight)), |c| app.frame(c));
        assert_eq!(app.assign[0], CANDIDATES);
        let _ = ctx.run(input(key(Key::Z)), |c| app.frame(c));
        assert_eq!(app.assign[0], INBOX);

        app.move_to(1, FINAL);
        app.move_to(2, REJECTED);
        let reloaded = Db::load();
        let fm = reloaded.folders.get(&folder_key(&pics)).expect("folder");
        assert_eq!(fm.assign.get("IMG_2141.JPG").map(String::as_str), Some(FINAL));
        assert_eq!(
            fm.assign.get("IMG_2142.JPG").map(String::as_str),
            Some(REJECTED)
        );

        app.export();
        let text = std::fs::read_to_string(pics.join("keepers-sortmap.json")).unwrap();
        let parsed: std::collections::BTreeMap<String, String> =
            serde_json::from_str(&text).unwrap();
        assert_eq!(parsed.get("IMG_2141.JPG").map(String::as_str), Some(FINAL));
        app.assign.iter_mut().for_each(|c| *c = INBOX.into());
        app.import();
        assert_eq!(app.assign[2], REJECTED);

        app.sel = vec![0, 1];
        app.start_compare();
        assert!(app.compare.is_some());
        pump(&mut app, &ctx, 2, 0);
        if let Some(c) = app.compare.as_mut() {
            c[0] = Some(CANDIDATES.to_string());
            c[1] = Some(REJECTED.to_string());
        }
        pump(&mut app, &ctx, 2, 0);
        assert!(app.compare.is_none());

        app.filter = Filter::Videos;
        assert!(app.photos_in(INBOX).is_empty());
        app.filter = Filter::All;
        app.search = "2143".into();
        assert_eq!(app.photos_in(INBOX).len(), 1);
        app.search.clear();

        for v in [View::Swipe, View::Grid] {
            app.view = v;
            pump(&mut app, &ctx, 2, 0);
        }
        app.settings_open = true;
        pump(&mut app, &ctx, 2, 0);

        let (_, files) = decode::cache_bytes();
        assert!(files >= 6);
        assert!(decode::clear_cache() > 0);
        assert_eq!(decode::cache_bytes().1, 0);
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
        assert!(t1.elapsed() < cold);
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
        let mut app = App::bare();
        app.init(&ctx);
        ctx.style_mut(|s| s.animation_time = 0.0);
        let mut shot = crate::shot::Shot::new(2880, 1800, 2.0);
        app.open_folder(&ctx, pics.clone());
        let mut spins = 0;
        while app.thumbs.done < app.thumbs.total && spins < 600 {
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
        while app.pv_tex.is_none() && spins < 300 {
            shot.pump(&mut app, &ctx, 1, 15);
            spins += 1;
        }
        app.toast = Some(("Cached 8 of 8 photos for offline use".into(), 40.0));
        shot.pump(&mut app, &ctx, 3, 10);
        if std::env::var("KEEPERS_SHOT_DEBUG").is_ok() {
            eprintln!("TEX {:?}", shot.store.tex_info());
            for pt in [(1300.0, 60.0), (1240.0, 250.0), (700.0, 200.0)] {
                eprintln!("HIT {:?} => {:?}", pt, shot.hits(pt.0, pt.1));
            }
            eprintln!("DOTS {:?}", app.dots.as_ref().map(|t| (t.id(), t.size())));
        }
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
        shot.pump(&mut app, &ctx, 4, 10);
        shot.save(&out.join("settings.png"));

        app.settings_open = false;
        app.photos.clear();
        app.assign.clear();
        app.folder = None;
        shot.pump(&mut app, &ctx, 3, 10);
        shot.save(&out.join("empty.png"));
    }
}
