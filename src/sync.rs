//! Bank sync through SimpleFIN Bridge (https://beta-bridge.simplefin.org).
//!
//! SimpleFIN is an open protocol made for personal apps like this one: the person signs up with
//! the bridge, links their banks and cards there, and gets a one-time setup token. CoinCraft
//! trades that token for a private access address and then reads balances and transactions
//! from it directly, with no developer account or server of our own. These calls are the only
//! time CoinCraft goes online, and only when the person connects or presses Sync.

use crate::data::{guess_account_kind, AccountKind, RemoteAccount, RemoteTx};
use base64::engine::general_purpose::{STANDARD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine as _;
use chrono::{DateTime, Duration, Utc};
use serde_json::Value;

const TIMEOUT_SECS: u64 = 45;
/// How far back the first sync reaches.
pub const HISTORY_DAYS: i64 = 90;

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
        .build()
}

fn describe(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(403, _) => "The bridge refused access. The token may have been used already, or access was revoked.".into(),
        ureq::Error::Status(402, _) => "The bridge says payment is needed on your SimpleFIN account.".into(),
        ureq::Error::Status(code, _) => format!("The bridge answered with error {code}."),
        ureq::Error::Transport(t) => format!("Couldn't reach the bridge: {t}"),
    }
}

/// Turns a setup token into the private access address. A token works only once.
pub fn claim(setup_token: &str) -> Result<String, String> {
    let token: String = setup_token.chars().filter(|c| !c.is_whitespace()).collect();
    if token.is_empty() {
        return Err("Paste your setup token first.".into());
    }
    let bytes = STANDARD
        .decode(&token)
        .or_else(|_| URL_SAFE.decode(&token))
        .or_else(|_| URL_SAFE_NO_PAD.decode(&token))
        .map_err(|_| "That doesn't look like a SimpleFIN setup token.".to_string())?;
    let claim_url = String::from_utf8(bytes).map_err(|_| "That doesn't look like a SimpleFIN setup token.".to_string())?;
    if !claim_url.starts_with("https://") {
        return Err("That doesn't look like a SimpleFIN setup token.".into());
    }
    let access = agent()
        .post(claim_url.trim())
        .set("Content-Length", "0")
        .call()
        .map_err(describe)?
        .into_string()
        .map_err(|e| e.to_string())?;
    let access = access.trim().to_string();
    if !access.starts_with("https://") {
        return Err("The bridge sent back something unexpected.".into());
    }
    Ok(access)
}

/// Splits "https://user:pass@host/path" into ("https://host/path", "Basic …").
fn split_access(access_url: &str) -> Result<(String, String), String> {
    let (scheme, rest) = access_url.split_once("://").ok_or("The saved access address is damaged.")?;
    let (userinfo, host_path) = rest.split_once('@').ok_or("The saved access address is damaged.")?;
    let header = format!("Basic {}", STANDARD.encode(userinfo));
    Ok((format!("{scheme}://{}", host_path.trim_end_matches('/')), header))
}

fn num(v: &Value) -> Option<f64> {
    match v {
        Value::String(s) => s.trim().parse().ok(),
        Value::Number(n) => n.as_f64(),
        _ => None,
    }
}

fn text(v: &Value, key: &str) -> String {
    match v.get(key) {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

/// Reads the SimpleFIN accounts document into accounts and transactions.
pub fn parse_accounts(body: &str) -> Result<(Vec<RemoteAccount>, Vec<String>), String> {
    let doc: Value = serde_json::from_str(body).map_err(|e| format!("The bridge sent data CoinCraft can't read ({e})."))?;
    let errors: Vec<String> = doc
        .get("errors")
        .and_then(|e| e.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let mut out = Vec::new();
    for a in doc.get("accounts").and_then(|a| a.as_array()).cloned().unwrap_or_default() {
        let id = text(&a, "id");
        if id.is_empty() {
            continue;
        }
        let name = text(&a, "name");
        let org = a.get("org").map(|o| {
            let n = text(o, "name");
            if n.is_empty() {
                text(o, "domain")
            } else {
                n
            }
        });
        let balance = a.get("balance").and_then(num);
        let mut kind = guess_account_kind(&name);
        // A negative balance on an account that doesn't say otherwise is most likely a card.
        if kind == AccountKind::Checking && balance.is_some_and(|b| b < 0.0) && name.to_lowercase().contains("card") {
            kind = AccountKind::CreditCard;
        }
        let mut transactions = Vec::new();
        for t in a.get("transactions").and_then(|t| t.as_array()).cloned().unwrap_or_default() {
            let posted = t
                .get("posted")
                .and_then(|p| p.as_i64())
                .or_else(|| t.get("transacted_at").and_then(|p| p.as_i64()));
            let (Some(ts), Some(amount)) = (posted, t.get("amount").and_then(num)) else {
                continue;
            };
            // A posted time of 0 means still pending; leave those for the next sync.
            if ts == 0 {
                continue;
            }
            let Some(date) = DateTime::<Utc>::from_timestamp(ts, 0).map(|d| d.date_naive()) else {
                continue;
            };
            let mut description = text(&t, "payee");
            if description.is_empty() {
                description = text(&t, "description");
            }
            if description.is_empty() {
                description = text(&t, "memo");
            }
            let ext_id = text(&t, "id");
            let ext_id = if ext_id.is_empty() {
                format!("{date}|{amount}|{description}")
            } else {
                ext_id
            };
            transactions.push(RemoteTx {
                ext_id,
                date,
                amount,
                description,
            });
        }
        out.push(RemoteAccount {
            ext_id: format!("simplefin:{id}"),
            name: if name.is_empty() { "Account".into() } else { name },
            institution: org.unwrap_or_default(),
            kind,
            balance,
            transactions,
        });
    }
    Ok((out, errors))
}

/// Downloads accounts and recent transactions. Returns them with any notes from the bridge.
pub fn fetch(access_url: &str, since_days: i64) -> Result<(Vec<RemoteAccount>, Vec<String>), String> {
    let (base, auth) = split_access(access_url)?;
    let start = (Utc::now() - Duration::days(since_days)).timestamp();
    let body = agent()
        .get(&format!("{base}/accounts"))
        .set("Authorization", &auth)
        .query("start-date", &start.to_string())
        .call()
        .map_err(describe)?
        .into_string()
        .map_err(|e| e.to_string())?;
    parse_accounts(&body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_access_address() {
        let (url, auth) = split_access("https://user:secret@bridge.example.org/simplefin/").unwrap();
        assert_eq!(url, "https://bridge.example.org/simplefin");
        assert_eq!(auth, format!("Basic {}", STANDARD.encode("user:secret")));
    }

    #[test]
    fn reads_accounts_document() {
        let body = r#"{"errors":["Connection to Example Bank may need attention"],
          "accounts":[
            {"org":{"domain":"examplebank.com","name":"Example Bank"},"id":"ACT-1","name":"Everyday Checking",
             "currency":"USD","balance":"1520.10","balance-date":1764000000,
             "transactions":[
               {"id":"T1","posted":1763900000,"amount":"-42.50","description":"SQ *CORNER MARKET #12"},
               {"id":"T2","posted":0,"amount":"-5.00","description":"Pending"}]},
            {"org":{"name":"Example Card Co"},"id":"ACT-2","name":"Rewards Credit Card","balance":"-310.00","transactions":[]}
          ]}"#;
        let (accounts, errors) = parse_accounts(body).unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(accounts.len(), 2);
        assert_eq!(accounts[0].ext_id, "simplefin:ACT-1");
        assert_eq!(accounts[0].institution, "Example Bank");
        assert_eq!(accounts[0].balance, Some(1520.10));
        assert_eq!(accounts[0].transactions.len(), 1, "pending transactions wait for the next sync");
        assert_eq!(accounts[0].transactions[0].amount, -42.50);
        assert_eq!(accounts[1].kind, AccountKind::CreditCard);
    }

    #[test]
    fn rejects_bad_tokens() {
        assert!(claim("").is_err());
        assert!(claim("not base64 at all!!").is_err());
        assert!(claim(&STANDARD.encode("http://insecure.example.org")).is_err());
    }
}
