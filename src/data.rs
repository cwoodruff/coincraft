//! Money data: accounts, transactions, subscriptions, budgets, bills, goals and credit.
//! Everything here is plain data plus the calculations the screens show.

use crate::vault::{Credential, VaultMeta};
use chrono::{Datelike, Duration, Local, Months, NaiveDate};
use eframe::egui::Color32;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

pub fn now_secs() -> i64 {
    Local::now().timestamp()
}

pub fn month_start(d: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(d.year(), d.month(), 1).unwrap()
}

pub fn add_months(d: NaiveDate, n: i32) -> NaiveDate {
    if n >= 0 {
        d.checked_add_months(Months::new(n as u32)).unwrap_or(d)
    } else {
        d.checked_sub_months(Months::new((-n) as u32)).unwrap_or(d)
    }
}

/// A date in a month, clamped to the month's last day.
fn day_in_month(month: NaiveDate, day: u32) -> NaiveDate {
    let mut d = day;
    loop {
        if let Some(x) = NaiveDate::from_ymd_opt(month.year(), month.month(), d) {
            return x;
        }
        d -= 1;
    }
}

pub fn money(v: f64) -> String {
    let neg = v < 0.0;
    let cents = (v.abs() * 100.0).round() as i64;
    let whole = cents / 100;
    let frac = cents % 100;
    let s = whole.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    format!("{}${}.{:02}", if neg { "-" } else { "" }, out, frac)
}

pub fn money0(v: f64) -> String {
    let s = money(v);
    s[..s.len() - 3].to_string()
}

// ---------------------------------------------------------------- categories

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Debug)]
pub enum Category {
    Income,
    Housing,
    Groceries,
    Dining,
    Transport,
    Utilities,
    Subscriptions,
    Entertainment,
    Shopping,
    Health,
    Travel,
    Transfer,
    Other,
    /// A category the person made; its name lives in `State::custom_categories`.
    Custom(u16),
}

/// Names of the person's own categories, kept here so `Category::name` can stay simple.
static CUSTOM: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

pub fn set_custom_categories(names: &[String]) {
    let mut g = CUSTOM.lock().unwrap();
    *g = names.iter().map(|n| &*Box::leak(n.clone().into_boxed_str())).collect();
}

impl Category {
    /// Built-in categories followed by the person's own.
    pub fn all() -> Vec<Category> {
        let n = CUSTOM.lock().unwrap().len();
        Category::ALL
            .iter()
            .copied()
            .chain((0..n).map(|i| Category::Custom(i as u16)))
            .collect()
    }

    pub const ALL: [Category; 13] = [
        Category::Income,
        Category::Housing,
        Category::Groceries,
        Category::Dining,
        Category::Transport,
        Category::Utilities,
        Category::Subscriptions,
        Category::Entertainment,
        Category::Shopping,
        Category::Health,
        Category::Travel,
        Category::Transfer,
        Category::Other,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Category::Income => "Income",
            Category::Housing => "Housing",
            Category::Groceries => "Groceries",
            Category::Dining => "Dining & Drinks",
            Category::Transport => "Auto & Transport",
            Category::Utilities => "Bills & Utilities",
            Category::Subscriptions => "Subscriptions",
            Category::Entertainment => "Entertainment",
            Category::Shopping => "Shopping",
            Category::Health => "Health",
            Category::Travel => "Travel",
            Category::Transfer => "Transfers & Payments",
            Category::Other => "Other",
            Category::Custom(i) => CUSTOM.lock().unwrap().get(i as usize).copied().unwrap_or("Custom"),
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Category::Income => "💵",
            Category::Housing => "🏠",
            Category::Groceries => "🍎",
            Category::Dining => "🍴",
            Category::Transport => "🚗",
            Category::Utilities => "💡",
            Category::Subscriptions => "🔁",
            Category::Entertainment => "🎬",
            Category::Shopping => "👜",
            Category::Health => "➕",
            Category::Travel => "✈",
            Category::Transfer => "↔",
            Category::Other => "•",
            Category::Custom(_) => "🏷",
        }
    }

    pub fn color(self) -> Color32 {
        match self {
            Category::Income => Color32::from_rgb(61, 214, 140),
            Category::Housing => Color32::from_rgb(120, 130, 255),
            Category::Groceries => Color32::from_rgb(90, 200, 250),
            Category::Dining => Color32::from_rgb(255, 159, 67),
            Category::Transport => Color32::from_rgb(245, 205, 70),
            Category::Utilities => Color32::from_rgb(180, 120, 255),
            Category::Subscriptions => Color32::from_rgb(242, 95, 92),
            Category::Entertainment => Color32::from_rgb(255, 120, 190),
            Category::Shopping => Color32::from_rgb(80, 225, 200),
            Category::Health => Color32::from_rgb(140, 220, 90),
            Category::Travel => Color32::from_rgb(100, 160, 255),
            Category::Transfer => Color32::from_rgb(150, 160, 180),
            Category::Other => Color32::from_rgb(120, 125, 140),
            Category::Custom(i) => {
                const P: [Color32; 5] = [
                    Color32::from_rgb(255, 214, 102),
                    Color32::from_rgb(130, 200, 255),
                    Color32::from_rgb(255, 140, 120),
                    Color32::from_rgb(160, 235, 160),
                    Color32::from_rgb(220, 160, 255),
                ];
                P[i as usize % P.len()]
            }
        }
    }

    /// Spending categories, i.e. everything that is not income or a transfer.
    pub fn is_spending(self) -> bool {
        !matches!(self, Category::Income | Category::Transfer)
    }

    /// Picks a category from a merchant or statement description.
    pub fn guess(desc: &str, amount: f64) -> Category {
        let d = desc.to_lowercase();
        let has = |words: &[&str]| words.iter().any(|w| d.contains(w));
        if amount > 0.0 {
            if has(&["transfer", "refund", "zelle", "venmo"]) {
                return Category::Transfer;
            }
            return Category::Income;
        }
        if has(&["payroll", "salary", "deposit"]) {
            Category::Income
        } else if has(&["rent", "mortgage", "apartment", "hoa"]) {
            Category::Housing
        } else if has(&["grocer", "market", "mart", "foods", "supermarket"]) && !has(&["online"]) {
            Category::Groceries
        } else if has(&[
            "cafe",
            "coffee",
            "restaurant",
            "pizza",
            "taco",
            "noodle",
            "burger",
            "bar ",
            "grill",
            "kitchen",
            "diner",
        ]) {
            Category::Dining
        } else if has(&["fuel", "gas ", "rideshare", "parking", "transit", "uber", "lyft", "toll", "auto"]) {
            Category::Transport
        } else if has(&[
            "electric",
            "power",
            "water",
            "internet",
            "mobile",
            "wireless",
            "cable",
            "phone",
            "utility",
            "insurance",
        ]) {
            Category::Utilities
        } else if has(&[
            "stream",
            "music",
            "tunes",
            "cloud",
            "storage",
            "news",
            "gym",
            "fit",
            "vault",
            "membership",
            "subscription",
            "plus",
            "premium",
            "prime",
        ]) {
            Category::Subscriptions
        } else if has(&["cinema", "movie", "theater", "concert", "game", "ticket"]) {
            Category::Entertainment
        } else if has(&["pharmacy", "clinic", "dental", "doctor", "health"]) {
            Category::Health
        } else if has(&["air", "hotel", "airline", "travel", "inn", "resort"]) {
            Category::Travel
        } else if has(&["payment", "transfer", "loan", "savings"]) {
            Category::Transfer
        } else if has(&["store", "shop", "online", "goods", "outlet"]) {
            Category::Shopping
        } else {
            Category::Other
        }
    }
}

/// Turns a raw bank description like "SQ *BEAN THERE CAFE #0423 SEATTLE WA" into "Bean There Cafe".
pub fn clean_merchant(raw: &str) -> String {
    const PREFIXES: [&str; 12] = [
        "SQ *",
        "SQ*",
        "TST*",
        "TST *",
        "PP*",
        "PAYPAL *",
        "POS ",
        "DEBIT ",
        "PURCHASE ",
        "ACH ",
        "CHECKCARD ",
        "CARD PURCHASE ",
    ];
    let mut s = raw.trim().to_string();
    // Banks stack these ("POS DEBIT ..."), so keep stripping until none is left.
    while let Some(p) = PREFIXES.iter().find(|p| s.to_uppercase().starts_with(*p)) {
        s = s[p.len()..].trim().to_string();
    }
    let mut words: Vec<&str> = s
        .split_whitespace()
        .filter(|w| !w.contains('#') && !w.chars().any(|c| c.is_ascii_digit()) && !w.starts_with("XXX"))
        .collect();
    // A trailing two-letter state code usually follows a city name; drop both.
    if words.len() > 3 {
        if let Some(last) = words.last() {
            if last.len() == 2 && last.chars().all(|c| c.is_ascii_uppercase()) {
                words.pop();
                words.pop();
            }
        }
    }
    let joined = words.join(" ");
    let shouting = joined.chars().any(|c| c.is_alphabetic()) && !joined.chars().any(|c| c.is_lowercase());
    let out = if shouting {
        joined
            .split(' ')
            .map(|w| {
                let lower = w.to_lowercase();
                let mut ch = lower.chars();
                match ch.next() {
                    Some(f) => f.to_uppercase().collect::<String>() + ch.as_str(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        joined
    };
    if out.trim().is_empty() {
        raw.trim().to_string()
    } else {
        out
    }
}

/// A transaction rule: descriptions containing `contains` get this category (and an optional new name).
#[derive(Clone, Serialize, Deserialize)]
pub struct Rule {
    pub contains: String,
    pub category: Category,
    pub rename: String,
}

impl Rule {
    pub fn matches(&self, merchant: &str) -> bool {
        !self.contains.trim().is_empty() && merchant.to_lowercase().contains(&self.contains.trim().to_lowercase())
    }
}

// ---------------------------------------------------------------- accounts

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum AccountKind {
    Checking,
    Savings,
    CreditCard,
    Investment,
    Property,
    Loan,
}

impl AccountKind {
    pub const ALL: [AccountKind; 6] = [
        AccountKind::Checking,
        AccountKind::Savings,
        AccountKind::CreditCard,
        AccountKind::Investment,
        AccountKind::Property,
        AccountKind::Loan,
    ];

    pub fn is_debt(self) -> bool {
        matches!(self, AccountKind::CreditCard | AccountKind::Loan)
    }

    pub fn name(self) -> &'static str {
        match self {
            AccountKind::Checking => "Checking",
            AccountKind::Savings => "Savings",
            AccountKind::CreditCard => "Credit card",
            AccountKind::Investment => "Investment",
            AccountKind::Property => "Property",
            AccountKind::Loan => "Loan",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Account {
    pub id: u32,
    pub name: String,
    pub kind: AccountKind,
    /// For debts this is the amount owed (positive).
    pub balance: f64,
    /// Credit limit for credit cards.
    pub limit: f64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: u64,
    pub date: NaiveDate,
    pub merchant: String,
    /// Negative is money out, positive is money in.
    pub amount: f64,
    pub category: Category,
    pub account: u32,
}

// ---------------------------------------------------------------- subscriptions

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, Debug)]
pub enum SubStatus {
    Active,
    /// A concierge cancellation is being handled; done at `ready_at` (unix seconds).
    Cancelling {
        ready_at: i64,
    },
    Cancelled,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub merchant: String,
    pub amount: f64,
    pub cadence_days: i64,
    pub last_charge: NaiveDate,
    pub category: Category,
    pub account: u32,
    pub status: SubStatus,
    pub remind: bool,
}

impl Subscription {
    pub fn next_charge(&self) -> NaiveDate {
        let t = today();
        let mut d = self.last_charge;
        loop {
            d = if self.cadence_days >= 28 && self.cadence_days <= 31 {
                add_months(d, 1)
            } else if self.cadence_days >= 360 {
                add_months(d, 12)
            } else {
                d + Duration::days(self.cadence_days)
            };
            if d >= t {
                return d;
            }
        }
    }

    pub fn monthly_cost(&self) -> f64 {
        self.amount * 30.44 / self.cadence_days as f64
    }

    pub fn cadence_name(&self) -> &'static str {
        match self.cadence_days {
            0..=8 => "Weekly",
            9..=16 => "Every 2 weeks",
            17..=45 => "Monthly",
            46..=100 => "Quarterly",
            _ => "Yearly",
        }
    }
}

fn merchant_key(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphabetic() || *c == ' ')
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Looks through transactions for charges that repeat at a steady interval with a steady amount.
pub fn scan_recurring(txs: &[Transaction]) -> Vec<Subscription> {
    let mut groups: BTreeMap<String, Vec<&Transaction>> = BTreeMap::new();
    for t in txs
        .iter()
        .filter(|t| t.amount < 0.0 && !matches!(t.category, Category::Housing | Category::Transfer))
    {
        groups.entry(merchant_key(&t.merchant)).or_default().push(t);
    }
    let mut out = Vec::new();
    for (_, mut list) in groups {
        if list.len() < 2 {
            continue;
        }
        list.sort_by_key(|t| t.date);
        let last = list[list.len() - 1];
        let amounts: Vec<f64> = list.iter().map(|t| -t.amount).collect();
        let avg = amounts.iter().sum::<f64>() / amounts.len() as f64;
        let steady = amounts.iter().all(|a| (a - avg).abs() <= avg * 0.08 + 0.01);
        if !steady {
            continue;
        }
        let gaps: Vec<i64> = list.windows(2).map(|w| (w[1].date - w[0].date).num_days()).collect();
        let avg_gap = gaps.iter().sum::<i64>() as f64 / gaps.len() as f64;
        let cadence = if (5.0..=9.0).contains(&avg_gap) {
            7
        } else if (12.0..=16.0).contains(&avg_gap) {
            14
        } else if (26.0..=35.0).contains(&avg_gap) {
            30
        } else if (85.0..=95.0).contains(&avg_gap) {
            91
        } else if (350.0..=380.0).contains(&avg_gap) {
            365
        } else {
            continue;
        };
        let regular = gaps.iter().all(|g| (*g as f64 - avg_gap).abs() <= 5.0);
        if !regular {
            continue;
        }
        // Too many separate charges a month means it's a shop, not a subscription.
        if cadence == 30 && list.len() as i64 > (list[list.len() - 1].date - list[0].date).num_days() / 20 + 2 {
            continue;
        }
        out.push(Subscription {
            merchant: last.merchant.clone(),
            amount: -last.amount,
            cadence_days: cadence,
            last_charge: last.date,
            category: last.category,
            account: last.account,
            status: SubStatus::Active,
            remind: true,
        });
    }
    out
}

// ---------------------------------------------------------------- budgets & alerts

#[derive(Clone, Serialize, Deserialize)]
pub struct Budget {
    pub category: Category,
    pub limit: f64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AlertSettings {
    pub low_balance: bool,
    pub low_balance_threshold: f64,
    pub overdraft: bool,
    pub budget: bool,
    pub renewals: bool,
    pub renewal_days: i64,
}

impl Default for AlertSettings {
    fn default() -> Self {
        Self {
            low_balance: true,
            low_balance_threshold: 500.0,
            overdraft: true,
            budget: true,
            renewals: true,
            renewal_days: 3,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum AlertLevel {
    Info,
    Warn,
    Danger,
    Good,
}

pub struct Alert {
    pub level: AlertLevel,
    pub title: String,
    pub detail: String,
}

// ---------------------------------------------------------------- bills

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum BillKind {
    Internet,
    Phone,
    Cable,
    Insurance,
    Electric,
    Other,
}

impl BillKind {
    pub const ALL: [BillKind; 6] = [
        BillKind::Internet,
        BillKind::Phone,
        BillKind::Cable,
        BillKind::Insurance,
        BillKind::Electric,
        BillKind::Other,
    ];

    pub fn name(self) -> &'static str {
        match self {
            BillKind::Internet => "Internet",
            BillKind::Phone => "Phone",
            BillKind::Cable => "Cable / TV",
            BillKind::Insurance => "Insurance",
            BillKind::Electric => "Electric",
            BillKind::Other => "Other",
        }
    }

    pub fn eligible(self) -> bool {
        matches!(self, BillKind::Internet | BillKind::Phone | BillKind::Cable | BillKind::Insurance)
    }
}

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, Debug)]
pub enum NegStatus {
    NotStarted,
    Negotiating { ready_at: i64 },
    Saved { new_monthly: f64, fee: f64 },
    NoSavings,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Bill {
    pub id: u32,
    pub provider: String,
    pub kind: BillKind,
    pub monthly: f64,
    pub status: NegStatus,
}

impl Bill {
    /// The simulated outcome of a negotiation, steady for a given provider and price.
    pub fn outcome(&self, fee_pct: f64) -> NegStatus {
        let mut h: u64 = 1469598103934665603;
        for b in self.provider.bytes() {
            h = (h ^ b as u64).wrapping_mul(1099511628211);
        }
        h ^= (self.monthly * 100.0) as u64;
        let roll = h % 100;
        if roll < 15 {
            return NegStatus::NoSavings;
        }
        let cut = 0.12 + (roll as f64 / 100.0) * 0.22; // 12% .. 34%
        let new_monthly = ((self.monthly * (1.0 - cut)) * 100.0).round() / 100.0;
        let yearly = (self.monthly - new_monthly) * 12.0;
        NegStatus::Saved {
            new_monthly,
            fee: (yearly * fee_pct / 100.0 * 100.0).round() / 100.0,
        }
    }
}

// ---------------------------------------------------------------- savings goals

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum Freq {
    Weekly,
    Biweekly,
    Monthly,
}

impl Freq {
    pub const ALL: [Freq; 3] = [Freq::Weekly, Freq::Biweekly, Freq::Monthly];
    pub fn name(self) -> &'static str {
        match self {
            Freq::Weekly => "Every week",
            Freq::Biweekly => "Every 2 weeks",
            Freq::Monthly => "Every month",
        }
    }
    pub fn advance(self, d: NaiveDate) -> NaiveDate {
        match self {
            Freq::Weekly => d + Duration::days(7),
            Freq::Biweekly => d + Duration::days(14),
            Freq::Monthly => add_months(d, 1),
        }
    }
    pub fn per_month(self) -> f64 {
        match self {
            Freq::Weekly => 4.33,
            Freq::Biweekly => 2.17,
            Freq::Monthly => 1.0,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Goal {
    pub id: u32,
    pub name: String,
    pub icon: String,
    pub target: f64,
    pub saved: f64,
    pub auto_on: bool,
    /// Smart Savings: the amount is picked from recent spending instead of fixed.
    pub smart: bool,
    pub auto_amount: f64,
    pub freq: Freq,
    pub next_transfer: NaiveDate,
    pub from_account: u32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct GoalTransfer {
    pub date: NaiveDate,
    pub goal: String,
    pub amount: f64,
    pub note: String,
}

// ---------------------------------------------------------------- credit

#[derive(Clone, Serialize, Deserialize)]
pub struct Credit {
    pub history: Vec<(NaiveDate, u16)>,
    pub on_time_pct: f64,
    pub age_years: f64,
    pub hard_inquiries: u32,
    pub derogatory: u32,
}

impl Credit {
    pub fn score(&self) -> u16 {
        self.history.last().map(|h| h.1).unwrap_or(0)
    }

    /// False until a score has been recorded (a fresh setup has none yet).
    pub fn has_score(&self) -> bool {
        !self.history.is_empty()
    }
}

pub fn score_band(score: u16) -> (&'static str, Color32) {
    match score {
        0..=579 => ("Poor", Color32::from_rgb(242, 95, 92)),
        580..=669 => ("Fair", Color32::from_rgb(255, 159, 67)),
        670..=739 => ("Good", Color32::from_rgb(245, 205, 70)),
        740..=799 => ("Very good", Color32::from_rgb(140, 220, 90)),
        _ => ("Excellent", Color32::from_rgb(61, 214, 140)),
    }
}

// ---------------------------------------------------------------- chat

#[derive(Clone, Serialize, Deserialize)]
pub struct ChatMsg {
    pub from_user: bool,
    pub text: String,
}

// ---------------------------------------------------------------- profile

/// Who uses the app, collected by the first-run setup wizard.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Profile {
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub phone: String,
    pub monthly_income: f64,
    /// True once the setup wizard has been finished.
    pub setup_complete: bool,
}

impl Profile {
    pub fn full_name(&self) -> String {
        format!("{} {}", self.first_name.trim(), self.last_name.trim()).trim().to_string()
    }

    pub fn initials(&self) -> String {
        let a = self.first_name.trim().chars().next();
        let b = self.last_name.trim().chars().next();
        a.into_iter().chain(b).flat_map(|c| c.to_uppercase()).collect()
    }
}

/// A clean slate for someone starting from scratch: one empty checking account and nothing else.
pub fn empty_state(profile: Profile) -> State {
    let mut s = State {
        accounts: vec![Account {
            id: 1,
            name: "Checking".into(),
            kind: AccountKind::Checking,
            balance: 0.0,
            limit: 0.0,
        }],
        transactions: Vec::new(),
        subscriptions: Vec::new(),
        budgets: Vec::new(),
        alerts: AlertSettings::default(),
        bills: Vec::new(),
        fee_pct: 40.0,
        goals: Vec::new(),
        transfers: Vec::new(),
        net_worth_history: Vec::new(),
        credit: Credit {
            history: Vec::new(),
            on_time_pct: 100.0,
            age_years: 0.0,
            hard_inquiries: 0,
            derogatory: 0,
        },
        chat: Vec::new(),
        next_id: 1,
        profile,
        custom_categories: Vec::new(),
        rules: Vec::new(),
        account_links: Vec::new(),
        synced_ids: BTreeSet::new(),
        vault: None,
        credentials: Vec::new(),
        connections: Vec::new(),
    };
    set_custom_categories(&s.custom_categories);
    s.refresh_net_worth();
    s
}

// ---------------------------------------------------------------- the whole state

#[derive(Clone, Serialize, Deserialize)]
pub struct State {
    pub accounts: Vec<Account>,
    pub transactions: Vec<Transaction>,
    pub subscriptions: Vec<Subscription>,
    pub budgets: Vec<Budget>,
    pub alerts: AlertSettings,
    pub bills: Vec<Bill>,
    pub fee_pct: f64,
    pub goals: Vec<Goal>,
    pub transfers: Vec<GoalTransfer>,
    pub net_worth_history: Vec<(NaiveDate, f64)>,
    pub credit: Credit,
    pub chat: Vec<ChatMsg>,
    pub next_id: u64,
    pub profile: Profile,
    pub custom_categories: Vec<String>,
    pub rules: Vec<Rule>,
    /// Which local account each linked or imported account maps to: (outside id, account id).
    pub account_links: Vec<(String, u32)>,
    /// Outside ids of transactions already brought in, so a sync or re-import never doubles them.
    pub synced_ids: BTreeSet<String>,
    /// The encrypted vault, once a master password has been set.
    pub vault: Option<VaultMeta>,
    /// Saved logins (sealed; see `vault`).
    pub credentials: Vec<Credential>,
    /// Bank-sync connections.
    pub connections: Vec<BankConnection>,
}

impl State {
    /// Cleans a raw description and picks a category, letting the person's rules win.
    pub fn categorize(&self, raw: &str, amount: f64) -> (String, Category) {
        let clean = clean_merchant(raw);
        for r in &self.rules {
            if r.matches(raw) || r.matches(&clean) {
                let name = if r.rename.trim().is_empty() {
                    clean
                } else {
                    r.rename.trim().to_string()
                };
                return (name, r.category);
            }
        }
        let cat = Category::guess(&clean, amount);
        (clean, cat)
    }

    /// Runs every rule over the existing transactions. Returns how many changed.
    pub fn apply_rules(&mut self) -> usize {
        let mut n = 0;
        for t in &mut self.transactions {
            if let Some(r) = self.rules.iter().find(|r| r.matches(&t.merchant)) {
                let name = if r.rename.trim().is_empty() {
                    t.merchant.clone()
                } else {
                    r.rename.trim().to_string()
                };
                if t.category != r.category || t.merchant != name {
                    t.category = r.category;
                    t.merchant = name;
                    n += 1;
                }
            }
        }
        n
    }

    /// What Smart Savings would move per transfer: a fifth of last month's money left over,
    /// spread over the transfers in a month, never leaving checking under the cushion.
    pub fn smart_amount(&self, freq: Freq) -> f64 {
        let last = add_months(today(), -1);
        let left_over = self.income_in(last) - self.spent_in(last, None);
        let per = (left_over * 0.2 / freq.per_month()).clamp(0.0, 500.0);
        let room = (self.checking_balance() - self.alerts.low_balance_threshold).max(0.0);
        ((per.min(room)) / 5.0).floor() * 5.0
    }

    pub fn account_name(&self, id: u32) -> String {
        self.accounts
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.name.clone())
            .unwrap_or_else(|| "Unknown".into())
    }

    pub fn checking_id(&self) -> u32 {
        self.accounts
            .iter()
            .find(|a| a.kind == AccountKind::Checking)
            .map(|a| a.id)
            .unwrap_or(0)
    }

    pub fn checking_balance(&self) -> f64 {
        self.accounts
            .iter()
            .filter(|a| a.kind == AccountKind::Checking)
            .map(|a| a.balance)
            .sum()
    }

    pub fn assets(&self) -> f64 {
        self.accounts.iter().filter(|a| !a.kind.is_debt()).map(|a| a.balance).sum::<f64>() + self.goals.iter().map(|g| g.saved).sum::<f64>()
    }

    pub fn debts(&self) -> f64 {
        self.accounts.iter().filter(|a| a.kind.is_debt()).map(|a| a.balance).sum()
    }

    pub fn net_worth(&self) -> f64 {
        self.assets() - self.debts()
    }

    /// Keeps the current month's point of the net worth history equal to today's net worth.
    pub fn refresh_net_worth(&mut self) {
        let nw = self.net_worth();
        let m = month_start(today());
        match self.net_worth_history.last_mut() {
            Some(last) if last.0 == m => last.1 = nw,
            _ => self.net_worth_history.push((m, nw)),
        }
    }

    pub fn card_utilization(&self) -> f64 {
        let (owed, limit) = self
            .accounts
            .iter()
            .filter(|a| a.kind == AccountKind::CreditCard)
            .fold((0.0, 0.0), |(o, l), a| (o + a.balance, l + a.limit));
        if limit > 0.0 {
            owed / limit
        } else {
            0.0
        }
    }

    pub fn in_month(&self, month: NaiveDate) -> impl Iterator<Item = &Transaction> {
        let m = month_start(month);
        self.transactions.iter().filter(move |t| month_start(t.date) == m)
    }

    pub fn spent_in(&self, month: NaiveDate, cat: Option<Category>) -> f64 {
        self.in_month(month)
            .filter(|t| t.amount < 0.0 && t.category.is_spending() && cat.map_or(true, |c| c == t.category))
            .map(|t| -t.amount)
            .sum()
    }

    pub fn income_in(&self, month: NaiveDate) -> f64 {
        self.in_month(month)
            .filter(|t| t.amount > 0.0 && t.category == Category::Income)
            .map(|t| t.amount)
            .sum()
    }

    pub fn spending_by_category(&self, month: NaiveDate) -> Vec<(Category, f64)> {
        let mut map: BTreeMap<Category, f64> = BTreeMap::new();
        for t in self.in_month(month).filter(|t| t.amount < 0.0 && t.category.is_spending()) {
            *map.entry(t.category).or_default() += -t.amount;
        }
        let mut v: Vec<_> = map.into_iter().collect();
        v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        v
    }

    pub fn active_subs(&self) -> impl Iterator<Item = &Subscription> {
        self.subscriptions.iter().filter(|s| !matches!(s.status, SubStatus::Cancelled))
    }

    pub fn subs_monthly_total(&self) -> f64 {
        self.active_subs().map(|s| s.monthly_cost()).sum()
    }

    /// Merges a fresh scan into the list, keeping what the person already decided.
    pub fn rescan(&mut self) -> usize {
        let found = scan_recurring(&self.transactions);
        let mut added = 0;
        for f in found {
            let key = merchant_key(&f.merchant);
            if let Some(existing) = self.subscriptions.iter_mut().find(|s| merchant_key(&s.merchant) == key) {
                if f.last_charge > existing.last_charge {
                    existing.last_charge = f.last_charge;
                    existing.amount = f.amount;
                }
            } else {
                self.subscriptions.push(f);
                added += 1;
            }
        }
        self.subscriptions
            .sort_by(|a, b| b.monthly_cost().partial_cmp(&a.monthly_cost()).unwrap());
        added
    }

    pub fn add_transaction(&mut self, date: NaiveDate, merchant: String, amount: f64, category: Category, account: u32) {
        self.next_id += 1;
        let id = self.next_id;
        self.transactions.push(Transaction {
            id,
            date,
            merchant,
            amount,
            category,
            account,
        });
        if date <= today() {
            if let Some(a) = self.accounts.iter_mut().find(|a| a.id == account) {
                if a.kind.is_debt() {
                    a.balance -= amount;
                } else {
                    a.balance += amount;
                }
            }
        }
    }

    /// Finishes concierge cancellations and bill negotiations whose time has come.
    pub fn tick(&mut self) -> Vec<String> {
        let now = now_secs();
        let mut news = Vec::new();
        for s in &mut self.subscriptions {
            if let SubStatus::Cancelling { ready_at } = s.status {
                if now >= ready_at {
                    s.status = SubStatus::Cancelled;
                    news.push(format!("{} was cancelled by our concierge team.", s.merchant));
                }
            }
        }
        let fee_pct = self.fee_pct;
        for b in &mut self.bills {
            if let NegStatus::Negotiating { ready_at } = b.status {
                if now >= ready_at {
                    b.status = b.outcome(fee_pct);
                    match b.status {
                        NegStatus::Saved { new_monthly, .. } => {
                            news.push(format!("Good news: {} lowered your bill to {}/mo.", b.provider, money(new_monthly)))
                        }
                        _ => news.push(format!("{} wouldn't lower your bill this time. No fee charged.", b.provider)),
                    }
                }
            }
        }
        news
    }

    /// Runs every automatic savings transfer that is due.
    pub fn run_due_transfers(&mut self) -> Vec<String> {
        let t = today();
        let threshold = self.alerts.low_balance_threshold;
        let mut notes = Vec::new();
        for gi in 0..self.goals.len() {
            loop {
                let g = &self.goals[gi];
                if !g.auto_on || g.next_transfer > t || g.saved >= g.target {
                    break;
                }
                let base = if g.smart { self.smart_amount(g.freq) } else { g.auto_amount };
                let amount = base.min(g.target - g.saved);
                if amount <= 0.0 {
                    let g = &mut self.goals[gi];
                    g.next_transfer = g.freq.advance(g.next_transfer);
                    continue;
                }
                let from = g.from_account;
                let date = g.next_transfer;
                let name = g.name.clone();
                let balance = self.accounts.iter().find(|a| a.id == from).map(|a| a.balance).unwrap_or(0.0);
                if balance - amount < threshold {
                    notes.push(format!(
                        "Skipped {} for {}: your balance would drop below {}.",
                        money(amount),
                        name,
                        money(threshold)
                    ));
                    self.transfers.push(GoalTransfer {
                        date,
                        goal: name,
                        amount: 0.0,
                        note: "Skipped - low balance".into(),
                    });
                } else {
                    if let Some(a) = self.accounts.iter_mut().find(|a| a.id == from) {
                        a.balance -= amount;
                    }
                    self.goals[gi].saved += amount;
                    self.transfers.push(GoalTransfer {
                        date,
                        goal: name,
                        amount,
                        note: "Automatic transfer".into(),
                    });
                }
                let g = &mut self.goals[gi];
                g.next_transfer = g.freq.advance(g.next_transfer);
            }
        }
        notes
    }

    pub fn compute_alerts(&self) -> Vec<Alert> {
        let mut out = Vec::new();
        let t = today();
        let s = &self.alerts;
        for a in self.accounts.iter().filter(|a| a.kind == AccountKind::Checking) {
            if s.overdraft && a.balance < 0.0 {
                out.push(Alert {
                    level: AlertLevel::Danger,
                    title: format!("{} is overdrawn", a.name),
                    detail: format!("Balance is {}. Move money in to avoid fees.", money(a.balance)),
                });
            } else if s.low_balance && a.balance < s.low_balance_threshold {
                out.push(Alert {
                    level: AlertLevel::Warn,
                    title: format!("Low balance in {}", a.name),
                    detail: format!("{} left, under your {} alert.", money(a.balance), money(s.low_balance_threshold)),
                });
            }
            if s.overdraft && a.balance >= 0.0 {
                let upcoming: f64 = self
                    .active_subs()
                    .filter(|x| x.account == a.id && (x.next_charge() - t).num_days() <= 14)
                    .map(|x| x.amount)
                    .sum();
                if a.balance - upcoming < 0.0 {
                    out.push(Alert {
                        level: AlertLevel::Danger,
                        title: "Overdraft risk in the next 2 weeks".into(),
                        detail: format!("{} in upcoming charges vs {} available.", money(upcoming), money(a.balance)),
                    });
                }
            }
        }
        if s.budget {
            for b in &self.budgets {
                let spent = self.spent_in(t, Some(b.category));
                if b.limit <= 0.0 {
                    continue;
                }
                let f = spent / b.limit;
                if f >= 1.0 {
                    out.push(Alert {
                        level: AlertLevel::Danger,
                        title: format!("Over budget: {}", b.category.name()),
                        detail: format!("{} spent of {} this month.", money(spent), money0(b.limit)),
                    });
                } else if f >= 0.85 {
                    out.push(Alert {
                        level: AlertLevel::Warn,
                        title: format!("Almost at budget: {}", b.category.name()),
                        detail: format!("{} left of {}.", money(b.limit - spent), money0(b.limit)),
                    });
                }
            }
        }
        if s.renewals {
            for sub in self.active_subs().filter(|x| x.remind && matches!(x.status, SubStatus::Active)) {
                let days = (sub.next_charge() - t).num_days();
                if days <= s.renewal_days {
                    out.push(Alert {
                        level: AlertLevel::Info,
                        title: format!(
                            "{} renews {}",
                            sub.merchant,
                            if days == 0 {
                                "today".to_string()
                            } else {
                                format!("in {} day{}", days, if days == 1 { "" } else { "s" })
                            }
                        ),
                        detail: format!("{} will be charged to {}.", money(sub.amount), self.account_name(sub.account)),
                    });
                }
            }
        }
        for b in &self.bills {
            if let NegStatus::Saved { new_monthly, .. } = b.status {
                out.push(Alert {
                    level: AlertLevel::Good,
                    title: format!("You're saving on {}", b.provider),
                    detail: format!("{} a year saved.", money((b.monthly - new_monthly) * 12.0)),
                });
            }
        }
        out
    }
}

// ---------------------------------------------------------------- CSV import

fn split_csv(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => out.push(std::mem::take(&mut cur).trim().to_string()),
            _ => cur.push(c),
        }
    }
    out.push(cur.trim().to_string());
    out
}

fn parse_date(s: &str) -> Option<NaiveDate> {
    for f in ["%Y-%m-%d", "%m/%d/%Y", "%m/%d/%y", "%d.%m.%Y", "%Y/%m/%d"] {
        if let Ok(d) = NaiveDate::parse_from_str(s.trim(), f) {
            return Some(d);
        }
    }
    None
}

fn parse_amount(s: &str) -> Option<f64> {
    let t = s.trim();
    let neg = t.starts_with('(') && t.ends_with(')');
    let clean: String = t.chars().filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
    let v: f64 = clean.parse().ok()?;
    Some(if neg { -v.abs() } else { v })
}

/// Reads a bank statement: date, description, amount (and an optional category).
/// Returns the rows it understood and how many lines it skipped.
pub fn parse_csv(text: &str) -> (Vec<(NaiveDate, String, f64, Option<Category>)>, usize) {
    let mut rows = Vec::new();
    let mut skipped = 0;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let f = split_csv(line);
        if f.len() < 3 {
            skipped += 1;
            continue;
        }
        let (Some(date), Some(amount)) = (parse_date(&f[0]), parse_amount(&f[2])) else {
            skipped += 1;
            continue;
        };
        let cat = f.get(3).and_then(|c| {
            let c = c.to_lowercase();
            Category::all()
                .into_iter()
                .find(|k| k.name().to_lowercase() == c || k.name().to_lowercase().starts_with(&c) && !c.is_empty())
        });
        rows.push((date, f[1].clone(), amount, cat));
    }
    (rows, skipped)
}

// ---------------------------------------------------------------- linked accounts

/// A bank-sync connection. Its private access address is sealed in the vault
/// (`credential_id`); only the label and sync status are kept in the clear.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct BankConnection {
    pub id: u32,
    pub label: String,
    pub credential_id: u32,
    pub last_sync: String,
    pub status: String,
}

/// An account read from outside: a bank-sync download or a statement file.
#[derive(Clone, Debug, PartialEq)]
pub struct RemoteAccount {
    /// Stable id from the source, e.g. "simplefin:ACT-123" or "ofx:1234567".
    pub ext_id: String,
    pub name: String,
    pub institution: String,
    pub kind: AccountKind,
    /// As the source reports it: negative for money owed on cards and loans.
    pub balance: Option<f64>,
    pub transactions: Vec<RemoteTx>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RemoteTx {
    pub ext_id: String,
    pub date: NaiveDate,
    pub amount: f64,
    pub description: String,
}

/// What a sync or import changed, for the message shown afterwards.
#[derive(Default, Debug, PartialEq)]
pub struct MergeReport {
    pub new_accounts: usize,
    pub new_transactions: usize,
    pub skipped_duplicates: usize,
}

/// Guesses the kind of account from its name or type text.
pub fn guess_account_kind(text: &str) -> AccountKind {
    let t = text.to_lowercase();
    if t.contains("credit") || t.contains("card") || t.contains("visa") || t.contains("mastercard") || t.contains("amex") {
        AccountKind::CreditCard
    } else if t.contains("saving") || t.contains("money market") || t.contains("moneymrkt") {
        AccountKind::Savings
    } else if t.contains("loan") || t.contains("mortgage") || t.contains("line of credit") {
        AccountKind::Loan
    } else if t.contains("brokerage") || t.contains("invest") || t.contains("401") || t.contains("ira") {
        AccountKind::Investment
    } else {
        AccountKind::Checking
    }
}

impl State {
    /// Brings outside accounts and transactions in. Accounts are matched by their outside id,
    /// transactions are never added twice, and balances follow the source.
    pub fn merge_remote(&mut self, remote: Vec<RemoteAccount>) -> MergeReport {
        let mut report = MergeReport::default();
        for ra in remote {
            let account_id = match self.account_links.iter().find(|(ext, _)| *ext == ra.ext_id).map(|l| l.1) {
                Some(id) if self.accounts.iter().any(|a| a.id == id) => id,
                _ => {
                    let id = self.accounts.iter().map(|a| a.id).max().unwrap_or(0) + 1;
                    let name = if ra.institution.trim().is_empty() || ra.name.contains(ra.institution.trim()) {
                        ra.name.clone()
                    } else {
                        format!("{} {}", ra.institution.trim(), ra.name)
                    };
                    self.accounts.push(Account {
                        id,
                        name,
                        kind: ra.kind,
                        balance: 0.0,
                        limit: 0.0,
                    });
                    self.account_links.retain(|(ext, _)| *ext != ra.ext_id);
                    self.account_links.push((ra.ext_id.clone(), id));
                    report.new_accounts += 1;
                    id
                }
            };
            if let Some(balance) = ra.balance {
                if let Some(a) = self.accounts.iter_mut().find(|a| a.id == account_id) {
                    a.balance = if a.kind.is_debt() { -balance } else { balance };
                }
            }
            for tx in ra.transactions {
                let key = format!("{}:{}", ra.ext_id, tx.ext_id);
                if !self.synced_ids.insert(key) {
                    report.skipped_duplicates += 1;
                    continue;
                }
                let (merchant, category) = self.categorize(&tx.description, tx.amount);
                self.next_id += 1;
                self.transactions.push(Transaction {
                    id: self.next_id,
                    date: tx.date,
                    merchant,
                    amount: tx.amount,
                    category,
                    account: account_id,
                });
                report.new_transactions += 1;
            }
        }
        self.transactions.sort_by(|a, b| b.date.cmp(&a.date).then(b.id.cmp(&a.id)));
        self.rescan();
        self.refresh_net_worth();
        report
    }
}

// ---------------------------------------------------------------- OFX / QFX statements

/// Upper-cases ASCII only, so byte positions line up with the original text.
fn ofx_upper(s: &str) -> String {
    s.to_ascii_uppercase()
}

/// The text right after `<TAG>` (up to the next tag or line end), in either SGML or XML style.
fn ofx_value(text: &str, upper: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let start = upper.find(&open)? + open.len();
    let rest = &text[start..];
    let end = rest.find(['<', '\n', '\r']).unwrap_or(rest.len());
    let v = rest[..end].trim();
    if v.is_empty() {
        None
    } else {
        Some(v.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">"))
    }
}

/// Splits `text` into the pieces that start with `<TAG>` (each runs to the next one).
fn ofx_blocks<'a>(text: &'a str, upper: &str, tag: &str) -> Vec<&'a str> {
    let open = format!("<{tag}>");
    let starts: Vec<usize> = upper.match_indices(&open).map(|(i, _)| i).collect();
    starts
        .iter()
        .enumerate()
        .map(|(n, &i)| {
            let end = starts.get(n + 1).copied().unwrap_or(text.len());
            &text[i..end]
        })
        .collect()
}

fn ofx_date(s: &str) -> Option<NaiveDate> {
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).take(8).collect();
    NaiveDate::parse_from_str(&digits, "%Y%m%d").ok()
}

fn ofx_amount(s: &str) -> Option<f64> {
    s.trim().replace(',', ".").parse::<f64>().ok()
}

/// Reads an OFX or QFX statement file, the format most banks and card issuers offer for
/// download. Handles bank (checking, savings) and credit card statements.
pub fn parse_ofx(text: &str) -> Result<Vec<RemoteAccount>, String> {
    let upper = ofx_upper(text);
    if !upper.contains("<OFX>") {
        return Err("This doesn't look like an OFX or QFX file.".into());
    }
    let institution = ofx_value(text, &upper, "ORG").unwrap_or_default();
    let mut out = Vec::new();
    for (tag, is_card) in [("STMTRS", false), ("CCSTMTRS", true)] {
        for block in ofx_blocks(text, &upper, tag) {
            let bu = ofx_upper(block);
            // The next statement of the other kind may follow; stop at its start.
            let other = if is_card { "<STMTRS>" } else { "<CCSTMTRS>" };
            let cut = bu[1..].find(other).map(|i| i + 1).unwrap_or(block.len());
            let (block, bu) = (&block[..cut], &bu[..cut]);
            let acct_id = ofx_value(block, bu, "ACCTID").unwrap_or_else(|| "unknown".into());
            let acct_type = ofx_value(block, bu, "ACCTTYPE").unwrap_or_default();
            let kind = if is_card {
                AccountKind::CreditCard
            } else {
                guess_account_kind(&acct_type)
            };
            let balance = bu
                .find("<LEDGERBAL>")
                .and_then(|i| ofx_value(&block[i..], &bu[i..], "BALAMT"))
                .and_then(|v| ofx_amount(&v));
            let last4: String = acct_id.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
            let name = format!(
                "{} ••{}",
                if is_card {
                    "Credit card".to_string()
                } else {
                    kind.name().to_string()
                },
                last4
            );
            let mut transactions = Vec::new();
            for t in ofx_blocks(block, bu, "STMTTRN") {
                let tu = ofx_upper(t);
                let (Some(date), Some(amount)) = (
                    ofx_value(t, &tu, "DTPOSTED").and_then(|d| ofx_date(&d)),
                    ofx_value(t, &tu, "TRNAMT").and_then(|a| ofx_amount(&a)),
                ) else {
                    continue;
                };
                let name = ofx_value(t, &tu, "NAME").unwrap_or_default();
                let memo = ofx_value(t, &tu, "MEMO").unwrap_or_default();
                let description = if name.is_empty() { memo.clone() } else { name };
                let ext_id = ofx_value(t, &tu, "FITID").unwrap_or_else(|| format!("{}|{}|{}", date, amount, description));
                transactions.push(RemoteTx {
                    ext_id,
                    date,
                    amount,
                    description,
                });
            }
            out.push(RemoteAccount {
                ext_id: format!("ofx:{}:{}", institution.to_lowercase(), acct_id),
                name,
                institution: institution.clone(),
                kind,
                balance,
                transactions,
            });
        }
    }
    if out.is_empty() {
        return Err("No bank or card statements were found in this file.".into());
    }
    Ok(out)
}

// ---------------------------------------------------------------- sample data

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f64) / (1u64 << 31) as f64
    }
    fn range(&mut self, a: f64, b: f64) -> f64 {
        ((a + (b - a) * self.next()) * 100.0).round() / 100.0
    }
    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[((self.next() * items.len() as f64) as usize).min(items.len() - 1)]
    }
}

pub fn sample() -> State {
    let t = today();
    let mut rng = Rng(42);
    let accounts = vec![
        Account {
            id: 1,
            name: "Everyday Checking".into(),
            kind: AccountKind::Checking,
            balance: 4820.55,
            limit: 0.0,
        },
        Account {
            id: 2,
            name: "High-Yield Savings".into(),
            kind: AccountKind::Savings,
            balance: 12400.00,
            limit: 0.0,
        },
        Account {
            id: 3,
            name: "Rewards Card".into(),
            kind: AccountKind::CreditCard,
            balance: 1284.30,
            limit: 6000.0,
        },
        Account {
            id: 4,
            name: "Travel Card".into(),
            kind: AccountKind::CreditCard,
            balance: 2150.00,
            limit: 10000.0,
        },
        Account {
            id: 5,
            name: "Brokerage".into(),
            kind: AccountKind::Investment,
            balance: 23780.12,
            limit: 0.0,
        },
        Account {
            id: 6,
            name: "Retirement 401(k)".into(),
            kind: AccountKind::Investment,
            balance: 41200.00,
            limit: 0.0,
        },
        Account {
            id: 7,
            name: "Car Loan".into(),
            kind: AccountKind::Loan,
            balance: 9860.40,
            limit: 0.0,
        },
        Account {
            id: 8,
            name: "Student Loan".into(),
            kind: AccountKind::Loan,
            balance: 18400.00,
            limit: 0.0,
        },
    ];

    let mut txs: Vec<Transaction> = Vec::new();
    let mut id = 0u64;
    let mut push = |txs: &mut Vec<Transaction>, date: NaiveDate, m: &str, amount: f64, cat: Category, acct: u32| {
        if date <= t {
            id += 1;
            txs.push(Transaction {
                id,
                date,
                merchant: m.to_string(),
                amount,
                category: cat,
                account: acct,
            });
        }
    };

    for back in (0..6).rev() {
        let month = add_months(month_start(t), -back);
        let d = |day: u32| day_in_month(month, day);
        push(&mut txs, d(1), "Acme Corp Payroll", 2850.00, Category::Income, 1);
        push(&mut txs, d(15), "Acme Corp Payroll", 2850.00, Category::Income, 1);
        push(&mut txs, d(1), "Maple Street Apartments", -1650.00, Category::Housing, 1);
        push(&mut txs, d(3), "FitClub Gym", -39.99, Category::Subscriptions, 3);
        push(&mut txs, d(5), "StreamFlix", -15.49, Category::Subscriptions, 3);
        push(&mut txs, d(9), "GameVault Plus", -14.99, Category::Subscriptions, 3);
        push(&mut txs, d(12), "Tunewave Music", -10.99, Category::Subscriptions, 3);
        push(&mut txs, d(20), "CloudBox Storage", -2.99, Category::Subscriptions, 3);
        if back < 4 {
            push(&mut txs, d(25), "Daily Ledger News", -12.00, Category::Subscriptions, 3);
        }
        push(&mut txs, d(8), "Lightspeed Internet", -79.99, Category::Utilities, 1);
        push(&mut txs, d(18), "Northwind Mobile", -85.00, Category::Utilities, 1);
        push(&mut txs, d(22), "Metro Cable TV", -64.99, Category::Utilities, 1);
        push(
            &mut txs,
            d(14),
            "City Power & Light",
            -rng.range(88.0, 136.0),
            Category::Utilities,
            1,
        );
        push(&mut txs, d(10), "Car Loan Payment", -385.00, Category::Transfer, 1);
        push(&mut txs, d(16), "Rewards Card Payment", -900.00, Category::Transfer, 1);
        push(&mut txs, d(27), "Transfer to Savings", -400.00, Category::Transfer, 1);

        for _ in 0..8 {
            let day = 1 + (rng.next() * 27.0) as u32;
            let m = rng.pick(&["FreshMart", "Green Grocer", "Corner Market"]);
            push(&mut txs, d(day), m, -rng.range(24.0, 138.0), Category::Groceries, 3);
        }
        for _ in 0..9 {
            let day = 1 + (rng.next() * 27.0) as u32;
            let m = rng.pick(&["Bean There Cafe", "Noodle House", "Taco Stand", "Slice Pizza Co", "Harbor Grill"]);
            push(&mut txs, d(day), m, -rng.range(6.5, 58.0), Category::Dining, 3);
        }
        for _ in 0..4 {
            let day = 1 + (rng.next() * 27.0) as u32;
            push(
                &mut txs,
                d(day),
                "QuickFuel Station",
                -rng.range(34.0, 62.0),
                Category::Transport,
                3,
            );
        }
        for _ in 0..3 {
            let day = 1 + (rng.next() * 27.0) as u32;
            push(
                &mut txs,
                d(day),
                "CityRide Rideshare",
                -rng.range(11.0, 32.0),
                Category::Transport,
                4,
            );
        }
        for _ in 0..3 {
            let day = 1 + (rng.next() * 27.0) as u32;
            let m = rng.pick(&["Online Mart", "Home Goods Store", "Outlet Shop"]);
            push(&mut txs, d(day), m, -rng.range(18.0, 160.0), Category::Shopping, 4);
        }
        let day = 1 + (rng.next() * 27.0) as u32;
        push(
            &mut txs,
            d(day),
            "Cinema 8 Tickets",
            -rng.range(24.0, 48.0),
            Category::Entertainment,
            4,
        );
        let day = 1 + (rng.next() * 27.0) as u32;
        push(&mut txs, d(day), "Pharmacy Plus", -rng.range(9.0, 45.0), Category::Health, 3);
        if back == 2 {
            push(&mut txs, d(11), "Skyway Airlines", -412.60, Category::Travel, 4);
            push(&mut txs, d(19), "Seaside Inn", -538.00, Category::Travel, 4);
        }
    }
    txs.sort_by(|a, b| b.date.cmp(&a.date).then(b.id.cmp(&a.id)));
    let next_id = txs.len() as u64 + 1;

    let budgets = vec![
        Budget {
            category: Category::Groceries,
            limit: 600.0,
        },
        Budget {
            category: Category::Dining,
            limit: 280.0,
        },
        Budget {
            category: Category::Transport,
            limit: 300.0,
        },
        Budget {
            category: Category::Subscriptions,
            limit: 100.0,
        },
        Budget {
            category: Category::Shopping,
            limit: 250.0,
        },
    ];

    let bills = vec![
        Bill {
            id: 1,
            provider: "Lightspeed Internet".into(),
            kind: BillKind::Internet,
            monthly: 79.99,
            status: NegStatus::NotStarted,
        },
        Bill {
            id: 2,
            provider: "Northwind Mobile".into(),
            kind: BillKind::Phone,
            monthly: 85.00,
            status: NegStatus::NotStarted,
        },
        Bill {
            id: 3,
            provider: "Metro Cable TV".into(),
            kind: BillKind::Cable,
            monthly: 64.99,
            status: NegStatus::NotStarted,
        },
        Bill {
            id: 4,
            provider: "City Power & Light".into(),
            kind: BillKind::Electric,
            monthly: 112.40,
            status: NegStatus::NotStarted,
        },
    ];

    let goals = vec![
        Goal {
            id: 1,
            name: "Emergency fund".into(),
            icon: "☂".into(),
            target: 10000.0,
            saved: 6250.0,
            auto_on: true,
            smart: true,
            auto_amount: 150.0,
            freq: Freq::Weekly,
            next_transfer: t + Duration::days(2),
            from_account: 1,
        },
        Goal {
            id: 2,
            name: "Summer trip".into(),
            icon: "✈".into(),
            target: 3000.0,
            saved: 1120.0,
            auto_on: true,
            smart: false,
            auto_amount: 200.0,
            freq: Freq::Monthly,
            next_transfer: add_months(month_start(t), 1),
            from_account: 1,
        },
        Goal {
            id: 3,
            name: "New laptop".into(),
            icon: "💻".into(),
            target: 1800.0,
            saved: 420.0,
            auto_on: false,
            smart: false,
            auto_amount: 50.0,
            freq: Freq::Biweekly,
            next_transfer: t + Duration::days(9),
            from_account: 1,
        },
    ];

    let mut state = State {
        accounts,
        transactions: txs,
        subscriptions: Vec::new(),
        budgets,
        alerts: AlertSettings::default(),
        bills,
        fee_pct: 40.0,
        goals,
        transfers: Vec::new(),
        net_worth_history: Vec::new(),
        credit: Credit {
            history: Vec::new(),
            on_time_pct: 100.0,
            age_years: 6.4,
            hard_inquiries: 1,
            derogatory: 0,
        },
        chat: Vec::new(),
        next_id,
        profile: Profile {
            first_name: "Alex".into(),
            setup_complete: true,
            ..Profile::default()
        },
        custom_categories: Vec::new(),
        rules: Vec::new(),
        account_links: Vec::new(),
        synced_ids: BTreeSet::new(),
        vault: None,
        credentials: Vec::new(),
        connections: Vec::new(),
    };
    set_custom_categories(&state.custom_categories);
    state.rescan();

    let nw = state.net_worth();
    let start = month_start(t);
    for i in (0..12).rev() {
        let v = nw - i as f64 * 1180.0 + (rng.next() - 0.5) * 1400.0 * (i.min(1) as f64);
        state.net_worth_history.push((add_months(start, -i), (v * 100.0).round() / 100.0));
    }
    state.refresh_net_worth();

    let scores = [702u16, 705, 704, 711, 716, 714, 721, 725, 729, 731, 734, 738];
    for (i, s) in scores.iter().enumerate() {
        state.credit.history.push((add_months(start, i as i32 - 11), *s));
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    const OFX: &str = "OFXHEADER:100\nDATA:OFXSGML\n\n<OFX>\n<SIGNONMSGSRSV1><SONRS><FI><ORG>Example Bank<FID>123</FI></SONRS></SIGNONMSGSRSV1>\n\
<BANKMSGSRSV1><STMTTRNRS><STMTRS><CURDEF>USD\n<BANKACCTFROM><BANKID>111<ACCTID>99887766<ACCTTYPE>CHECKING</BANKACCTFROM>\n\
<BANKTRANLIST><DTSTART>20261101<DTEND>20261124\n\
<STMTTRN><TRNTYPE>DEBIT<DTPOSTED>20261120120000[-5:EST]<TRNAMT>-42.50<FITID>A1<NAME>SQ *CORNER MARKET #12</STMTTRN>\n\
<STMTTRN><TRNTYPE>CREDIT<DTPOSTED>20261115<TRNAMT>2850.00<FITID>A2<NAME>ACME CORP PAYROLL</STMTTRN>\n\
</BANKTRANLIST><LEDGERBAL><BALAMT>1520.10<DTASOF>20261124</LEDGERBAL><AVAILBAL><BALAMT>1400.00</AVAILBAL></STMTRS></STMTTRNRS></BANKMSGSRSV1>\n\
<CREDITCARDMSGSRSV1><CCSTMTTRNRS><CCSTMTRS><CCACCTFROM><ACCTID>4111222233334444</CCACCTFROM>\n\
<BANKTRANLIST><STMTTRN><TRNTYPE>DEBIT<DTPOSTED>20261118<TRNAMT>-15.49<FITID>C1<NAME>STREAMFLIX</STMTTRN></BANKTRANLIST>\n\
<LEDGERBAL><BALAMT>-310.00</LEDGERBAL></CCSTMTRS></CCSTMTTRNRS></CREDITCARDMSGSRSV1></OFX>";

    #[test]
    fn reads_ofx_bank_and_card_statements() {
        let accounts = parse_ofx(OFX).unwrap();
        assert_eq!(accounts.len(), 2);
        let bank = &accounts[0];
        assert_eq!(bank.kind, AccountKind::Checking);
        assert_eq!(bank.institution, "Example Bank");
        assert_eq!(bank.balance, Some(1520.10), "ledger balance, not available balance");
        assert_eq!(bank.transactions.len(), 2);
        assert_eq!(bank.transactions[0].date, NaiveDate::from_ymd_opt(2026, 11, 20).unwrap());
        assert_eq!(bank.transactions[0].amount, -42.50);
        let card = &accounts[1];
        assert_eq!(card.kind, AccountKind::CreditCard);
        assert_eq!(card.name, "Credit card ••4444");
        assert_eq!(card.transactions.len(), 1);
        assert!(parse_ofx("date,description,amount").is_err());
    }

    #[test]
    fn merging_twice_adds_nothing_new() {
        let mut s = empty_state(Profile::default());
        let first = s.merge_remote(parse_ofx(OFX).unwrap());
        assert_eq!(first.new_accounts, 2);
        assert_eq!(first.new_transactions, 3);
        let card = s.accounts.iter().find(|a| a.kind == AccountKind::CreditCard).unwrap();
        assert_eq!(card.balance, 310.0, "money owed on a card is kept as a positive debt");
        assert!(s.transactions.iter().any(|t| t.merchant == "Corner Market"), "names are cleaned up");

        let again = s.merge_remote(parse_ofx(OFX).unwrap());
        assert_eq!(
            again,
            MergeReport {
                new_accounts: 0,
                new_transactions: 0,
                skipped_duplicates: 3
            }
        );
        assert_eq!(s.transactions.len(), 3);
    }
}
