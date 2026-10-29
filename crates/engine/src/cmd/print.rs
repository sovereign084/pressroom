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
            "{printer?, copies?: 1, pages?: \"1-3,5\", sides?: default|one|longEdge|shortEdge, grayscale?: bool, layout?: pages|booklet (booklet: two pages a sheet in folding order), dryRun?: bool (export and preview submission without printing), …the PDF settings of file.exportPdf (the PDF it writes is what prints)} → {printer, copies, pages, command, bytes, warnings}",
            has_doc,
            print
        ),
    ]
}

fn printers() -> Value {
    let printers = craft_print::printers();
    let default = printers.iter().find(|p| p.default).map(|p| p.name.clone());
    let names: Vec<_> = printers.into_iter().map(|p| p.name).collect();
    json!({"printers": names, "default": default})
}

fn print(s: &mut Session, p: &Value) -> Result<Value> {
    #[cfg(target_arch = "wasm32")]
    {
        super::export::options(p, s.doc()?.doc.page_count())?;
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
    // A booklet prints the pages two to a sheet in folding order (Publisher's "Booklet, side-fold").
    let booklet = str_param(p, "layout") == Some("booklet");
    let (bytes, pages, warnings) = if booklet {
        let bo = designcraft_pdf::BookletOptions { kind: designcraft_pdf::BookletKind::SaddleStitch, space_between: 0.0, title: None };
        let r = designcraft_pdf::export_booklet(&crate::cmd::interchange::with_pdf_boxes(d), &s.cache, &bo)
            .map_err(|e| bad("file.print", e.to_string()))?;
        (r.bytes, r.pages, serde_json::to_value(r.warnings).unwrap_or_default())
    } else {
        let pages = opts.pages.as_ref().map_or(d.page_count(), |v| v.len());
        // The PDF that Export PDF writes with these settings.
        let pdf = super::export::pdf_export(s, p)?;
        (pdf.bytes, pages, serde_json::to_value(pdf.warnings).unwrap_or_default())
    };
    let command = craft_print::command_preview(&job).map_err(|e| bad("file.print", e.to_string()))?;
    let out = json!({"printer": job.printer, "copies": copies, "pages": pages, "command": command, "bytes": bytes.len(), "warnings": warnings});
    if !dry {
        submit(&bytes, &job).map_err(|e| bad("file.print", e.to_string()))?;
    }
    Ok(out)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

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
