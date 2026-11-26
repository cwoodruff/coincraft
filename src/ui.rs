//! The window: a sidebar of screens and one page per feature.

mod pages_connect;
mod pages_money;
mod pages_plan;
mod pages_wealth;
mod widgets;
mod wizard;

use crate::data::*;
use crate::db::{self, Db};
use eframe::egui::{self, vec2, Align, Align2, Color32, FontId, Frame, Layout, Margin, Rect, RichText, Sense, Stroke, TextStyle, Ui};
use std::path::PathBuf;
use widgets::*;

// ---------------------------------------------------------------- colours

const BG: Color32 = Color32::from_rgb(12, 18, 34);
const SIDEBAR: Color32 = Color32::from_rgb(16, 24, 44);
const CARD: Color32 = Color32::from_rgb(22, 32, 56);
const CARD_HI: Color32 = Color32::from_rgb(30, 42, 70);
const BORDER: Color32 = Color32::from_rgb(38, 52, 84);
const TEXT: Color32 = Color32::from_rgb(232, 236, 245);
const MUTED: Color32 = Color32::from_rgb(140, 152, 178);
const ACCENT: Color32 = Color32::from_rgb(242, 95, 92);
const GREEN: Color32 = Color32::from_rgb(61, 214, 140);
const AMBER: Color32 = Color32::from_rgb(245, 184, 65);
const BLUE: Color32 = Color32::from_rgb(98, 150, 255);
const PURPLE: Color32 = Color32::from_rgb(170, 120, 255);

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Dashboard,
    Recurring,
    Spending,
    Budgets,
    Transactions,
    Connections,
    NetWorth,
    Savings,
    Credit,
    Bills,
    Assistant,
    Settings,
}

impl Page {
    const NAV: [Page; 12] = [
        Page::Dashboard,
        Page::Recurring,
        Page::Spending,
        Page::Budgets,
        Page::Transactions,
        Page::Connections,
        Page::NetWorth,
        Page::Savings,
        Page::Credit,
        Page::Bills,
        Page::Assistant,
        Page::Settings,
    ];
    fn label(self) -> &'static str {
        match self {
            Page::Dashboard => "Dashboard",
            Page::Recurring => "Recurring",
            Page::Spending => "Spending",
            Page::Budgets => "Budgets",
            Page::Transactions => "Transactions",
            Page::Connections => "Connections",
            Page::NetWorth => "Net Worth",
            Page::Savings => "Savings",
            Page::Credit => "Credit Score",
            Page::Bills => "Bill Negotiation",
            Page::Assistant => "Assistant",
            Page::Settings => "Settings",
        }
    }
    fn icon(self) -> &'static str {
        match self {
            Page::Dashboard => "🏠",
            Page::Recurring => "🔁",
            Page::Spending => "📊",
            Page::Budgets => "🎯",
            Page::Transactions => "📄",
            Page::Connections => "↔",
            Page::NetWorth => "📈",
            Page::Savings => "💰",
            Page::Credit => "💳",
            Page::Bills => "📞",
            Page::Assistant => "💬",
            Page::Settings => "⚙",
        }
    }
}

struct NewTx {
    date: String,
    merchant: String,
    amount: f64,
    income: bool,
    category: Option<Category>,
    account: u32,
}

pub struct App {
    state: State,
    page: Page,
    /// The local SQLite database; None while taking a check picture or if it couldn't be opened.
    db: Option<Db>,
    /// A plain description of where data is saved, or what went wrong.
    db_status: String,
    /// What was last written to the database, to skip saves when nothing changed.
    last_saved: String,
    next_save_at: f64,
    /// The first-run setup wizard, while it is showing.
    wizard: Option<wizard::Wizard>,
    screenshot: Option<PathBuf>,
    frames: u64,
    shot_requested: bool,
    toasts: Vec<(String, f64)>,
    started: bool,

    tx_search: String,
    tx_cat: Option<Category>,
    tx_account: Option<u32>,
    new_tx: NewTx,
    csv_path: String,
    csv_account: u32,
    csv_msg: String,

    spend_offset: i32,
    new_budget_cat: Category,
    new_budget_limit: f64,

    goal_name: String,
    goal_target: f64,
    goal_auto: f64,
    goal_freq: Freq,

    bill_provider: String,
    bill_kind: BillKind,
    bill_monthly: f64,

    acct_name: String,
    acct_kind: AccountKind,
    acct_balance: f64,
    acct_limit: f64,

    chat_input: String,
    confirm_reset: Option<bool>,

    rule_contains: String,
    rule_cat: Category,
    rule_rename: String,
    new_custom_cat: String,

    /// The vault key, only while unlocked. Never saved.
    vault_key: Option<crate::vault::Key>,
    vault_last_used: f64,
    vault_pw: String,
    vault_pw2: String,
    vault_msg: String,
    confirm_vault_reset: bool,
    /// The login being added or edited.
    cred_edit: Option<pages_connect::CredEdit>,
    show_password: bool,
    confirm_delete_cred: Option<u32>,
    clipboard_clear_at: Option<f64>,
    /// Bank sync running in the background, if any.
    sync_rx: Option<std::sync::mpsc::Receiver<pages_connect::SyncMsg>>,
    sync_label: String,
    sf_token: String,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>, start_page: Option<String>, force_wizard: bool) -> Self {
        setup_style(&cc.egui_ctx);

        // A check picture uses sample data and never touches the person's database.
        let (db, db_status, saved) = if screenshot.is_some() {
            (None, "Check mode: nothing is saved.".to_string(), None)
        } else {
            let path = std::env::var_os("COINCRAFT_DB").map(PathBuf::from).unwrap_or_else(db::default_path);
            match Db::open(&path) {
                Ok(db) => {
                    let status = format!("Saved in a local SQLite database at {}", path.display());
                    match db.load_state() {
                        Ok(state) => (Some(db), status, state),
                        Err(e) => (Some(db), format!("Couldn't read the database ({e}). Starting fresh."), None),
                    }
                }
                Err(e) => (
                    None,
                    format!("Couldn't open the database at {} ({e}). Changes won't be saved.", path.display()),
                    None,
                ),
            }
        };
        let setup_done = saved.as_ref().is_some_and(|s| s.profile.setup_complete);
        let needs_wizard = force_wizard || (screenshot.is_none() && !setup_done);
        let wizard = needs_wizard.then(|| wizard::Wizard::new(saved.as_ref().map(|s| s.profile.clone()), saved.is_some()));
        let state = saved.unwrap_or_else(sample);
        set_custom_categories(&state.custom_categories);
        let checking = state.checking_id();
        let page = start_page
            .and_then(|name| {
                let name = name.to_lowercase().replace(['-', '_', ' '], "");
                Page::NAV
                    .into_iter()
                    .find(|p| p.label().to_lowercase().replace(' ', "").starts_with(&name))
            })
            .unwrap_or(Page::Dashboard);
        let last_saved = if wizard.is_none() {
            serde_json::to_string(&state).unwrap_or_default()
        } else {
            String::new()
        };
        Self {
            state,
            page,
            db,
            db_status,
            last_saved,
            next_save_at: 0.0,
            wizard,
            screenshot,
            frames: 0,
            shot_requested: false,
            toasts: Vec::new(),
            started: false,
            tx_search: String::new(),
            tx_cat: None,
            tx_account: None,
            new_tx: NewTx {
                date: today().format("%Y-%m-%d").to_string(),
                merchant: String::new(),
                amount: 0.0,
                income: false,
                category: None,
                account: checking,
            },
            csv_path: String::new(),
            csv_account: checking,
            csv_msg: String::new(),
            spend_offset: 0,
            new_budget_cat: Category::Health,
            new_budget_limit: 100.0,
            goal_name: String::new(),
            goal_target: 1000.0,
            goal_auto: 50.0,
            goal_freq: Freq::Monthly,
            bill_provider: String::new(),
            bill_kind: BillKind::Internet,
            bill_monthly: 50.0,
            acct_name: String::new(),
            acct_kind: AccountKind::Savings,
            acct_balance: 0.0,
            acct_limit: 0.0,
            chat_input: String::new(),
            confirm_reset: None,
            rule_contains: String::new(),
            rule_cat: Category::Dining,
            rule_rename: String::new(),
            new_custom_cat: String::new(),
            vault_key: None,
            vault_last_used: 0.0,
            vault_pw: String::new(),
            vault_pw2: String::new(),
            vault_msg: String::new(),
            confirm_vault_reset: false,
            cred_edit: None,
            show_password: false,
            confirm_delete_cred: None,
            clipboard_clear_at: None,
            sync_rx: None,
            sync_label: String::new(),
            sf_token: String::new(),
        }
    }

    /// Writes the state to the database if anything changed since the last save.
    fn persist(&mut self) {
        if self.wizard.is_some() {
            return;
        }
        let Some(db) = self.db.as_mut() else { return };
        let snapshot = serde_json::to_string(&self.state).unwrap_or_default();
        if snapshot == self.last_saved {
            return;
        }
        match db.save_state(&self.state) {
            Ok(()) => self.last_saved = snapshot,
            Err(e) => self.db_status = format!("Couldn't save to the database: {e}"),
        }
    }

    /// Replaces everything with a fresh start for this profile and saves it straight away.
    fn start_fresh(&mut self, profile: Profile, with_sample_data: bool) {
        // Saved logins and bank connections belong to the person, not the data, so they stay.
        let vault = self.state.vault.take();
        let credentials = std::mem::take(&mut self.state.credentials);
        let connections = std::mem::take(&mut self.state.connections);
        self.state = if with_sample_data {
            let mut s = sample();
            s.profile = profile;
            s
        } else {
            empty_state(profile)
        };
        self.state.vault = vault;
        self.state.credentials = credentials;
        self.state.connections = connections;
        set_custom_categories(&self.state.custom_categories);
        let checking = self.state.checking_id();
        self.new_tx.account = checking;
        self.csv_account = checking;
        self.tx_account = None;
        self.page = Page::Dashboard;
        self.started = false;
        self.last_saved.clear();
        self.persist();
    }

    /// Opens the setup wizard again, keeping the current profile details.
    fn rerun_setup(&mut self) {
        if let Some(db) = &self.db {
            if let Err(e) = db.reset_setup() {
                self.db_status = format!("Couldn't update the database: {e}");
            }
        }
        self.wizard = Some(wizard::Wizard::new(Some(self.state.profile.clone()), true));
    }

    fn toast(&mut self, ctx: &egui::Context, text: impl Into<String>) {
        let t = ctx.input(|i| i.time);
        self.toasts.push((text.into(), t + 5.0));
    }

    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        let Some(path) = self.screenshot.clone() else { return };
        self.frames += 1;
        ctx.request_repaint();
        let image = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(img) = image {
            let [w, h] = img.size;
            let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    let _ = std::fs::create_dir_all(parent);
                }
            }
            match image::RgbaImage::from_raw(w as u32, h as u32, bytes) {
                Some(buf) => {
                    if let Err(e) = buf.save(&path) {
                        eprintln!("could not save screenshot: {e}");
                    }
                }
                None => eprintln!("screenshot had an unexpected size"),
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else if self.frames >= 120 && !self.shot_requested {
            self.shot_requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
        } else if self.frames > 2000 {
            eprintln!("screenshot never arrived");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn sidebar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            logo(ui, 34.0);
            ui.label(RichText::new("CoinCraft").size(20.0).strong());
        });
        ui.add_space(18.0);
        let soon = self
            .state
            .active_subs()
            .filter(|s| matches!(s.status, SubStatus::Active) && (s.next_charge() - today()).num_days() <= self.state.alerts.renewal_days)
            .count();
        for page in Page::NAV {
            if page == Page::Settings {
                ui.add_space(10.0);
            }
            let selected = self.page == page;
            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::click());
            let p = ui.painter();
            if selected {
                p.rect_filled(rect, 9.0, Color32::from_rgb(52, 32, 48));
                p.rect_filled(Rect::from_min_size(rect.min + vec2(0.0, 8.0), vec2(3.0, 22.0)), 2.0, ACCENT);
            } else if resp.hovered() {
                p.rect_filled(rect, 9.0, CARD);
            }
            let col = if selected { TEXT } else { MUTED };
            p.text(
                rect.left_center() + vec2(16.0, 0.0),
                Align2::LEFT_CENTER,
                page.icon(),
                FontId::proportional(16.0),
                if selected { ACCENT } else { MUTED },
            );
            p.text(
                rect.left_center() + vec2(44.0, 0.0),
                Align2::LEFT_CENTER,
                page.label(),
                FontId::proportional(14.5),
                col,
            );
            if page == Page::Recurring && soon > 0 {
                let c = rect.right_center() - vec2(18.0, 0.0);
                p.circle_filled(c, 10.0, ACCENT);
                p.text(
                    c,
                    Align2::CENTER_CENTER,
                    soon.to_string(),
                    FontId::proportional(11.0),
                    Color32::WHITE,
                );
            }
            if resp.clicked() {
                self.page = page;
            }
        }
        // The person's card sits near the bottom of the sidebar and opens Settings.
        ui.add_space((ui.available_height() - 78.0).max(12.0));
        let profile = &self.state.profile;
        let mut open_settings = false;
        card(ui, |ui| {
            ui.horizontal(|ui| {
                let (r, resp) = ui.allocate_exact_size(vec2(36.0, 36.0), Sense::click());
                ui.painter().circle_filled(r.center(), 18.0, Color32::from_rgb(52, 32, 48));
                let initials = profile.initials();
                let initials = if initials.is_empty() { "?".to_string() } else { initials };
                ui.painter()
                    .text(r.center(), Align2::CENTER_CENTER, initials, FontId::proportional(14.0), ACCENT);
                open_settings |= resp.clicked();
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    let name = if profile.full_name().is_empty() {
                        "You".to_string()
                    } else {
                        profile.full_name()
                    };
                    open_settings |= ui
                        .add(egui::Label::new(RichText::new(name).strong()).sense(Sense::click()))
                        .clicked();
                    ui.label(RichText::new("🔒 Stored on this computer").color(MUTED).size(11.5));
                });
            });
        });
        if open_settings {
            self.page = Page::Settings;
        }
    }

    fn draw_toasts(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        self.toasts.retain(|t| t.1 > now);
        if self.toasts.is_empty() {
            return;
        }
        egui::Area::new(egui::Id::new("toasts"))
            .anchor(Align2::RIGHT_TOP, vec2(-20.0, 20.0))
            .show(ctx, |ui| {
                for (text, _) in self.toasts.iter().rev().take(4) {
                    Frame::none()
                        .fill(Color32::from_rgb(8, 12, 24))
                        .stroke(Stroke::new(1.0_f32, ACCENT))
                        .rounding(10.0)
                        .inner_margin(Margin::symmetric(14.0, 10.0))
                        .show(ui, |ui| {
                            ui.set_max_width(340.0);
                            ui.label(text.as_str());
                        });
                    ui.add_space(6.0);
                }
            });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_screenshot(ctx);

        // First run: only the setup wizard shows until it is finished.
        if let Some(w) = self.wizard.as_mut() {
            let note = self.db_status.clone();
            if let Some(result) = w.show(ctx, &note) {
                self.wizard = None;
                match result.start {
                    wizard::StartWith::KeepMyData => {
                        self.state.profile = result.profile;
                        self.last_saved.clear();
                        self.persist();
                    }
                    wizard::StartWith::SampleData => self.start_fresh(result.profile, true),
                    wizard::StartWith::CleanSlate => self.start_fresh(result.profile, false),
                }
                let name = self.state.profile.first_name.clone();
                self.toast(ctx, format!("Welcome to CoinCraft, {name}!"));
            }
            self.draw_toasts(ctx);
            return;
        }

        self.vault_housekeeping(ctx);
        self.poll_sync(ctx);

        // Save to the database every few seconds when something changed.
        let now = ctx.input(|i| i.time);
        if now >= self.next_save_at {
            self.next_save_at = now + 3.0;
            self.persist();
        }
        ctx.request_repaint_after(std::time::Duration::from_secs(3));

        if !self.started {
            self.started = true;
            let notes = self.state.run_due_transfers();
            for n in notes {
                self.toast(ctx, n);
            }
            self.state.refresh_net_worth();
        }
        let news = self.state.tick();
        if !news.is_empty() {
            // A lowered bill also lowers the matching recurring charge.
            for b in &self.state.bills {
                if let NegStatus::Saved { new_monthly, .. } = b.status {
                    for s in &mut self.state.subscriptions {
                        if s.merchant.eq_ignore_ascii_case(&b.provider) {
                            s.amount = new_monthly;
                        }
                    }
                }
            }
            for n in news {
                self.toast(ctx, n);
            }
        }
        let pending = self
            .state
            .subscriptions
            .iter()
            .any(|s| matches!(s.status, SubStatus::Cancelling { .. }))
            || self.state.bills.iter().any(|b| matches!(b.status, NegStatus::Negotiating { .. }));
        if pending || !self.toasts.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }

        egui::SidePanel::left("nav")
            .resizable(false)
            .exact_width(220.0)
            .frame(Frame::none().fill(SIDEBAR).inner_margin(Margin::symmetric(14.0, 18.0)))
            .show(ctx, |ui| self.sidebar(ui));

        egui::CentralPanel::default()
            .frame(Frame::none().fill(BG).inner_margin(Margin::symmetric(28.0, 22.0)))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.page {
                        Page::Dashboard => self.page_dashboard(ui),
                        Page::Recurring => self.page_recurring(ui),
                        Page::Spending => self.page_spending(ui),
                        Page::Budgets => self.page_budgets(ui),
                        Page::Transactions => self.page_transactions(ui),
                        Page::Connections => self.page_connections(ui),
                        Page::NetWorth => self.page_net_worth(ui),
                        Page::Savings => self.page_savings(ui),
                        Page::Credit => self.page_credit(ui),
                        Page::Bills => self.page_bills(ui),
                        Page::Assistant => self.page_assistant(ui),
                        Page::Settings => self.page_settings(ui),
                    });
            });

        self.draw_toasts(ctx);
    }

    /// eframe calls this now and then and when closing; the data goes to SQLite, not eframe's storage.
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.persist();
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.persist();
    }
}

fn setup_style(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = CARD;
    v.extreme_bg_color = Color32::from_rgb(14, 21, 40);
    v.faint_bg_color = Color32::from_rgb(26, 37, 63);
    v.override_text_color = Some(TEXT);
    v.hyperlink_color = ACCENT;
    v.selection.bg_fill = Color32::from_rgb(120, 52, 60);
    v.selection.stroke = Stroke::new(1.0_f32, ACCENT);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT);
    v.widgets.inactive.weak_bg_fill = CARD_HI;
    v.widgets.inactive.bg_fill = Color32::from_rgb(44, 58, 92);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(42, 56, 90);
    v.widgets.hovered.bg_fill = Color32::from_rgb(54, 70, 108);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(52, 68, 104);
    // Bold text takes its colour from here.
    v.widgets.active.fg_stroke = Stroke::new(1.0_f32, Color32::WHITE);
    // Always dark, even when the computer is in light mode.
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.set_visuals_of(egui::Theme::Dark, v);

    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    style.spacing.item_spacing = vec2(8.0, 8.0);
    style.spacing.button_padding = vec2(12.0, 6.0);
    style.spacing.interact_size.y = 28.0;
    style.text_styles.insert(TextStyle::Body, FontId::proportional(14.5));
    style.text_styles.insert(TextStyle::Button, FontId::proportional(14.0));
    style.text_styles.insert(TextStyle::Heading, FontId::proportional(24.0));
    style.text_styles.insert(TextStyle::Small, FontId::proportional(12.0));
    ctx.set_style_of(egui::Theme::Dark, style);
}
