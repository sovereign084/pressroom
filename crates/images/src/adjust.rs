//! Picture corrections and recolouring (PressRoom's Corrections and Recolor galleries), and making a
//! picture smaller (Compress Pictures). Pixels are straight RGBA8 in display sRGB, as
//! [`crate::decode_rgba`] gives them; alpha is left as it is.

pub use image::RgbaImage;

/// A Recolor choice: a colour mode, or a duotone in a colour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Recolor {
    #[default]
    None,
    Grayscale,
    Sepia,
    /// Pale and low in contrast, for a picture behind text.
    Washout,
    /// Black and white at a 50% threshold.
    BlackWhite,
    /// Black through to the colour (Dark Variations).
    Dark([u8; 3]),
    /// The colour through to white (Light Variations).
    Light([u8; 3]),
}

impl Recolor {
    /// The name the `picture.adjust` command takes for it (its tint apart).
    pub fn name(self) -> &'static str {
        match self {
            Recolor::None => "none",
            Recolor::Grayscale => "grayscale",
            Recolor::Sepia => "sepia",
            Recolor::Washout => "washout",
            Recolor::BlackWhite => "blackWhite",
            Recolor::Dark(_) => "dark",
            Recolor::Light(_) => "light",
        }
    }

    /// The choice named `name`; `tint` colours the dark and light variations.
    pub fn from_name(name: &str, tint: [u8; 3]) -> Option<Recolor> {
        Some(match name {
            "none" | "" => Recolor::None,
            "grayscale" => Recolor::Grayscale,
            "sepia" => Recolor::Sepia,
            "washout" => Recolor::Washout,
            "blackWhite" => Recolor::BlackWhite,
            "dark" => Recolor::Dark(tint),
            "light" => Recolor::Light(tint),
            _ => return None,
        })
    }

    pub fn tint(self) -> Option<[u8; 3]> {
        match self {
            Recolor::Dark(c) | Recolor::Light(c) => Some(c),
            _ => None,
        }
    }
}

/// Brightness and contrast (each −100…100 percent, as Office's corrections), then a recolour.
pub fn adjust(img: &mut RgbaImage, brightness: i32, contrast: i32, recolor: Recolor) {
    let b = f32::from(brightness.clamp(-100, 100) as i16) / 100.0;
    let c = f32::from(contrast.clamp(-100, 100) as i16) / 100.0;
    // Contrast stretches or flattens around middle grey; +100% would be a step, so it stops short.
    let gain = if c >= 0.0 { 1.0 / (1.0 - c.min(0.95)) } else { 1.0 + c };
    let lut: Vec<f32> = (0..256u16)
        .map(|v| {
            let x = f32::from(v) / 255.0;
            ((x - 0.5) * gain + 0.5 + b).clamp(0.0, 1.0)
        })
        .collect();
    let level = |v: u8| lut.get(usize::from(v)).copied().unwrap_or(0.0);
    for p in img.pixels_mut() {
        let [r, g, bl, a] = p.0;
        let (r, g, bl) = (level(r), level(g), level(bl));
        let y = 0.299 * r + 0.587 * g + 0.114 * bl;
        let mix = |c: [u8; 3], k: usize| f32::from(c.get(k).copied().unwrap_or(0)) / 255.0;
        let (r, g, bl) = match recolor {
            Recolor::None => (r, g, bl),
            Recolor::Grayscale => (y, y, y),
            Recolor::Sepia => (
                (0.393 * r + 0.769 * g + 0.189 * bl).min(1.0),
                (0.349 * r + 0.686 * g + 0.168 * bl).min(1.0),
                (0.272 * r + 0.534 * g + 0.131 * bl).min(1.0),
            ),
            Recolor::Washout => (0.72 + 0.28 * r, 0.72 + 0.28 * g, 0.72 + 0.28 * bl),
            Recolor::BlackWhite => {
                let v = if y >= 0.5 { 1.0 } else { 0.0 };
                (v, v, v)
            }
            Recolor::Dark(c) => (mix(c, 0) * y, mix(c, 1) * y, mix(c, 2) * y),
            Recolor::Light(c) => {
                let t = |k: usize| mix(c, k) + (1.0 - mix(c, k)) * y;
                (t(0), t(1), t(2))
            }
        };
        let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        p.0 = [byte(r), byte(g), byte(bl), a];
    }
}

/// Whether every pixel is opaque (a JPEG can hold it).
pub fn is_opaque(img: &RgbaImage) -> bool {
    img.pixels().all(|p| p.0[3] == 255)
}

/// `img` encoded as a JPEG (when `jpeg` and it is opaque) or else a PNG: the bytes and their MIME type.
pub fn encode(img: &RgbaImage, jpeg: bool) -> Option<(Vec<u8>, &'static str)> {
    let mut out = Vec::new();
    if jpeg && is_opaque(img) {
        let rgb = image::DynamicImage::ImageRgba8(img.clone()).to_rgb8();
        let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 88);
        enc.encode_image(&rgb).ok()?;
        return Some((out, "image/jpeg"));
    }
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).ok()?;
    Some((out, "image/png"))
}

/// `img` resampled to `w`×`h` pixels (at least one each).
pub fn resize(img: &RgbaImage, w: u32, h: u32) -> RgbaImage {
    image::imageops::resize(img, w.max(1), h.max(1), image::imageops::FilterType::CatmullRom)
}

/// The `w`×`h` part of `img` from (`x`, `y`), kept inside the picture.
pub fn crop(img: &RgbaImage, x: u32, y: u32, w: u32, h: u32) -> RgbaImage {
    let (iw, ih) = img.dimensions();
    let (x, y) = (x.min(iw.saturating_sub(1)), y.min(ih.saturating_sub(1)));
    let (w, h) = (w.clamp(1, iw.saturating_sub(x).max(1)), h.clamp(1, ih.saturating_sub(y).max(1)));
    image::imageops::crop_imm(img, x, y, w, h).to_image()
}

/// A small copy of `img` whose longer side is at most `side` pixels, for a gallery.
pub fn thumbnail(img: &RgbaImage, side: u32) -> RgbaImage {
    let (w, h) = img.dimensions();
    let long = w.max(h).max(1);
    if long <= side {
        return img.clone();
    }
    let k = f64::from(side) / f64::from(long);
    let (tw, th) = ((f64::from(w) * k).round() as u32, (f64::from(h) * k).round() as u32);
    image::imageops::thumbnail(img, tw.max(1), th.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(v: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(2, 2, image::Rgba(v))
    }

    fn after(v: [u8; 4], b: i32, c: i32, r: Recolor) -> [u8; 4] {
        let mut img = px(v);
        adjust(&mut img, b, c, r);
        img.get_pixel(0, 0).0
    }

    #[test]
    fn no_correction_leaves_the_picture_as_it_is() {
        assert_eq!(after([10, 128, 250, 200], 0, 0, Recolor::None), [10, 128, 250, 200]);
    }

    #[test]
    fn brightness_lightens_and_contrast_spreads_from_middle_grey() {
        assert_eq!(after([100, 100, 100, 255], 20, 0, Recolor::None), [151, 151, 151, 255]);
        assert_eq!(after([100, 100, 100, 255], -100, 0, Recolor::None), [0, 0, 0, 255]);
        let up = after([100, 160, 128, 255], 0, 40, Recolor::None);
        assert!(up[0] < 100 && up[1] > 160, "{up:?}");
        let flat = after([0, 255, 128, 255], 0, -100, Recolor::None);
        assert_eq!(flat, [128, 128, 128, 255]);
    }

    #[test]
    fn recolors_keep_alpha_and_shape_the_colour() {
        let g = after([255, 0, 0, 77], 0, 0, Recolor::Grayscale);
        assert_eq!((g[0], g[1], g[2], g[3]), (76, 76, 76, 77));
        let w = after([0, 0, 0, 255], 0, 0, Recolor::Washout);
        assert!(w[0] > 170, "washout is pale: {w:?}");
        assert_eq!(after([200, 200, 200, 255], 0, 0, Recolor::BlackWhite), [255, 255, 255, 255]);
        assert_eq!(after([40, 40, 40, 255], 0, 0, Recolor::BlackWhite), [0, 0, 0, 255]);
        assert_eq!(after([255, 255, 255, 255], 0, 0, Recolor::Dark([30, 90, 160])), [30, 90, 160, 255]);
        assert_eq!(after([0, 0, 0, 255], 0, 0, Recolor::Light([30, 90, 160])), [30, 90, 160, 255]);
        let s = after([128, 128, 128, 255], 0, 0, Recolor::Sepia);
        assert!(s[0] > s[1] && s[1] > s[2], "sepia is warm: {s:?}");
    }

    #[test]
    fn recolor_names_round_trip() {
        for r in [
            Recolor::None,
            Recolor::Grayscale,
            Recolor::Sepia,
            Recolor::Washout,
            Recolor::BlackWhite,
            Recolor::Dark([1, 2, 3]),
            Recolor::Light([1, 2, 3]),
        ] {
            assert_eq!(Recolor::from_name(r.name(), r.tint().unwrap_or([1, 2, 3])), Some(r));
        }
        assert_eq!(Recolor::from_name("purple", [0; 3]), None);
    }

    #[test]
    fn opaque_pictures_encode_as_jpeg_and_see_through_ones_as_png() {
        let (bytes, mime) = encode(&px([10, 20, 30, 255]), true).unwrap();
        assert_eq!(mime, "image/jpeg");
        assert_eq!(crate::pixel_size(&bytes), Some((2, 2)));
        assert_eq!(encode(&px([10, 20, 30, 128]), true).unwrap().1, "image/png");
        assert_eq!(encode(&px([10, 20, 30, 255]), false).unwrap().1, "image/png");
    }

    #[test]
    fn crops_and_resizes_stay_inside_the_picture() {
        let img = RgbaImage::new(10, 6);
        assert_eq!(crop(&img, 8, 4, 50, 50).dimensions(), (2, 2));
        assert_eq!(crop(&img, 99, 99, 0, 0).dimensions(), (1, 1));
        assert_eq!(resize(&img, 0, 3).dimensions(), (1, 3));
        assert_eq!(thumbnail(&img, 5).dimensions(), (5, 3));
        assert_eq!(thumbnail(&img, 50).dimensions(), (10, 6));
    }
}
