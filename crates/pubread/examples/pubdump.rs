//! Prints what the reader finds in a `.pub`, page by page, for checking against Publisher.

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        println!("usage: pubdump FILE.pub");
        return;
    };
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            println!("{path}: {e}");
            return;
        }
    };
    let p = match designcraft_pubread::read(&bytes) {
        Ok(p) => p,
        Err(e) => {
            println!("{path}: {e}");
            return;
        }
    };
    println!(
        "page {} x {} pts, {} pages, {} stories, {} fonts, {} colours, {} store entries",
        p.width,
        p.height,
        p.pages.len(),
        p.stories.len(),
        p.fonts.len(),
        p.colors.len(),
        p.store.len()
    );
    println!("fonts: {:?}", p.fonts);
    for n in &p.notes {
        println!("NOTE: {n}");
    }
    if std::env::args().nth(2).as_deref() == Some("--props") {
        for (i, page) in p.pages.iter().enumerate() {
            for s in &page.shapes {
                let complex: Vec<String> = s
                    .complex
                    .iter()
                    .map(|(k, v)| format!("{k:03x}:{}", String::from_utf16_lossy(&designcraft_pubread::bytes::utf16_units(v))))
                    .collect();
                println!(
                    "{{\"page\":{i},\"seq\":{},\"kind\":{},\"rect\":{:?},\"props\":{:?},\"tert\":{:?},\"layout\":{:?},\"complex\":{complex:?}}}",
                    s.seq, s.kind, s.rect, s.props, s.tertiary, s.layout
                );
            }
        }
        return;
    }
    let show = |label: String, page: &designcraft_pubread::model::Page| {
        println!("== {label} (seq {}): {} shapes", page.seq, page.shapes.len());
        for s in &page.shapes {
            let text = s.story.and_then(|id| p.stories.get(&id)).map(|t| String::from_utf16_lossy(&t.units));
            let shown: String = text.unwrap_or_default().chars().take(50).collect();
            println!(
                "seq {} kind {} rect [{:.2},{:.2},{:.2},{:.2}] rot {} fill {:?} line {:?} story {:?}/{} pic {:?} crop {:?} {:?}",
                s.seq,
                s.kind,
                s.rect[0],
                s.rect[1],
                s.rect[2],
                s.rect[3],
                s.rotation,
                s.fill,
                s.line,
                s.story,
                s.frame_index,
                s.picture,
                s.crop,
                shown
            );
        }
    };
    if let Some(m) = &p.master {
        show("master".into(), m);
    }
    for (i, page) in p.pages.iter().enumerate() {
        show(format!("page {}", i + 1), page);
    }
}
