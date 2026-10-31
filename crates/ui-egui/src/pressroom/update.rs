//! PressRoom's updates. At start, and from File › Account › Check for Updates, PressRoom asks
//! GitHub for its newest release (`REPO`); a newer one shows in a message bar under the ribbon, the
//! way Office offers its updates. Update Now downloads the installer, checked against the SHA-256
//! GitHub publishes for it; Restart and Update closes PressRoom (once everything is saved), runs the
//! installer and opens PressRoom again. DesignCraft's own updater stays off (`updates_ui`): it only
//! installs the Craft App Suite's packages.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use egui::{Color32, Sense, Stroke, Ui, vec2};

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

    /// A downloaded update, ready to install (tests).
    #[cfg(test)]
    pub fn downloaded(&self, version: &str, msi: PathBuf) {
        self.with(|s| s.dismissed = false);
        self.set(Phase::Ready { version: version.into(), msi });
    }

    /// Ask GitHub for a newer release, in the background. `manual`: the user asked, so "you're up
    /// to date" and failures are shown too; the check at start stays quiet about them.
    pub fn check(&self, ctx: &egui::Context, manual: bool) {
        let busy = self.with(|s| {
            s.ctx = Some(ctx.clone());
            if manual {
                s.dismissed = false;
            }
            matches!(s.phase, Phase::Checking { .. } | Phase::Downloading { .. })
        });
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

    /// Close PressRoom and install the downloaded update, which opens PressRoom again when it's
    /// done. Refused while a publication has unsaved changes.
    pub fn install(&self, app: &mut DesignApp, ctx: &egui::Context) {
        let Phase::Ready { msi, .. } = self.phase() else { return };
        if app.session.documents().iter().any(|d| d.is_dirty()) {
            app.status("Save your publications first, then click Restart and Update.");
            return;
        }
        match spawn_installer(&msi) {
            Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            Err(e) => self.set(Phase::Failed(e)),
        }
    }
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

/// The PowerShell that waits for PressRoom (process `pid`) to close, runs the installer with a
/// progress bar and no questions, and opens PressRoom again. The installer's log goes beside it
/// (the .msi's name with .log), for when an update doesn't go in.
pub fn helper_script(pid: u32, msi: &Path, exe: &Path) -> String {
    // Single-quoted PowerShell strings: a quote inside is written twice.
    let quote = |p: &Path| p.to_string_lossy().replace('\'', "''");
    format!(
        "Wait-Process -Id {pid} -ErrorAction SilentlyContinue; Start-Process -FilePath 'msiexec.exe' -ArgumentList '/i \"{}\" /passive /norestart /l*v \"{}\"' -Wait; Start-Process -FilePath '{}'",
        quote(msi),
        quote(&msi.with_extension("log")),
        quote(exe)
    )
}

/// `script` for PowerShell's `-EncodedCommand`: UTF-16LE in base64, which needs no quoting.
pub fn encode_command(script: &str) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let at = |i: usize| u32::from(chunk.get(i).copied().unwrap_or(0));
        let n = (at(0) << 16) | (at(1) << 8) | at(2);
        for (k, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            out.push(if k <= chunk.len() { char::from(ALPHABET[((n >> shift) & 63) as usize]) } else { '=' });
        }
    }
    out
}

#[cfg(windows)]
fn spawn_installer(msi: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let exe = std::env::current_exe().map_err(|e| format!("Couldn't find PressRoom's program file: {e}"))?;
    let script = helper_script(std::process::id(), msi, &exe);
    std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden", "-EncodedCommand"])
        .arg(encode_command(&script))
        // No console window, and its own process group so it outlives PressRoom.
        .creation_flags(0x0800_0000 | 0x0000_0200)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Couldn't start the update: {e}"))
}

#[cfg(not(windows))]
fn spawn_installer(_msi: &Path) -> Result<(), String> {
    Err("PressRoom installs its updates on Windows only.".into())
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
    fn commands_are_encoded_as_powershell_reads_them() {
        assert_eq!(encode_command("a"), "YQA=");
        assert_eq!(encode_command("Wait-Process -Id 42"), "VwBhAGkAdAAtAFAAcgBvAGMAZQBzAHMAIAAtAEkAZAAgADQAMgA=");
        assert_eq!(encode_command(""), "");
    }

    #[test]
    fn the_helper_waits_installs_and_reopens_with_quotes_doubled() {
        let s = helper_script(
            42,
            Path::new(r"C:\Users\O'Brien\AppData\Local\Temp\PressRoom updates\pressroom-1.0.0-windows-x64.msi"),
            Path::new(r"C:\Apps\PressRoom.exe"),
        );
        assert!(s.starts_with("Wait-Process -Id 42 "), "{s}");
        assert!(
            s.contains(r#"'/i "C:\Users\O''Brien\AppData\Local\Temp\PressRoom updates\pressroom-1.0.0-windows-x64.msi" /passive /norestart /l*v "C:\Users\O''Brien\AppData\Local\Temp\PressRoom updates\pressroom-1.0.0-windows-x64.log"' -Wait"#),
            "{s}"
        );
        assert!(s.ends_with(r"Start-Process -FilePath 'C:\Apps\PressRoom.exe'"), "{s}");
    }

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
