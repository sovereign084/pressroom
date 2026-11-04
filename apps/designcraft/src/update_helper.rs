//! Windows: the update helper, which is a copy of this program started with
//! [`update_job::HELPER_FLAG`]. It shows one small native window for the whole update, so PressRoom
//! never just disappears; the steps are in `designcraft_ui_egui::pressroom::update_job`. The window
//! needs no graphics driver, so a driver that keeps PressRoom from starting doesn't stop its update,
//! and a window that can't be made at all doesn't stop it either.

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use designcraft_ui_egui::pressroom::update_job::{self as job, Helper, Record};
use winsafe::{self as w, co, gui, prelude::*};

/// What the window shows and what was answered, shared with the thread doing the work.
#[derive(Default)]
struct Shared {
    text: String,
    /// The window is offering to put PressRoom back.
    asking: bool,
    answer: Option<bool>,
    finished: bool,
    no_window: bool,
}

type View = Arc<Mutex<Shared>>;

fn with<R>(view: &View, f: impl FnOnce(&mut Shared) -> R) -> R {
    f(&mut view.lock().unwrap_or_else(PoisonError::into_inner))
}

struct RealHelper {
    view: View,
    record_path: PathBuf,
    msi: PathBuf,
    exe: PathBuf,
}

impl Helper for RealHelper {
    fn wait_for_close(&mut self, pid: u32, within: Duration) -> bool {
        craft_update::process::wait_for_exit(pid, within)
    }

    fn other_windows_open(&mut self) -> bool {
        craft_update::process::is_running(&self.exe)
    }

    fn run_installer(&mut self, repair: bool) -> Result<i32, String> {
        let root = std::env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
        let mut cmd = std::process::Command::new(root.join("System32").join("msiexec.exe"));
        cmd.arg("/i").arg(&self.msi).args(["/passive", "/norestart", "/l*v"]).arg(self.msi.with_extension("log"));
        // Only what is missing or older. Forcing every file rewrites the fonts as well, which took
        // nearly 3 minutes and then failed on the one font a program held (2026-11-03).
        if repair {
            cmd.args(["REINSTALL=ALL", "REINSTALLMODE=omus"]);
        }
        let mut child = cmd.spawn().map_err(|e| e.to_string())?;
        let started = Instant::now();
        let mut noted = false;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return status.code().ok_or_else(|| "the installer ended without an answer".to_string()),
                Ok(None) => {}
                Err(e) => return Err(e.to_string()),
            }
            if !noted && started.elapsed() > Duration::from_secs(120) {
                noted = true;
                self.say(job::SAY_LONG);
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    fn program_exists(&self) -> bool {
        self.exe.is_file()
    }

    fn open_program(&mut self) -> Result<(), String> {
        std::process::Command::new(&self.exe).spawn().map(|_| ()).map_err(|e| e.to_string())
    }

    fn sleep(&mut self, how_long: Duration) {
        std::thread::sleep(how_long);
    }

    fn say(&mut self, text: &str) {
        with(&self.view, |s| s.text = text.to_string());
    }

    fn ask_reinstall(&mut self, text: &str) -> bool {
        with(&self.view, |s| {
            s.text = text.to_string();
            s.answer = None;
            s.asking = true;
        });
        loop {
            if let Some(answer) = with(&self.view, |s| if s.no_window { Some(false) } else { s.answer }) {
                with(&self.view, |s| s.asking = false);
                return answer;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn save(&mut self, record: &Record) {
        if let Err(e) = job::write_record(&self.record_path, record) {
            log::warn!("the update helper couldn't write its record: {e}");
        }
    }
}

/// The window, until the work is done. An error when it can't be made.
fn window(view: &View) -> w::AnyResult<i32> {
    const TIMER: usize = 1;
    let wnd = gui::WindowMain::new(gui::WindowMainOpts {
        title: "Updating PressRoom",
        size: gui::dpi(440, 170),
        style: co::WS::CAPTION | co::WS::SYSMENU | co::WS::CLIPCHILDREN | co::WS::BORDER | co::WS::VISIBLE,
        ..Default::default()
    });
    let label = gui::Label::new(&wnd, gui::LabelOpts { text: "", position: gui::dpi(16, 14), size: gui::dpi(408, 100), ..Default::default() });
    let reinstall = gui::Button::new(
        &wnd,
        gui::ButtonOpts { text: "&Reinstall PressRoom", position: gui::dpi(196, 126), width: gui::dpi_x(150), ..Default::default() },
    );
    let close = gui::Button::new(&wnd, gui::ButtonOpts { text: "&Close", position: gui::dpi(354, 126), width: gui::dpi_x(70), ..Default::default() });

    let (w2, b1, b2) = (wnd.clone(), reinstall.clone(), close.clone());
    wnd.on().wm_create(move |_| {
        b1.hwnd().ShowWindow(co::SW::HIDE);
        b2.hwnd().ShowWindow(co::SW::HIDE);
        w2.hwnd().SetTimer(TIMER, 200, None)?;
        Ok(0)
    });

    let (w2, l2, b1, b2, v2) = (wnd.clone(), label.clone(), reinstall.clone(), close.clone(), view.clone());
    let shown = std::cell::RefCell::new((String::new(), false));
    wnd.on().wm_timer(TIMER, move || {
        let (text, asking, finished) = with(&v2, |s| (s.text.clone(), s.asking, s.finished));
        let mut last = shown.borrow_mut();
        if last.0 != text {
            l2.hwnd().SetWindowText(&text)?;
            last.0 = text;
        }
        if last.1 != asking {
            let how = if asking { co::SW::SHOW } else { co::SW::HIDE };
            b1.hwnd().ShowWindow(how);
            b2.hwnd().ShowWindow(how);
            last.1 = asking;
        }
        if finished {
            w2.close();
        }
        Ok(())
    });

    let v2 = view.clone();
    reinstall.on().bn_clicked(move || {
        with(&v2, |s| s.answer = Some(true));
        Ok(())
    });
    let v2 = view.clone();
    close.on().bn_clicked(move || {
        with(&v2, |s| s.answer = Some(false));
        Ok(())
    });

    // The X does nothing while the installer runs: it closes by itself. While it asks, the X is No.
    let (w2, v2) = (wnd.clone(), view.clone());
    wnd.on().wm_close(move || {
        let finished = with(&v2, |s| {
            if s.asking {
                s.answer = Some(false);
            }
            s.finished
        });
        if finished {
            w2.hwnd().DestroyWindow()?;
        }
        Ok(())
    });
    wnd.run_main(None)
}

/// Runs the update when this program was started as its helper: `None` otherwise. `args` are the
/// program's arguments without its name.
pub fn run(args: &[OsString]) -> Option<i32> {
    if args.first().is_none_or(|a| a != job::HELPER_FLAG) {
        return None;
    }
    let path = PathBuf::from(args.get(1)?);
    let Some(mut record) = job::read_record(&path) else { return Some(1) };
    if job::acceptable(&record).is_err() {
        return Some(1);
    }
    // PressRoom is waiting for this before it closes.
    record.helper_pid = Some(std::process::id());
    record.stage = job::Stage::HelperRunning;
    if job::write_record(&path, &record).is_err() {
        // PressRoom will see no answer, stay open and say so.
        return Some(1);
    }
    let view: View = Arc::new(Mutex::new(Shared { text: job::SAY_WAITING.to_string(), ..Shared::default() }));
    let mut helper = RealHelper { view: view.clone(), record_path: path, msi: record.msi.clone(), exe: record.exe.clone() };
    let worker = std::thread::spawn(move || job::run_update(&mut record, &mut helper));
    let done = view.clone();
    // Told when the work ends, so the window closes then, a panic in the work included.
    let watcher = std::thread::spawn(move || {
        let ended = worker.join().is_ok();
        with(&done, |s| s.finished = true);
        ended
    });
    let shown = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| window(&view)));
    if !matches!(shown, Ok(Ok(_))) {
        with(&view, |s| s.no_window = true);
    }
    let ended = watcher.join().unwrap_or(false);
    Some(i32::from(!ended))
}
