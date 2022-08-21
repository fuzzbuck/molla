use chrono::{DateTime, Duration, Local, Utc};
use ed25519_compact::{KeyPair, PublicKey, SecretKey, Signature};
use hex::FromHexError;
use once_cell::sync::Lazy;
use regex::Regex;
use std::default;
use std::path::Component::Prefix;

// The macro in the spec didn't work and I don't know regex.. so just gonna use 83e for now.
pub(crate) static KEY_VALIDATOR: Lazy<Regex> = Lazy::new(|| Regex::new(r"83e").unwrap());

pub(crate) struct Spring83Key {
    signature: Signature,
    key: PublicKey,
    expiry: DateTime<Utc>,
}

impl Spring83Key {
    pub(crate) fn new(sig: Signature, key: [u8; 32], expiry: DateTime<Utc>) -> Self {
        Self {
            signature: sig,
            key: PublicKey::new(key),
            expiry,
        }
    }
    /// Whether this key is expired or further than 2 years into the future judging by MMYY as last 4 digits of the key
    pub(crate) fn expired_or_too_far_in_future(&self) -> bool {
        let date_now = Local::now().with_timezone(&Utc);
        self.expiry < date_now || self.expiry > date_now + Duration::days(730)
    }
    /// Create a mew Spring83Key from a 64 character hex public key, and 128 character hex signature
    pub(crate) fn from_hex(public_key: &str, signature: &str) -> Result<Self, FromHexError> {
        let mut key_bytes: [u8; 32] = [0u8; 32];
        key_bytes.copy_from_slice(&hex::decode(public_key)?);
        let mut signature_bytes: [u8; 64] = [0u8; 64];
        signature_bytes.copy_from_slice(&hex::decode(signature)?);

        // get 4 last digits of key (MMYY)
        let key_last4d = &public_key[public_key.len() - 4..];

        // parse as expiry date (MMYY), or use date mon. 00, year 00 if failed (which will fail later)
        let expiry = DateTime::parse_from_str(key_last4d, "%m%y")
            .unwrap_or(DateTime::default())
            .with_timezone(&Utc);

        Ok(Self::new(
            Signature::new(signature_bytes),
            key_bytes,
            expiry,
        ))
    }
    /// Check whether this key was properly signed by the provided signature
    pub(crate) fn verify(&self) -> bool {
        self.key
            .verify(hex::encode(&self.key.to_vec()), &self.signature)
            .is_ok()
    }
    /// Check whether a string appears to be a valid Spring83Key
    pub(crate) fn validate(str: &str) -> bool {
        str.len() == 64 && KEY_VALIDATOR.shortest_match(&str).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expired_key() {
        let key = Spring83Key::from_hex(
            "ab589f4dde9fce4180fcf42c7b05185b0a02a5d682e353fa39177995083e0583",
            "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"
        ).unwrap();

        assert!(key.expired_or_too_far_in_future())
    }
}
