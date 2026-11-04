//! Publisher names a font as Windows does ("Arial Narrow", bold); PressRoom by its typographic family
//! and style ("Arial", "Narrow Bold"). The installed font files say which is which, and how far apart
//! Publisher sets single-spaced lines in each.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use skrifa::MetadataProvider;
use skrifa::raw::{FileRef, TableProvider};
use skrifa::string::StringId;

use crate::steps::{FontChoice, Fonts, style_name};

/// Most font files looked at, and the largest read.
const MAX_FILES: usize = 20_000;
const MAX_FONT_BYTES: u64 = 256 * 1024 * 1024;
/// The spacing used when a font's file can't be found.
const DEFAULT_FACTOR: f64 = 1.2;

#[derive(Clone, Debug)]
struct Face {
    family: String,
    style: String,
    win32_style: String,
    weight: u16,
    italic: bool,
    factor: Option<f64>,
}

/// The fonts on this computer, read once when the first is asked for.
#[derive(Default)]
pub struct SystemFonts {
    faces: Mutex<Option<HashMap<String, Vec<Face>>>>,
    /// Microsoft 365's cloud fonts: only Office can draw them, but their files give the spacing.
    clouds: Mutex<HashMap<String, Option<f64>>>,
}

fn english(font: &skrifa::FontRef<'_>, id: StringId) -> Option<String> {
    font.localized_strings(id).english_or_first().map(|s| s.to_string()).filter(|s| !s.trim().is_empty())
}

/// The typographic ascender, descender and line gap (the OS/2 table) as a multiple of the size:
/// Publisher's single line spacing. Arial: 1.09, Times New Roman: 1.06, Calibri: 1.22.
fn typo_factor(font: &skrifa::FontRef<'_>) -> Option<f64> {
    let upem = f64::from(font.head().ok()?.units_per_em());
    let os2 = font.os2().ok()?;
    let f = (f64::from(os2.s_typo_ascender()) - f64::from(os2.s_typo_descender()) + f64::from(os2.s_typo_line_gap())) / upem;
    (upem > 0.0 && f > 0.5 && f < 3.0).then_some(f)
}

fn read_faces(path: &Path, into: &mut HashMap<String, Vec<Face>>) {
    let Ok(meta) = std::fs::metadata(path) else { return };
    if meta.len() > MAX_FONT_BYTES {
        return;
    }
    let Ok(data) = std::fs::read(path) else { return };
    let Ok(file) = FileRef::new(&data) else { return };
    for font in file.fonts().flatten() {
        let Some(win32) = english(&font, StringId::FAMILY_NAME) else { continue };
        let win32_style = english(&font, StringId::SUBFAMILY_NAME).unwrap_or_else(|| "Regular".into());
        let family = english(&font, StringId::TYPOGRAPHIC_FAMILY_NAME).unwrap_or_else(|| win32.clone());
        let style = english(&font, StringId::TYPOGRAPHIC_SUBFAMILY_NAME).unwrap_or_else(|| win32_style.clone());
        let attrs = font.attributes();
        let face = Face {
            family,
            style,
            win32_style,
            weight: attrs.weight.value() as u16,
            italic: attrs.style != skrifa::attribute::Style::Normal,
            factor: typo_factor(&font),
        };
        into.entry(win32.to_lowercase()).or_default().push(face);
    }
}

fn is_font_file(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "ttf" | "ttc" | "otf" | "otc"))
}

fn font_folders() -> Vec<PathBuf> {
    designcraft_fonts::system_font_dirs()
}

fn scan() -> HashMap<String, Vec<Face>> {
    let mut out = HashMap::new();
    let mut count = 0usize;
    for dir in font_folders() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            if is_font_file(&p) {
                count += 1;
                if count > MAX_FILES {
                    return out;
                }
                read_faces(&p, &mut out);
            }
        }
    }
    out
}

fn cloud_factor(name: &str) -> Option<f64> {
    let root = PathBuf::from(std::env::var_os("LOCALAPPDATA")?).join("Microsoft").join("FontCache").join("4").join("CloudFonts");
    let lower = name.to_lowercase();
    let mut folders: Vec<PathBuf> = std::fs::read_dir(root)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p.file_name().is_some_and(|n| lower.starts_with(&n.to_string_lossy().to_lowercase())))
        .collect();
    // The longest match: Kermit Expanded is in the Kermit folder, Georgia Pro Black in Georgia Pro.
    folders.sort_by_key(|p| std::cmp::Reverse(p.as_os_str().len()));
    let folder = folders.first()?;
    let file = std::fs::read_dir(folder).ok()?.flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("ttf")))?;
    let data = std::fs::read(file).ok()?;
    let font = skrifa::FontRef::from_index(&data, 0).ok()?;
    typo_factor(&font)
}

/// The face of `faces` for bold and italic: the one Windows calls that, else the nearest in weight and
/// slant (Lucida Bright's bold is Demibold).
fn pick(faces: &[Face], bold: bool, italic: bool) -> Option<&Face> {
    let want = style_name(bold, italic);
    faces.iter().find(|f| f.win32_style.eq_ignore_ascii_case(want)).or_else(|| {
        let weight = if bold { 700i32 } else { 400 };
        faces.iter().min_by_key(|f| (i32::from(f.weight) - weight).abs() + if f.italic == italic { 0 } else { 1000 })
    })
}

impl Fonts for SystemFonts {
    fn choose(&self, name: &str, bold: bool, italic: bool) -> FontChoice {
        let mut guard = self.faces.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let faces = guard.get_or_insert_with(scan);
        if let Some(face) = faces.get(&name.to_lowercase()).and_then(|list| pick(list, bold, italic)) {
            return FontChoice { family: face.family.clone(), style: face.style.clone(), line_factor: face.factor.unwrap_or(DEFAULT_FACTOR) };
        }
        drop(guard);
        let mut clouds = self.clouds.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let factor = *clouds.entry(name.to_lowercase()).or_insert_with(|| cloud_factor(name));
        FontChoice { family: name.to_string(), style: style_name(bold, italic).to_string(), line_factor: factor.unwrap_or(DEFAULT_FACTOR) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face(win32_style: &str, weight: u16, italic: bool) -> Face {
        Face { family: "F".into(), style: win32_style.into(), win32_style: win32_style.into(), weight, italic, factor: None }
    }

    #[test]
    fn a_style_windows_names_is_taken_else_the_nearest() {
        let faces = [face("Regular", 400, false), face("Demibold", 600, false), face("Italic", 400, true)];
        assert_eq!(pick(&faces, false, false).map(|f| f.style.as_str()), Some("Regular"));
        assert_eq!(pick(&faces, true, false).map(|f| f.style.as_str()), Some("Demibold"));
        assert_eq!(pick(&faces, false, true).map(|f| f.style.as_str()), Some("Italic"));
        assert!(pick(&[], true, true).is_none());
    }

    #[test]
    fn a_font_nobody_has_keeps_its_name() {
        let fonts = SystemFonts { faces: Mutex::new(Some(HashMap::new())), clouds: Mutex::new(HashMap::new()) };
        let c = fonts.choose("No Such Font At All", true, false);
        assert_eq!((c.family.as_str(), c.style.as_str()), ("No Such Font At All", "Bold"));
        assert!((c.line_factor - DEFAULT_FACTOR).abs() < 1e-9);
    }
}
