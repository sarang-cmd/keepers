use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub struct Frame {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

pub fn cache_dir() -> PathBuf {
    let p = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Keepers")
        .join("thumbs");
    let _ = std::fs::create_dir_all(&p);
    p
}

fn key(path: &Path, max: u32) -> String {
    let meta = std::fs::metadata(path).ok();
    let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
    let mtime = meta
        .as_ref()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut h: u64 = 1469598103934665603;
    for b in path.to_string_lossy().to_lowercase().bytes() {
        h = (h ^ b as u64).wrapping_mul(1099511628211);
    }
    for b in size.to_le_bytes().iter().chain(mtime.to_le_bytes().iter()) {
        h = (h ^ *b as u64).wrapping_mul(1099511628211);
    }
    h = (h ^ max as u64).wrapping_mul(1099511628211);
    format!("{:016x}", h)
}

/// Decode a file, longest side capped at `max` pixels. Disk cache first.
pub fn load(path: &Path, max: u32) -> Result<Frame, String> {
    let cf = cache_dir().join(format!("{}.kfc", key(path, max)));
    if let Ok(bytes) = std::fs::read(&cf) {
        if bytes.len() > 12 {
            let w = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            let h = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
            if w > 0 && h > 0 && bytes.len() >= 12 + (w as usize * h as usize * 4) {
                return Ok(Frame {
                    w,
                    h,
                    rgba: bytes[12..12 + w as usize * h as usize * 4].to_vec(),
                });
            }
        }
    }
    let f = decode(path, max)?;
    save_cache(&cf, &f);
    Ok(f)
}

/// Decode only, no cache read. Used by the preview path.
pub fn decode(path: &Path, max: u32) -> Result<Frame, String> {
    imp::decode(path, max)
}

fn save_cache(cf: &Path, f: &Frame) {
    let mut buf = Vec::with_capacity(12 + f.rgba.len());
    buf.extend_from_slice(&f.w.to_le_bytes());
    buf.extend_from_slice(&f.h.to_le_bytes());
    buf.extend_from_slice(&[0u8; 4]);
    buf.extend_from_slice(&f.rgba);
    let _ = std::fs::write(cf, buf);
}

pub fn cache_bytes() -> (u64, u64) {
    let mut total = 0u64;
    let mut n = 0u64;
    if let Ok(rd) = std::fs::read_dir(cache_dir()) {
        for e in rd.flatten() {
            if let Ok(m) = e.metadata() {
                total += m.len();
                n += 1;
            }
        }
    }
    (total, n)
}

pub fn clear_cache() -> u64 {
    let mut freed = 0u64;
    if let Ok(rd) = std::fs::read_dir(cache_dir()) {
        for e in rd.flatten() {
            if let Ok(m) = e.metadata() {
                freed += m.len();
            }
            let _ = std::fs::remove_file(e.path());
        }
    }
    freed
}

pub fn fit(w: u32, h: u32, max: u32) -> (u32, u32) {
    if w == 0 || h == 0 {
        return (1, 1);
    }
    let longest = w.max(h);
    if max == 0 || longest <= max {
        return (w, h);
    }
    let s = max as f32 / longest as f32;
    (
        ((w as f32 * s).round() as u32).max(1),
        ((h as f32 * s).round() as u32).max(1),
    )
}

pub static CODEC_NOTE: OnceLock<String> = OnceLock::new();

pub fn codec_note() -> String {
    CODEC_NOTE
        .get_or_init(|| {
            if cfg!(windows) {
                "Windows Imaging Component, HEIC comes from the HEIF extension".into()
            } else {
                "image crate decoders, no HEIC on this build".into()
            }
        })
        .clone()
}

#[cfg(windows)]
mod imp {
    use super::{fit, Frame};
    use std::path::Path;
    use windows::core::HSTRING;
    use windows::Win32::Foundation::GENERIC_READ;
    use windows::Win32::Graphics::Imaging::*;
    use windows::Win32::System::Com::*;

    thread_local! {
        static FACTORY: std::cell::RefCell<Option<IWICImagingFactory>> = std::cell::RefCell::new(None);
    }

    fn factory() -> Result<IWICImagingFactory, String> {
        FACTORY.with(|f| {
            if let Some(fac) = f.borrow().as_ref() {
                return Ok(fac.clone());
            }
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                let fac: IWICImagingFactory = CoCreateInstance(
                    &CLSID_WICImagingFactory,
                    None,
                    CLSCTX_INPROC_SERVER,
                )
                .map_err(|e| format!("wic factory: {e}"))?;
                *f.borrow_mut() = Some(fac.clone());
                Ok(fac)
            }
        })
    }

    fn generic_decode(path: &Path, max: u32) -> Result<Frame, String> {
        let fac = factory()?;
        unsafe {
            let wide = HSTRING::from(path.as_os_str());
            let dec = fac
                .CreateDecoderFromFilename(&wide, None, GENERIC_READ, WICDecodeMetadataCacheOnDemand)
                .map_err(|e| format!("open: {e}"))?;
            let frame = dec.GetFrame(0).map_err(|e| format!("frame: {e}"))?;
            let (mut w, mut h) = (0u32, 0u32);
            frame
                .GetSize(&mut w, &mut h)
                .map_err(|e| format!("size: {e}"))?;
            let (tw, th) = fit(w, h, max);
            let scaler = fac.CreateBitmapScaler().map_err(|e| format!("scaler: {e}"))?;
            scaler
                .Initialize(&frame, tw, th, WICBitmapInterpolationModeFant)
                .map_err(|e| format!("scale: {e}"))?;
            let conv = fac
                .CreateFormatConverter()
                .map_err(|e| format!("converter: {e}"))?;
            conv.Initialize(
                &scaler,
                &GUID_WICPixelFormat32bppRGBA,
                WICBitmapDitherTypeNone,
                None,
                0.0,
                WICBitmapPaletteTypeCustom,
            )
            .map_err(|e| format!("format: {e}"))?;
            let mut buf = vec![0u8; tw as usize * th as usize * 4];
            conv.CopyPixels(std::ptr::null(), tw * 4, &mut buf)
                .map_err(|e| format!("pixels: {e}"))?;
            let rot = orientation(&frame);
            let f = if rot != 1 {
                rotate(Frame { w: tw, h: th, rgba: buf }, rot)
            } else {
                Frame { w: tw, h: th, rgba: buf }
            };
            Ok(f)
        }
    }

    unsafe fn orientation(frame: &IWICBitmapFrameDecode) -> u16 {
        let r = frame.GetMetadataQueryReader();
        let Ok(r) = r else { return 1 };
        for name in [
            "/ifd/orientation",
            "/app1/ifd/{ushort=274}",
            "System.Photo.Orientation",
        ] {
            let key = HSTRING::from(name);
            let mut v = windows::core::PROPVARIANT::default();
            if r.GetMetadataByName(&key, &mut v).is_err() {
                continue;
            }
            // A VT_UI2 variant keeps its value 8 bytes into the struct.
            let raw = &v as *const _ as *const u16;
            if unsafe { *raw } == 18 {
                let val = unsafe { *raw.add(4) };
                if (1..=8).contains(&val) {
                    return val;
                }
            }
        }
        1
    }

    fn rotate(f: Frame, o: u16) -> Frame {
        if o <= 1 {
            return f;
        }
        let (w, h) = (f.w as usize, f.h as usize);
        let (nw, nh) = if o >= 5 { (h, w) } else { (w, h) };
        let mut out = vec![0u8; nw * nh * 4];
        for y in 0..h {
            for x in 0..w {
                let (nx, ny) = match o {
                    2 => (w - 1 - x, y),
                    3 => (w - 1 - x, h - 1 - y),
                    4 => (x, h - 1 - y),
                    5 => (y, x),
                    6 => (h - 1 - y, x),
                    7 => (h - 1 - y, w - 1 - x),
                    _ => (y, w - 1 - x),
                };
                let si = (y * w + x) * 4;
                let di = (ny * nw + nx) * 4;
                out[di..di + 4].copy_from_slice(&f.rgba[si..si + 4]);
            }
        }
        Frame {
            w: nw as u32,
            h: nh as u32,
            rgba: out,
        }
    }

    pub fn decode(path: &Path, max: u32) -> Result<Frame, String> {
        generic_decode(path, max)
    }
}

#[cfg(not(windows))]
mod imp {
    use super::{fit, Frame};
    use std::path::Path;

    pub fn probe() -> bool {
        true
    }

    pub fn decode(path: &Path, max: u32) -> Result<Frame, String> {
        let img = image::open(path).map_err(|e| format!("{e}"))?;
        let (w, h) = (img.width(), img.height());
        let (tw, th) = fit(w, h, max);
        let img = if (tw, th) != (w, h) {
            img.resize_exact(tw, th, image::imageops::FilterType::Triangle)
        } else {
            img
        };
        let rgba = img.to_rgba8().into_raw();
        Ok(Frame {
            w: tw,
            h: th,
            rgba,
        })
    }
}
