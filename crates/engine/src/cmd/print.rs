//! File › Print: the pages go to a printer through the system print spooler (craft-print), as PDF.

use serde_json::{Value, json};

#[cfg(not(target_arch = "wasm32"))]
use super::str_param;
use super::{CommandSpec, bad, cmd, has_doc};
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "file.printers", "Printers", [], None, "{} → {printers: [names], default}", super::always, |_, _| Ok(printers())),
        cmd!(
            noundo "file.print",
            "Print…",
            ["File"],
            Some("Cmd+P"),
            "{printer?, copies?: 1, pages?: \"1-3,5\", sides?: default|one|longEdge|shortEdge, grayscale?: bool, layout?: pages|booklet|copiesPerSheet|pagesPerSheet (booklet: two pages a sheet in folding order), paper?: [w, h] (points, Letter by default: the sheet copiesPerSheet and pagesPerSheet fill), sheets?: \"1-2\" (physical sheets, when imposed), sheetSides?: front|back (one side of each sheet, for printing both sides by hand), reverse?: bool (the sides last first), turned?: bool (upside down), dryRun?: bool (export and preview submission without printing), …the PDF settings of file.exportPdf (the PDF it writes is what prints; imposed sheets use its defaults)} → {printer, copies, pages, command, bytes, warnings, plan?: [[1-based pages of each side]]}",
            has_doc,
            print
        ),
        cmd!(query "file.printPlan", "Print Plan", [], None,
            "{layout?, pages?, paper?, sheets?, sheetSides?, reverse?, turned?} → {sides: [{width, height, placed: [{page (0-based), x, y, scale}], turned}]}, the sides a print job with these settings makes",
            has_doc, print_plan),
    ]
}

/// The printers the system knows and its default, as `file.printers` gives them. Asking a printer
/// that can't be reached can take a long time: a window runs this on a thread of its own.
pub fn printer_list() -> Value {
    printers()
}

fn printers() -> Value {
    let printers = craft_print::printers();
    let default = printers.iter().find(|p| p.default).map(|p| p.name.clone());
    let two_sided: serde_json::Map<String, Value> = printers.iter().filter_map(|p| Some((p.name.clone(), json!(p.two_sided?)))).collect();
    let names: Vec<_> = printers.into_iter().map(|p| p.name).collect();
    json!({"printers": names, "default": default, "twoSided": two_sided})
}

/// The physical sides these settings print, or None for plain pages (the path Export PDF shares).
fn plan_sides(s: &Session, p: &Value) -> Result<Option<Vec<designcraft_pdf::SheetSide>>> {
    use designcraft_pdf::{BookletKind, booklet_sides, copies_per_sheet_sides, one_up_sides, pages_per_sheet_sides};
    const ID: &str = "file.print";
    let d = &s.doc()?.doc;
    let manual = p.get("sheetSides").is_some() || p.get("sheets").is_some();
    let layout = p.get("layout").and_then(Value::as_str).unwrap_or("pages");
    if layout == "pages" && !manual {
        return Ok(None);
    }
    let sizes: Vec<(f64, f64)> = (0..d.page_count()).filter_map(|i| d.page(i).map(|pg| (pg.width, pg.height))).collect();
    let order: Vec<usize> = super::export::options(p, d.page_count())?.pages.unwrap_or_else(|| (0..sizes.len()).collect());
    let paper = match p.get("paper").and_then(Value::as_array).map(Vec::as_slice) {
        None => (612.0, 792.0),
        Some([w, h]) => match (w.as_f64(), h.as_f64()) {
            (Some(w), Some(h)) if w.is_finite() && h.is_finite() && w >= 36.0 && h >= 36.0 => (w, h),
            _ => return Err(bad(ID, "`paper` must be a width and height in points, at least half an inch")),
        },
        Some(_) => return Err(bad(ID, "`paper` must be [width, height]")),
    };
    let mut sides = match layout {
        "booklet" => booklet_sides(&sizes, BookletKind::SaddleStitch, 0.0),
        "copiesPerSheet" => copies_per_sheet_sides(&sizes, &order, paper),
        "pagesPerSheet" => pages_per_sheet_sides(&sizes, &order, paper),
        "pages" => one_up_sides(&sizes, &order),
        other => return Err(bad(ID, format!("unknown layout `{other}` (pages, booklet, copiesPerSheet, pagesPerSheet)"))),
    };
    // Both sides of a sheet: a sheet is two sides, and a lone front keeps a blank back.
    let by_sheet = layout == "booklet" || manual;
    if by_sheet && sides.len() % 2 == 1 {
        let (width, height) = sides.first().map_or((612.0, 792.0), |s| (s.width, s.height));
        sides.push(designcraft_pdf::SheetSide { width, height, placed: vec![], turned: false });
    }
    if let Some(range) = p.get("sheets").and_then(Value::as_str) {
        let per = if by_sheet { 2 } else { 1 };
        let wanted = parse_sheets(range, sides.len().div_ceil(per)).ok_or_else(|| bad(ID, format!("`sheets`: no such sheets as `{range}`")))?;
        let mut chosen = Vec::new();
        for sheet in wanted {
            chosen.extend(sides.iter().skip(sheet * per).take(per).cloned());
        }
        sides = chosen;
    }
    match p.get("sheetSides").and_then(Value::as_str) {
        None => {}
        Some("front") => sides = sides.into_iter().step_by(2).collect(),
        Some("back") => sides = sides.into_iter().skip(1).step_by(2).collect(),
        Some(o) => return Err(bad(ID, format!("unknown sheetSides `{o}` (front, back)"))),
    }
    if p.get("reverse").and_then(Value::as_bool).unwrap_or(false) {
        sides.reverse();
    }
    if p.get("turned").and_then(Value::as_bool).unwrap_or(false) {
        for side in &mut sides {
            side.turned = true;
        }
    }
    if sides.is_empty() {
        return Err(bad(ID, "nothing to print with these settings"));
    }
    Ok(Some(sides))
}

/// `1-3,5` as 0-based sheet indices, each one in range; None if any isn't.
fn parse_sheets(text: &str, count: usize) -> Option<Vec<usize>> {
    let mut out = Vec::new();
    for part in text.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        let (a, b) = part.split_once('-').map_or((part, part), |(a, b)| (a.trim(), b.trim()));
        let (a, b) = (a.parse::<usize>().ok()?, b.parse::<usize>().ok()?);
        if a == 0 || b < a || b > count {
            return None;
        }
        out.extend((a - 1)..b);
    }
    (!out.is_empty()).then_some(out)
}

fn print_plan(s: &mut Session, p: &Value) -> Result<Value> {
    let sides = match plan_sides(s, p)? {
        Some(sides) => sides,
        None => {
            let d = &s.doc()?.doc;
            let order = super::export::options(p, d.page_count())?.pages.unwrap_or_else(|| (0..d.page_count()).collect());
            let sizes: Vec<(f64, f64)> = (0..d.page_count()).filter_map(|i| d.page(i).map(|pg| (pg.width, pg.height))).collect();
            designcraft_pdf::one_up_sides(&sizes, &order)
        }
    };
    let list: Vec<Value> = sides
        .iter()
        .map(|side| {
            let placed: Vec<Value> = side.placed.iter().map(|q| json!({"page": q.page, "x": q.x, "y": q.y, "scale": q.scale})).collect();
            json!({"width": side.width, "height": side.height, "placed": placed, "turned": side.turned})
        })
        .collect();
    Ok(json!({"sides": list}))
}

fn print(s: &mut Session, p: &Value) -> Result<Value> {
    #[cfg(target_arch = "wasm32")]
    {
        plan_sides(s, p)?;
        Err(bad("file.print", "printing isn't available on the web: export a PDF and print it"))
    }
    #[cfg(not(target_arch = "wasm32"))]
    print_with(s, p, craft_print::submit)
}

#[cfg(not(target_arch = "wasm32"))]
fn print_with(
    s: &mut Session,
    p: &Value,
    submit: impl FnOnce(&[u8], &craft_print::Job) -> std::result::Result<String, craft_print::Error>,
) -> Result<Value> {
    let d = &s.doc()?.doc;
    let opts = super::export::options(p, d.page_count())?;
    let copies = p.get("copies").and_then(Value::as_u64).unwrap_or(1).clamp(1, 999) as u32;
    // PressRoom: sides (one-sided, or both sides flipped on the long or short edge) and grey.
    let duplex = match str_param(p, "sides") {
        None | Some("default") => None,
        Some("one") => Some(craft_print::Duplex::Off),
        Some("longEdge") => Some(craft_print::Duplex::LongEdge),
        Some("shortEdge") => Some(craft_print::Duplex::ShortEdge),
        Some(o) => return Err(bad("file.print", format!("unknown sides `{o}` (one, longEdge, shortEdge)"))),
    };
    let job = craft_print::Job {
        printer: str_param(p, "printer").map(str::to_string),
        copies,
        duplex,
        grayscale: p.get("grayscale").and_then(Value::as_bool).unwrap_or(false),
        scaling: craft_print::Scaling::ShrinkToFit,
        title: d.title.clone(),
        ..craft_print::Job::default()
    };
    let dry = p.get("dryRun").and_then(Value::as_bool).unwrap_or(false);
    // Imposed: a booklet prints the pages two to a sheet in folding order (Publisher's "Booklet,
    // side-fold"), and the sides can go one at a time for printing both sides by hand.
    let sides = plan_sides(s, p)?;
    let plan = sides.as_ref().map(|sides| sides.iter().map(|side| side.placed.iter().map(|q| q.page + 1).collect::<Vec<_>>()).collect::<Vec<_>>());
    let (bytes, pages, warnings) = if let Some(sides) = &sides {
        let r = designcraft_pdf::export_sheets(&crate::cmd::interchange::with_pdf_boxes(d), &s.cache, sides, None)
            .map_err(|e| bad("file.print", e.to_string()))?;
        (r.bytes, r.pages, serde_json::to_value(r.warnings).unwrap_or_default())
    } else {
        let pages = opts.pages.as_ref().map_or(d.page_count(), |v| v.len());
        // The PDF that Export PDF writes with these settings.
        let pdf = super::export::pdf_export(s, p)?;
        (pdf.bytes, pages, serde_json::to_value(pdf.warnings).unwrap_or_default())
    };
    let command = craft_print::command_preview(&job).map_err(|e| bad("file.print", e.to_string()))?;
    let mut out = json!({"printer": job.printer, "copies": copies, "pages": pages, "command": command, "bytes": bytes.len(), "warnings": warnings});
    if let Some(plan) = plan {
        out["plan"] = json!(plan);
    }
    if !dry {
        submit(&bytes, &job).map_err(|e| bad("file.print", e.to_string()))?;
    }
    Ok(out)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn manual_two_sided_prints_fronts_then_backs_in_the_order_asked() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"pages": 8, "facingPages": true})).unwrap();
        let dry = |s: &mut Session, extra: Value| {
            let mut p = json!({"layout": "booklet", "dryRun": true});
            for (k, v) in extra.as_object().unwrap() {
                p[k] = v.clone();
            }
            s.execute("file.print", &p).unwrap()["plan"].clone()
        };
        assert_eq!(dry(&mut s, json!({})), json!([[8, 1], [2, 7], [6, 3], [4, 5]]));
        assert_eq!(dry(&mut s, json!({"sheetSides": "front"})), json!([[8, 1], [6, 3]]));
        assert_eq!(dry(&mut s, json!({"sheetSides": "back"})), json!([[2, 7], [4, 5]]));
        assert_eq!(dry(&mut s, json!({"sheetSides": "back", "reverse": true})), json!([[4, 5], [2, 7]]));
        // One jammed sheet again: its two sides, and just its back.
        assert_eq!(dry(&mut s, json!({"sheets": "2"})), json!([[6, 3], [4, 5]]));
        assert_eq!(dry(&mut s, json!({"sheets": "2", "sheetSides": "back"})), json!([[4, 5]]));
    }

    #[test]
    fn plain_pages_can_also_go_one_side_at_a_time_with_a_blank_back_for_the_last() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"pages": 3})).unwrap();
        let plan = |s: &mut Session, side: &str| s.execute("file.print", &json!({"sheetSides": side, "dryRun": true})).unwrap()["plan"].clone();
        assert_eq!(plan(&mut s, "front"), json!([[1], [3]]));
        assert_eq!(plan(&mut s, "back"), json!([[2], []]));
    }

    #[test]
    fn bad_print_settings_are_refused_before_any_paper_goes() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"pages": 4})).unwrap();
        for bad in [
            json!({"layout": "tiled"}),
            json!({"layout": "booklet", "sheets": "9"}),
            json!({"layout": "booklet", "sheets": "0"}),
            json!({"layout": "booklet", "sheets": "2-1"}),
            json!({"layout": "booklet", "sheets": "x"}),
            json!({"layout": "booklet", "sheetSides": "edge"}),
            json!({"layout": "copiesPerSheet", "paper": [1, 2]}),
            json!({"layout": "copiesPerSheet", "paper": [612]}),
        ] {
            assert!(print_with(&mut s, &bad, |_, _| panic!("{bad}: refused settings must not spool")).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_print_plan_says_what_a_job_would_print() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"pages": 2, "width": 252, "height": 144})).unwrap();
        let r = s.execute("file.printPlan", &json!({"layout": "copiesPerSheet"})).unwrap();
        assert_eq!(r["sides"].as_array().unwrap().len(), 2);
        assert_eq!(r["sides"][0]["placed"].as_array().unwrap().len(), 10);
        let r = s.execute("file.printPlan", &json!({})).unwrap();
        assert_eq!(r["sides"][1]["placed"][0]["page"], 1);
    }

    #[test]
    fn print_builds_the_spool_command() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"pages": 4})).unwrap();
        let r = s.execute("file.print", &json!({"printer": "Office", "copies": 2, "pages": "2-3", "dryRun": true})).unwrap();
        assert_eq!(r["pages"], 2);
        let command: Vec<&str> = r["command"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
        if craft_print::backend() == Some(craft_print::Backend::Cups) {
            assert_eq!(&command[..5], ["lp", "-d", "Office", "-n", "2"]);
            assert!(command.contains(&"print-scaling=auto-fit"));
            assert!(!command.iter().any(|a| a.starts_with("sides=")));
        } else {
            // Without a grayscale request the printer's own colour setting applies (craft-print).
            assert_eq!(
                command,
                [
                    "xps-print",
                    "printer=Office",
                    "copies=2",
                    "collate=true",
                    "duplex=(queue default)",
                    "scaling=shrink-to-fit",
                    "color=(driver default)"
                ]
            );
        }
        assert!(r["bytes"].as_u64().unwrap() > 500);
        assert!(s.execute("file.print", &json!({"pages": "9", "dryRun": true})).is_err());
        assert!(s.execute("file.printers", &json!({})).unwrap()["printers"].is_array());
    }

    #[test]
    fn print_takes_sides_grayscale_and_booklet() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"pages": 8, "facingPages": true})).unwrap();
        let mut job = None;
        let r = print_with(&mut s, &json!({"sides": "shortEdge", "grayscale": true, "layout": "booklet"}), |pdf, j| {
            assert!(pdf.starts_with(b"%PDF-"));
            job = Some(j.clone());
            Ok("request id is Office-1".into())
        })
        .unwrap();
        let job = job.unwrap();
        assert_eq!((job.duplex, job.grayscale), (Some(craft_print::Duplex::ShortEdge), true));
        // Eight pages fold into two sheets, each side two pages: four booklet pages.
        assert_eq!(r["pages"], 4, "{r}");
        for (sides, duplex) in [("one", Some(craft_print::Duplex::Off)), ("longEdge", Some(craft_print::Duplex::LongEdge)), ("default", None)] {
            print_with(&mut s, &json!({"sides": sides}), |_, j| {
                assert_eq!(j.duplex, duplex, "{sides}");
                assert!(!j.grayscale);
                Ok(String::new())
            })
            .unwrap();
        }
        assert!(print_with(&mut s, &json!({"sides": "both"}), |_, _| panic!("unknown sides must not spool")).is_err());
    }

    #[test]
    fn print_submits_the_selected_export_and_reports_refusal() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"pages": 4})).unwrap();
        // A fixed creation date: both exports would otherwise take the clock's second.
        let params = json!({"printer": "Office (east)", "copies": 2, "pages": "2-3", "marks": true, "bleed": true, "created": 1_700_000_000});
        let expected = super::super::export::pdf_export(&s, &params).unwrap().bytes;
        let r = print_with(&mut s, &params, |pdf, job| {
            // The production export options and full bytes reach the shared spooler boundary.
            assert_eq!(pdf, expected);
            assert!(pdf.starts_with(b"%PDF-"));
            assert_eq!(job.printer.as_deref(), Some("Office (east)"));
            assert_eq!(job.copies, 2);
            assert_eq!(job.duplex, None);
            assert_eq!(job.scaling, craft_print::Scaling::ShrinkToFit);
            assert!(!job.title.is_empty());
            Ok("request id is Office-1".into())
        })
        .unwrap();
        assert_eq!(r["pages"], 2);
        let refusal = print_with(&mut s, &params, |_, _| Err(craft_print::Error::Spool("printer offline".into()))).unwrap_err();
        assert!(refusal.to_string().contains("printer offline"));
        print_with(&mut s, &json!({"dryRun": true}), |_, _| panic!("dry runs must not spool")).unwrap();
        assert!(print_with(&mut s, &json!({"pages": "9"}), |_, _| panic!("invalid ranges must not spool")).is_err());
        assert!(print_with(&mut s, &json!({"created": "today"}), |_, _| panic!("an invalid date must not spool")).is_err());
    }

    /// File › Print spools the PDF that File › Export PDF writes with the same settings: soft
    /// effects as images, CMYK print color, the same warnings.
    #[test]
    fn print_spools_what_export_pdf_writes() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let id = s.execute("frame.create", &json!({"rect": [72, 72, 200, 200]})).unwrap()["id"].clone();
        s.execute("object.fill", &json!({"ids": [id], "swatch": "C=100 M=0 Y=0 K=0"})).unwrap();
        s.execute("object.dropShadow", &json!({"ids": [id], "on": true, "distance": 8, "size": 6})).unwrap();
        let params = json!({"standard": "x4"});
        // The creation date is in whole seconds: an attempt can straddle a second boundary.
        let mut matched = false;
        for _ in 0..3 {
            let exported = s.execute("file.exportPdf", &params).unwrap();
            let mut printed = Vec::new();
            let r = print_with(&mut s, &params, |pdf, _| {
                printed = pdf.to_vec();
                Ok("request id is Office-1".into())
            })
            .unwrap();
            assert_eq!(r["warnings"], exported["warnings"]);
            assert!(r["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains("soft effects")), "{r}");
            if printed == super::super::file::base64_decode(exported["base64"].as_str().unwrap()) {
                matched = true;
                break;
            }
        }
        assert!(matched, "File › Print spools Export PDF's bytes");
    }
}
