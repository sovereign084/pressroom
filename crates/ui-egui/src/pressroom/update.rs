//! PressRoom's updates. At start, and from File › Account › Check for Updates, PressRoom asks
//! GitHub for its newest release (`REPO`); a newer one shows in a message bar under the ribbon, the
//! way Office offers its updates. Update Now downloads the installer, checked against the SHA-256
//! GitHub publishes for it; Restart and Update (once everything is saved) starts the update helper,
//! closes PressRoom when the helper says it is running, and the helper installs and opens PressRoom
//! again (`update_job`). The next start says how it went, here in the bar, and opens again what was
//! open. DesignCraft's own updater stays off (`updates_ui`): it only installs the Craft App Suite's
//! packages.

#[cfg(windows)]
use std::path::Path;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use egui::{Color32, Sense, Stroke, Ui, vec2};

use super::update_job::{self as job, Arrival};
use crate::DesignApp;

/// PressRoom's version, stamped in by `packaging/pressroom/build-installer.ps1`. Developer builds
/// have none and never look for updates.
pub const VERSION: Option<&str> = option_env!("PRESSROOM_VERSION");
/// Where PressRoom's releases are published. Each carries `pressroom-<version>-windows-x64.msi`.
pub const REPO: &str = "sovereign084/pressroom";

const BAR_H: f32 = 34.0;
/// Office's message bar: pale yellow, or a dark olive in the dark themes.
fn bar_colors() -> (Color32, Color32) {
    if super::pal().dark {
        (Color32::from_rgb(61, 56, 30), Color32::from_rgb(98, 90, 52))
    } else {
        (Color32::from_rgb(255, 244, 206), Color32::from_rgb(230, 214, 160))
    }
}

/// Where an update stands.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Phase {
    #[default]
    Idle,
    /// Asking GitHub; `manual` when the user asked, so the answer is shown even when it's "none".
    Checking {
        manual: bool,
    },
    UpToDate,
    /// A newer release with an installer for this computer.
    Available {
        version: String,
        notes_url: Option<String>,
    },
    Downloading {
        version: String,
        done: u64,
        total: u64,
    },
    /// The installer is downloaded and its checksum matches.
    Ready {
        version: String,
        msi: PathBuf,
    },
    /// Restart and Update was clicked: PressRoom closes when the helper says it is running.
    Starting {
        version: String,
    },
    /// An update that didn't go in, with the installer it kept: Try Again runs it again.
    NotInstalled {
        message: String,
        version: String,
        msi: PathBuf,
    },
    /// The last update worked, or is still going: words for the bar.
    Note(String),
    Failed(String),
}

/// The release's installer. (The web build never downloads it.)
#[derive(Clone, Debug)]
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
struct Installer {
    url: String,
    size: u64,
    sha256: String,
    name: String,
}

#[derive(Default)]
struct Inner {
    phase: Phase,
    installer: Option<Installer>,
    /// The bar was closed; a new check shows it again.
    dismissed: bool,
    /// The last update's record has been looked at.
    arrived: bool,
    ctx: Option<egui::Context>,
}

/// One window's update state, shared with the threads that check and download.
#[derive(Clone, Default)]
pub struct Updates(Arc<Mutex<Inner>>);

impl Updates {
    fn with<R>(&self, f: impl FnOnce(&mut Inner) -> R) -> R {
        f(&mut self.0.lock().unwrap_or_else(PoisonError::into_inner))
    }

    pub fn phase(&self) -> Phase {
        self.with(|s| s.phase.clone())
    }

    fn set(&self, phase: Phase) {
        let ctx = self.with(|s| {
            s.phase = phase;
            s.ctx.clone()
        });
        if let Some(ctx) = ctx {
            ctx.request_repaint();
        }
    }

    pub fn dismiss(&self) {
        self.with(|s| s.dismissed = true);
    }

    /// Whether the bar has something to say.
    pub fn showing(&self) -> bool {
        self.with(|s| !s.dismissed && !matches!(s.phase, Phase::Idle | Phase::Checking { manual: false }))
    }

    /// A downloaded update, ready to install: the control channel's way in, for testing the real
    /// update path with an installer that isn't published, and the tests'.
    pub fn downloaded(&self, version: &str, msi: PathBuf) {
        self.with(|s| s.dismissed = false);
        self.set(Phase::Ready { version: version.into(), msi });
    }

    /// A newer release, as a check would find it (tests).
    #[cfg(test)]
    pub fn offer(&self, version: &str) {
        self.with(|s| {
            s.installer =
                Some(Installer { url: String::new(), size: 0, sha256: String::new(), name: format!("pressroom-{version}-windows-x64.msi") });
            s.dismissed = false;
        });
        self.set(Phase::Available { version: version.into(), notes_url: None });
    }

    /// Ask GitHub for a newer release, in the background. `manual`: the user asked, so "you're up
    /// to date" and failures are shown too; the check at start stays quiet about them.
    pub fn check(&self, ctx: &egui::Context, manual: bool) {
        let busy = self.with(|s| {
            s.ctx = Some(ctx.clone());
            if manual {
                s.dismissed = false;
            }
            // What the last update did stays in the bar until it is read: the check at start
            // would otherwise replace it with the same offer again.
            matches!(s.phase, Phase::Checking { .. } | Phase::Downloading { .. } | Phase::Starting { .. })
        }) || (!manual && self.has_update_news());
        if busy {
            return;
        }
        let Some(current) = VERSION else {
            if manual {
                self.set(Phase::Failed("This copy of PressRoom was built without a version number, so it can't look for updates.".into()));
            }
            return;
        };
        self.set(Phase::Checking { manual });
        #[cfg(not(target_arch = "wasm32"))]
        {
            let me = self.clone();
            std::thread::spawn(move || {
                let found = newest(current);
                // The check at start begins before the last update's record is read: its answer
                // doesn't replace what that update has to say.
                if !manual && me.has_update_news() {
                    return;
                }
                let quiet = |phase: Phase| if manual { phase } else { Phase::Idle };
                match found {
                    Ok(Some((version, notes_url, installer))) => {
                        me.with(|s| s.installer = Some(installer));
                        me.set(Phase::Available { version, notes_url });
                    }
                    Ok(None) => me.set(quiet(Phase::UpToDate)),
                    Err(e) => {
                        log::warn!("PressRoom update check: {e}");
                        me.set(quiet(Phase::Failed(format!("Couldn't check for updates: {e}"))));
                    }
                }
            });
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = current;
            self.set(Phase::Failed("PressRoom can't update itself in a web browser.".into()));
        }
    }

    /// Whether the bar is saying what the last update did.
    fn has_update_news(&self) -> bool {
        self.with(|s| matches!(s.phase, Phase::NotInstalled { .. } | Phase::Note(_)))
    }

    /// Download the offered installer, in the background.
    pub fn download(&self) {
        let (Phase::Available { version, .. }, Some(installer)) = self.with(|s| (s.phase.clone(), s.installer.clone())) else {
            return;
        };
        self.set(Phase::Downloading { version: version.clone(), done: 0, total: installer.size });
        #[cfg(not(target_arch = "wasm32"))]
        {
            let me = self.clone();
            std::thread::spawn(move || {
                let dest = std::env::temp_dir().join("PressRoom updates").join(&installer.name);
                let result = craft_update::Http::new(&agent()).and_then(|http| {
                    let stop = craft_update::Stop::default();
                    http.download(&installer.url, &dest, installer.size, &installer.sha256, &stop, &mut |t| {
                        let (craft_update::http::Transfer::Bytes { done, total } | craft_update::http::Transfer::Reconnecting { done, total }) = t;
                        me.set(Phase::Downloading { version: version.clone(), done, total });
                    })
                });
                me.set(match result {
                    Ok(()) => Phase::Ready { version, msi: dest },
                    Err(e) => Phase::Failed(format!("Couldn't download the update: {e}")),
                });
            });
        }
        #[cfg(target_arch = "wasm32")]
        let _ = (version, installer);
    }

    /// Start the update: PressRoom closes when the helper says it is running, and the helper opens
    /// it again. Refused while a publication has unsaved changes.
    pub fn install(&self, app: &mut DesignApp, ctx: &egui::Context) {
        let Phase::Ready { version, msi } = self.phase() else { return };
        let unsaved: Vec<String> = app
            .session
            .documents()
            .iter()
            .filter(|d| d.is_dirty())
            .map(|d| {
                d.path
                    .as_deref()
                    .and_then(|p| std::path::Path::new(p).file_name())
                    .map_or_else(|| "a new publication".to_string(), |n| n.to_string_lossy().to_string())
            })
            .collect();
        if !unsaved.is_empty() {
            let message = format!(
                "Please save your work first: {}.\n\nPressRoom closes to update, and opens your saved publications again afterwards.",
                unsaved.join(", ")
            );
            app.status("Save your publications first, then click Restart and Update.");
            app.ui.dialog = Some(crate::dialogs::Dialog::new("alert", serde_json::json!({"title": "Restart and Update", "message": message})));
            return;
        }
        let publications: Vec<String> = app.session.documents().iter().filter_map(|d| d.path.clone()).collect();
        self.with(|s| s.dismissed = false);
        self.set(Phase::Starting { version: version.clone() });
        match start_helper(&version, &msi, publications) {
            Ok(handed) => self.watch_helper(handed, ctx.clone(), version, msi),
            Err(e) => {
                log::warn!("PressRoom's update couldn't start: {e}");
                let message = format!("{COULDNT_START} You're still on {} and your work is safe.", VERSION.unwrap_or_default());
                self.set(Phase::NotInstalled { message, version, msi });
            }
        }
    }

    /// Try Again: the installer the update kept, or a fresh check when it has gone.
    pub fn retry(&self, app: &mut DesignApp, ctx: &egui::Context) {
        let Phase::NotInstalled { version, msi, .. } = self.phase() else { return };
        if !msi.is_file() {
            self.set(Phase::Idle);
            return self.check(ctx, true);
        }
        self.set(Phase::Ready { version, msi });
        self.install(app, ctx);
    }

    /// Follows the helper: PressRoom closes once it says it is running. If it never does (something
    /// stopped it, such as antivirus), PressRoom stays open and says so. Afterwards, if PressRoom
    /// is somehow still open when the helper gives up on it, the bar says that too.
    fn watch_helper(&self, handed: Handed, ctx: egui::Context, version: String, msi: PathBuf) {
        let me = self.clone();
        std::thread::spawn(move || {
            let Handed { mut child, record } = handed;
            let started = web_time::Instant::now();
            let safe = format!("You're still on {} and your work is safe.", VERSION.unwrap_or_default());
            loop {
                let stage = job::read_record(&record).map(|r| r.stage).unwrap_or_default();
                if stage != job::Stage::Started {
                    break;
                }
                let exited = matches!(child.try_wait(), Ok(Some(_)) | Err(_));
                if exited || started.elapsed() > HELPER_TIMEOUT {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = std::fs::remove_file(&record);
                    if let Some(dir) = record.parent() {
                        let _ = std::fs::remove_file(dir.join(job::HELPER_NAME));
                    }
                    me.set(Phase::NotInstalled { message: format!("{COULDNT_START} {safe}"), version, msi });
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            ctx.request_repaint();
            // Normally this process ends here. If it doesn't, the helper's answer still reaches it.
            loop {
                std::thread::sleep(std::time::Duration::from_millis(500));
                if let Some(r) = job::read_record(&record).filter(|r| r.stage == job::Stage::Finished) {
                    if let Arrival::NotUpdated { message, version, msi, .. } = job::arrival(&r, VERSION.unwrap_or_default(), false) {
                        let _ = std::fs::remove_file(&record);
                        me.set(Phase::NotInstalled { message, version, msi });
                    }
                    return;
                }
            }
        });
    }

    /// Once, at start: what the last update did, in the bar, and what was open opens again.
    fn arrive(&self, app: &mut DesignApp, ctx: &egui::Context) {
        if self.with(|s| std::mem::replace(&mut s.arrived, true)) {
            return;
        }
        self.with(|s| s.ctx = Some(ctx.clone()));
        // A developer's build (and the tests) has no update to report.
        let Some(current) = VERSION else { return };
        let dir = job::update_dir();
        let path = job::record_path(&dir);
        let Some(record) = job::read_record(&path) else { return };
        let alive = record.helper_pid.is_some_and(helper_alive);
        let reopen = match job::arrival(&record, current, alive) {
            Arrival::Updated { message, publications } => {
                self.set(Phase::Note(message));
                clean_up_after_update(&dir);
                publications
            }
            Arrival::NotUpdated { message, version, msi, publications } => {
                self.set(Phase::NotInstalled { message, version, msi });
                // The installer and its log stay, for Try Again and for finding out why.
                let _ = std::fs::remove_file(&path);
                let _ = std::fs::remove_file(dir.join(job::HELPER_NAME));
                publications
            }
            Arrival::StillUpdating { message } => {
                self.set(Phase::Note(message));
                Vec::new()
            }
        };
        for file in reopen {
            if designcraft_engine::io::exists(&file)
                && let Err(e) = app.open_file("file.open", serde_json::json!({"path": file}))
            {
                app.status(format!("Couldn't open {file} again: {e}"));
            }
        }
    }
}

/// What the bar says when the helper never got going, whichever way it was stopped.
const COULDNT_START: &str = "The update couldn't start. Something on this computer, such as antivirus, may have stopped it.";

/// How long the helper has to say it is running: antivirus can hold a new program a good while.
const HELPER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// A helper that has been started: the process, and the record it will answer in.
struct Handed {
    child: std::process::Child,
    record: PathBuf,
}

/// Whether the helper process is still running.
#[cfg(windows)]
fn helper_alive(pid: u32) -> bool {
    !craft_update::process::wait_for_exit(pid, std::time::Duration::ZERO)
}

#[cfg(not(windows))]
fn helper_alive(_pid: u32) -> bool {
    false
}

/// The update worked: nothing it kept is needed any more. A file still held is removed next time.
fn clean_up_after_update(dir: &std::path::Path) {
    let dir = dir.to_path_buf();
    #[cfg(not(target_arch = "wasm32"))]
    std::thread::spawn(move || {
        for _ in 0..20 {
            if std::fs::remove_dir_all(&dir).is_ok() || !dir.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        let _ = std::fs::remove_dir_all(std::env::temp_dir().join("PressRoom updates"));
    });
    #[cfg(target_arch = "wasm32")]
    let _ = dir;
}

#[cfg(not(target_arch = "wasm32"))]
fn agent() -> String {
    format!("PressRoom/{}", VERSION.unwrap_or("dev"))
}

/// The newest release if it's newer than `current`: its version, release-notes page and installer.
#[cfg(not(target_arch = "wasm32"))]
fn newest(current: &str) -> Result<Option<(String, Option<String>, Installer)>, String> {
    let app = craft_update::App::new("pressroom", "PressRoom", "Desktop publishing", REPO);
    let http = craft_update::Http::new(&agent())?;
    let Some(release) = craft_update::check_for_update(&app, &http, current)? else { return Ok(None) };
    let platform = craft_update::Platform::current().ok_or("this computer's system isn't one PressRoom is built for")?;
    let (asset, _) = craft_update::pick_asset(&release, &[app.id], platform, &[craft_update::PackageKind::Msi])
        .ok_or_else(|| format!("PressRoom {} has no installer for this computer", release.version_label()))?;
    let sha256 = asset.sha256.clone().ok_or("the update's installer has no published checksum")?;
    if !craft_update::release::valid_file_name(&asset.name) {
        return Err("the update's installer has an unexpected name".into());
    }
    let notes = craft_update::release::safe_release_html_url(REPO, &release.html_url);
    Ok(Some((release.version_label(), notes, Installer { url: asset.url.clone(), size: asset.size, sha256, name: asset.name.clone() })))
}

/// Keeps the installer and a copy of this program in the update's folder, writes the record, and
/// starts the copy as the helper. The record is written first, so a helper that dies with nothing
/// said is noticed at the next start.
#[cfg(windows)]
fn start_helper(version: &str, msi: &Path, publications: Vec<String>) -> Result<Handed, String> {
    use std::os::windows::process::CommandExt;
    let dir = job::update_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("couldn't make its folder ({e})"))?;
    let name = msi.file_name().ok_or("the installer has no file name")?;
    let kept = dir.join(name);
    if kept != msi {
        std::fs::copy(msi, &kept).map_err(|e| format!("couldn't keep the installer ({e})"))?;
    }
    let exe = std::env::current_exe().map_err(|e| format!("couldn't find PressRoom's program file ({e})"))?;
    let helper = dir.join(job::HELPER_NAME);
    std::fs::copy(&exe, &helper).map_err(|e| format!("couldn't prepare it ({e})"))?;
    let record = job::Record {
        from: VERSION.unwrap_or_default().to_string(),
        to: version.to_string(),
        msi: kept,
        exe,
        pid: std::process::id(),
        publications,
        ..job::Record::default()
    };
    let path = job::record_path(&dir);
    job::write_record(&path, &record).map_err(|e| format!("couldn't write down what it's doing ({e})"))?;
    let spawned = std::process::Command::new(&helper)
        .arg(job::HELPER_FLAG)
        .arg(&path)
        // Its own process group, so it outlives PressRoom.
        .creation_flags(0x0000_0200)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    match spawned {
        Ok(child) => Ok(Handed { child, record: path }),
        Err(e) => {
            let _ = std::fs::remove_file(&path);
            Err(format!("couldn't start it ({e})"))
        }
    }
}

#[cfg(not(windows))]
fn start_helper(_version: &str, _msi: &std::path::Path, _publications: Vec<String>) -> Result<Handed, String> {
    Err("PressRoom installs its updates on Windows only".into())
}

/// A message-bar button: white, with a grey border, as in Office.
fn button(ui: &mut Ui, label: &str) -> bool {
    let text = egui::RichText::new(label).size(12.0).color(super::pal().text);
    ui.add(egui::Button::new(text).fill(super::pal().card).stroke(Stroke::new(1.0, super::pal().hover_border)).min_size(vec2(0.0, 22.0))).clicked()
}

/// The message bar under the ribbon (under the title bar on the File screen), while there's an
/// update to offer or a check's answer to give.
pub fn bar(app: &mut DesignApp, ui: &mut Ui) {
    let updates = app.updates_rt.pressroom.clone();
    updates.arrive(app, ui.ctx());
    if !updates.showing() {
        return;
    }
    let phase = updates.phase();
    let (title, message) = match &phase {
        Phase::Idle | Phase::Checking { manual: false } => return,
        Phase::Checking { manual: true } => ("UPDATES", "Looking for a newer version of PressRoom\u{2026}".to_string()),
        Phase::UpToDate => ("UPDATES", format!("You're up to date. PressRoom {} is the newest version.", VERSION.unwrap_or(""))),
        Phase::Available { version, .. } => ("UPDATES AVAILABLE", format!("A new version of PressRoom ({version}) is ready to install.")),
        Phase::Downloading { version, done, total } => {
            let pct = (*done).saturating_mul(100).checked_div(*total).unwrap_or(0);
            ("UPDATES AVAILABLE", format!("Downloading PressRoom {version}\u{2026} {pct}%"))
        }
        Phase::Ready { version, .. } => {
            ("UPDATES READY", format!("PressRoom {version} is downloaded. PressRoom will close, update and open again with your publications saved."))
        }
        Phase::Starting { version } => ("UPDATES READY", format!("Starting the update to PressRoom {version}\u{2026}")),
        Phase::NotInstalled { message, .. } => ("UPDATE DIDN'T INSTALL", message.clone()),
        Phase::Note(message) => ("UPDATES", message.clone()),
        Phase::Failed(e) => ("UPDATES", e.clone()),
    };
    let (bar_fill, bar_edge) = bar_colors();
    egui::Panel::top("pr_update_bar").exact_size(BAR_H).frame(egui::Frame::NONE.fill(bar_fill)).show(ui, |ui| {
        let full = ui.max_rect();
        ui.painter().line_segment([full.left_bottom(), full.right_bottom()], Stroke::new(1.0, bar_edge));
        ui.style_mut().visuals = ui.ctx().global_style().visuals.clone();
        ui.horizontal_centered(|ui| {
            ui.add_space(12.0);
            ui.label(egui::RichText::new(title).size(12.0).strong().color(super::pal().text));
            ui.add_space(10.0);
            ui.label(egui::RichText::new(message).size(12.0).color(super::pal().text));
            ui.add_space(10.0);
            match &phase {
                Phase::Available { notes_url, .. } => {
                    if button(ui, "Update Now") {
                        updates.download();
                    }
                    if let Some(url) = notes_url
                        && button(ui, "What's New")
                    {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                    }
                }
                Phase::Ready { .. } => {
                    let restart = button(ui, "Restart and Update");
                    if restart {
                        updates.install(app, ui.ctx());
                    }
                }
                Phase::NotInstalled { .. } => {
                    let again = button(ui, "Try Again");
                    if again {
                        updates.retry(app, ui.ctx());
                    }
                }
                Phase::Failed(_) => {
                    let again = button(ui, "Try Again");
                    if again {
                        updates.check(ui.ctx(), true);
                    }
                }
                _ => {}
            }
            // Close, at the right end.
            let x = egui::Rect::from_center_size(egui::pos2(full.max.x - 20.0, full.center().y), vec2(22.0, 22.0));
            let resp = ui.interact(x, egui::Id::unique("pr_update_close"), Sense::click()).on_hover_text("Close");
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Close the update message"));
            if resp.hovered() {
                ui.painter().rect_filled(x, 2.0, bar_edge);
            }
            let (c, d) = (x.center(), 4.0);
            ui.painter().line_segment([c + vec2(-d, -d), c + vec2(d, d)], Stroke::new(1.2, super::pal().text));
            ui.painter().line_segment([c + vec2(-d, d), c + vec2(d, -d)], Stroke::new(1.2, super::pal().text));
            if resp.clicked() {
                updates.dismiss();
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_build_without_a_version_never_looks_but_says_so_when_asked() {
        if VERSION.is_some() {
            return;
        }
        let ctx = egui::Context::default();
        let quiet = Updates::default();
        quiet.check(&ctx, false);
        assert_eq!(quiet.phase(), Phase::Idle);
        assert!(!quiet.showing());
        let asked = Updates::default();
        asked.check(&ctx, true);
        assert!(matches!(asked.phase(), Phase::Failed(m) if m.contains("without a version")));
        assert!(asked.showing());
    }

    #[test]
    fn the_check_at_start_doesnt_replace_what_the_last_update_said() {
        let ctx = egui::Context::default();
        let u = Updates::default();
        let phase = Phase::NotInstalled { message: "didn't install".into(), version: "9.9.9".into(), msi: PathBuf::from("a.msi") };
        u.set(phase.clone());
        u.check(&ctx, false);
        assert_eq!(u.phase(), phase);
        u.set(Phase::Note("updated".into()));
        u.check(&ctx, false);
        assert_eq!(u.phase(), Phase::Note("updated".into()));
        // The check that began before the record was read is what the thread looks at this for:
        // on 0.7.90 its answer wiped the message a moment after it showed.
        assert!(u.has_update_news());
        u.set(Phase::UpToDate);
        assert!(!u.has_update_news());
    }

    #[test]
    fn closing_the_bar_hides_it_until_the_next_offer() {
        let u = Updates::default();
        u.offer("9.9.9");
        assert!(u.showing());
        u.dismiss();
        assert!(!u.showing());
        u.offer("9.9.10");
        assert!(u.showing());
    }
}
