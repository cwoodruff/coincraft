//! The local SQLite database. Everything CoinCraft knows lives in one file on this computer.
//!
//! Each kind of record has its own table. Saving rewrites the tables inside one transaction,
//! so the file is never left half-written. Choices like a category or a status are stored as
//! short JSON text so new kinds can be added without changing the schema.

use crate::data::*;
use crate::vault::{Credential, ServiceKind, VaultMeta};
use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Bump this and add a step to `migrate` whenever the schema changes.
const SCHEMA_VERSION: i32 = 2;

const SCHEMA_V1: &str = "
CREATE TABLE IF NOT EXISTS profile (
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    first_name      TEXT    NOT NULL,
    last_name       TEXT    NOT NULL DEFAULT '',
    email           TEXT    NOT NULL DEFAULT '',
    phone           TEXT    NOT NULL DEFAULT '',
    monthly_income  REAL    NOT NULL DEFAULT 0,
    setup_complete  INTEGER NOT NULL DEFAULT 0,
    updated_at      TEXT    NOT NULL
);
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS accounts (
    id            INTEGER PRIMARY KEY,
    name          TEXT NOT NULL,
    kind          TEXT NOT NULL,
    balance       REAL NOT NULL,
    credit_limit  REAL NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS transactions (
    id          INTEGER PRIMARY KEY,
    date        TEXT    NOT NULL,
    merchant    TEXT    NOT NULL,
    amount      REAL    NOT NULL,
    category    TEXT    NOT NULL,
    account_id  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_transactions_date ON transactions(date);
CREATE TABLE IF NOT EXISTS subscriptions (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    merchant      TEXT    NOT NULL,
    amount        REAL    NOT NULL,
    cadence_days  INTEGER NOT NULL,
    last_charge   TEXT    NOT NULL,
    category      TEXT    NOT NULL,
    account_id    INTEGER NOT NULL,
    status        TEXT    NOT NULL,
    remind        INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS budgets (
    category       TEXT PRIMARY KEY,
    monthly_limit  REAL NOT NULL
);
CREATE TABLE IF NOT EXISTS bills (
    id        INTEGER PRIMARY KEY,
    provider  TEXT NOT NULL,
    kind      TEXT NOT NULL,
    monthly   REAL NOT NULL,
    status    TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS goals (
    id             INTEGER PRIMARY KEY,
    name           TEXT    NOT NULL,
    icon           TEXT    NOT NULL,
    target         REAL    NOT NULL,
    saved          REAL    NOT NULL,
    auto_on        INTEGER NOT NULL,
    smart          INTEGER NOT NULL,
    auto_amount    REAL    NOT NULL,
    freq           TEXT    NOT NULL,
    next_transfer  TEXT    NOT NULL,
    from_account   INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS goal_transfers (
    id      INTEGER PRIMARY KEY AUTOINCREMENT,
    date    TEXT NOT NULL,
    goal    TEXT NOT NULL,
    amount  REAL NOT NULL,
    note    TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS net_worth_history (
    month  TEXT PRIMARY KEY,
    value  REAL NOT NULL
);
CREATE TABLE IF NOT EXISTS credit_history (
    month  TEXT PRIMARY KEY,
    score  INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS credit_profile (
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    on_time_pct     REAL    NOT NULL,
    age_years       REAL    NOT NULL,
    hard_inquiries  INTEGER NOT NULL,
    derogatory      INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS rules (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    contains  TEXT NOT NULL,
    category  TEXT NOT NULL,
    rename    TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS custom_categories (
    position  INTEGER PRIMARY KEY,
    name      TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS chat_messages (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    from_user  INTEGER NOT NULL,
    text       TEXT    NOT NULL
);
";

/// Version 2: linked accounts, the encrypted vault and bank-sync connections.
/// Secrets are stored only as sealed bytes (`nonce` + `cipher`); see `vault.rs`.
const SCHEMA_V2: &str = "
CREATE TABLE IF NOT EXISTS account_links (
    ext_id      TEXT PRIMARY KEY,
    account_id  INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS synced_transactions (
    ext_id  TEXT PRIMARY KEY
);
CREATE TABLE IF NOT EXISTS vault_meta (
    id            INTEGER PRIMARY KEY CHECK (id = 1),
    salt          BLOB NOT NULL,
    check_nonce   BLOB NOT NULL,
    check_cipher  BLOB NOT NULL
);
CREATE TABLE IF NOT EXISTS credentials (
    id           INTEGER PRIMARY KEY,
    institution  TEXT    NOT NULL,
    kind         TEXT    NOT NULL,
    account_id   INTEGER,
    nonce        BLOB    NOT NULL,
    cipher       BLOB    NOT NULL,
    updated      TEXT    NOT NULL
);
CREATE TABLE IF NOT EXISTS bank_connections (
    id             INTEGER PRIMARY KEY,
    label          TEXT    NOT NULL,
    credential_id  INTEGER NOT NULL,
    last_sync      TEXT    NOT NULL,
    status         TEXT    NOT NULL
);
";

/// Tables rewritten on every save (the profile is kept separately).
const DATA_TABLES: [&str; 19] = [
    "settings",
    "accounts",
    "transactions",
    "subscriptions",
    "budgets",
    "bills",
    "goals",
    "goal_transfers",
    "net_worth_history",
    "credit_history",
    "credit_profile",
    "rules",
    "custom_categories",
    "chat_messages",
    "account_links",
    "synced_transactions",
    "vault_meta",
    "credentials",
    "bank_connections",
];

pub type DbResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn enc<T: Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

fn dec<T: DeserializeOwned>(s: &str) -> rusqlite::Result<T> {
    serde_json::from_str(s).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
}

fn date_str(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

fn date(s: &str) -> rusqlite::Result<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))
}

/// Where the database file lives: the usual per-user app data folder for each system.
pub fn default_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_default();
    let base = if cfg!(target_os = "macos") {
        home.join("Library").join("Application Support")
    } else if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or(home)
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local").join("share"))
    };
    base.join("CoinCraft").join("coincraft.db")
}

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open(path: &Path) -> DbResult<Db> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(err)?;
        }
        let conn = Connection::open(path).map_err(err)?;
        conn.execute_batch("PRAGMA foreign_keys = ON;").map_err(err)?;
        let db = Db { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> DbResult<()> {
        let version: i32 = self.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(err)?;
        if version < 1 {
            self.conn.execute_batch(SCHEMA_V1).map_err(err)?;
        }
        if version < 2 {
            self.conn.execute_batch(SCHEMA_V2).map_err(err)?;
        }
        if version < SCHEMA_VERSION {
            self.conn
                .execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))
                .map_err(err)?;
        }
        Ok(())
    }

    pub fn load_profile(&self) -> DbResult<Option<Profile>> {
        self.conn
            .query_row(
                "SELECT first_name, last_name, email, phone, monthly_income, setup_complete FROM profile WHERE id = 1",
                [],
                |r| {
                    Ok(Profile {
                        first_name: r.get(0)?,
                        last_name: r.get(1)?,
                        email: r.get(2)?,
                        phone: r.get(3)?,
                        monthly_income: r.get(4)?,
                        setup_complete: r.get::<_, i64>(5)? != 0,
                    })
                },
            )
            .optional()
            .map_err(err)
    }

    fn write_profile(conn: &Connection, p: &Profile) -> rusqlite::Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO profile
                (id, first_name, last_name, email, phone, monthly_income, setup_complete, updated_at)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))",
            params![
                p.first_name,
                p.last_name,
                p.email,
                p.phone,
                p.monthly_income,
                p.setup_complete as i64
            ],
        )?;
        Ok(())
    }

    /// Everything saved so far, or None on the very first run. Check `profile.setup_complete`
    /// to know whether the setup wizard was finished.
    pub fn load_state(&self) -> DbResult<Option<State>> {
        let Some(profile) = self.load_profile()? else { return Ok(None) };
        self.read_state(profile).map(Some).map_err(err)
    }

    fn read_state(&self, profile: Profile) -> rusqlite::Result<State> {
        let c = &self.conn;
        let setting = |key: &str| -> rusqlite::Result<Option<String>> {
            c.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
                .optional()
        };

        let custom_categories: Vec<String> = c
            .prepare("SELECT name FROM custom_categories ORDER BY position")?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        // Category names must be known before any category is read back.
        set_custom_categories(&custom_categories);

        let accounts = c
            .prepare("SELECT id, name, kind, balance, credit_limit FROM accounts ORDER BY id")?
            .query_map([], |r| {
                Ok(Account {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    kind: dec(&r.get::<_, String>(2)?)?,
                    balance: r.get(3)?,
                    limit: r.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;

        let transactions = c
            .prepare("SELECT id, date, merchant, amount, category, account_id FROM transactions ORDER BY date DESC, id DESC")?
            .query_map([], |r| {
                Ok(Transaction {
                    id: r.get::<_, i64>(0)? as u64,
                    date: date(&r.get::<_, String>(1)?)?,
                    merchant: r.get(2)?,
                    amount: r.get(3)?,
                    category: dec(&r.get::<_, String>(4)?)?,
                    account: r.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;

        let subscriptions = c
            .prepare(
                "SELECT merchant, amount, cadence_days, last_charge, category, account_id, status, remind
                 FROM subscriptions ORDER BY id",
            )?
            .query_map([], |r| {
                Ok(Subscription {
                    merchant: r.get(0)?,
                    amount: r.get(1)?,
                    cadence_days: r.get(2)?,
                    last_charge: date(&r.get::<_, String>(3)?)?,
                    category: dec(&r.get::<_, String>(4)?)?,
                    account: r.get(5)?,
                    status: dec(&r.get::<_, String>(6)?)?,
                    remind: r.get::<_, i64>(7)? != 0,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;

        let budgets = c
            .prepare("SELECT category, monthly_limit FROM budgets ORDER BY rowid")?
            .query_map([], |r| {
                Ok(Budget {
                    category: dec(&r.get::<_, String>(0)?)?,
                    limit: r.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;

        let bills = c
            .prepare("SELECT id, provider, kind, monthly, status FROM bills ORDER BY id")?
            .query_map([], |r| {
                Ok(Bill {
                    id: r.get(0)?,
                    provider: r.get(1)?,
                    kind: dec(&r.get::<_, String>(2)?)?,
                    monthly: r.get(3)?,
                    status: dec(&r.get::<_, String>(4)?)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;

        let goals = c
            .prepare(
                "SELECT id, name, icon, target, saved, auto_on, smart, auto_amount, freq, next_transfer, from_account
                 FROM goals ORDER BY id",
            )?
            .query_map([], |r| {
                Ok(Goal {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    icon: r.get(2)?,
                    target: r.get(3)?,
                    saved: r.get(4)?,
                    auto_on: r.get::<_, i64>(5)? != 0,
                    smart: r.get::<_, i64>(6)? != 0,
                    auto_amount: r.get(7)?,
                    freq: dec(&r.get::<_, String>(8)?)?,
                    next_transfer: date(&r.get::<_, String>(9)?)?,
                    from_account: r.get(10)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;

        let transfers = c
            .prepare("SELECT date, goal, amount, note FROM goal_transfers ORDER BY id")?
            .query_map([], |r| {
                Ok(GoalTransfer {
                    date: date(&r.get::<_, String>(0)?)?,
                    goal: r.get(1)?,
                    amount: r.get(2)?,
                    note: r.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;

        let net_worth_history = c
            .prepare("SELECT month, value FROM net_worth_history ORDER BY month")?
            .query_map([], |r| Ok((date(&r.get::<_, String>(0)?)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;

        let history = c
            .prepare("SELECT month, score FROM credit_history ORDER BY month")?
            .query_map([], |r| Ok((date(&r.get::<_, String>(0)?)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let credit = c
            .query_row(
                "SELECT on_time_pct, age_years, hard_inquiries, derogatory FROM credit_profile WHERE id = 1",
                [],
                |r| {
                    Ok(Credit {
                        history: Vec::new(),
                        on_time_pct: r.get(0)?,
                        age_years: r.get(1)?,
                        hard_inquiries: r.get(2)?,
                        derogatory: r.get(3)?,
                    })
                },
            )
            .optional()?
            .map(|cr| Credit { history, ..cr })
            .unwrap_or(Credit {
                history: Vec::new(),
                on_time_pct: 100.0,
                age_years: 0.0,
                hard_inquiries: 0,
                derogatory: 0,
            });

        let rules = c
            .prepare("SELECT contains, category, rename FROM rules ORDER BY id")?
            .query_map([], |r| {
                Ok(Rule {
                    contains: r.get(0)?,
                    category: dec(&r.get::<_, String>(1)?)?,
                    rename: r.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;

        let chat = c
            .prepare("SELECT from_user, text FROM chat_messages ORDER BY id")?
            .query_map([], |r| {
                Ok(ChatMsg {
                    from_user: r.get::<_, i64>(0)? != 0,
                    text: r.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;

        let account_links = c
            .prepare("SELECT ext_id, account_id FROM account_links ORDER BY rowid")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        let synced_ids = c
            .prepare("SELECT ext_id FROM synced_transactions")?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let vault = c
            .query_row("SELECT salt, check_nonce, check_cipher FROM vault_meta WHERE id = 1", [], |r| {
                Ok(VaultMeta {
                    salt: r.get(0)?,
                    check_nonce: r.get(1)?,
                    check_cipher: r.get(2)?,
                })
            })
            .optional()?;
        let credentials = c
            .prepare("SELECT id, institution, kind, account_id, nonce, cipher, updated FROM credentials ORDER BY id")?
            .query_map([], |r| {
                Ok(Credential {
                    id: r.get(0)?,
                    institution: r.get(1)?,
                    kind: dec::<ServiceKind>(&r.get::<_, String>(2)?)?,
                    account_id: r.get(3)?,
                    nonce: r.get(4)?,
                    cipher: r.get(5)?,
                    updated: r.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        let connections = c
            .prepare("SELECT id, label, credential_id, last_sync, status FROM bank_connections ORDER BY id")?
            .query_map([], |r| {
                Ok(BankConnection {
                    id: r.get(0)?,
                    label: r.get(1)?,
                    credential_id: r.get(2)?,
                    last_sync: r.get(3)?,
                    status: r.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;

        let alerts = match setting("alerts")? {
            Some(s) => dec(&s)?,
            None => AlertSettings::default(),
        };
        let fee_pct = setting("fee_pct")?.and_then(|s| s.parse().ok()).unwrap_or(40.0);
        let next_id = setting("next_id")?.and_then(|s| s.parse().ok()).unwrap_or(1);

        Ok(State {
            accounts,
            transactions,
            subscriptions,
            budgets,
            alerts,
            bills,
            fee_pct,
            goals,
            transfers,
            net_worth_history,
            credit,
            chat,
            next_id,
            profile,
            custom_categories,
            rules,
            account_links,
            synced_ids,
            vault,
            credentials,
            connections,
        })
    }

    /// Writes the whole state in one transaction.
    pub fn save_state(&mut self, s: &State) -> DbResult<()> {
        let tx = self.conn.transaction().map_err(err)?;
        Self::write_state(&tx, s).map_err(err)?;
        tx.commit().map_err(err)
    }

    fn write_state(c: &Connection, s: &State) -> rusqlite::Result<()> {
        Self::write_profile(c, &s.profile)?;
        for t in DATA_TABLES {
            c.execute(&format!("DELETE FROM {t}"), [])?;
        }

        let mut q = c.prepare("INSERT INTO settings (key, value) VALUES (?1, ?2)")?;
        q.execute(params!["alerts", enc(&s.alerts)])?;
        q.execute(params!["fee_pct", s.fee_pct.to_string()])?;
        q.execute(params!["next_id", s.next_id.to_string()])?;

        let mut q = c.prepare("INSERT INTO accounts (id, name, kind, balance, credit_limit) VALUES (?1, ?2, ?3, ?4, ?5)")?;
        for a in &s.accounts {
            q.execute(params![a.id, a.name, enc(&a.kind), a.balance, a.limit])?;
        }

        let mut q =
            c.prepare("INSERT INTO transactions (id, date, merchant, amount, category, account_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")?;
        for t in &s.transactions {
            q.execute(params![
                t.id as i64,
                date_str(t.date),
                t.merchant,
                t.amount,
                enc(&t.category),
                t.account
            ])?;
        }

        let mut q = c.prepare(
            "INSERT INTO subscriptions (merchant, amount, cadence_days, last_charge, category, account_id, status, remind)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )?;
        for x in &s.subscriptions {
            q.execute(params![
                x.merchant,
                x.amount,
                x.cadence_days,
                date_str(x.last_charge),
                enc(&x.category),
                x.account,
                enc(&x.status),
                x.remind as i64
            ])?;
        }

        let mut q = c.prepare("INSERT OR REPLACE INTO budgets (category, monthly_limit) VALUES (?1, ?2)")?;
        for b in &s.budgets {
            q.execute(params![enc(&b.category), b.limit])?;
        }

        let mut q = c.prepare("INSERT INTO bills (id, provider, kind, monthly, status) VALUES (?1, ?2, ?3, ?4, ?5)")?;
        for b in &s.bills {
            q.execute(params![b.id, b.provider, enc(&b.kind), b.monthly, enc(&b.status)])?;
        }

        let mut q = c.prepare(
            "INSERT INTO goals (id, name, icon, target, saved, auto_on, smart, auto_amount, freq, next_transfer, from_account)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        )?;
        for g in &s.goals {
            q.execute(params![
                g.id,
                g.name,
                g.icon,
                g.target,
                g.saved,
                g.auto_on as i64,
                g.smart as i64,
                g.auto_amount,
                enc(&g.freq),
                date_str(g.next_transfer),
                g.from_account
            ])?;
        }

        let mut q = c.prepare("INSERT INTO goal_transfers (date, goal, amount, note) VALUES (?1, ?2, ?3, ?4)")?;
        for t in &s.transfers {
            q.execute(params![date_str(t.date), t.goal, t.amount, t.note])?;
        }

        let mut q = c.prepare("INSERT OR REPLACE INTO net_worth_history (month, value) VALUES (?1, ?2)")?;
        for (m, v) in &s.net_worth_history {
            q.execute(params![date_str(*m), v])?;
        }

        let mut q = c.prepare("INSERT OR REPLACE INTO credit_history (month, score) VALUES (?1, ?2)")?;
        for (m, v) in &s.credit.history {
            q.execute(params![date_str(*m), v])?;
        }
        let cr = &s.credit;
        c.execute(
            "INSERT INTO credit_profile (id, on_time_pct, age_years, hard_inquiries, derogatory) VALUES (1, ?1, ?2, ?3, ?4)",
            params![cr.on_time_pct, cr.age_years, cr.hard_inquiries, cr.derogatory],
        )?;

        let mut q = c.prepare("INSERT INTO rules (contains, category, rename) VALUES (?1, ?2, ?3)")?;
        for r in &s.rules {
            q.execute(params![r.contains, enc(&r.category), r.rename])?;
        }

        let mut q = c.prepare("INSERT INTO custom_categories (position, name) VALUES (?1, ?2)")?;
        for (i, n) in s.custom_categories.iter().enumerate() {
            q.execute(params![i as i64, n])?;
        }

        let mut q = c.prepare("INSERT INTO chat_messages (from_user, text) VALUES (?1, ?2)")?;
        for m in &s.chat {
            q.execute(params![m.from_user as i64, m.text])?;
        }

        let mut q = c.prepare("INSERT OR REPLACE INTO account_links (ext_id, account_id) VALUES (?1, ?2)")?;
        for (ext, id) in &s.account_links {
            q.execute(params![ext, id])?;
        }
        let mut q = c.prepare("INSERT OR IGNORE INTO synced_transactions (ext_id) VALUES (?1)")?;
        for ext in &s.synced_ids {
            q.execute(params![ext])?;
        }
        if let Some(v) = &s.vault {
            c.execute(
                "INSERT INTO vault_meta (id, salt, check_nonce, check_cipher) VALUES (1, ?1, ?2, ?3)",
                params![v.salt, v.check_nonce, v.check_cipher],
            )?;
        }
        let mut q = c.prepare(
            "INSERT INTO credentials (id, institution, kind, account_id, nonce, cipher, updated)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;
        for k in &s.credentials {
            q.execute(params![
                k.id,
                k.institution,
                enc(&k.kind),
                k.account_id,
                k.nonce,
                k.cipher,
                k.updated
            ])?;
        }
        let mut q = c.prepare("INSERT INTO bank_connections (id, label, credential_id, last_sync, status) VALUES (?1, ?2, ?3, ?4, ?5)")?;
        for b in &s.connections {
            q.execute(params![b.id, b.label, b.credential_id, b.last_sync, b.status])?;
        }
        Ok(())
    }

    /// Marks setup as not done, so the wizard runs again next time (data is kept).
    pub fn reset_setup(&self) -> DbResult<()> {
        self.conn
            .execute("UPDATE profile SET setup_complete = 0 WHERE id = 1", [])
            .map(|_| ())
            .map_err(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Saving and loading gives back exactly what was saved.
    #[test]
    fn round_trip() {
        let path = std::env::temp_dir().join(format!("coincraft-test-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut state = sample();
        state.custom_categories.push("Pets".into());
        set_custom_categories(&state.custom_categories);
        state.rules.push(Rule {
            contains: "vet".into(),
            category: Category::Custom(0),
            rename: "Vet".into(),
        });
        state.chat.push(ChatMsg {
            from_user: true,
            text: "Hi".into(),
        });
        let (meta, key) = crate::vault::create("master password 1").unwrap();
        let secret = crate::vault::CredentialSecret {
            password: "pw".into(),
            ..Default::default()
        };
        let (nonce, cipher) = crate::vault::seal_secret(&key, &secret).unwrap();
        state.vault = Some(meta);
        state.credentials.push(Credential {
            id: 1,
            institution: "Example Bank".into(),
            kind: ServiceKind::Bank,
            account_id: Some(1),
            nonce,
            cipher,
            updated: "2026-11-25".into(),
        });
        state.connections.push(BankConnection {
            id: 1,
            label: "Bridge".into(),
            credential_id: 1,
            last_sync: String::new(),
            status: "Not synced yet".into(),
        });
        state.account_links.push(("ofx:x:1".into(), 1));
        state.synced_ids.insert("ofx:x:1:T1".into());

        let mut db = Db::open(&path).unwrap();
        assert!(db.load_state().unwrap().is_none(), "a new database starts empty");
        db.save_state(&state).unwrap();
        db.save_state(&state).unwrap(); // saving twice must not duplicate anything
        drop(db);

        let db = Db::open(&path).unwrap();
        let loaded = db.load_state().unwrap().expect("state was saved");
        assert_eq!(serde_json::to_string(&loaded).unwrap(), serde_json::to_string(&state).unwrap());
        assert!(loaded.profile.setup_complete);

        db.reset_setup().unwrap();
        assert!(!db.load_profile().unwrap().unwrap().setup_complete);
        let _ = std::fs::remove_file(&path);
    }
}
