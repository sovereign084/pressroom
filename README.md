<p align="center">
  <img src="packaging/pressroom/pressroom-256.png" width="128" height="128" alt="PressRoom icon: an open booklet on a blue-teal tile">
</p>

<h1 align="center">PressRoom</h1>

<p align="center">
  <b>Make church bulletins, newsletters and flyers the way you always have.</b><br>
  A free desktop publishing program for Windows that looks and works like the publishing program
  you already know, and opens your old <code>.pub</code> files.
</p>

## Download and install

1. **[Download PressRoom](https://github.com/sovereign084/pressroom/releases/latest/download/PressRoom-Setup.msi)**
   (this link always gets the newest version).
2. Open the downloaded file. If Windows says **"Windows protected your PC"**, click
   **More info**, then **Run anyway**. (PressRoom isn't signed with a paid certificate yet, so
   Windows doesn't recognise it.)
3. PressRoom appears on your desktop and in the Start menu.

**Updating:** download from the same link and open it again. The new version replaces the old one;
your publications and settings stay as they are. What changed in each version is on the
[releases page](https://github.com/sovereign084/pressroom/releases).

**Your old Publisher files:** in PressRoom, choose **File › Open** and pick the `.pub` file. PressRoom
converts it into a `.pressroom` file next to it. Converting needs Microsoft Publisher still
installed on the same computer, so convert your files while you have it.

## What's in it

- The window you know: ribbon tabs, the Pages pane on the left, the File screen, Print with
  booklet printing.
- Text boxes, pictures, shapes, tables and calendars you can edit, with text that flows from box to box.
- Opens `.pub` files with their layout, fonts, colours (spot inks too), tables and Hebrew text.
- About 80 free font families come with it: the free fonts in Microsoft 365's online font list
  (Montserrat, Lobster, Pacifico, Playfair Display, Bebas Neue, Amatic SC and more) under the same
  names, and free look-alikes for some Microsoft-only ones.
- No account, no subscription, nothing sent anywhere.

## For developers

PressRoom is the `pressroom` branch of a fork of [DesignCraft](https://github.com/storytold/designcraft),
an open-source page-layout app in Rust by the ArtCraft team. PressRoom adds a Publisher-style window
(`crates/ui-egui/src/pressroom/`), the `.pub` converter (`tools/pub-import/`) and its Windows installer
(`packaging/pressroom/`). The layout engine, commands, file format and control channel are DesignCraft's;
its own documentation is in `docs/` and the upstream repository.

- Build: `cargo build --release -p designcraft -p designcraft-cli`
- Installer: `packaging/pressroom/build-installer.ps1 -Version X.Y.Z` (WiX 5) builds
  `target-pkg/PressRoom-Setup-X.Y.Z.msi`.
- Release: `packaging/pressroom/release.ps1 -Version X.Y.Z -Notes "…"` builds the installer and
  publishes it as a GitHub release (needs the GitHub CLI, signed in).

## Licence

MIT OR Apache-2.0, as DesignCraft (`LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`). The bundled fonts are
under the SIL Open Font License (their licences install with them). PressRoom is based on DesignCraft
by the ArtCraft team; it is not an ArtCraft product and carries none of the ArtCraft marks
(`docs/brand/LICENSE-brand.txt`). PressRoom is not made by or affiliated with Microsoft; Microsoft
Publisher is a trademark of Microsoft Corporation.
