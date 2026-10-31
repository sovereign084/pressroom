//! Opening a Publisher file (.pub). On a Windows PC where Publisher is installed, the converter
//! installed beside PressRoom (`pub2pressroom.ps1` with the command-line tool) has Publisher read
//! the file and rebuilds it as a .pressroom publication next to the original, which then opens. It
//! runs in the background; the window says so meanwhile. The original .pub is never changed, and an
//! earlier conversion is never overwritten.
//!
//! A whole folder can be brought in at once (File › Open › Convert a Folder): every Publisher file
//! in it and its folders that has no publication beside it yet, one after another.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, channel};

use designcraft_engine::{file_access, io};
use serde_json::json;

use crate::DesignApp;

/// Folders deep a folder conversion looks, and the most files it takes on.
const FOLDER_DEPTH: usize = 6;
const FOLDER_MAX: usize = 1000;

/// What the converting thread reports.
enum Msg {
    /// A folder conversion moved on to its `n`th file (1-based) of `of`.
    Next { name: String, n: usize, of: usize },
    /// One file is done: the publication's path, or why not.
    One(Result<String, String>),
    /// A folder is done: how many were converted, and the files that couldn't be.
    Folder { converted: usize, failed: Vec<String>, stopped: Option<String> },
}

/// The conversion under way: the file's name, where it is in a folder, and its reports.
struct Job {
    name: String,
    of: Option<(usize, usize)>,
    rx: Receiver<Msg>,
}

static JOB: Mutex<Option<Job>> = Mutex::new(None);

pub fn is_pub(path: &str) -> bool {
    path.to_ascii_lowercase().ends_with(".pub")
}

/// The name of the file being brought in, while a conversion runs.
pub fn busy() -> Option<String> {
    JOB.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref().map(|j| j.name.clone())
}

/// Where a folder conversion is: the file it's on (1-based) and how many there are.
pub fn progress() -> Option<(usize, usize)> {
    JOB.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref().and_then(|j| j.of)
}

/// The .pressroom path beside `pub_path` a first conversion writes.
fn first_output(pub_path: &Path) -> PathBuf {
    let dir = pub_path.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = pub_path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Publication".into());
    dir.join(format!("{stem}.pressroom"))
}

/// A .pressroom path beside `pub_path` that doesn't exist yet.
fn free_output(pub_path: &Path) -> PathBuf {
    let first = first_output(pub_path);
    if !io::exists(&first) {
        return first;
    }
    let dir = pub_path.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = pub_path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Publication".into());
    (2..1000).map(|n| dir.join(format!("{stem} ({n}).pressroom"))).find(|p| !io::exists(p)).unwrap_or(first)
}

/// The Publisher files in `folder` and its folders (not too deep, not too many) that haven't been
/// converted yet, in name order, and how many already had a publication beside them.
fn unconverted(folder: &Path) -> (Vec<PathBuf>, usize) {
    let mut found = Vec::new();
    let mut stack = vec![(folder.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = io::read_dir(&dir) else { continue };
        let mut dirs = Vec::new();
        for e in entries.flatten() {
            let p = e.path();
            let hidden = p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(['.', '~', '$']));
            if hidden {
                continue;
            }
            if io::is_dir(&p) {
                if depth < FOLDER_DEPTH {
                    dirs.push(p);
                }
            } else if is_pub(&p.to_string_lossy()) && io::is_file(&p) && found.len() < FOLDER_MAX {
                found.push(p);
            }
        }
        dirs.sort();
        stack.extend(dirs.into_iter().rev().map(|d| (d, depth + 1)));
    }
    found.sort();
    let before = found.len();
    found.retain(|p| !io::exists(first_output(p)));
    let skipped = before - found.len();
    (found, skipped)
}

/// The converter installed beside PressRoom (script, command-line tool), or why there's none.
fn converter() -> Result<(PathBuf, PathBuf), String> {
    let exe_dir = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf));
    let Some(dir) = exe_dir else {
        return Err("Couldn't find PressRoom's folder to bring in the Publisher file.".into());
    };
    // The app's own folder, outside any automation roots.
    let (script, cli) = file_access::unconfined(|| {
        let script = dir.join("pub2pressroom.ps1");
        let cli = ["PressRoom-cli.exe", "designcraft-cli.exe"].iter().map(|n| dir.join(n)).find(|p| io::is_file(p));
        (io::is_file(&script).then_some(script), cli)
    });
    match (script, cli) {
        (Some(script), Some(cli)) => Ok((script, cli)),
        _ => Err("The Publisher converter isn't installed with this copy of PressRoom.".into()),
    }
}

const NO_PUBLISHER: &str =
    "Publisher isn't installed on this computer, so its files can't be read here. Bring the file to a PC with Publisher to convert it.";

/// Converts `src` into `out` with Publisher, waiting for it: the publication's path, or why not.
/// `no_publisher` is set when Publisher itself is missing.
fn convert(script: &Path, cli: &Path, src: &Path, out: &Path) -> (Result<String, String>, bool) {
    let name = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let mut cmd = std::process::Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"]).arg(script).arg("-In").arg(src).arg("-Out").arg(out).arg("-Cli").arg(cli);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // No console window flashing up.
        cmd.creation_flags(0x0800_0000);
    }
    match cmd.output() {
        Ok(_) if io::is_file(out) => (Ok(out.to_string_lossy().to_string()), false),
        Ok(o) => {
            let text = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
            if text.contains("Publisher.Application") || text.contains("80040154") {
                return (Err(NO_PUBLISHER.into()), true);
            }
            let tail: String = text.lines().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" ");
            (Err(format!("Couldn't bring in {name}: {tail}")), false)
        }
        Err(e) => (Err(format!("Couldn't start the Publisher converter: {e}")), true),
    }
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| p.to_string_lossy().to_string())
}

fn set_job(name: String, of: Option<(usize, usize)>, rx: Receiver<Msg>) {
    *JOB.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Job { name, of, rx });
}

/// Start bringing in `path`.
pub fn start(app: &mut DesignApp, path: &str) {
    if busy().is_some() {
        app.status("Another Publisher file is still being brought in. Please wait for it to finish.");
        return;
    }
    let (script, cli) = match converter() {
        Ok(c) => c,
        Err(e) => return app.status(e),
    };
    let src = PathBuf::from(path);
    let out = free_output(&src);
    // The converter reads the Publisher file and writes the publication beside it: inside the
    // automation roots when they are in force.
    if let Err(e) = file_access::check_read(&src).and_then(|()| file_access::check_write(&out)) {
        app.status(format!("Couldn't bring in the Publisher file: {e}"));
        return;
    }
    let name = file_name(&src);
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let _ = tx.send(Msg::One(convert(&script, &cli, &src, &out).0));
    });
    set_job(name.clone(), None, rx);
    app.status(format!("Bringing in \u{201c}{name}\u{201d} from Publisher\u{2026} this can take a minute."));
}

/// Start bringing in every Publisher file in `folder` that hasn't been converted yet.
pub fn start_folder(app: &mut DesignApp, folder: &str) {
    if busy().is_some() {
        app.status("Publisher files are still being brought in. Please wait for them to finish.");
        return;
    }
    let (script, cli) = match converter() {
        Ok(c) => c,
        Err(e) => return app.status(e),
    };
    let dir = PathBuf::from(folder);
    if let Err(e) = file_access::check_read(&dir) {
        app.status(format!("Couldn't look in that folder: {e}"));
        return;
    }
    let (files, skipped) = unconverted(&dir);
    let Some(first) = files.first() else {
        let message = if skipped > 0 {
            format!("Every Publisher file in this folder has already been converted ({skipped}). Their PressRoom publications are beside them.")
        } else {
            "There are no Publisher files in this folder.".to_string()
        };
        alert(app, &file_name(&dir), message);
        return;
    };
    let of = files.len();
    let (tx, rx) = channel();
    set_job(file_name(first), Some((1, of)), rx);
    std::thread::spawn(move || convert_all(&script, &cli, files, &tx));
    app.status(format!("Bringing in {of} Publisher files\u{2026} this takes about a minute each."));
}

fn convert_all(script: &Path, cli: &Path, files: Vec<PathBuf>, tx: &Sender<Msg>) {
    let of = files.len();
    let (mut converted, mut failed, mut stopped) = (0, Vec::new(), None);
    for (i, src) in files.iter().enumerate() {
        let _ = tx.send(Msg::Next { name: file_name(src), n: i + 1, of });
        let out = free_output(src);
        if file_access::check_read(src).and_then(|()| file_access::check_write(&out)).is_err() {
            failed.push(file_name(src));
            continue;
        }
        match convert(script, cli, src, &out) {
            (Ok(_), _) => converted += 1,
            // Publisher is missing: the rest would fail the same way.
            (Err(e), true) => {
                stopped = Some(e);
                break;
            }
            (Err(_), false) => failed.push(file_name(src)),
        }
    }
    let _ = tx.send(Msg::Folder { converted, failed, stopped });
}

fn alert(app: &mut DesignApp, file: &str, message: String) {
    app.ui.dialog = Some(crate::dialogs::Dialog::new("alert", json!({"title": "Convert Publisher Files", "file": file, "message": message})));
}

/// What a folder conversion says when it's done.
fn folder_message(converted: usize, failed: &[String], stopped: Option<&str>) -> String {
    let mut s = match converted {
        0 => "No Publisher files were converted.".to_string(),
        1 => "1 Publisher file was converted. Its PressRoom publication is beside it, with the same name.".to_string(),
        n => format!("{n} Publisher files were converted. Each PressRoom publication is beside its Publisher file, with the same name."),
    };
    if let Some(why) = stopped {
        s.push_str("\n\n");
        s.push_str(why);
    }
    if !failed.is_empty() {
        let shown: Vec<&str> = failed.iter().take(8).map(String::as_str).collect();
        let more = failed.len().saturating_sub(shown.len());
        s.push_str(&format!("\n\nThese couldn't be converted: {}", shown.join(", ")));
        if more > 0 {
            s.push_str(&format!(" and {more} more"));
        }
        s.push('.');
    }
    s
}

/// Each frame: follow the conversion, and open the result when one file is done.
pub fn poll(app: &mut DesignApp, ctx: &egui::Context) {
    let msgs: Vec<Msg> = {
        let g = JOB.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        g.as_ref().map(|j| j.rx.try_iter().collect()).unwrap_or_default()
    };
    for msg in msgs {
        match msg {
            Msg::Next { name, n, of } => {
                if let Some(j) = JOB.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_mut() {
                    j.name = name;
                    j.of = Some((n, of));
                }
            }
            Msg::One(result) => {
                let name = JOB.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take().map(|j| j.name).unwrap_or_default();
                match result {
                    Ok(out) => {
                        super::close_backstage(ctx);
                        match app.open_file("file.open", json!({"path": out})) {
                            Ok(_) => {
                                let _ = app.run("view.fitPage", json!({}));
                                crate::canvas::go_to_page(app, 0);
                                app.status(format!("Brought in \u{201c}{name}\u{201d} from Publisher. It's saved as {out}."));
                            }
                            Err(e) => app.status(format!("Converted {name}, but couldn't open the result: {e}")),
                        }
                    }
                    Err(e) => app.status(e),
                }
            }
            Msg::Folder { converted, failed, stopped } => {
                *JOB.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = None;
                app.status(format!("Converted {converted} Publisher files."));
                alert(app, "", folder_message(converted, &failed, stopped.as_deref()));
            }
        }
    }
    if busy().is_some() {
        ctx.request_repaint_after(std::time::Duration::from_millis(400));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_conversion_finds_publisher_files_in_its_folders_and_skips_converted_ones() {
        let root = std::env::temp_dir().join(format!("pr-pubfolder-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("2025").join("Easter")).unwrap();
        std::fs::create_dir_all(root.join(".hidden")).unwrap();
        for f in ["a.pub", "B.PUB", "notes.txt", "2025/c.pub", "2025/Easter/d.pub", "2025/Easter/d.pressroom", ".hidden/e.pub", "~$f.pub"] {
            std::fs::write(root.join(f), b"x").unwrap();
        }
        let (found, skipped) = unconverted(&root);
        let names: Vec<String> = found.iter().map(|p| file_name(p)).collect();
        // In path order; hidden and Office lock files are left alone.
        assert_eq!(names, ["c.pub", "B.PUB", "a.pub"]);
        assert_eq!(skipped, 1, "d.pub already has its publication");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_folder_conversion_says_what_it_did() {
        assert!(folder_message(3, &[], None).starts_with("3 Publisher files were converted."));
        let m = folder_message(1, &["x.pub".into()], None);
        assert!(m.contains("1 Publisher file was converted") && m.contains("These couldn't be converted: x.pub."), "{m}");
        let m = folder_message(0, &[], Some(NO_PUBLISHER));
        assert!(m.contains("No Publisher files") && m.contains("Publisher isn't installed"), "{m}");
        let many: Vec<String> = (0..10).map(|i| format!("{i}.pub")).collect();
        assert!(folder_message(0, &many, None).ends_with(" and 2 more."));
    }
}
