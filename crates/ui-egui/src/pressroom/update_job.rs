//! One update, from the click to the next start. PressRoom copies itself out of its install folder
//! and starts the copy as the update helper (`apps/designcraft/src/update_helper.rs`), which shows a
//! small window, waits for PressRoom to close, runs the installer, reads what it answered and opens
//! PressRoom again. A record on disk carries the update from one to the other: written before the
//! helper starts, it is how PressRoom knows the helper is running before it closes, and how the next
//! start knows what happened, even when the helper died with nothing said.
//!
//! The pattern is `craft_update::self_update`'s, whose module doesn't fit as it stands: it refuses
//! unsigned programs, which PressRoom is, it deletes the installer at once, and it shows no window.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// The argument that starts this program as the update helper: then the record's path.
pub const HELPER_FLAG: &str = "--pressroom-update";
/// The helper's file name, unlike the program it updates, so looking for open PressRoom windows
/// doesn't find it.
pub const HELPER_NAME: &str = "PressRoom Update.exe";
const RECORD_NAME: &str = "update.json";

/// How often the helper looks for PressRoom closing, and how many times before it gives up.
const CLOSE_STEP: Duration = Duration::from_secs(5);
const CLOSE_TRIES: u32 = 60;
/// After this many looks it says PressRoom hasn't closed.
const CLOSE_NOTE_AFTER: u32 = 12;
/// How long another installation is waited for: Windows runs one at a time.
const BUSY_STEP: Duration = Duration::from_secs(15);
const BUSY_TRIES: u32 = 20;

/// How far an update got.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stage {
    /// PressRoom wrote this before starting the helper.
    #[default]
    Started,
    /// The helper is running: PressRoom may close.
    HelperRunning,
    /// The installer is running.
    Installing,
    /// The helper is done, and `outcome` says how it went.
    Finished,
}

/// How an update ended.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Outcome {
    Installed {
        restart_needed: bool,
    },
    InstallFailed {
        code: i32,
    },
    Cancelled,
    OtherInstall,
    DidntClose,
    /// Windows wouldn't start the installer.
    CouldntRun(String),
    /// The program file is gone and wasn't put back.
    ProgramMissing,
}

/// What an update is, written where both PressRoom and the helper can read it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub from: String,
    pub to: String,
    pub msi: PathBuf,
    pub exe: PathBuf,
    /// The PressRoom the helper waits for.
    pub pid: u32,
    pub helper_pid: Option<u32>,
    pub stage: Stage,
    pub outcome: Option<Outcome>,
    /// What was open, to open again.
    pub publications: Vec<String>,
}

/// The folder an update keeps its record, installer, log and helper in: the user's own, not the
/// temporary folder, whose cleaners could take the installer before the update is proven.
pub fn update_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA").map_or_else(|| std::env::temp_dir().join("PressRoom"), |d| PathBuf::from(d).join("PressRoom")).join("Update")
}

pub fn record_path(dir: &Path) -> PathBuf {
    dir.join(RECORD_NAME)
}

/// Writes the record whole or not at all.
pub fn write_record(path: &Path, record: &Record) -> Result<(), String> {
    let text = serde_json::to_vec_pretty(record).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

pub fn read_record(path: &Path) -> Option<Record> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

/// Whether the record is one the helper should act on: an installer, and PressRoom's own program.
pub fn acceptable(record: &Record) -> Result<(), String> {
    let has_ext = |p: &Path, ext: &str| p.extension().is_some_and(|e| e.eq_ignore_ascii_case(ext));
    if !has_ext(&record.msi, "msi") {
        return Err("the update isn't an installer".into());
    }
    if !has_ext(&record.exe, "exe") || record.exe.file_name().is_some_and(|n| n.eq_ignore_ascii_case(HELPER_NAME)) {
        return Err("the update doesn't name PressRoom's program".into());
    }
    Ok(())
}

/// What the installer's exit code means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Installed {
        restart_needed: bool,
    },
    /// 1618: Windows is running another installation. Worth waiting for.
    AnotherInstall,
    /// 1602: someone pressed Cancel on the installer's window.
    Cancelled,
    Failed,
}

pub fn verdict(code: i32) -> Verdict {
    match code {
        0 => Verdict::Installed { restart_needed: false },
        // 3010: done, and some files finish when the computer restarts. 1641: the same, restarting.
        3010 | 1641 => Verdict::Installed { restart_needed: true },
        1618 => Verdict::AnotherInstall,
        1602 => Verdict::Cancelled,
        _ => Verdict::Failed,
    }
}

/// What the helper's window says.
pub const SAY_WAITING: &str = "Updating PressRoom\u{2026}\n\nWaiting for PressRoom to close.";
pub const SAY_NOT_CLOSED: &str =
    "Updating PressRoom\u{2026}\n\nPressRoom hasn't closed yet. If a PressRoom window is asking you something, please answer it, or close it.";
pub const SAY_INSTALLING: &str = "Updating PressRoom\u{2026}\n\nPlease wait. PressRoom will open again by itself.";
pub const SAY_LONG: &str = "Updating PressRoom\u{2026}\n\nThis is taking longer than usual, which can happen when another program is using one of its fonts. Please leave this open. Your work is safe.";
pub const SAY_OTHER_INSTALL: &str =
    "Updating PressRoom\u{2026}\n\nAnother program is being installed on this computer. PressRoom will update when it has finished.";
pub const SAY_OPENING: &str = "Opening PressRoom\u{2026}";
pub const SAY_MISSING: &str = "The update didn't finish, and PressRoom's program file is missing.\n\nYour publications are safe: they are separate files. Click Reinstall PressRoom to put it back.";
pub const SAY_STILL_MISSING: &str = "That didn't put PressRoom back. Your publications are still safe.\n\nClick Reinstall PressRoom to try again, or restart the computer first if another program may be in the way.";

/// What the update helper needs from the computer, so the steps can be tested without one.
pub trait Helper {
    /// Waits up to `within` for PressRoom to close, and says whether it has.
    fn wait_for_close(&mut self, pid: u32, within: Duration) -> bool;
    /// Whether another window of PressRoom is open: Windows can't replace a program in use.
    fn other_windows_open(&mut self) -> bool;
    /// Runs the installer to its end: its exit code, or why it couldn't start. `repair` puts every
    /// file back even though Windows counts this version as installed: run plainly then, the
    /// installer has nothing to do and says all is well.
    fn run_installer(&mut self, repair: bool) -> Result<i32, String>;
    fn program_exists(&self) -> bool;
    fn open_program(&mut self) -> Result<(), String>;
    fn sleep(&mut self, how_long: Duration);
    fn say(&mut self, text: &str);
    /// Offers to put PressRoom back, and says whether the person wants that. `false` with no window.
    fn ask_reinstall(&mut self, text: &str) -> bool;
    fn save(&mut self, record: &Record);
}

fn finish<H: Helper>(record: &mut Record, helper: &mut H, outcome: Outcome) {
    record.stage = Stage::Finished;
    record.outcome = Some(outcome);
    helper.save(record);
}

/// The helper's work, after it has said it is running: waits for PressRoom to close, installs, says
/// how it went and opens PressRoom again. Whatever happens, the record says so.
pub fn run_update<H: Helper>(record: &mut Record, helper: &mut H) {
    helper.say(SAY_WAITING);
    let mut tries = 0;
    loop {
        let closed = helper.wait_for_close(record.pid, CLOSE_STEP);
        if closed && !helper.other_windows_open() {
            break;
        }
        if closed {
            helper.sleep(CLOSE_STEP);
        }
        tries += 1;
        if tries == CLOSE_NOTE_AFTER {
            helper.say(SAY_NOT_CLOSED);
        }
        if tries >= CLOSE_TRIES {
            return finish(record, helper, Outcome::DidntClose);
        }
    }

    record.stage = Stage::Installing;
    helper.save(record);
    helper.say(SAY_INSTALLING);
    let mut outcome = install_with_waiting(helper, false);

    // However the installer ended, the program must be there: a failed rollback left nothing
    // installed on 2026-10-31. The installer is still on disk, so it can be put back.
    if !helper.program_exists() {
        let mut asking = SAY_MISSING;
        loop {
            if !helper.ask_reinstall(asking) {
                return finish(record, helper, Outcome::ProgramMissing);
            }
            asking = SAY_STILL_MISSING;
            helper.say(SAY_INSTALLING);
            outcome = install_with_waiting(helper, false);
            if !helper.program_exists() {
                outcome = install_with_waiting(helper, true);
            }
            if helper.program_exists() {
                break;
            }
        }
    }
    finish(record, helper, outcome);
    helper.say(SAY_OPENING);
    if let Err(e) = helper.open_program() {
        log::warn!("the update helper couldn't open PressRoom: {e}");
    }
}

fn install_with_waiting<H: Helper>(helper: &mut H, repair: bool) -> Outcome {
    for _ in 0..BUSY_TRIES {
        let code = match helper.run_installer(repair) {
            Ok(code) => code,
            Err(e) => return Outcome::CouldntRun(e),
        };
        match verdict(code) {
            Verdict::Installed { restart_needed } => return Outcome::Installed { restart_needed },
            Verdict::Cancelled => return Outcome::Cancelled,
            Verdict::Failed => return Outcome::InstallFailed { code },
            Verdict::AnotherInstall => {
                helper.say(SAY_OTHER_INSTALL);
                helper.sleep(BUSY_STEP);
            }
        }
    }
    Outcome::OtherInstall
}

/// What a start of PressRoom makes of the record it finds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Arrival {
    /// The update worked: this is the version it went to. Everything it kept can go.
    Updated { message: String, publications: Vec<String> },
    /// It didn't: the bar says so and offers Try Again with the installer it kept.
    NotUpdated { message: String, version: String, msi: PathBuf, publications: Vec<String> },
    /// The helper is still at it, and this window was opened meanwhile.
    StillUpdating { message: String },
}

/// `current` is this program's version, `helper_alive` whether the helper is still running.
pub fn arrival(record: &Record, current: &str, helper_alive: bool) -> Arrival {
    let publications = record.publications.clone();
    // This program being the new version is the proof, whatever else the record says: the helper
    // can die after the installer has done its work.
    if current == record.to {
        let restart = matches!(record.outcome, Some(Outcome::Installed { restart_needed: true }));
        let mut message = format!("PressRoom was updated to {current}.");
        if restart {
            message.push_str(" Restart your computer when you can, to finish the update.");
        }
        return Arrival::Updated { message, publications };
    }
    if record.stage != Stage::Finished && helper_alive {
        return Arrival::StillUpdating { message: "PressRoom is still updating. Please close this window and wait for it to finish.".into() };
    }
    let safe = format!("You're still on {current} and your work is safe.");
    let message = match &record.outcome {
        None => format!("The update stopped before it finished. {safe}"),
        Some(Outcome::InstallFailed { code }) => format!("The update didn't install (installer code {code}). {safe}"),
        Some(Outcome::Installed { .. }) => format!("The installer finished, but this is still the old version. {safe}"),
        Some(Outcome::Cancelled) => format!("The update was cancelled. {safe}"),
        Some(Outcome::OtherInstall) => format!("The update couldn't install because another program was being installed. {safe}"),
        Some(Outcome::DidntClose) => format!("The update didn't install because PressRoom didn't close in time. {safe}"),
        Some(Outcome::CouldntRun(_)) => format!("Windows wouldn't start the installer, so nothing changed. {safe}"),
        Some(Outcome::ProgramMissing) => format!("PressRoom had to be put back after a failed update. {safe}"),
    };
    Arrival::NotUpdated { message, version: record.to.clone(), msi: record.msi.clone(), publications }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A computer the steps can be run on: installer answers in order, what is said kept.
    #[derive(Default)]
    struct Fake {
        asked_with: Vec<String>,
        closes_after: u32,
        looks: u32,
        other_windows: u32,
        codes: Vec<Result<i32, String>>,
        ran: u32,
        repairs: u32,
        program: bool,
        /// The installer puts the program back when it runs this time.
        restores_on_run: Option<u32>,
        reinstall: bool,
        asked: u32,
        opened: u32,
        said: Vec<String>,
        saved: Vec<Stage>,
    }

    impl Helper for Fake {
        fn wait_for_close(&mut self, _pid: u32, _within: Duration) -> bool {
            self.looks += 1;
            self.looks > self.closes_after
        }
        fn other_windows_open(&mut self) -> bool {
            if self.other_windows > 0 {
                self.other_windows -= 1;
                return true;
            }
            false
        }
        fn run_installer(&mut self, repair: bool) -> Result<i32, String> {
            self.ran += 1;
            self.repairs += u32::from(repair);
            if self.restores_on_run == Some(self.ran) {
                self.program = true;
            }
            if self.codes.is_empty() { Ok(1603) } else { self.codes.remove(0) }
        }
        fn program_exists(&self) -> bool {
            self.program
        }
        fn open_program(&mut self) -> Result<(), String> {
            self.opened += 1;
            Ok(())
        }
        fn sleep(&mut self, _how_long: Duration) {}
        fn say(&mut self, text: &str) {
            self.said.push(text.to_string());
        }
        fn ask_reinstall(&mut self, text: &str) -> bool {
            self.asked_with.push(text.to_string());
            self.asked += 1;
            self.reinstall
        }
        fn save(&mut self, record: &Record) {
            self.saved.push(record.stage);
        }
    }

    fn record() -> Record {
        Record {
            from: "0.8.0".into(),
            to: "0.8.1".into(),
            msi: PathBuf::from(r"C:\u\Update\pressroom-0.8.1-windows-x64.msi"),
            exe: PathBuf::from(r"C:\p\PressRoom.exe"),
            pid: 42,
            ..Record::default()
        }
    }

    fn run(fake: &mut Fake) -> Record {
        let mut r = record();
        run_update(&mut r, fake);
        r
    }

    #[test]
    fn a_good_install_opens_the_program_and_says_so() {
        let mut f = Fake { codes: vec![Ok(0)], program: true, ..Fake::default() };
        let r = run(&mut f);
        assert_eq!(r.outcome, Some(Outcome::Installed { restart_needed: false }));
        assert_eq!(r.stage, Stage::Finished);
        assert_eq!(f.opened, 1);
        assert_eq!(f.saved, [Stage::Installing, Stage::Finished], "the record is saved before the program opens");
    }

    #[test]
    fn a_restart_being_needed_is_success_not_failure() {
        for code in [3010, 1641] {
            let mut f = Fake { codes: vec![Ok(code)], program: true, ..Fake::default() };
            assert_eq!(run(&mut f).outcome, Some(Outcome::Installed { restart_needed: true }));
            assert_eq!(f.ran, 1, "installed: not run again");
        }
    }

    #[test]
    fn a_failed_install_reopens_the_old_version_and_records_the_code() {
        let mut f = Fake { codes: vec![Ok(1603)], program: true, ..Fake::default() };
        assert_eq!(run(&mut f).outcome, Some(Outcome::InstallFailed { code: 1603 }));
        assert_eq!((f.ran, f.opened), (1, 1));
    }

    #[test]
    fn another_install_is_waited_for_then_the_update_goes_in() {
        let mut f = Fake { codes: vec![Ok(1618), Ok(1618), Ok(0)], program: true, ..Fake::default() };
        assert_eq!(run(&mut f).outcome, Some(Outcome::Installed { restart_needed: false }));
        assert_eq!(f.ran, 3);
        assert!(f.said.iter().any(|s| s == SAY_OTHER_INSTALL));
    }

    #[test]
    fn another_install_that_never_ends_is_not_called_a_failed_install() {
        let mut f = Fake { codes: vec![Ok(1618); BUSY_TRIES as usize], program: true, ..Fake::default() };
        assert_eq!(run(&mut f).outcome, Some(Outcome::OtherInstall));
        assert_eq!(f.ran, BUSY_TRIES);
        assert_eq!(f.opened, 1);
    }

    #[test]
    fn cancelling_the_installer_is_not_called_a_failure() {
        let mut f = Fake { codes: vec![Ok(1602)], program: true, ..Fake::default() };
        assert_eq!(run(&mut f).outcome, Some(Outcome::Cancelled));
    }

    #[test]
    fn an_installer_windows_wont_start_is_said_and_the_program_reopens() {
        let mut f = Fake { codes: vec![Err("blocked".into())], program: true, ..Fake::default() };
        assert_eq!(run(&mut f).outcome, Some(Outcome::CouldntRun("blocked".into())));
        assert_eq!(f.opened, 1);
    }

    #[test]
    fn a_pressroom_that_never_closes_gets_no_installer() {
        let mut f = Fake { closes_after: u32::MAX, program: true, ..Fake::default() };
        assert_eq!(run(&mut f).outcome, Some(Outcome::DidntClose));
        assert_eq!(f.ran, 0, "nothing installs over a running program");
        assert_eq!(f.opened, 0, "it is still open");
        assert!(f.said.iter().any(|s| s == SAY_NOT_CLOSED));
    }

    #[test]
    fn another_window_holds_the_install_until_it_closes() {
        let mut f = Fake { other_windows: 2, codes: vec![Ok(0)], program: true, ..Fake::default() };
        assert_eq!(run(&mut f).outcome, Some(Outcome::Installed { restart_needed: false }));
    }

    #[test]
    fn a_missing_program_is_offered_back_and_reinstalled() {
        // The first run fails and takes the program with it; the second puts it back.
        let mut f = Fake { codes: vec![Ok(1603), Ok(0)], restores_on_run: Some(2), reinstall: true, ..Fake::default() };
        let r = run(&mut f);
        assert_eq!(f.asked, 1);
        assert_eq!(r.outcome, Some(Outcome::Installed { restart_needed: false }));
        assert_eq!(f.opened, 1);
    }

    #[test]
    fn a_reinstall_turned_down_is_recorded_and_nothing_opens() {
        let mut f = Fake { codes: vec![Ok(1603)], reinstall: false, ..Fake::default() };
        assert_eq!(run(&mut f).outcome, Some(Outcome::ProgramMissing));
        assert_eq!((f.asked, f.opened), (1, 0));
    }

    #[test]
    fn a_program_the_installer_thinks_is_there_takes_a_repair() {
        // The update went in and the file went afterwards: run plainly, the installer has nothing
        // to do and answers 0.
        let mut f = Fake { codes: vec![Ok(0), Ok(0), Ok(0)], restores_on_run: Some(3), reinstall: true, ..Fake::default() };
        assert_eq!(run(&mut f).outcome, Some(Outcome::Installed { restart_needed: false }));
        assert_eq!((f.asked, f.ran, f.repairs, f.opened), (1, 3, 1, 1));
    }

    #[test]
    fn a_reinstall_that_fails_is_offered_again() {
        let mut f = Fake { codes: vec![Ok(1603), Ok(1603), Ok(1603), Ok(0)], restores_on_run: Some(4), reinstall: true, ..Fake::default() };
        run(&mut f);
        assert_eq!((f.asked, f.ran, f.opened), (2, 4, 1));
        assert_eq!(f.asked_with, [SAY_MISSING, SAY_STILL_MISSING], "the second time it says the first didn't work");
    }

    #[test]
    fn being_the_new_version_is_the_proof() {
        // The helper was killed after the installer did its work: the record stops at Installing.
        let mut r = record();
        r.stage = Stage::Installing;
        r.publications = vec!["a.pressroom".into()];
        let Arrival::Updated { message, publications } = arrival(&r, "0.8.1", false) else { panic!() };
        assert_eq!(message, "PressRoom was updated to 0.8.1.");
        assert_eq!(publications, ["a.pressroom"]);
    }

    #[test]
    fn a_restart_being_needed_is_said() {
        let mut r = record();
        r.stage = Stage::Finished;
        r.outcome = Some(Outcome::Installed { restart_needed: true });
        let Arrival::Updated { message, .. } = arrival(&r, "0.8.1", false) else { panic!() };
        assert!(message.contains("Restart your computer"), "{message}");
    }

    #[test]
    fn every_way_of_not_updating_says_the_work_is_safe_and_offers_the_installer() {
        let finished = |o: Outcome| Record { stage: Stage::Finished, outcome: Some(o), ..record() };
        let cases = [
            (finished(Outcome::InstallFailed { code: 1603 }), "installer code 1603"),
            (finished(Outcome::Installed { restart_needed: false }), "still the old version"),
            (finished(Outcome::Cancelled), "cancelled"),
            (finished(Outcome::OtherInstall), "another program"),
            (finished(Outcome::DidntClose), "didn't close"),
            (finished(Outcome::CouldntRun("x".into())), "wouldn't start"),
            (finished(Outcome::ProgramMissing), "put back"),
            // The helper died, or never ran, with nothing said.
            (Record { stage: Stage::Started, ..record() }, "stopped before"),
            (Record { stage: Stage::HelperRunning, ..record() }, "stopped before"),
            (Record { stage: Stage::Installing, ..record() }, "stopped before"),
        ];
        for (r, words) in cases {
            let Arrival::NotUpdated { message, version, msi, .. } = arrival(&r, "0.8.0", false) else { panic!("{r:?}") };
            assert!(message.contains(words), "{message}");
            assert!(message.contains("You're still on 0.8.0 and your work is safe."), "{message}");
            assert_eq!(version, "0.8.1");
            assert_eq!(msi, r.msi);
        }
    }

    #[test]
    fn a_window_opened_during_the_update_is_told_to_wait() {
        let r = Record { stage: Stage::Installing, ..record() };
        assert!(matches!(arrival(&r, "0.8.0", true), Arrival::StillUpdating { .. }));
    }

    #[test]
    fn the_record_comes_back_as_written() {
        let dir = std::env::temp_dir().join(format!("pr-update-record-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = record_path(&dir);
        let r = Record {
            stage: Stage::Finished,
            outcome: Some(Outcome::CouldntRun("O'Brien".into())),
            publications: vec![r"C:\a b\c.pressroom".into()],
            ..record()
        };
        write_record(&path, &r).unwrap();
        assert_eq!(read_record(&path), Some(r));
        std::fs::write(&path, b"{ not a record").unwrap();
        assert_eq!(read_record(&path), None, "a damaged record is no record, not a crash");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_helper_only_acts_on_an_installer_and_a_program() {
        assert!(acceptable(&record()).is_ok());
        assert!(acceptable(&Record { msi: "a.exe".into(), ..record() }).is_err());
        assert!(acceptable(&Record { exe: "a.msi".into(), ..record() }).is_err());
        assert!(acceptable(&Record { exe: PathBuf::from(HELPER_NAME), ..record() }).is_err());
    }

    #[test]
    fn exit_codes_mean_what_windows_says() {
        assert_eq!(verdict(0), Verdict::Installed { restart_needed: false });
        assert_eq!(verdict(3010), Verdict::Installed { restart_needed: true });
        assert_eq!(verdict(1641), Verdict::Installed { restart_needed: true });
        assert_eq!(verdict(1618), Verdict::AnotherInstall);
        assert_eq!(verdict(1602), Verdict::Cancelled);
        for code in [1, 1603, 1620, -1] {
            assert_eq!(verdict(code), Verdict::Failed);
        }
    }
}
