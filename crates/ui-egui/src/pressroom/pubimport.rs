//! Opening a Publisher file (.pub). On a Windows PC where Publisher is installed, the converter
//! installed beside PressRoom (`pub2pressroom.ps1` with the command-line tool) has Publisher read
//! the file and rebuilds it as a .pressroom publication next to the original, which then opens. It
//! runs in the background; the window says so meanwhile. The original .pub is never changed, and an
//! earlier conversion is never overwritten.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, channel};

use serde_json::json;

use crate::DesignApp;

/// The conversion under way: the file's name and where its result arrives.
static JOB: Mutex<Option<(String, Receiver<Result<String, String>>)>> = Mutex::new(None);

pub fn is_pub(path: &str) -> bool {
    path.to_ascii_lowercase().ends_with(".pub")
}

/// The name of the file being brought in, while a conversion runs.
pub fn busy() -> Option<String> {
    JOB.lock().unwrap_or_else(std::sync::PoisonError::into_inner).as_ref().map(|(n, _)| n.clone())
}

/// A .pressroom path beside `pub_path` that doesn't exist yet.
fn free_output(pub_path: &Path) -> PathBuf {
    let dir = pub_path.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = pub_path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Publication".into());
    let first = dir.join(format!("{stem}.pressroom"));
    if !first.exists() {
        return first;
    }
    (2..1000).map(|n| dir.join(format!("{stem} ({n}).pressroom"))).find(|p| !p.exists()).unwrap_or(first)
}

/// Start bringing in `path`.
pub fn start(app: &mut DesignApp, path: &str) {
    if busy().is_some() {
        app.status("Another Publisher file is still being brought in. Please wait for it to finish.");
        return;
    }
    let exe_dir = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf));
    let Some(dir) = exe_dir else {
        app.status("Couldn't find PressRoom's folder to bring in the Publisher file.");
        return;
    };
    let script = dir.join("pub2pressroom.ps1");
    let cli = ["PressRoom-cli.exe", "designcraft-cli.exe"].iter().map(|n| dir.join(n)).find(|p| p.exists());
    let (true, Some(cli)) = (script.exists(), cli) else {
        app.status("The Publisher converter isn't installed with this copy of PressRoom.");
        return;
    };
    let src = PathBuf::from(path);
    let out = free_output(&src);
    let name = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.to_string());
    let (tx, rx) = channel();
    let job_name = name.clone();
    std::thread::spawn(move || {
        let mut cmd = std::process::Command::new("powershell.exe");
        cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"]).arg(&script).arg("-In").arg(&src).arg("-Out").arg(&out).arg("-Cli").arg(&cli);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // No console window flashing up.
            cmd.creation_flags(0x0800_0000);
        }
        let result = match cmd.output() {
            Ok(_) if out.exists() => Ok(out.to_string_lossy().to_string()),
            Ok(o) => {
                let text = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
                let tail: String = text.lines().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" ");
                Err(if text.contains("Publisher.Application") || text.contains("80040154") {
                    "Publisher isn't installed on this computer, so its files can't be read here. Bring the file to a PC with Publisher (Jayden's) to convert it.".to_string()
                } else {
                    format!("Couldn't bring in {job_name}: {tail}")
                })
            }
            Err(e) => Err(format!("Couldn't start the Publisher converter: {e}")),
        };
        let _ = tx.send(result);
    });
    *JOB.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = Some((name.clone(), rx));
    app.status(format!("Bringing in \u{201c}{name}\u{201d} from Publisher\u{2026} this can take a minute."));
}

/// Each frame: open the result when the conversion is done.
pub fn poll(app: &mut DesignApp, ctx: &egui::Context) {
    let done = {
        let mut g = JOB.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let ready = g.as_ref().and_then(|(_, rx)| rx.try_recv().ok());
        if ready.is_some() {
            g.take().map(|(n, _)| n).zip(ready)
        } else {
            None
        }
    };
    match done {
        Some((name, Ok(out))) => {
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
        Some((_, Err(e))) => app.status(e),
        None => {
            if busy().is_some() {
                ctx.request_repaint_after(std::time::Duration::from_millis(400));
            }
        }
    }
}
