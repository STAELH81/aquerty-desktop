use chrono::{Datelike, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// HMAC secret embedded for offline validation. Keep in sync with gen-license.
const LICENSE_SECRET: &[u8] = b"aquerty-stop-prod-9f3c2a71e8b64d05";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LicenseInfo {
    pub is_pro: bool,
    pub key: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LicenseKind {
    Lifetime,
    Annual { expires_yyyy_mm_dd: u32 },
}

pub fn validate_key(key: &str) -> bool {
    parse_key(key).is_some()
}

/// Returns license kind if signature is valid and not expired.
pub fn parse_key(key: &str) -> Option<LicenseKind> {
    let normalized = key.trim().to_uppercase().replace(' ', "");
    if normalized.is_empty() {
        return None;
    }

    // Format: AQUERTY-<PAYLOAD>-<HEX8>
    let parts: Vec<&str> = normalized.split('-').collect();
    if parts.len() < 3 || parts[0] != "AQUERTY" {
        return None;
    }

    let signature = parts.last().copied().unwrap_or("");
    if signature.len() != 8 || !signature.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    let payload = parts[1..parts.len() - 1].join("-");
    let expected = sign_payload(&payload);
    if !expected.eq_ignore_ascii_case(signature) {
        return None;
    }

    classify_payload(&payload)
}

fn classify_payload(payload: &str) -> Option<LicenseKind> {
    let parts: Vec<&str> = payload.split('-').collect();
    if parts.is_empty() {
        return None;
    }

    match parts[0] {
        "LIFE" => {
            if parts.len() < 2 || parts[1].is_empty() {
                return None;
            }
            Some(LicenseKind::Lifetime)
        }
        "YR" => {
            // YR-YYYYMMDD-<id…>
            if parts.len() < 3 {
                return None;
            }
            let date = parts[1];
            if date.len() != 8 || !date.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            let yyyymmdd: u32 = date.parse().ok()?;
            if !annual_still_valid(yyyymmdd) {
                return None;
            }
            Some(LicenseKind::Annual {
                expires_yyyy_mm_dd: yyyymmdd,
            })
        }
        _ => None,
    }
}

fn annual_still_valid(expires_yyyy_mm_dd: u32) -> bool {
    let y = (expires_yyyy_mm_dd / 10_000) as i32;
    let m = ((expires_yyyy_mm_dd / 100) % 100) as u32;
    let d = (expires_yyyy_mm_dd % 100) as u32;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return false;
    }
    let today = Utc::now().date_naive();
    let Some(expiry) = chrono::NaiveDate::from_ymd_opt(y, m, d) else {
        return false;
    };
    today <= expiry
}

fn sign_payload(payload: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(LICENSE_SECRET).expect("HMAC key length is valid");
    mac.update(payload.as_bytes());
    let result = mac.finalize().into_bytes();
    hex::encode(&result[..4]).to_uppercase()
}

pub fn generate_key(payload: &str) -> String {
    let payload = payload.trim().to_uppercase().replace(' ', "");
    let sig = sign_payload(&payload);
    format!("AQUERTY-{payload}-{sig}")
}

/// Lifetime: AQUERTY-LIFE-<ID>-<SIG>
pub fn generate_lifetime(id: &str) -> String {
    let id = sanitize_id(id);
    generate_key(&format!("LIFE-{id}"))
}

/// Annual: AQUERTY-YR-<YYYYMMDD>-<ID>-<SIG> (valid through expiry day UTC)
pub fn generate_annual(id: &str, expires_yyyy_mm_dd: u32) -> String {
    let id = sanitize_id(id);
    generate_key(&format!("YR-{expires_yyyy_mm_dd:08}-{id}"))
}

/// Default annual expiry: same month/day next year (or +365-ish via calendar year).
pub fn default_annual_expiry() -> u32 {
    let d = Utc::now().date_naive() + chrono::Duration::days(365);
    (d.year() as u32) * 10_000 + d.month() * 100 + d.day()
}

fn sanitize_id(id: &str) -> String {
    id.trim()
        .to_uppercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { 'X' })
        .take(24)
        .collect()
}

pub fn info_from_key(key: Option<&str>) -> LicenseInfo {
    match key {
        Some(k) => match parse_key(k) {
            Some(LicenseKind::Lifetime) => LicenseInfo {
                is_pro: true,
                key: Some(k.trim().to_uppercase()),
                message: "Licence Pro à vie".into(),
            },
            Some(LicenseKind::Annual { expires_yyyy_mm_dd }) => LicenseInfo {
                is_pro: true,
                key: Some(k.trim().to_uppercase()),
                message: format!("Licence Pro annuelle (jusqu'au {expires_yyyy_mm_dd})"),
            },
            None => LicenseInfo {
                is_pro: false,
                key: None,
                message: "Clé invalide ou expirée".into(),
            },
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
    fn demo_key_rejected() {
        assert!(!validate_key("AQUERTY-PRO-DEMO-2026"));
    }

    #[test]
    fn lifetime_roundtrip() {
        let key = generate_lifetime("GUMROAD01");
        assert!(validate_key(&key));
        assert_eq!(parse_key(&key), Some(LicenseKind::Lifetime));
    }

    #[test]
    fn annual_roundtrip() {
        let exp = default_annual_expiry();
        let key = generate_annual("ORDER99", exp);
        assert!(validate_key(&key));
        assert!(matches!(
            parse_key(&key),
            Some(LicenseKind::Annual { expires_yyyy_mm_dd }) if expires_yyyy_mm_dd == exp
        ));
    }

    #[test]
    fn expired_annual_rejected() {
        let key = generate_annual("OLD", 20200101);
        assert!(!validate_key(&key));
    }
}
