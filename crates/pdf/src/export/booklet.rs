use super::common::{tf, to_path};
use super::images::ImageContentCache;
use super::metadata::file_info_metadata;
use super::pipeline::print_color;
use super::*;

/// One PDF from several (a book's documents), every page carried over as vectors; each part
/// comes with its pages' sizes (points).
pub fn merge_pdfs(parts: &[(Vec<u8>, Vec<(f32, f32)>)], title: Option<&str>) -> Result<Vec<u8>> {
    write::guard(|| merge_pdfs_inner(parts, title))
}

fn merge_pdfs_inner(parts: &[(Vec<u8>, Vec<(f32, f32)>)], title: Option<&str>) -> Result<Vec<u8>> {
    let metadata = Metadata { creator: "DesignCraft".into(), producer: "DesignCraft".into(), title: title.map(str::to_owned), ..Default::default() };
    let mut writer = Writer::new(krilla::SerializeSettings::default(), metadata);
    let pdf = writer.document_mut();
    let mut pages = 0;
    let mut has_optional_content = false;
    for (bytes, sizes) in parts {
        let variant = crate::optional_content::prepare(&Arc::new(bytes.clone()), None)?;
        has_optional_content |= !sizes.is_empty() && variant.has_optional_content;
        let src = krilla::pdf::Pdf::new(variant.data).map_err(|e| PdfError::Write(format!("{e:?}")))?;
        let doc = krilla::pdf::PdfDocument::new(Arc::new(src));
        for (i, &(w, h)) in sizes.iter().enumerate() {
            let size = Size::from_wh(w.max(1.0), h.max(1.0)).ok_or(PdfError::NoPages)?;
            let mut page = pdf.start_page_with(PageSettings::new(size));
            let mut s = page.surface();
            #[cfg(all(test, not(target_arch = "wasm32")))]
            unwind_tests::inject(&mut s);
            s.draw_pdf_page(&doc, size, i);
            s.finish();
            page.finish();
            pages += 1;
        }
    }
    if pages == 0 {
        return Err(PdfError::NoPages);
    }
    let bytes = writer.finish().map_err(PdfError::from)?;
    if has_optional_content { crate::optional_content::register(&bytes) } else { Ok(bytes) }
}

/// File › Print Booklet: how pages pair up on printer spreads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BookletKind {
    /// Folded and stapled: pages padded to a multiple of 4, outer pages together.
    #[default]
    SaddleStitch,
    /// Two pages side by side in reading order.
    TwoUpConsecutive,
}

#[derive(Clone, Debug, Default)]
pub struct BookletOptions {
    pub kind: BookletKind,
    /// Gap between the two pages of a printer spread (points).
    pub space_between: f64,
    pub title: Option<String>,
}

/// The printer spreads for `n` pages: (left, right) absolute page indices, `None` = blank.
pub fn booklet_pairs(n: usize, kind: BookletKind) -> Vec<(Option<usize>, Option<usize>)> {
    let page = |i: usize| (i < n).then_some(i);
    match kind {
        BookletKind::TwoUpConsecutive => (0..n.div_ceil(2)).map(|k| (page(2 * k), page(2 * k + 1))).collect(),
        BookletKind::SaddleStitch => {
            let m = n.div_ceil(4).max(1) * 4;
            (0..m / 2).map(|i| if i % 2 == 0 { (page(m - 1 - i), page(i)) } else { (page(i), page(m - 1 - i)) }).collect()
        }
    }
}

/// One document page on a printed side: where its top-left corner sits and how big it's drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    /// 0-based absolute page index.
    pub page: usize,
    pub x: f64,
    pub y: f64,
    pub scale: f64,
}

/// One side of a printed sheet. The exporter draws these; a window's print preview and a print
/// job's dry run read the same list, so what's previewed is what prints.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SheetSide {
    pub width: f64,
    pub height: f64,
    pub placed: Vec<Placed>,
    /// Drawn upside down: the backs of a manually two-sided job on a printer that needs it.
    pub turned: bool,
}

/// Saddle-stitched or side-by-side sides, from each page's size.
pub fn booklet_sides(pages: &[(f64, f64)], kind: BookletKind, gap: f64) -> Vec<SheetSide> {
    let (pw, ph) = pages.iter().fold((0.0f64, 0.0f64), |(w, h), p| (w.max(p.0), h.max(p.1)));
    let gap = gap.max(0.0);
    booklet_pairs(pages.len(), kind)
        .into_iter()
        .map(|(left, right)| {
            let mut placed = Vec::new();
            for (slot, abs) in [(0.0, left), (pw + gap, right)] {
                let Some(page) = abs else { continue };
                let Some(&(w, h)) = pages.get(page) else { continue };
                // Pages narrower than the slot sit against the fold.
                let dx = if slot == 0.0 { pw - w } else { 0.0 };
                placed.push(Placed { page, x: slot + dx, y: (ph - h) / 2.0, scale: 1.0 });
            }
            SheetSide { width: 2.0 * pw + gap, height: ph, placed, turned: false }
        })
        .collect()
}

/// One document page to a side, each at its own size.
pub fn one_up_sides(pages: &[(f64, f64)], order: &[usize]) -> Vec<SheetSide> {
    order
        .iter()
        .filter_map(|&page| {
            let &(w, h) = pages.get(page)?;
            Some(SheetSide { width: w, height: h, placed: vec![Placed { page, x: 0.0, y: 0.0, scale: 1.0 }], turned: false })
        })
        .collect()
}

/// The space kept at the paper's edge, where printers can't print, and between items on a sheet.
const SHEET_MARGIN: f64 = 18.0;
const SHEET_GAP: f64 = 9.0;

/// How a page repeats on paper: across, down, the sheet's size, and the scale (below 1 only when a
/// single page is bigger than the paper). The sheet turns sideways when more fit that way.
fn sheet_grid(page: (f64, f64), paper: (f64, f64)) -> (usize, usize, f64, f64, f64) {
    let (w, h) = (page.0.max(1.0), page.1.max(1.0));
    let fit = |space: f64, size: f64| (((space - 2.0 * SHEET_MARGIN + SHEET_GAP) / (size + SHEET_GAP)).floor().max(0.0)) as usize;
    let portrait = (paper.0.min(paper.1), paper.0.max(paper.1));
    let landscape = (portrait.1, portrait.0);
    let count = |s: (f64, f64)| fit(s.0, w) * fit(s.1, h);
    let sheet = if count(landscape) > count(portrait) { landscape } else { portrait };
    let (across, down) = (fit(sheet.0, w), fit(sheet.1, h));
    if across == 0 || down == 0 {
        let scale = ((sheet.0 - 2.0 * SHEET_MARGIN) / w).min((sheet.1 - 2.0 * SHEET_MARGIN) / h).clamp(0.01, 1.0);
        return (1, 1, sheet.0, sheet.1, scale);
    }
    (across, down, sheet.0, sheet.1, 1.0)
}

/// The grid's slots on a side, row by row, centred on the sheet.
fn grid_slots(page: (f64, f64), across: usize, down: usize, sheet: (f64, f64), scale: f64) -> Vec<(f64, f64)> {
    let (w, h) = (page.0 * scale, page.1 * scale);
    let total_w = across as f64 * w + across.saturating_sub(1) as f64 * SHEET_GAP;
    let total_h = down as f64 * h + down.saturating_sub(1) as f64 * SHEET_GAP;
    let (x0, y0) = ((sheet.0 - total_w) / 2.0, (sheet.1 - total_h) / 2.0);
    let mut slots = Vec::with_capacity(across * down);
    for r in 0..down {
        for c in 0..across {
            slots.push((x0 + c as f64 * (w + SHEET_GAP), y0 + r as f64 * (h + SHEET_GAP)));
        }
    }
    slots
}

/// Each page repeated to fill a sheet (tickets, name tags, small cards).
pub fn copies_per_sheet_sides(pages: &[(f64, f64)], order: &[usize], paper: (f64, f64)) -> Vec<SheetSide> {
    order
        .iter()
        .filter_map(|&page| {
            let &size = pages.get(page)?;
            let (across, down, sw, sh, scale) = sheet_grid(size, paper);
            let placed = grid_slots(size, across, down, (sw, sh), scale).into_iter().map(|(x, y)| Placed { page, x, y, scale }).collect();
            Some(SheetSide { width: sw, height: sh, placed, turned: false })
        })
        .collect()
}

/// The pages in turn, as many to a sheet as fit.
pub fn pages_per_sheet_sides(pages: &[(f64, f64)], order: &[usize], paper: (f64, f64)) -> Vec<SheetSide> {
    let biggest = order.iter().filter_map(|&p| pages.get(p)).fold((1.0f64, 1.0f64), |(w, h), p| (w.max(p.0), h.max(p.1)));
    let (across, down, sw, sh, scale) = sheet_grid(biggest, paper);
    let slots = grid_slots(biggest, across, down, (sw, sh), scale);
    order
        .chunks(slots.len().max(1))
        .map(|chunk| SheetSide {
            width: sw,
            height: sh,
            placed: chunk.iter().zip(&slots).map(|(&page, &(x, y))| Placed { page, x, y, scale }).collect(),
            turned: false,
        })
        .collect()
}

/// An imposed PDF: two document pages per sheet, in booklet order.
pub fn export_booklet(doc: &Document, cache: &Cache, opts: &BookletOptions) -> Result<ExportReport> {
    let sizes: Vec<(f64, f64)> = (0..doc.page_count()).filter_map(|i| doc.page(i).map(|p| (p.width, p.height))).collect();
    if sizes.is_empty() {
        return Err(PdfError::NoPages);
    }
    export_sheets(doc, cache, &booklet_sides(&sizes, opts.kind, opts.space_between), opts.title.as_deref())
}

/// A PDF of the printed sides, one PDF page to a side.
pub fn export_sheets(doc: &Document, cache: &Cache, sides: &[SheetSide], title: Option<&str>) -> Result<ExportReport> {
    write::guard(|| export_sheets_inner(doc, cache, sides, title))
}

fn export_sheets_inner(doc: &Document, cache: &Cache, sides: &[SheetSide], title: Option<&str>) -> Result<ExportReport> {
    if sides.is_empty() {
        return Err(PdfError::NoPages);
    }
    let pdf_opts = PdfOptions { title: title.map(str::to_owned), ..PdfOptions::default() };
    let mut warnings = Vec::new();
    let base = if print_color(doc, &pdf_opts) {
        let (settings, warning) = crate::pdfx::print_settings(false)?;
        warnings.extend(warning);
        settings
    } else {
        krilla::SerializeSettings::default()
    };
    let settings = krilla::SerializeSettings { compress_content_streams: true, ..base };
    let metadata = file_info_metadata(doc, title, None);
    let mut writer = Writer::new(settings, metadata);
    let pdf = writer.document_mut();
    let mut ex = Exporter {
        drawing_items: Vec::new(),
        inks: crate::inks::ink_set(doc),
        doc,
        cache,
        composed: HashMap::new(),
        opts: &pdf_opts,
        clip: Rect::ZERO,
        warnings,
        images: HashMap::new(),
        images_by_content: ImageContentCache::default(),
        #[cfg(test)]
        image_conversions: 0,
        pdfs: HashMap::new(),
        variants: HashMap::new(),
        has_optional_content: false,
        #[cfg(test)]
        pdf_preparations: 0,
        layer_error: None,
        // krilla's default.
        version: PdfVersion::Pdf17,
        too_new: Vec::new(),
        svgs: HashMap::new(),
        fonts: HashMap::new(),
        reverse_cmaps: HashMap::new(),
        rgb_only: false,
        tags: Vec::new(),
        story_tags: HashMap::new(),
        tag_story: None,
        text_area: None,
        para_tags: HashMap::new(),
    };
    for side in sides {
        let size = Size::from_wh(side.width as f32, side.height as f32).ok_or(PdfError::NoPages)?;
        let mut page = pdf.start_page_with(PageSettings::new(size));
        let mut s = page.surface();
        #[cfg(all(test, not(target_arch = "wasm32")))]
        unwind_tests::inject(&mut s);
        if side.turned {
            s.push_transform(&tf(Affine::translate((side.width, side.height)) * Affine::rotate(std::f64::consts::PI)));
        }
        for placed in &side.placed {
            let Some((si, pi)) = doc.page_loc(placed.page) else { continue };
            let p = &doc.spreads[si].pages[pi];
            let r = Rect::new(p.x, 0.0, p.x + p.width, p.height);
            s.push_transform(&tf(Affine::translate((placed.x, placed.y)) * Affine::scale(placed.scale) * Affine::translate((-p.x, 0.0))));
            ex.clip = r;
            if let Some(clip) = to_path(&r.to_path(0.1)) {
                s.push_clip_path(&clip, &FillRule::NonZero);
                ex.spread(&mut s, si);
                s.pop();
            }
            s.pop();
        }
        if side.turned {
            s.pop();
        }
        s.finish();
        page.finish();
    }
    ex.check_placed_versions()?;
    if let Some(error) = ex.layer_error {
        return Err(error);
    }
    let mut bytes = writer.finish().map_err(PdfError::from)?;
    if ex.has_optional_content {
        bytes = crate::optional_content::register(&bytes)?;
    }
    let mut warnings = ex.warnings;
    warnings.dedup();
    Ok(ExportReport { bytes, pages: sides.len(), warnings })
}

#[cfg(test)]
mod sheet_tests {
    use super::*;

    const LETTER: (f64, f64) = (612.0, 792.0);

    fn pages_of(side: &SheetSide) -> Vec<usize> {
        side.placed.iter().map(|p| p.page).collect()
    }

    #[test]
    fn a_booklet_of_eight_pages_folds_in_order() {
        let sides = booklet_sides(&[(396.0, 612.0); 8], BookletKind::SaddleStitch, 0.0);
        let order: Vec<Vec<usize>> = sides.iter().map(pages_of).collect();
        assert_eq!(order, [vec![7, 0], vec![1, 6], vec![5, 2], vec![3, 4]]);
        assert_eq!((sides[0].width, sides[0].height), (792.0, 612.0));
    }

    #[test]
    fn a_booklet_with_an_odd_count_leaves_the_right_slots_blank() {
        let sides = booklet_sides(&[(396.0, 612.0); 5], BookletKind::SaddleStitch, 0.0);
        assert_eq!(sides.len(), 4);
        assert_eq!(pages_of(&sides[0]), [0], "the back cover is blank");
        assert_eq!(sides[0].placed[0].x, 396.0, "page 1 stays on the right");
    }

    #[test]
    fn copies_fill_the_sheet_and_turn_it_when_more_fit() {
        // Business cards: 3.5 x 2 inches.
        let sides = copies_per_sheet_sides(&[(252.0, 144.0)], &[0], LETTER);
        assert_eq!(sides.len(), 1);
        assert_eq!(sides[0].placed.len(), 10, "2 across, 5 down");
        assert_eq!((sides[0].width, sides[0].height), LETTER);
        // A postcard fits twice only sideways.
        let sides = copies_per_sheet_sides(&[(288.0, 432.0)], &[0], LETTER);
        assert_eq!(sides[0].placed.len(), 2);
        assert_eq!((sides[0].width, sides[0].height), (792.0, 612.0));
        for p in &sides[0].placed {
            assert!(p.x >= 0.0 && p.x + 288.0 <= sides[0].width && p.y >= 0.0 && p.y + 432.0 <= sides[0].height, "{p:?}");
        }
    }

    #[test]
    fn a_page_bigger_than_the_paper_shrinks_to_fit() {
        let sides = copies_per_sheet_sides(&[(1224.0, 1584.0)], &[0], LETTER);
        let p = sides[0].placed[0];
        assert_eq!(sides[0].placed.len(), 1);
        assert!(p.scale < 1.0 && 1224.0 * p.scale <= 612.0 && 1584.0 * p.scale <= 792.0, "{p:?}");
    }

    #[test]
    fn pages_go_on_sheets_in_turn() {
        let sides = pages_per_sheet_sides(&[(252.0, 144.0); 12], &(0..12).collect::<Vec<_>>(), LETTER);
        assert_eq!(sides.len(), 2);
        assert_eq!(pages_of(&sides[1]), [10, 11]);
    }

    #[test]
    fn no_pages_is_an_error_not_a_panic() {
        let doc = Document::new(&designcraft_doc::build::NewDocument { pages: 1, ..Default::default() });
        let cache = designcraft_compose::Cache::default();
        assert!(export_sheets(&doc, &cache, &[], None).is_err());
    }
}
