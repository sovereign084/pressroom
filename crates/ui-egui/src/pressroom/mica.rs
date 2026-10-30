//! Windows 11's Mica, imitated: the title bar and the band round the ribbon take on a faint wash
//! of the desktop wallpaper's colours, as Office's do on Windows 11, without the window itself
//! turning see-through. The wallpaper is read once, shrunk and heavily blurred on a background
//! thread; each frame lays the part of it behind the window over the bar, recoloured to keep the
//! bar's own brightness (so text on it reads exactly as before) and showing only the wallpaper's
//! hue. Nothing changes on Windows 10, with Windows' transparency effects turned off, on the
//! Colorful theme (Office's teal bar has no Mica either), in an inactive window, or when the
//! wallpaper can't be read.

use std::sync::{Mutex, PoisonError};

use egui::{Color32, ColorImage, Context, Painter, Rect, TextureHandle, TextureOptions, pos2};

/// How strongly the wallpaper's colour shows through: the share of each pixel's colour (its
/// distance from grey) that is kept.
const CHROMA: f32 = 0.32;

enum Wall {
    NotStarted,
    Loading,
    /// The blurred wallpaper, or `None` where Mica doesn't apply.
    Ready(Option<ColorImage>),
}

static WALL: Mutex<Wall> = Mutex::new(Wall::NotStarted);

/// Textures already recoloured for a bar colour, kept per colour (the title bar and the ribbon
/// band can differ).
#[derive(Clone, Default)]
struct Washes(Vec<(Color32, TextureHandle)>);

/// Lay the Mica wash over `rect` (in window points), whose plain colour is `base`.
pub fn paint(ctx: &Context, painter: &Painter, rect: Rect, base: Color32) {
    if cfg!(test) || super::pal().colorful || !ctx.input(|i| i.viewport().focused.unwrap_or(true)) {
        return;
    }
    let Some(tex) = wash(ctx, base) else {
        return;
    };
    let (window, monitor) = ctx.input(|i| (i.viewport().inner_rect, i.viewport().monitor_size));
    let (Some(window), Some(monitor)) = (window, monitor) else {
        return;
    };
    if monitor.x < 1.0 || monitor.y < 1.0 {
        return;
    }
    // Windows fills the screen with the wallpaper, cropping it to the screen's shape around its
    // centre.
    let [w, h] = tex.size();
    let (iw, ih) = (w.max(1) as f32, h.max(1) as f32);
    let (shown_w, shown_h) =
        if iw / ih > monitor.x / monitor.y { (monitor.x / monitor.y * ih / iw, 1.0) } else { (1.0, monitor.y / monitor.x * iw / ih) };
    let (off_u, off_v) = ((1.0 - shown_w) / 2.0, (1.0 - shown_h) / 2.0);
    // Where the rect sits on the screen (a window on a second screen wraps onto the same picture).
    let x0 = (window.min.x + rect.min.x).rem_euclid(monitor.x);
    let y0 = (window.min.y + rect.min.y).rem_euclid(monitor.y);
    let u = |x: f32| (off_u + (x / monitor.x) * shown_w).clamp(0.0, 1.0);
    let v = |y: f32| (off_v + (y / monitor.y) * shown_h).clamp(0.0, 1.0);
    let uv = Rect::from_min_max(pos2(u(x0), v(y0)), pos2(u(x0 + rect.width()), v(y0 + rect.height())));
    painter.image(tex.id(), rect, uv, Color32::WHITE);
}

/// The wallpaper recoloured for a bar of colour `base`, once it has loaded.
fn wash(ctx: &Context, base: Color32) -> Option<TextureHandle> {
    let id = egui::Id::unique("pr_mica_washes");
    let mut washes = ctx.data(|d| d.get_temp::<Washes>(id)).unwrap_or_default();
    if let Some((_, t)) = washes.0.iter().find(|(c, _)| *c == base) {
        return Some(t.clone());
    }
    let wall = {
        let mut guard = WALL.lock().unwrap_or_else(PoisonError::into_inner);
        match &*guard {
            Wall::NotStarted => {
                *guard = Wall::Loading;
                start(ctx);
                return None;
            }
            Wall::Loading => return None,
            Wall::Ready(None) => return None,
            Wall::Ready(Some(img)) => img.clone(),
        }
    };
    let tex = ctx.load_texture(format!("pr_mica_{:?}", base.to_array()), recolour(&wall, base), TextureOptions::LINEAR);
    washes.0.push((base, tex.clone()));
    ctx.data_mut(|d| d.insert_temp(id, washes));
    Some(tex)
}

/// `img` with every pixel moved to `base`'s brightness, keeping a share of its own colour.
fn recolour(img: &ColorImage, base: Color32) -> ColorImage {
    let b = [base.r() as f32, base.g() as f32, base.b() as f32];
    let pixels = img
        .pixels
        .iter()
        .map(|p| {
            let c = [p.r() as f32, p.g() as f32, p.b() as f32];
            let grey = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
            let ch = |i: usize| (b.get(i).copied().unwrap_or(0.0) + (c.get(i).copied().unwrap_or(0.0) - grey) * CHROMA).clamp(0.0, 255.0) as u8;
            Color32::from_rgb(ch(0), ch(1), ch(2))
        })
        .collect();
    ColorImage::new(img.size, pixels)
}

#[cfg(windows)]
fn start(ctx: &Context) {
    let ctx = ctx.clone();
    let spawned = std::thread::Builder::new().name("pr-mica".into()).spawn(move || {
        let img = load();
        *WALL.lock().unwrap_or_else(PoisonError::into_inner) = Wall::Ready(img);
        ctx.request_repaint();
    });
    if spawned.is_err() {
        *WALL.lock().unwrap_or_else(PoisonError::into_inner) = Wall::Ready(None);
    }
}

#[cfg(not(windows))]
fn start(_ctx: &Context) {
    *WALL.lock().unwrap_or_else(PoisonError::into_inner) = Wall::Ready(None);
}

/// The desktop wallpaper, shrunk and blurred, where Windows would draw Mica.
#[cfg(windows)]
fn load() -> Option<ColorImage> {
    // Mica arrived with Windows 11 (build 22000) and is off with the transparency effects.
    let build = reg("HKLM\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion", "CurrentBuildNumber")?;
    if build.trim().parse::<u32>().ok()? < 22000 {
        return None;
    }
    if reg("HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize", "EnableTransparency").is_some_and(|v| v.trim() == "0x0") {
        return None;
    }
    // No picture: the desktop is one colour.
    let picture = reg("HKCU\\Control Panel\\Desktop", "WallPaper").is_some_and(|v| !v.trim().is_empty());
    if !picture {
        let rgb: Vec<u8> = reg("HKCU\\Control Panel\\Colors", "Background")?.split_whitespace().filter_map(|n| n.parse().ok()).collect();
        let [r, g, b] = rgb.as_slice() else {
            return None;
        };
        return Some(ColorImage::new([2, 2], vec![Color32::from_rgb(*r, *g, *b); 4]));
    }
    let path = std::path::Path::new(&std::env::var_os("APPDATA")?).join("Microsoft\\Windows\\Themes\\TranscodedWallpaper");
    let bytes = std::fs::read(path).ok()?;
    let pm = designcraft_render::decode_pixmap(&bytes)?;
    let (w, h) = (usize::from(pm.width()), usize::from(pm.height()));
    let px: Vec<[u8; 3]> = pm.data().iter().map(|p| [p.r, p.g, p.b]).collect();
    Some(blur(shrink(&px, w, h, 96)?))
}

/// One value from the registry, read with Windows' own `reg` tool (no window shows).
#[cfg(windows)]
fn reg(key: &str, name: &str) -> Option<String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new("reg").args(["query", key, "/v", name]).creation_flags(CREATE_NO_WINDOW).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    // "    Name    REG_SZ    value"
    text.lines().find_map(|l| {
        let mut parts = l.trim().splitn(3, "    ");
        (parts.next()? == name).then_some(())?;
        parts.next()?;
        Some(parts.next().unwrap_or("").to_string())
    })
}

/// A `w` × `h` picture averaged down to `out_w` wide, keeping its shape.
#[cfg(windows)]
fn shrink(px: &[[u8; 3]], w: usize, h: usize, out_w: usize) -> Option<(Vec<[f32; 3]>, usize, usize)> {
    if w == 0 || h == 0 || px.len() < w.checked_mul(h)? {
        return None;
    }
    let ow = out_w.min(w).max(1);
    let oh = (h * ow / w).max(1);
    let mut sum = vec![[0.0f32; 3]; ow * oh];
    let mut count = vec![0u32; ow * oh];
    for y in 0..h {
        let oy = (y * oh / h).min(oh - 1);
        for x in 0..w {
            let ox = (x * ow / w).min(ow - 1);
            let (Some(p), Some(s), Some(n)) = (px.get(y * w + x), sum.get_mut(oy * ow + ox), count.get_mut(oy * ow + ox)) else {
                continue;
            };
            for (acc, v) in s.iter_mut().zip(p) {
                *acc += f32::from(*v);
            }
            *n += 1;
        }
    }
    let avg = sum.iter().zip(&count).map(|(s, &n)| s.map(|v| v / n.max(1) as f32)).collect();
    Some((avg, ow, oh))
}

/// Three box blurs each way: a wide, soft Gaussian-like blur.
#[cfg(windows)]
fn blur((mut px, w, h): (Vec<[f32; 3]>, usize, usize)) -> ColorImage {
    let r = (w / 12).max(1) as isize;
    for _ in 0..3 {
        for horizontal in [true, false] {
            let src = px.clone();
            let (len, lines) = if horizontal { (w, h) } else { (h, w) };
            for line in 0..lines {
                for i in 0..len {
                    let mut acc = [0.0f32; 3];
                    let mut n = 0.0f32;
                    for d in -r..=r {
                        let j = (i as isize + d).clamp(0, len as isize - 1) as usize;
                        let at = if horizontal { line * w + j } else { j * w + line };
                        if let Some(p) = src.get(at) {
                            for (a, v) in acc.iter_mut().zip(p) {
                                *a += v;
                            }
                            n += 1.0;
                        }
                    }
                    let at = if horizontal { line * w + i } else { i * w + line };
                    if let Some(p) = px.get_mut(at) {
                        *p = acc.map(|a| a / n.max(1.0));
                    }
                }
            }
        }
    }
    let pixels = px.iter().map(|p| Color32::from_rgb(p[0] as u8, p[1] as u8, p[2] as u8)).collect();
    ColorImage::new([w, h], pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recolour_keeps_the_bar_brightness_and_some_hue() {
        let img = ColorImage::new([2, 1], vec![Color32::from_rgb(20, 60, 200), Color32::from_gray(90)]);
        let out = recolour(&img, Color32::from_gray(243));
        let blue = out.pixels.first().copied().unwrap_or_default();
        let grey = out.pixels.get(1).copied().unwrap_or_default();
        // A grey wallpaper leaves the bar as it was; a blue one tints it blue, near its brightness.
        assert_eq!(grey, Color32::from_gray(243));
        assert!(blue.b() > blue.r(), "{blue:?}");
        let lum = 0.2126 * blue.r() as f32 + 0.7152 * blue.g() as f32 + 0.0722 * blue.b() as f32;
        assert!((lum - 243.0).abs() < 12.0, "{lum}");
    }
}
