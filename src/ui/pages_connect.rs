//! Connections: bank sync, statement files, and the encrypted vault of saved logins.

use super::widgets::*;
use super::*;
use crate::sync;
use crate::vault::{self, Credential, CredentialSecret, ServiceKind};
use std::sync::mpsc;

/// Lock the vault after this long without using it.
const VAULT_IDLE_SECS: f64 = 10.0 * 60.0;
/// Clear a copied password from the clipboard after this long.
const CLIPBOARD_SECS: f64 = 30.0;
const BRIDGE_SITE: &str = "https://beta-bridge.simplefin.org";

type Fetched = Result<(Vec<RemoteAccount>, Vec<String>), String>;

/// Results coming back from the background bank-sync thread.
pub(super) enum SyncMsg {
    /// A new connection: the claimed access address (or why it failed), then the first download.
    Connected {
        access: Result<String, String>,
        data: Option<Fetched>,
    },
    /// A sync of an existing connection.
    Synced { conn_id: u32, data: Fetched },
}

/// The login form while adding or editing.
pub(super) struct CredEdit {
    id: Option<u32>,
    institution: String,
    kind: ServiceKind,
    account_id: Option<u32>,
    secret: CredentialSecret,
    error: String,
}

impl CredEdit {
    fn new() -> Self {
        Self {
            id: None,
            institution: String::new(),
            kind: ServiceKind::Bank,
            account_id: None,
            secret: CredentialSecret::default(),
            error: String::new(),
        }
    }
}

fn now_text() -> String {
    chrono::Local::now().format("%b %-d, %Y %-I:%M %p").to_string()
}

fn web_address(site: &str) -> String {
    let site = site.trim();
    if site.starts_with("http://") || site.starts_with("https://") {
        site.to_string()
    } else {
        format!("https://{site}")
    }
}

impl App {
    /// Locks the vault when idle and clears a copied password after a short while.
    pub(super) fn vault_housekeeping(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if self.vault_key.is_some() && now - self.vault_last_used > VAULT_IDLE_SECS {
            self.lock_vault();
            self.toast(ctx, "The vault locked itself after 10 minutes.");
        }
        if let Some(at) = self.clipboard_clear_at {
            if now >= at {
                ctx.copy_text(String::new());
                self.clipboard_clear_at = None;
            } else {
                ctx.request_repaint_after(std::time::Duration::from_secs(1));
            }
        }
        if self.vault_key.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_secs(5));
        }
    }

    fn lock_vault(&mut self) {
        self.vault_key = None;
        self.cred_edit = None;
        self.show_password = false;
        self.confirm_delete_cred = None;
    }

    fn touch_vault(&mut self, ctx: &egui::Context) {
        self.vault_last_used = ctx.input(|i| i.time);
    }

    /// Picks up finished bank-sync work from the background thread.
    pub(super) fn poll_sync(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.sync_rx else { return };
        let msg = match rx.try_recv() {
            Ok(m) => m,
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
                return;
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.sync_rx = None;
                self.sync_label.clear();
                return;
            }
        };
        self.sync_rx = None;
        self.sync_label.clear();
        match msg {
            SyncMsg::Connected { access: Err(e), .. } => self.toast(ctx, format!("Couldn't connect: {e}")),
            SyncMsg::Connected { access: Ok(access), data } => {
                let Some(key) = self.vault_key else {
                    self.toast(
                        ctx,
                        "The vault locked before the connection finished. Please connect again with a new token.",
                    );
                    return;
                };
                let secret = CredentialSecret {
                    access_url: access,
                    website: BRIDGE_SITE.into(),
                    ..Default::default()
                };
                let (nonce, cipher) = match vault::seal_secret(&key, &secret) {
                    Ok(x) => x,
                    Err(e) => {
                        self.toast(ctx, format!("Couldn't save the connection securely: {e}"));
                        return;
                    }
                };
                let cred_id = self.state.credentials.iter().map(|c| c.id).max().unwrap_or(0) + 1;
                self.state.credentials.push(Credential {
                    id: cred_id,
                    institution: "SimpleFIN Bridge".into(),
                    kind: ServiceKind::BankSync,
                    account_id: None,
                    nonce,
                    cipher,
                    updated: now_text(),
                });
                let conn_id = self.state.connections.iter().map(|c| c.id).max().unwrap_or(0) + 1;
                self.state.connections.push(BankConnection {
                    id: conn_id,
                    label: "SimpleFIN Bridge".into(),
                    credential_id: cred_id,
                    last_sync: String::new(),
                    status: "Connected".into(),
                });
                self.sf_token.clear();
                if let Some(data) = data {
                    self.apply_sync(ctx, conn_id, data);
                }
            }
            SyncMsg::Synced { conn_id, data } => self.apply_sync(ctx, conn_id, data),
        }
    }

    fn apply_sync(&mut self, ctx: &egui::Context, conn_id: u32, data: Fetched) {
        let status = match data {
            Ok((accounts, notes)) => {
                let n = accounts.len();
                let r = self.state.merge_remote(accounts);
                let mut s = format!(
                    "{} account{}, {} new transaction{}",
                    n,
                    if n == 1 { "" } else { "s" },
                    r.new_transactions,
                    if r.new_transactions == 1 { "" } else { "s" }
                );
                if !notes.is_empty() {
                    s += &format!(". Note from the bridge: {}", notes.join("; "));
                }
                self.toast(ctx, format!("Bank sync finished: {s}."));
                s
            }
            Err(e) => {
                self.toast(ctx, format!("Bank sync didn't work: {e}"));
                format!("Failed: {e}")
            }
        };
        if let Some(c) = self.state.connections.iter_mut().find(|c| c.id == conn_id) {
            c.last_sync = now_text();
            c.status = status;
        }
    }

    fn start_connect(&mut self, ctx: &egui::Context) {
        let token = self.sf_token.clone();
        let (tx, rx) = mpsc::channel();
        let ctx2 = ctx.clone();
        std::thread::spawn(move || {
            let access = sync::claim(&token);
            let data = access.as_ref().ok().map(|a| sync::fetch(a, sync::HISTORY_DAYS));
            let _ = tx.send(SyncMsg::Connected { access, data });
            ctx2.request_repaint();
        });
        self.sync_rx = Some(rx);
        self.sync_label = "Connecting to SimpleFIN Bridge…".into();
    }

    fn start_sync(&mut self, ctx: &egui::Context, conn_id: u32) {
        let Some(key) = self.vault_key else { return };
        let Some(conn) = self.state.connections.iter().find(|c| c.id == conn_id) else {
            return;
        };
        let Some(cred) = self.state.credentials.iter().find(|c| c.id == conn.credential_id) else {
            self.toast(ctx, "This connection's saved access is missing. Remove it and connect again.");
            return;
        };
        let access = match vault::open_secret(&key, cred) {
            Ok(s) => s.access_url,
            Err(e) => {
                self.toast(ctx, format!("Couldn't open the saved access: {e}"));
                return;
            }
        };
        let (tx, rx) = mpsc::channel();
        let ctx2 = ctx.clone();
        std::thread::spawn(move || {
            let data = sync::fetch(&access, sync::HISTORY_DAYS);
            let _ = tx.send(SyncMsg::Synced { conn_id, data });
            ctx2.request_repaint();
        });
        self.sync_rx = Some(rx);
        self.sync_label = format!("Syncing {}…", conn.label);
    }

    pub(super) fn page_connections(&mut self, ui: &mut Ui) {
        title(
            ui,
            "Connections",
            "Bring in your bank, card and other accounts, and keep their logins in an encrypted vault on this computer.",
        );
        let ctx = ui.ctx().clone();
        self.ways_to_connect(ui, &ctx);
        ui.add_space(6.0);
        self.bank_sync_card(ui, &ctx);
        ui.add_space(6.0);
        self.vault_card(ui, &ctx);
    }

    fn ways_to_connect(&mut self, ui: &mut Ui, _ctx: &egui::Context) {
        let mut go_import = false;
        ui.columns(3, |c| {
            card(&mut c[0], |ui| {
                row_right(ui, |ui| section(ui, "↔ Bank sync"), |ui| chip(ui, "Online", BLUE));
                muted(
                    ui,
                    "Checking, savings, credit cards and more from thousands of US banks, through SimpleFIN Bridge. You sign up with the bridge yourself (it charges a small yearly fee), link your banks there, and paste a setup token below. Your data comes straight to this computer.",
                );
            });
            card(&mut c[1], |ui| {
                row_right(ui, |ui| section(ui, "📄 Statement files"), |ui| chip(ui, "Offline", GREEN));
                muted(
                    ui,
                    "Almost every bank and card issuer lets you download OFX, QFX (Quicken) or CSV files from their website. Import them and CoinCraft adds the account and its transactions, skipping any it already has.",
                );
                if ui.button("Import a file ▶").clicked() {
                    go_import = true;
                }
            });
            card(&mut c[2], |ui| {
                row_right(ui, |ui| section(ui, "🔒 Saved logins"), |ui| chip(ui, "Encrypted", AMBER));
                muted(
                    ui,
                    "For anything else (loans, insurance, investment sites, cards your bank sync doesn't cover), keep the website, username, password and account number here, locked with a master password.",
                );
                muted(ui, "Planned for later: direct links through Plaid, MX or Finicity, and Open Banking in the UK and EU. These need a developer account and a small server.");
            });
        });
        if go_import {
            self.page = Page::Transactions;
        }
    }

    fn bank_sync_card(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let unlocked = self.vault_key.is_some();
        let busy = self.sync_rx.is_some();
        let mut connect = false;
        let mut sync_id = None;
        let mut remove_id = None;
        let mut open_site = false;
        card(ui, |ui| {
            row_right(
                ui,
                |ui| section(ui, "Bank sync (SimpleFIN Bridge)"),
                |ui| {
                    if ui.link("Open SimpleFIN Bridge").clicked() {
                        open_site = true;
                    }
                },
            );
            if busy {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(self.sync_label.as_str());
                });
            }
            if self.state.connections.is_empty() {
                muted(ui, "No banks connected yet.");
            }
            for c in &self.state.connections {
                row_right(
                    ui,
                    |ui| {
                        ui.label(RichText::new("↔").color(BLUE));
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 1.0;
                            ui.label(RichText::new(c.label.as_str()).strong());
                            let when = if c.last_sync.is_empty() {
                                "never synced".to_string()
                            } else {
                                format!("last synced {}", c.last_sync)
                            };
                            ui.label(RichText::new(format!("{} · {}", c.status, when)).color(MUTED).size(12.5));
                        });
                    },
                    |ui| {
                        if ui
                            .small_button("Remove")
                            .on_hover_text("Stops syncing. Accounts and transactions already brought in stay.")
                            .clicked()
                        {
                            remove_id = Some(c.id);
                        }
                        if ui.add_enabled(unlocked && !busy, egui::Button::new("⟳ Sync now")).clicked() {
                            sync_id = Some(c.id);
                        }
                    },
                );
            }
            ui.separator();
            ui.label(RichText::new("Connect a new bridge").strong());
            muted(
                ui,
                "1. Create an account at SimpleFIN Bridge and connect your banks and cards there.  2. Ask it for a setup token and copy it.  3. Paste it here. The token works once; the access it gives is saved in your encrypted vault.",
            );
            ui.horizontal(|ui| {
                let w = (ui.available_width() - 130.0).max(200.0);
                ui.add_enabled(
                    unlocked && !busy,
                    egui::TextEdit::singleline(&mut self.sf_token)
                        .hint_text(hint("Paste setup token"))
                        .desired_width(w)
                        .password(true),
                );
                if ui
                    .add_enabled(unlocked && !busy && !self.sf_token.trim().is_empty(), egui::Button::new("Connect"))
                    .clicked()
                {
                    connect = true;
                }
            });
            if !unlocked {
                ui.label(
                    RichText::new("🔒 Unlock or set up the vault below first, so the connection can be stored securely.")
                        .color(AMBER)
                        .size(12.5),
                );
            }
        });
        if open_site {
            ctx.open_url(egui::OpenUrl::new_tab(BRIDGE_SITE));
        }
        if connect {
            self.touch_vault(ctx);
            self.start_connect(ctx);
        }
        if let Some(id) = sync_id {
            self.touch_vault(ctx);
            self.start_sync(ctx, id);
        }
        if let Some(id) = remove_id {
            if let Some(pos) = self.state.connections.iter().position(|c| c.id == id) {
                let c = self.state.connections.remove(pos);
                self.state.credentials.retain(|k| k.id != c.credential_id);
                self.toast(ctx, "Connection removed. Its saved access was deleted from the vault.");
            }
        }
    }

    fn vault_card(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        if self.state.vault.is_none() {
            self.vault_setup(ui, ctx);
        } else if self.vault_key.is_none() {
            self.vault_locked(ui, ctx);
        } else {
            self.vault_open(ui, ctx);
        }
    }

    fn vault_setup(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let mut create = false;
        card(ui, |ui| {
            section(ui, "🔒 Saved logins");
            muted(
                ui,
                "Choose a master password to lock your saved logins. Logins are encrypted with it on this computer. Write it down somewhere safe: it can't be recovered, and without it the saved logins can't be opened.",
            );
            egui::Grid::new("vault_setup").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label("Master password");
                ui.add(
                    egui::TextEdit::singleline(&mut self.vault_pw)
                        .password(true)
                        .desired_width(260.0)
                        .hint_text(hint("At least 8 characters")),
                );
                ui.end_row();
                ui.label("Type it again");
                let r = ui.add(egui::TextEdit::singleline(&mut self.vault_pw2).password(true).desired_width(260.0));
                create |= r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.end_row();
            });
            create |= primary(ui, "Create vault").clicked();
            if !self.vault_msg.is_empty() {
                ui.label(RichText::new(self.vault_msg.as_str()).color(ACCENT));
            }
        });
        if create {
            if self.vault_pw != self.vault_pw2 {
                self.vault_msg = "The two passwords don't match.".into();
                return;
            }
            match vault::create(&self.vault_pw) {
                Ok((meta, key)) => {
                    self.state.vault = Some(meta);
                    self.vault_key = Some(key);
                    self.vault_pw.clear();
                    self.vault_pw2.clear();
                    self.vault_msg.clear();
                    self.touch_vault(ctx);
                    self.toast(ctx, "Vault created and unlocked.");
                }
                Err(e) => self.vault_msg = e,
            }
        }
    }

    fn vault_locked(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let mut unlock = false;
        let mut reset = false;
        card(ui, |ui| {
            row_right(ui, |ui| section(ui, "🔒 Saved logins"), |ui| chip(ui, "Locked", AMBER));
            let n = self.state.credentials.iter().filter(|c| c.kind != ServiceKind::BankSync).count();
            muted(
                ui,
                format!("{} saved login{}. Unlock to see or use them.", n, if n == 1 { "" } else { "s" }),
            );
            ui.horizontal_wrapped(|ui| {
                for c in self.state.credentials.iter().filter(|c| c.kind != ServiceKind::BankSync) {
                    chip(ui, &format!("{} {}", c.kind.icon(), c.institution), MUTED);
                }
            });
            ui.horizontal(|ui| {
                let r = ui.add(
                    egui::TextEdit::singleline(&mut self.vault_pw)
                        .password(true)
                        .desired_width(260.0)
                        .hint_text(hint("Master password")),
                );
                unlock |= r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                unlock |= primary(ui, "Unlock").clicked();
            });
            if !self.vault_msg.is_empty() {
                ui.label(RichText::new(self.vault_msg.as_str()).color(ACCENT));
            }
            ui.add_space(4.0);
            if !self.confirm_vault_reset {
                if ui.small_button("Forgot the master password?").clicked() {
                    self.confirm_vault_reset = true;
                }
            } else {
                ui.label(
                    RichText::new("A forgotten master password can't be recovered. Resetting deletes every saved login and bank connection; your accounts and transactions stay.")
                        .color(AMBER),
                );
                ui.horizontal(|ui| {
                    if primary(ui, "Reset the vault").clicked() {
                        reset = true;
                    }
                    if ui.button("Cancel").clicked() {
                        self.confirm_vault_reset = false;
                    }
                });
            }
        });
        if unlock {
            let result = self.state.vault.as_ref().map(|m| vault::unlock(m, &self.vault_pw));
            match result {
                Some(Ok(key)) => {
                    self.vault_key = Some(key);
                    self.vault_pw.clear();
                    self.vault_msg.clear();
                    self.touch_vault(ctx);
                }
                Some(Err(e)) => self.vault_msg = e,
                None => {}
            }
        }
        if reset {
            self.state.vault = None;
            self.state.credentials.clear();
            self.state.connections.clear();
            self.confirm_vault_reset = false;
            self.vault_msg.clear();
            self.toast(ctx, "The vault was reset. Choose a new master password.");
        }
    }

    fn vault_open(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let key = self.vault_key.expect("unlocked");
        enum Act {
            Lock,
            Add,
            Edit(u32),
            Delete(u32),
            ConfirmDelete(u32),
            CancelDelete,
            Copy(String, &'static str),
            Open(String),
        }
        let mut act = None;
        let accounts: Vec<(u32, String)> = self.state.accounts.iter().map(|a| (a.id, a.name.clone())).collect();
        let account_name = |id: Option<u32>| id.and_then(|id| accounts.iter().find(|a| a.0 == id).map(|a| a.1.clone()));
        card(ui, |ui| {
            row_right(
                ui,
                |ui| {
                    section(ui, "🔓 Saved logins");
                    chip(ui, "Unlocked", GREEN);
                },
                |ui| {
                    if ui.button("🔒 Lock").clicked() {
                        act = Some(Act::Lock);
                    }
                    if primary(ui, "➕ Add a login").clicked() {
                        act = Some(Act::Add);
                    }
                },
            );
            let items: Vec<&Credential> = self.state.credentials.iter().filter(|c| c.kind != ServiceKind::BankSync).collect();
            if items.is_empty() {
                muted(
                    ui,
                    "No saved logins yet. Add your bank, card issuer, lender or any other financial service.",
                );
            }
            egui::Grid::new("vault_items")
                .num_columns(4)
                .spacing([18.0, 10.0])
                .striped(true)
                .show(ui, |ui| {
                    for c in items {
                        let secret = vault::open_secret(&key, c);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(c.kind.icon()).color(ACCENT));
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 1.0;
                                ui.label(RichText::new(c.institution.as_str()).strong());
                                let mut sub = c.kind.name().to_string();
                                if let Some(n) = account_name(c.account_id) {
                                    sub += &format!(" · {n}");
                                }
                                ui.label(RichText::new(sub).color(MUTED).size(12.5));
                            });
                        });
                        match secret {
                            Err(e) => {
                                ui.label(RichText::new(format!("Can't open: {e}")).color(ACCENT));
                                ui.label("");
                            }
                            Ok(s) => {
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 1.0;
                                    ui.label(if s.username.is_empty() {
                                        "—".to_string()
                                    } else {
                                        s.username.clone()
                                    });
                                    let acct = s.masked_account();
                                    if !acct.is_empty() {
                                        ui.label(RichText::new(acct).color(MUTED).size(12.5));
                                    }
                                });
                                ui.horizontal(|ui| {
                                    if !s.username.is_empty() && ui.small_button("Copy user").clicked() {
                                        act = Some(Act::Copy(s.username.clone(), "Username"));
                                    }
                                    if !s.password.is_empty() && ui.small_button("Copy password").clicked() {
                                        act = Some(Act::Copy(s.password.clone(), "Password"));
                                    }
                                    if !s.website.trim().is_empty() && ui.small_button("Open site").clicked() {
                                        act = Some(Act::Open(web_address(&s.website)));
                                    }
                                });
                            }
                        }
                        ui.horizontal(|ui| {
                            if self.confirm_delete_cred == Some(c.id) {
                                ui.label(RichText::new("Delete?").color(AMBER));
                                if ui.small_button("Yes").clicked() {
                                    act = Some(Act::Delete(c.id));
                                }
                                if ui.small_button("No").clicked() {
                                    act = Some(Act::CancelDelete);
                                }
                            } else {
                                if ui.small_button("Edit").clicked() {
                                    act = Some(Act::Edit(c.id));
                                }
                                if ui.small_button("✖").on_hover_text("Delete").clicked() {
                                    act = Some(Act::ConfirmDelete(c.id));
                                }
                            }
                        });
                        ui.end_row();
                    }
                });
            muted(
                ui,
                "Locks itself after 10 minutes. Copied passwords are cleared from the clipboard after 30 seconds.",
            );
        });

        if self.cred_edit.is_some() {
            ui.add_space(6.0);
            self.cred_form(ui, ctx, &key, &accounts);
        }

        if act.is_some() {
            self.touch_vault(ctx);
        }
        match act {
            Some(Act::Lock) => self.lock_vault(),
            Some(Act::Add) => {
                self.cred_edit = Some(CredEdit::new());
                self.show_password = false;
            }
            Some(Act::Edit(id)) => {
                if let Some(c) = self.state.credentials.iter().find(|c| c.id == id) {
                    match vault::open_secret(&key, c) {
                        Ok(secret) => {
                            self.cred_edit = Some(CredEdit {
                                id: Some(id),
                                institution: c.institution.clone(),
                                kind: c.kind,
                                account_id: c.account_id,
                                secret,
                                error: String::new(),
                            });
                            self.show_password = false;
                        }
                        Err(e) => self.toast(ctx, format!("Couldn't open that login: {e}")),
                    }
                }
            }
            Some(Act::ConfirmDelete(id)) => self.confirm_delete_cred = Some(id),
            Some(Act::CancelDelete) => self.confirm_delete_cred = None,
            Some(Act::Delete(id)) => {
                self.state.credentials.retain(|c| c.id != id);
                self.confirm_delete_cred = None;
                if self.cred_edit.as_ref().is_some_and(|e| e.id == Some(id)) {
                    self.cred_edit = None;
                }
            }
            Some(Act::Copy(text, what)) => {
                ctx.copy_text(text);
                if what == "Password" {
                    self.clipboard_clear_at = Some(ctx.input(|i| i.time) + CLIPBOARD_SECS);
                    self.toast(ctx, "Password copied. It will be cleared from the clipboard in 30 seconds.");
                } else {
                    self.toast(ctx, format!("{what} copied."));
                }
            }
            Some(Act::Open(url)) => ctx.open_url(egui::OpenUrl::new_tab(url)),
            None => {}
        }
    }

    fn cred_form(&mut self, ui: &mut Ui, ctx: &egui::Context, key: &vault::Key, accounts: &[(u32, String)]) {
        let Some(e) = self.cred_edit.as_mut() else { return };
        let mut save = false;
        let mut cancel = false;
        let show_password = &mut self.show_password;
        card(ui, |ui| {
            section(ui, if e.id.is_some() { "Edit login" } else { "Add a login" });
            muted(ui, "Everything below except the name, type and linked account is encrypted.");
            egui::Grid::new("cred_form").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label("Name *");
                ui.add(
                    egui::TextEdit::singleline(&mut e.institution)
                        .hint_text(hint("e.g. Example Bank"))
                        .desired_width(320.0),
                );
                ui.end_row();
                ui.label("Type");
                egui::ComboBox::new("cred_kind", "").selected_text(e.kind.name()).show_ui(ui, |ui| {
                    for k in ServiceKind::PICKABLE {
                        ui.selectable_value(&mut e.kind, k, k.name());
                    }
                });
                ui.end_row();
                ui.label("Linked account");
                let current = e
                    .account_id
                    .and_then(|id| accounts.iter().find(|a| a.0 == id).map(|a| a.1.clone()))
                    .unwrap_or_else(|| "None".into());
                egui::ComboBox::new("cred_account", "").selected_text(current).show_ui(ui, |ui| {
                    ui.selectable_value(&mut e.account_id, None, "None");
                    for (id, name) in accounts {
                        ui.selectable_value(&mut e.account_id, Some(*id), name.as_str());
                    }
                });
                ui.end_row();
                ui.label("Website");
                ui.add(
                    egui::TextEdit::singleline(&mut e.secret.website)
                        .hint_text(hint("examplebank.com"))
                        .desired_width(320.0),
                );
                ui.end_row();
                ui.label("Username");
                ui.add(egui::TextEdit::singleline(&mut e.secret.username).desired_width(320.0));
                ui.end_row();
                ui.label("Password");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut e.secret.password)
                            .password(!*show_password)
                            .desired_width(250.0),
                    );
                    ui.checkbox(show_password, "Show");
                });
                ui.end_row();
                ui.label("Account number");
                ui.add(egui::TextEdit::singleline(&mut e.secret.account_number).desired_width(320.0));
                ui.end_row();
                ui.label("Phone");
                ui.add(
                    egui::TextEdit::singleline(&mut e.secret.phone)
                        .hint_text(hint("Customer service"))
                        .desired_width(320.0),
                );
                ui.end_row();
                ui.label("Notes");
                ui.add(egui::TextEdit::multiline(&mut e.secret.notes).desired_rows(2).desired_width(320.0));
                ui.end_row();
            });
            if !e.error.is_empty() {
                ui.label(RichText::new(e.error.as_str()).color(ACCENT));
            }
            ui.horizontal(|ui| {
                save = primary(ui, "Save securely").clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
        if cancel {
            self.cred_edit = None;
            return;
        }
        if !save {
            return;
        }
        self.touch_vault(ctx);
        let Some(e) = self.cred_edit.as_mut() else { return };
        if e.institution.trim().is_empty() {
            e.error = "Give this login a name.".into();
            return;
        }
        let (nonce, cipher) = match vault::seal_secret(key, &e.secret) {
            Ok(x) => x,
            Err(err) => {
                e.error = err;
                return;
            }
        };
        let id =
            e.id.unwrap_or_else(|| self.state.credentials.iter().map(|c| c.id).max().unwrap_or(0) + 1);
        let cred = Credential {
            id,
            institution: e.institution.trim().to_string(),
            kind: e.kind,
            account_id: e.account_id,
            nonce,
            cipher,
            updated: now_text(),
        };
        match self.state.credentials.iter_mut().find(|c| c.id == id) {
            Some(existing) => *existing = cred,
            None => self.state.credentials.push(cred),
        }
        self.cred_edit = None;
        self.toast(ctx, "Login saved and encrypted.");
    }
}
