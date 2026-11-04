//! Writes the steps the reader makes from a `.pub` as JSON, for checking against Publisher's list.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (Some(input), Some(output), Some(folder)) = (args.get(1), args.get(2), args.get(3)) else {
        println!("usage: pub2steps FILE.pub OUT.json PICTURE_FOLDER");
        return;
    };
    let run = || -> Result<(), String> {
        let bytes = std::fs::read(input).map_err(|e| e.to_string())?;
        let p = designcraft_pubread::read(&bytes).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
        let out = designcraft_pubread::steps::steps(&p, &designcraft_pubread::fontmap::SystemFonts::default(), std::path::Path::new(folder));
        let text = serde_json::to_string(&out.steps).map_err(|e| e.to_string())?;
        std::fs::write(output, text).map_err(|e| e.to_string())?;
        println!("{} steps, {:?}", out.steps.len(), out.stats);
        for n in &out.notes {
            println!("NOTE: {n}");
        }
        Ok(())
    };
    if let Err(e) = run() {
        println!("{input}: {e}");
    }
}
