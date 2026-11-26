//! The encrypted vault for logins to banks, cards and other financial services.
//!
//! A master password is turned into a 256-bit key with Argon2id (slow on purpose, so guessing
//! is expensive). Each saved login is sealed with XChaCha20-Poly1305 using that key and a fresh
//! random nonce, so the database only ever holds scrambled bytes. The key lives in memory only
//! while the vault is unlocked; the master password itself is never stored.

use argon2::Argon2;
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};

/// Known text sealed with the key, used to tell a right master password from a wrong one.
const CHECK_TEXT: &[u8] = b"coincraft-vault-v1";
pub const MIN_PASSWORD_LEN: usize = 8;

pub type Key = [u8; 32];

/// What is stored about the vault itself: the salt and a sealed check value. Never the key.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct VaultMeta {
    pub salt: Vec<u8>,
    pub check_nonce: Vec<u8>,
    pub check_cipher: Vec<u8>,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum ServiceKind {
    Bank,
    CreditCard,
    Loan,
    Investment,
    Insurance,
    Utility,
    BankSync,
    Other,
}

impl ServiceKind {
    /// Kinds a person picks from (BankSync entries are made by the bank-sync connection).
    pub const PICKABLE: [ServiceKind; 7] = [
        ServiceKind::Bank,
        ServiceKind::CreditCard,
        ServiceKind::Loan,
        ServiceKind::Investment,
        ServiceKind::Insurance,
        ServiceKind::Utility,
        ServiceKind::Other,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ServiceKind::Bank => "Bank",
            ServiceKind::CreditCard => "Credit card",
            ServiceKind::Loan => "Loan",
            ServiceKind::Investment => "Investment",
            ServiceKind::Insurance => "Insurance",
            ServiceKind::Utility => "Utility",
            ServiceKind::BankSync => "Bank sync",
            ServiceKind::Other => "Other",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            ServiceKind::Bank => "🏠",
            ServiceKind::CreditCard => "💳",
            ServiceKind::Loan => "📄",
            ServiceKind::Investment => "📈",
            ServiceKind::Insurance => "☂",
            ServiceKind::Utility => "💡",
            ServiceKind::BankSync => "↔",
            ServiceKind::Other => "🔒",
        }
    }
}

/// A saved login. The name, kind and linked account are visible while locked so the list can
/// be shown; everything in `CredentialSecret` is sealed.
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct Credential {
    pub id: u32,
    pub institution: String,
    pub kind: ServiceKind,
    pub account_id: Option<u32>,
    pub nonce: Vec<u8>,
    pub cipher: Vec<u8>,
    pub updated: String,
}

/// The sealed part of a saved login.
#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Debug)]
pub struct CredentialSecret {
    pub website: String,
    pub username: String,
    pub password: String,
    pub account_number: String,
    pub phone: String,
    pub notes: String,
    /// For bank-sync connections: the private address that reads the accounts.
    #[serde(default)]
    pub access_url: String,
}

impl CredentialSecret {
    /// Account numbers are shown as "•••• 1234".
    pub fn masked_account(&self) -> String {
        let digits: String = self.account_number.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        if digits.len() <= 4 {
            digits
        } else {
            format!("•••• {}", &digits[digits.len() - 4..])
        }
    }
}

fn derive_key(password: &str, salt: &[u8]) -> Result<Key, String> {
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| e.to_string())?;
    Ok(key)
}

fn random_bytes() -> Vec<u8> {
    XChaCha20Poly1305::generate_nonce(&mut OsRng).to_vec()
}

/// Seals bytes with the key. Returns (nonce, ciphertext).
pub fn seal(key: &Key, plain: &[u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|e| e.to_string())?;
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let sealed = cipher.encrypt(&nonce, plain).map_err(|_| "Couldn't encrypt.".to_string())?;
    Ok((nonce.to_vec(), sealed))
}

/// Opens sealed bytes; fails if the key is wrong or the bytes were changed.
pub fn open(key: &Key, nonce: &[u8], sealed: &[u8]) -> Result<Vec<u8>, String> {
    if nonce.len() != 24 {
        return Err("The saved data is damaged.".into());
    }
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|e| e.to_string())?;
    cipher
        .decrypt(XNonce::from_slice(nonce), sealed)
        .map_err(|_| "Couldn't decrypt.".to_string())
}

/// Sets up a new vault with this master password.
pub fn create(password: &str) -> Result<(VaultMeta, Key), String> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(format!("Use at least {MIN_PASSWORD_LEN} characters."));
    }
    let salt = random_bytes();
    let key = derive_key(password, &salt)?;
    let (check_nonce, check_cipher) = seal(&key, CHECK_TEXT)?;
    Ok((
        VaultMeta {
            salt,
            check_nonce,
            check_cipher,
        },
        key,
    ))
}

/// Gives the key back if the master password is right.
pub fn unlock(meta: &VaultMeta, password: &str) -> Result<Key, String> {
    let key = derive_key(password, &meta.salt)?;
    match open(&key, &meta.check_nonce, &meta.check_cipher) {
        Ok(text) if text == CHECK_TEXT => Ok(key),
        _ => Err("That master password isn't right.".into()),
    }
}

pub fn seal_secret(key: &Key, secret: &CredentialSecret) -> Result<(Vec<u8>, Vec<u8>), String> {
    let json = serde_json::to_vec(secret).map_err(|e| e.to_string())?;
    seal(key, &json)
}

pub fn open_secret(key: &Key, c: &Credential) -> Result<CredentialSecret, String> {
    let bytes = open(key, &c.nonce, &c.cipher)?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_unlock_seal_open() {
        assert!(create("short").is_err());
        let (meta, key) = create("correct horse battery").unwrap();
        assert_eq!(unlock(&meta, "correct horse battery").unwrap(), key);
        assert!(unlock(&meta, "wrong password!").is_err());

        let secret = CredentialSecret {
            username: "alex".into(),
            password: "p@ss".into(),
            ..Default::default()
        };
        let (nonce, sealed) = seal_secret(&key, &secret).unwrap();
        assert!(
            !String::from_utf8_lossy(&sealed).contains("p@ss"),
            "the password must not be readable"
        );
        let c = Credential {
            id: 1,
            institution: "Bank".into(),
            kind: ServiceKind::Bank,
            account_id: None,
            nonce,
            cipher: sealed,
            updated: String::new(),
        };
        assert_eq!(open_secret(&key, &c).unwrap(), secret);
        let other = unlock(&create("another password").unwrap().0, "another password").unwrap();
        assert!(open_secret(&other, &c).is_err());
    }

    #[test]
    fn masks_account_numbers() {
        let s = CredentialSecret {
            account_number: "1234-5678-9012".into(),
            ..Default::default()
        };
        assert_eq!(s.masked_account(), "•••• 9012");
    }
}
