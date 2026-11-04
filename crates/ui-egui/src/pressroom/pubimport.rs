//! Opening a Publisher file (.pub). On a Windows PC where Publisher is installed, the converter
//! installed beside PressRoom (`pub2pressroom.ps1` with the command-line tool) has Publisher read
//! the file and rebuilds it as a .pressroom publication next to the original, which then opens. Where
//! Publisher is missing, refuses the file or gets stuck, PressRoom's own reader
//! ([`designcraft_engine::pubconvert`]) reads it instead. It runs in the background; the window says
//! so meanwhile. The original .pub is never changed, and an earlier conversion is never overwritten.
//!
//! What didn't come across, whichever way the file was read, is said in a message when the file opens
//! and kept in a notes file beside the publication: nothing goes missing without the person being told.
//!
//! A whole folder can be brought in at once (File › Open › Convert a Folder): every Publisher file
//! in it and its folders that has no publication beside it yet, one after another.
//!
//! The converter says what it is doing a line at a time, and the window shows it. It can be
//! cancelled, and a Publisher that goes quiet for [`STALL`] (waiting on something nobody can see,
//! such as a printer it can't reach) is stopped rather than waited on for ever.

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

#[cfg(not(target_arch = "wasm32"))]
use designcraft_engine::pubconvert::{keep_notes, kept_notes, notes_path};
use designcraft_engine::{file_access, io};
use serde_json::json;

use crate::DesignApp;

/// Folders deep a folder conversion looks, and the most files it takes on.
const FOLDER_DEPTH: usize = 6;
const FOLDER_MAX: usize = 1000;

/// How long the converter may go without a word before it is taken as stuck.
const STALL: std::time::Duration = std::time::Duration::from_secs(5 * 60);

/// How long a converter that has finished its work may take to close.
const LINGER: std::time::Duration = std::time::Duration::from_secs(10);

/// The last thing the converter says: "a.pub -> a.pressroom: 8 pages, ... 2855 steps, 0 problems".
fn is_final_line(line: &str) -> bool {
    line.contains(" -> ") && line.contains(" steps, ") && line.trim_end().ends_with(" problems")
}

/// Cancel, asked for from the window.
static CANCEL: AtomicBool = AtomicBool::new(false);

/// What the converting thread reports.
enum Msg {
    /// A folder conversion moved on to its `n`th file (1-based) of `of`.
    Next { name: String, n: usize, of: usize },
    /// What the converter is doing ("Reading page 3 of 8").
    Status(String),
    /// One file is done: the publication's path, or why not, and what didn't come across.
    One(Result<String, String>, Vec<String>),
    /// A folder is done: how many were converted, the files that couldn't be, and those where
    /// something didn't come across.
    Folder { converted: usize, failed: Vec<String>, stopped: Option<String>, noted: Vec<String> },
}

/// The conversion under way: the file's name, where it is in a folder, and its reports.
struct Job {
    name: String,
    of: Option<(usize, usize)>,
    status: String,
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

/// What the converter is doing now.
pub fn status() -> Option<String> {
    JOB.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref().map(|j| j.status.clone())
}

/// Stops the conversion under way (Cancel on its card).
pub fn cancel() {
    CANCEL.store(true, Ordering::SeqCst);
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

const CANCELLED: &str = "Cancelled.";

/// Set to `first` to try PressRoom's own reader before Publisher: for testing the reader on a PC
/// that has Publisher.
const READER_SWITCH: &str = "PRESSROOM_PUB_READER";

/// The web build has no folder of files for a reader to work in, and so no notes beside them.
#[cfg(target_arch = "wasm32")]
fn with_reader(_src: &Path, _out: &Path, _tx: &Sender<Msg>) -> Result<Vec<String>, String> {
    Err(NO_PUBLISHER.into())
}
#[cfg(target_arch = "wasm32")]
fn kept_notes(_out: &Path) -> Vec<String> {
    Vec::new()
}
#[cfg(target_arch = "wasm32")]
fn notes_path(out: &Path) -> PathBuf {
    out.to_path_buf()
}

/// PressRoom's own reader: the publication's path, and what didn't come across.
#[cfg(not(target_arch = "wasm32"))]
fn with_reader(src: &Path, out: &Path, tx: &Sender<Msg>) -> Result<Vec<String>, String> {
    let done = designcraft_engine::pubconvert::convert(src, out, &mut |status| {
        let _ = tx.send(Msg::Status(status.to_string()));
    })?;
    let name = file_name(src);
    save_log(&format!("{name}, read by PressRoom's own reader: {}\n{}", done.summary, done.notes.join("\n")));
    keep_notes(&name, out, &done.notes);
    Ok(done.notes)
}

/// Converts `src` into `out` whichever way works: Publisher where it is here and answers, else
/// PressRoom's own reader. The flag is set when the files after this one should not be tried.
fn convert_any(publisher: Option<&(PathBuf, PathBuf)>, src: &Path, out: &Path, tx: &Sender<Msg>) -> (Result<String, String>, bool, Vec<String>) {
    let done = |notes| (Ok(out.to_string_lossy().to_string()), false, notes);
    let reader_first = std::env::var_os(READER_SWITCH).is_some_and(|v| v == "first");
    if reader_first {
        match with_reader(src, out, tx) {
            Ok(notes) => return done(notes),
            Err(e) => log::warn!("PressRoom's own reader couldn't read {}: {e}", file_name(src)),
        }
    }
    let mut publisher_said = None;
    if let Some((script, cli)) = publisher {
        match convert(script, cli, src, out, tx).0 {
            // The route's script keeps these beside the publication.
            Ok(path) => return (Ok(path), false, kept_notes(out)),
            Err(e) if e == CANCELLED => return (Err(e), true, Vec::new()),
            Err(e) => publisher_said = Some(e),
        }
    }
    if reader_first {
        return (Err(publisher_said.unwrap_or_else(|| NO_PUBLISHER.into())), false, Vec::new());
    }
    match with_reader(src, out, tx) {
        Ok(notes) => done(notes),
        Err(own) => {
            let why = match publisher_said {
                Some(p) => format!("{p}\n\nPressRoom's own reader couldn't read it either. {own}"),
                None => own,
            };
            (Err(why), false, Vec::new())
        }
    }
}

const STUCK: &str = "Publisher stopped answering, so PressRoom stopped it. If it happens again, open the file in Publisher itself once to see whether it is asking something (about a printer or a missing font, say), answer it there, close Publisher and try again.";

/// A hidden PowerShell running `args` (no console window flashing up).
fn powershell<I, S>(args: I) -> std::process::Command
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let mut cmd = std::process::Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass"]).args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd
}

/// Ends the Publisher the converter started (the one running for automation; a Publisher the user
/// opened is left alone) and puts back the default printer it had switched.
fn stop_publisher(script: &Path) {
    let kill = "Get-CimInstance Win32_Process | Where-Object { $_.Name -eq 'MSPUB.EXE' -and $_.CommandLine -match 'Embedding|Automation' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }";
    let _ = powershell(["-Command", kill]).output();
    let _ = powershell([std::ffi::OsStr::new("-File"), script.as_os_str(), std::ffi::OsStr::new("-RestorePrinter")]).output();
}

/// The converter's own account of the last run (in the temporary folder), kept for when
/// something goes wrong.
fn save_log(text: &str) {
    let p = std::env::temp_dir().join("PressRoom-pub-import-log.txt");
    file_access::unconfined(|| {
        let _ = io::write_file(&p, text.as_bytes());
    });
}

/// Converts `src` into `out` with Publisher, saying what it is doing on `tx`: the publication's
/// path, or why not. The flag is set when the files after this one would fail the same way
/// (Publisher missing, stuck or cancelled).
fn convert(script: &Path, cli: &Path, src: &Path, out: &Path, tx: &Sender<Msg>) -> (Result<String, String>, bool) {
    let name = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let mut cmd = powershell([std::ffi::OsStr::new("-File"), script.as_os_str()]);
    cmd.arg("-In").arg(src).arg("-Out").arg(out).arg("-Cli").arg(cli);
    cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).stdin(std::process::Stdio::null());
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return (Err(format!("Couldn't start the Publisher converter: {e}")), true),
    };
    // Everything it says, and when it last said something.
    let text = Arc::new(Mutex::new(String::new()));
    let heard = Arc::new(Mutex::new(web_time::Instant::now()));
    let mut readers = Vec::new();
    if let Some(out_pipe) = child.stdout.take() {
        let (text, heard, tx) = (Arc::clone(&text), Arc::clone(&heard), tx.clone());
        readers.push(std::thread::spawn(move || {
            // Windows PowerShell writes in the console's code page: a name or a message with
            // accents mustn't end the reading (and leave the converter writing into a full pipe).
            let mut r = BufReader::new(out_pipe);
            let mut bytes = Vec::new();
            while r.read_until(b'\n', &mut bytes).is_ok_and(|n| n > 0) {
                let line = String::from_utf8_lossy(&bytes).trim_end_matches(['\r', '\n']).to_string();
                bytes.clear();
                *heard.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = web_time::Instant::now();
                if let Some(status) = line.strip_prefix("PRESSROOM: ") {
                    let _ = tx.send(Msg::Status(status.trim().to_string()));
                }
                let mut t = text.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                t.push_str(&line);
                t.push('\n');
            }
        }));
    }
    if let Some(mut err_pipe) = child.stderr.take() {
        let text = Arc::clone(&text);
        readers.push(std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = err_pipe.read_to_end(&mut buf);
            text.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push_str(&String::from_utf8_lossy(&buf));
        }));
    }
    let mut finished_at = None;
    let stopped: Option<&str> = loop {
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => break None,
            Ok(None) => {}
        }
        if CANCEL.load(Ordering::SeqCst) {
            break Some(CANCELLED);
        }
        // The publication is made and the converter has said so, but a Publisher that won't close
        // (one waiting on a printer it can't reach) can hold the converter open: the work is done.
        if io::is_file(out) && text.lock().unwrap_or_else(std::sync::PoisonError::into_inner).lines().any(is_final_line) {
            let at = *finished_at.get_or_insert_with(web_time::Instant::now);
            if at.elapsed() > LINGER {
                let _ = child.kill();
                let _ = child.wait();
                stop_publisher(script);
                break None;
            }
        }
        if heard.lock().unwrap_or_else(std::sync::PoisonError::into_inner).elapsed() > STALL {
            break Some(STUCK);
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    };
    if let Some(why) = stopped {
        let _ = child.kill();
        let _ = child.wait();
        stop_publisher(script);
        for r in readers {
            let _ = r.join();
        }
        let log = text.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
        save_log(&format!("{name}: {why}\n\n{log}"));
        return (Err(why.to_string()), true);
    }
    for r in readers {
        let _ = r.join();
    }
    let log = text.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
    save_log(&format!("{name}\n\n{log}"));
    if io::is_file(out) {
        return (Ok(out.to_string_lossy().to_string()), false);
    }
    if log.contains("Publisher.Application") || log.contains("80040154") {
        return (Err(NO_PUBLISHER.into()), true);
    }
    if let Some(why) = failure(&name, &log) {
        return (Err(why), false);
    }
    let tail: String =
        log.lines().filter(|l| !l.starts_with("PRESSROOM: ")).rev().take(4).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" ");
    (Err(format!("Couldn't bring in {name}: {tail}")), false)
}

/// Why the converter stopped, in words, from its `PRESSROOM-ERROR: kind|step|words` line.
fn failure(name: &str, log: &str) -> Option<String> {
    let line = log.lines().find_map(|l| l.strip_prefix("PRESSROOM-ERROR: "))?;
    let mut parts = line.splitn(3, '|');
    let kind = parts.next()?;
    let step = parts.next().unwrap_or_default().trim();
    let words = parts.next().unwrap_or_default().trim();
    Some(match kind {
        // Publisher's own check turned the file away: nothing PressRoom can do on this computer.
        "open" => format!(
            "The Publisher on this computer can't open {name} either, so PressRoom can't bring it in here. Publisher says: \u{201c}{words}\u{201d}\n\n\
             On a computer whose Publisher opens it, PressRoom can bring it in. Then copy the .pressroom file it makes to this computer."
        ),
        _ if step.is_empty() => format!("Couldn't bring in {name}: {words}"),
        _ => format!("Couldn't bring in {name}. It stopped at \u{201c}{step}\u{201d}: {words}"),
    })
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| p.to_string_lossy().to_string())
}

fn set_job(name: String, of: Option<(usize, usize)>, rx: Receiver<Msg>) {
    CANCEL.store(false, Ordering::SeqCst);
    let status = "Opening the file".to_string();
    *JOB.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Job { name, of, status, rx });
}

/// Start bringing in `path`.
pub fn start(app: &mut DesignApp, path: &str) {
    if busy().is_some() {
        app.status("Another Publisher file is still being brought in. Please wait for it to finish.");
        return;
    }
    let publisher = converter().ok();
    let src = PathBuf::from(path);
    // Already brought in: open that copy, with its edits, rather than make a second one.
    let existing = first_output(&src);
    if io::exists(&existing) {
        let name = existing.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        return match app.open_file("file.open", json!({"path": existing.to_string_lossy()})) {
            Ok(_) => app.status(format!("Opened {name}, the copy brought in earlier. To bring the Publisher file in again, rename {name} first.")),
            Err(e) => app.status(format!("Couldn't open {name}: {e}")),
        };
    }
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
        let (r, _, notes) = convert_any(publisher.as_ref(), &src, &out, &tx);
        let _ = tx.send(Msg::One(r, notes));
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
    let publisher = converter().ok();
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
    std::thread::spawn(move || convert_all(publisher.as_ref(), files, &tx));
    app.status(format!("Bringing in {of} Publisher files\u{2026} this takes about a minute each."));
}

fn convert_all(publisher: Option<&(PathBuf, PathBuf)>, files: Vec<PathBuf>, tx: &Sender<Msg>) {
    let of = files.len();
    let (mut converted, mut failed, mut stopped, mut noted) = (0, Vec::new(), None, Vec::new());
    for (i, src) in files.iter().enumerate() {
        let _ = tx.send(Msg::Next { name: file_name(src), n: i + 1, of });
        let out = free_output(src);
        if file_access::check_read(src).and_then(|()| file_access::check_write(&out)).is_err() {
            failed.push(file_name(src));
            continue;
        }
        match convert_any(publisher, src, &out, tx) {
            (Ok(_), _, notes) => {
                converted += 1;
                if !notes.is_empty() {
                    noted.push(file_name(src));
                }
            }
            // Cancelled.
            (Err(e), true, _) => {
                stopped = Some(e);
                break;
            }
            (Err(_), false, _) => failed.push(file_name(src)),
        }
    }
    let _ = tx.send(Msg::Folder { converted, failed, stopped, noted });
}

fn alert(app: &mut DesignApp, file: &str, message: String) {
    app.ui.dialog = Some(crate::dialogs::Dialog::new("alert", json!({"title": "Convert Publisher Files", "file": file, "message": message})));
}

/// What opening a file says when something didn't come across exactly.
fn reader_message(notes: &[String], out: &str) -> String {
    let shown: Vec<&str> = notes.iter().take(8).map(String::as_str).collect();
    let more = notes.len().saturating_sub(shown.len());
    let mut s = format!("This publication came in, but some things didn't come across exactly:\n\n\u{2022} {}", shown.join("\n\u{2022} "));
    if more > 0 {
        s.push_str(&format!("\n\u{2022} and {more} more"));
    }
    s.push_str(&format!("\n\nThis list is also saved beside the publication: {}", file_name(&notes_path(Path::new(out)))));
    s
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
                    j.status = "Opening the file".into();
                }
            }
            Msg::Status(text) => {
                if let Some(j) = JOB.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_mut() {
                    j.status = text;
                }
            }
            Msg::One(result, notes) => {
                let name = JOB.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take().map(|j| j.name).unwrap_or_default();
                match result {
                    Ok(out) => {
                        super::close_backstage(ctx);
                        match app.open_file("file.open", json!({"path": out})) {
                            Ok(_) => {
                                let _ = app.run("view.fitPage", json!({}));
                                crate::canvas::go_to_page(app, 0);
                                app.status(format!("Brought in \u{201c}{name}\u{201d} from Publisher. It's saved as {out}."));
                                if !notes.is_empty() {
                                    alert(app, &name, reader_message(&notes, &out));
                                }
                            }
                            Err(e) => app.status(format!("Converted {name}, but couldn't open the result: {e}")),
                        }
                    }
                    Err(e) => {
                        app.status(e.clone());
                        if e != CANCELLED {
                            alert(app, &name, e);
                        }
                    }
                }
            }
            Msg::Folder { converted, failed, stopped, noted } => {
                *JOB.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = None;
                app.status(format!("Converted {converted} Publisher files."));
                let mut message = folder_message(converted, &failed, stopped.as_deref());
                if !noted.is_empty() {
                    let shown: Vec<&str> = noted.iter().take(8).map(String::as_str).collect();
                    message.push_str(&format!(
                        "\n\nSome things didn't come across exactly from: {}{}. A \u{201c}what didn't come across\u{201d} file beside each says what.",
                        shown.join(", "),
                        if noted.len() > shown.len() { format!(" and {} more", noted.len() - shown.len()) } else { String::new() }
                    ));
                }
                alert(app, "", message);
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
    fn a_failed_conversion_says_why_in_words() {
        // As on his mother's laptop: her Publisher turned the file away.
        let log = "PRESSROOM: Publisher is opening the file\nPRESSROOM-ERROR: open|Publisher is opening the file|Publisher has detected a problem in the file you are trying to open. To protect your computer, Publisher will not open the file.\nException calling \"Open\"\n";
        let m = failure("News.pub", log).unwrap();
        assert!(m.starts_with("The Publisher on this computer can't open News.pub either"), "{m}");
        assert!(m.contains("\u{201c}Publisher has detected a problem") && m.contains("copy the .pressroom file"), "{m}");
        let m = failure("News.pub", "PRESSROOM-ERROR: other|Reading page 3 of 8|The RPC server is unavailable.\n").unwrap();
        assert_eq!(m, "Couldn't bring in News.pub. It stopped at \u{201c}Reading page 3 of 8\u{201d}: The RPC server is unavailable.");
        assert_eq!(failure("News.pub", "PRESSROOM-ERROR: other||Give -In.").unwrap(), "Couldn't bring in News.pub: Give -In.");
        assert!(failure("News.pub", "PRESSROOM: Reading page 1 of 8\n+ CategoryInfo : NotSpecified").is_none());
    }

    #[test]
    fn the_converter_is_finished_when_it_says_so() {
        assert!(is_final_line("plain.pub -> plain.pressroom: 4 pages, 8 text boxes; 206 steps, 0 problems"));
        assert!(!is_final_line("PRESSROOM: Building the PressRoom publication"));
        assert!(!is_final_line("a -> b"));
    }

    #[test]
    fn what_didnt_come_across_is_said_and_where_it_is_kept() {
        let m = reader_message(&["A bevel was left flat.".to_string()], "C:\\f\\News.pressroom");
        assert!(m.contains("A bevel was left flat."), "{m}");
        assert!(m.contains("News - what didn't come across.txt"), "{m}");
        let many: Vec<String> = (0..10).map(|i| format!("n{i}")).collect();
        assert!(reader_message(&many, "a.pressroom").contains("and 2 more"));
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
