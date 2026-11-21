use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const INBOX: &str = "Inbox";
pub const CANDIDATES: &str = "Candidates";
pub const LATER: &str = "Save for later";
pub const FINAL: &str = "Final cut";
pub const REJECTED: &str = "Rejected";
pub const DEFAULTS: [&str; 5] = [INBOX, CANDIDATES, LATER, FINAL, REJECTED];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Photo,
    Video,
}

#[derive(Clone)]
pub struct Photo {
    pub path: PathBuf,
    pub name: String,
    pub kind: Kind,
    pub bytes: u64,
}

impl Photo {
    pub fn title(&self) -> &str {
        &self.name
    }
}

pub fn is_video(path: &Path) -> bool {
    matches!(
        ext(path).as_str(),
        "mov" | "mp4" | "m4v" | "avi" | "mkv" | "webm"
    )
}

pub fn is_photo(path: &Path) -> bool {
    matches!(
        ext(path).as_str(),
        "heic" | "heif" | "hif" | "jpg" | "jpeg" | "png" | "webp" | "bmp" | "tif" | "tiff" | "avif"
    )
}

fn ext(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

pub fn scan(dir: &Path) -> Vec<Photo> {
    let mut out = Vec::new();
    let rd = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return out,
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let kind = if is_photo(&path) {
            Kind::Photo
        } else if is_video(&path) {
            Kind::Video
        } else {
            continue;
        };
        let bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
        out.push(Photo {
            path,
            name,
            kind,
            bytes,
        });
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

pub fn folder_key(dir: &Path) -> String {
    dir.to_string_lossy().to_lowercase()
}

#[derive(Serialize, Deserialize, Clone)]
pub struct FolderMap {
    #[serde(default)]
    pub assign: BTreeMap<String, String>,
    #[serde(default)]
    pub custom: Vec<String>,
}

impl Default for FolderMap {
    fn default() -> Self {
        Self {
            assign: BTreeMap::new(),
            custom: Vec::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Settings {
    #[serde(default = "d_theme")]
    pub theme: String,
    #[serde(default = "d_thumb")]
    pub thumb: u32,
    #[serde(default = "d_guides")]
    pub guides: String,
    #[serde(default = "d_preview")]
    pub preview: u32,
}

fn d_theme() -> String {
    "dark".into()
}
fn d_thumb() -> u32 {
    240
}
fn d_guides() -> String {
    "thirds".into()
}
fn d_preview() -> u32 {
    1600
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: d_theme(),
            thumb: d_thumb(),
            guides: d_guides(),
            preview: d_preview(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Db {
    #[serde(default)]
    pub settings: Settings,
    #[serde(default)]
    pub folders: BTreeMap<String, FolderMap>,
}

impl Db {
    pub fn path() -> PathBuf {
        if let Ok(dir) = std::env::var("KEEPERS_DATA") {
            return PathBuf::from(dir).join("keepers.json");
        }
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Keepers")
            .join("keepers.json")
    }

    pub fn load() -> Self {
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let p = Self::path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let tmp = p.with_extension("json.tmp");
            if std::fs::write(&tmp, s).is_ok() {
                let _ = std::fs::rename(&tmp, &p);
            }
        }
    }
}

pub fn coll_color(name: &str, dark: bool) -> egui::Color32 {
    let base = match name {
        INBOX => egui::Color32::from_rgb(122, 138, 160),
        CANDIDATES => egui::Color32::from_rgb(74, 222, 128),
        LATER => egui::Color32::from_rgb(234, 179, 8),
        FINAL => egui::Color32::from_rgb(167, 139, 250),
        REJECTED => egui::Color32::from_rgb(248, 113, 113),
        _ => {
            let mut h: u64 = 1469598103934665603;
            for b in name.bytes() {
                h = (h ^ b as u64).wrapping_mul(1099511628211);
            }
            let hue = (h % 360) as f32 / 360.0;
            let (r, g, b) = hsv(hue, 0.55, 0.95);
            egui::Color32::from_rgb(r, g, b)
        }
    };
    if dark {
        base
    } else {
        let f = |c: u8| (c as f32 * 0.78) as u8;
        egui::Color32::from_rgb(f(base.r()), f(base.g()), f(base.b()))
    }
}

fn hsv(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let i = (h * 6.0).floor() as i32;
    let f = h * 6.0 - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match i.rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

pub fn human_mb(bytes: u64) -> String {
    let mb = bytes as f64 / (1024.0 * 1024.0);
    if mb < 10.0 {
        format!("{:.1} MB", mb)
    } else {
        format!("{:.0} MB", mb)
    }
}
