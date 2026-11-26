# Packaging CoinCraft for Windows, macOS and Linux

CoinCraft is a single program with nothing to install alongside it: the database, fonts and
icon are all built in, and each person's data is kept in their own app data folder. Packaging
just wraps that program in the installer each system expects.

We use [cargo-packager](https://github.com/crabnebula-dev/cargo-packager), set up in
`Cargo.toml` under `[package.metadata.packager]`. Each system has to be packaged on that
system (or on a matching build machine, see "Automatic builds" below).

| System  | What people download                   | Files in `dist/` (names include the version) |
|---------|----------------------------------------|----------------------------------|
| Windows | Setup program, or an MSI for IT admins | `CoinCraft_0.1.0_x64-setup.exe`, `.msi` |
| macOS   | Disk image with the app to drag in     | `CoinCraft_0.1.0_aarch64.dmg` (and `.app`) |
| Linux   | AppImage (runs anywhere), or a .deb    | `coincraft_0.1.0_x86_64.AppImage`, `.deb` |

## Build installers on your own computer

1. Install Rust from <https://rustup.rs>.
2. In this folder, run the script for your system:
   - macOS or Linux: `./scripts/package.sh`
   - Windows (PowerShell): `.\scripts\package.ps1`
3. The installers appear in `dist/`.

The script installs cargo-packager the first time, builds an optimised release, and packages it.
To do it by hand instead: `cargo install cargo-packager --locked`, then `cargo packager --release`.

Extra tools by system:
- **Windows:** nothing extra for the setup .exe. The `.msi` also needs the WiX Toolset; the
  packager downloads it automatically.
- **macOS:** the Xcode command line tools (`xcode-select --install`).
- **Linux (Debian/Ubuntu):** `sudo apt install libxkbcommon-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev pkg-config`
  to build. People who install the `.deb` get the libraries they need listed as dependencies.

## Automatic builds for every system

`.github/workflows/release.yml` builds all installers on GitHub's Windows, macOS (Apple Silicon) and Linux machines. Put this folder in a GitHub repository, then either:
- push a version tag such as `v0.1.0`: the installers are attached to a new GitHub Release
  that people can download from, or
- run the "Release installers" workflow by hand from the Actions tab: the installers appear
  as downloadable artifacts on that run.

## Before you share it publicly

- **Name and identifier.** Change `identifier` in `Cargo.toml` to a reverse domain you own
  (for example `com.yourcompany.coincraft`). Each system uses it to recognise the app, so
  don't change it after people have installed it.
- **Version.** Bump `version` in `Cargo.toml` for each release; installers use it to upgrade.
- **Code signing.** Without it, CoinCraft still installs, but people see warnings:
  - *macOS* says the app "can't be opened because the developer cannot be verified"; people
    have to right-click it and choose Open. Fixing this needs an Apple Developer account
    (US$99/year). Then set `APPLE_SIGNING_IDENTITY`, plus `APPLE_ID`, `APPLE_PASSWORD` (an
    app-specific password) and `APPLE_TEAM_ID` for notarization, before running the packager.
  - *Windows* SmartScreen shows "Windows protected your PC"; people click More info, then Run
    anyway. Fixing this needs a code-signing certificate (or Azure Trusted Signing). Then add
    a `signing-command` under `[package.metadata.packager.windows]`.
  - *Linux* needs no signing.
- **Icon.** The icons in `assets/icons/` are drawn by `cargo run --example make_icons`. Edit
  that file and run it again to change the design, or replace the PNGs and `icon.ico` with
  your own artwork at the same sizes.

## Where people's data lives after installing

Uninstalling CoinCraft doesn't delete anyone's data. The database stays at:
- Windows: `%APPDATA%\CoinCraft\coincraft.db`
- macOS: `~/Library/Application Support/CoinCraft/coincraft.db`
- Linux: `~/.local/share/CoinCraft/coincraft.db`
