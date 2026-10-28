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
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
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
