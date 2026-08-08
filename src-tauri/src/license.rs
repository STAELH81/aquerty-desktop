use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Shared secret for local license signing. Replace before shipping paid builds.
const LICENSE_SECRET: &[u8] = b"aquerty-stop-v1-change-me-before-ship";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LicenseInfo {
    pub is_pro: bool,
    pub key: Option<String>,
    pub message: String,
}

pub fn validate_key(key: &str) -> bool {
    let normalized = key.trim().to_uppercase().replace(' ', "");
    if normalized.is_empty() {
        return false;
    }

    // Accept demo key for testing / reviews
    if normalized == "AQUERTY-PRO-DEMO-2026" {
        return true;
    }

    // Format: AQUERTY-<PAYLOAD>-<HEX8>
    let parts: Vec<&str> = normalized.split('-').collect();
    if parts.len() < 3 || parts[0] != "AQUERTY" {
        return false;
    }

    let signature = parts.last().copied().unwrap_or("");
    if signature.len() != 8 || !signature.chars().all(|c| c.is_ascii_hexdigit()) {
        return false;
    }

    let payload = parts[1..parts.len() - 1].join("-");
    let expected = sign_payload(&payload);
    expected.eq_ignore_ascii_case(signature)
}

fn sign_payload(payload: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(LICENSE_SECRET).expect("HMAC key length is valid");
    mac.update(payload.as_bytes());
    let result = mac.finalize().into_bytes();
    hex::encode(&result[..4]).to_uppercase()
}

/// Helper for generating keys offline: cargo test or a small CLI later.
#[allow(dead_code)]
pub fn generate_key(payload: &str) -> String {
    let sig = sign_payload(&payload.to_uppercase());
    format!("AQUERTY-{}-{}", payload.to_uppercase(), sig)
}

pub fn info_from_key(key: Option<&str>) -> LicenseInfo {
    match key {
        Some(k) if validate_key(k) => LicenseInfo {
            is_pro: true,
            key: Some(k.trim().to_uppercase()),
            message: "Licence Pro active".into(),
        },
        Some(_) => LicenseInfo {
            is_pro: false,
            key: None,
            message: "Clé invalide".into(),
        },
        None => LicenseInfo {
            is_pro: false,
            key: None,
            message: "Version gratuite".into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_key_works() {
        assert!(validate_key("AQUERTY-PRO-DEMO-2026"));
    }

    #[test]
    fn generated_key_roundtrips() {
        let key = generate_key("SACHA");
        assert!(validate_key(&key));
    }
}
