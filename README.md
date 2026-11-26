# CoinCraft

[![Build and test](https://github.com/cwoodruff/coincraft/actions/workflows/ci.yml/badge.svg)](https://github.com/cwoodruff/coincraft/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

A personal finance app for Windows, macOS and Linux that keeps your data on your own computer.

- **Dashboard:** net worth, this month's spending, recurring charges, credit score and alerts.
- **Recurring charges:** found automatically from your transactions, with renewal reminders
  and help cancelling.
- **Spending and budgets:** categories, monthly budgets, cash flow, and low-balance and
  overdraft alerts.
- **Transactions:** import OFX, QFX or CSV statements, with rules and your own categories.
- **Connections:** bank sync through SimpleFIN Bridge, and an encrypted vault for the logins
  to your banks, cards and other financial services.
- **Net worth, savings goals and credit score:** goals with automatic or "smart" transfers.
- **Bill negotiation** tracker and a **money assistant** that answers questions from your own data.

Everything is stored in a local SQLite database. The only time CoinCraft goes online is when you
connect or sync a bank.

## Run it

1. Install Rust from <https://rustup.rs>.
2. On Linux, also install the window libraries:
   `sudo apt install libxkbcommon-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev pkg-config`
3. In this folder: `cargo run --release`

The first time it opens, a short setup wizard asks for your name and whether to start with
sample data or a clean slate.

Useful options: `--page <name>` opens a screen directly (for example `--page budgets`),
`--wizard` shows the setup wizard again, and `--screenshot <file.png>` saves a picture of the
window and exits.

## Test it

`cargo test` checks the database, the encrypted vault, statement import and bank-sync data reading.

## Install it

See [PACKAGING.md](PACKAGING.md) to build installers (.exe and .msi, .dmg, .deb and AppImage),
either on your own computer or automatically on GitHub.

## Where your data lives

- Windows: `%APPDATA%\CoinCraft\coincraft.db`
- macOS: `~/Library/Application Support/CoinCraft/coincraft.db`
- Linux: `~/.local/share/CoinCraft/coincraft.db`

Saved logins are encrypted with your master password (Argon2id and XChaCha20-Poly1305). The
master password itself is never stored and can't be recovered.

## Project layout

| Path | What it is |
|------|------------|
| `src/main.rs` | Starts the app and opens the window |
| `src/data.rs` | Accounts, transactions, budgets, goals, and the calculations behind the screens |
| `src/db.rs` | The SQLite database: tables, upgrades between versions, saving and loading |
| `src/vault.rs` | Encryption for saved logins |
| `src/sync.rs` | SimpleFIN Bridge bank sync |
| `src/assistant.rs` | The money assistant |
| `src/ui.rs`, `src/ui/` | The screens, the setup wizard and shared drawing helpers |
| `assets/icons/` | App icons, drawn by `cargo run --example make_icons` |
| `scripts/` | Packaging and publishing helpers |
| `.github/workflows/` | Automatic tests on every change, and installer builds for releases |

## License

CoinCraft is released under the [MIT License](LICENSE): you may use, copy, change and share it,
including commercially, as long as the copyright and license notice stay with it.
