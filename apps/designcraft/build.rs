//! Windows only: embed the app icon and version info (VERSIONINFO) into `designcraft.exe`, so it
//! shows in Explorer, the taskbar, the Start menu and Alt-Tab.
//!
//! On every other target this does nothing. A missing resource compiler is a warning, so a
//! cross-compile from macOS or Linux still links, unless `DESIGNCRAFT_REQUIRE_WINRES=1` turns it
//! into an error (for release builds).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../packaging/pressroom/pressroom.ico");
    println!("cargo:rerun-if-env-changed=DESIGNCRAFT_REQUIRE_WINRES");
    println!("cargo:rerun-if-env-changed=PRESSROOM_VERSION");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    // The installer build's PressRoom version (build-installer.ps1), shown in the file's Properties.
    if let Ok(version) = std::env::var("PRESSROOM_VERSION") {
        res.set("ProductVersion", &version);
        // The file version Windows Installer compares when an update replaces PressRoom.exe (the new
        // version goes in before the old comes out): 1.x.y.z for PressRoom x.y.z, above the 0.13.0.0
        // (DesignCraft's version) earlier PressRooms carry and higher with every release, so an
        // update always puts the new program in.
        if let Some((parts, number)) = file_version(&version) {
            res.set_version_info(winresource::VersionInfo::FILEVERSION, number);
            res.set("FileVersion", &parts);
        }
    }
    res.set_icon("../../packaging/pressroom/pressroom.ico")
        .set("ProductName", "PressRoom")
        .set("FileDescription", "PressRoom")
        .set("CompanyName", "PressRoom (built on DesignCraft)")
        .set("LegalCopyright", "Copyright (c) the DesignCraft authors and the PressRoom contributors. MIT OR Apache-2.0.")
        .set("OriginalFilename", "PressRoom.exe")
        .set("InternalName", "designcraft");
    if let Err(e) = res.compile() {
        if std::env::var_os("DESIGNCRAFT_REQUIRE_WINRES").is_some() {
            eprintln!("embedding Windows resources failed: {e}");
            std::process::exit(1);
        }
        println!("cargo:warning=designcraft.exe built without icon/version resources: {e}");
    }
}

/// PressRoom `x.y.z` as the file version 1.x.y.z: its text and its VERSIONINFO number.
fn file_version(version: &str) -> Option<(String, u64)> {
    let mut parts = version.trim().split('.').map(|p| p.parse::<u16>().ok());
    let (x, y, z) = (parts.next()??, parts.next().flatten().unwrap_or(0), parts.next().flatten().unwrap_or(0));
    Some((format!("1.{x}.{y}.{z}"), (1u64 << 48) | (u64::from(x) << 32) | (u64::from(y) << 16) | u64::from(z)))
}
