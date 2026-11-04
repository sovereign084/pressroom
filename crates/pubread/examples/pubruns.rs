//! Prints each story's formatting runs and the style sheet, as read. Usage: pubruns FILE.pub [how many runs a story]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(path) = args.get(1) else {
        println!("usage: pubruns FILE.pub [runs]");
        return;
    };
    let limit: usize = args.get(2).and_then(|n| n.parse().ok()).unwrap_or(3);
    let Ok(bytes) = std::fs::read(path) else {
        println!("can't read {path}");
        return;
    };
    let p = match designcraft_pubread::read(&bytes) {
        Ok(p) => p,
        Err(e) => {
            println!("{e}");
            return;
        }
    };
    for (i, s) in p.char_styles.iter().enumerate() {
        println!("char style {i}: {s:?}");
    }
    for (id, story) in &p.stories {
        let text = String::from_utf16_lossy(&story.units);
        println!("== story {id}: {:?}", text.chars().take(40).collect::<String>());
        for r in story.runs.iter().take(limit) {
            println!("  run {}..{} {:?}", r.start, r.end, r.props);
        }
        println!("  {} paragraphs", story.paragraphs.len());
        for r in story.paragraphs.iter().take(limit) {
            println!("  para {:?}", r);
        }
    }
}
