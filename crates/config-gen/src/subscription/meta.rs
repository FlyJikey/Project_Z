//! Заголовки ответа подписки (ТЗ §5.1) и `ProfileView` для UI.

use ipc_proto::{ProfileSupport, ProfileView, SubscriptionUsage};

use super::model::{ParsedSubscription, ProfileOutcome};
use crate::redact::sanitize_for_log;

const MAX_TITLE_CHARS: usize = 128;
const MAX_URL_LEN: usize = 2048;
const MAX_INTERVAL_HOURS: u32 = 24 * 365;

/// Разобранные заголовки ответа.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SubscriptionMeta {
    pub title: Option<String>,
    pub usage: Option<SubscriptionUsage>,
    pub update_interval_hours: Option<u32>,
    pub support_url: Option<String>,
}

/// Разбирает заголовки ответа. Имена — без учёта регистра; значения
/// недоверенные: всё, что не разбирается, молча игнорируется.
pub fn parse_response_headers<'a>(
    headers: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> SubscriptionMeta {
    let mut meta = SubscriptionMeta::default();
    for (name, value) in headers {
        match name.trim().to_ascii_lowercase().as_str() {
            "profile-title" => meta.title = parse_title(value),
            "subscription-userinfo" => meta.usage = parse_userinfo(value),
            "profile-update-interval" => meta.update_interval_hours = parse_interval_hours(value),
            "profile-web-page-url" => meta.support_url = parse_support_url(value),
            _ => {}
        }
    }
    meta
}

fn parse_title(value: &str) -> Option<String> {
    let v = value.trim();
    let text = match v
        .strip_prefix("base64:")
        .or_else(|| v.strip_prefix("Base64:"))
    {
        Some(b64) => String::from_utf8_lossy(&decode_base64(b64.trim())?).into_owned(),
        None => v.to_string(),
    };
    let clean = sanitize_for_log(text.trim(), MAX_TITLE_CHARS);
    if clean.is_empty() { None } else { Some(clean) }
}

/// `upload=0; download=123; total=456; expire=1700000000`.
/// `expire=0` и `total=0` означают «без ограничения» и превращаются в `None`.
fn parse_userinfo(value: &str) -> Option<SubscriptionUsage> {
    let mut usage = SubscriptionUsage::default();
    let mut any = false;
    for part in value.split(';') {
        let Some((k, v)) = part.split_once('=') else {
            continue;
        };
        let Some(n) = parse_u64_lenient(v.trim()) else {
            continue;
        };
        match k.trim().to_ascii_lowercase().as_str() {
            "upload" => usage.upload = Some(n),
            "download" => usage.download = Some(n),
            "total" => usage.total = if n == 0 { None } else { Some(n) },
            "expire" => usage.expire = if n == 0 { None } else { Some(n) },
            _ => continue,
        }
        any = true;
    }
    any.then_some(usage)
}

fn parse_u64_lenient(s: &str) -> Option<u64> {
    if let Ok(n) = s.parse::<u64>() {
        return Some(n);
    }
    let f: f64 = s.parse().ok()?;
    if f.is_finite() && (0.0..1.0e18).contains(&f) {
        Some(f as u64)
    } else {
        None
    }
}

fn parse_interval_hours(value: &str) -> Option<u32> {
    let n = parse_u64_lenient(value.trim())?;
    if n == 0 {
        None
    } else {
        Some(u32::try_from(n).unwrap_or(u32::MAX).min(MAX_INTERVAL_HOURS))
    }
}

fn parse_support_url(value: &str) -> Option<String> {
    let v = value.trim();
    if v.is_empty() || v.len() > MAX_URL_LEN {
        return None;
    }
    let u = url::Url::parse(v).ok()?;
    if !matches!(u.scheme(), "http" | "https")
        || !u.username().is_empty()
        || u.password().is_some()
        || u.host_str().is_none()
    {
        return None;
    }
    Some(u.to_string())
}

/// Base64 (стандартный и URL-алфавит, с «=» и без).
pub(crate) fn decode_base64(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0;
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' | b'\r' | b'\n' | b' ' => continue,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xff) as u8);
        }
    }
    Some(out)
}

/// Профили для UI (`ipc_proto::ProfileView`).
pub fn profile_views(sub: &ParsedSubscription) -> Vec<ProfileView> {
    sub.profiles
        .iter()
        .map(|p| match &p.outcome {
            ProfileOutcome::Supported(cfg) => ProfileView {
                index: p.index,
                name: p.name.clone(),
                server_count: u32::try_from(cfg.server_count()).unwrap_or(u32::MAX),
                support: ProfileSupport::Supported,
            },
            ProfileOutcome::Unsupported { reason } => ProfileView {
                index: p.index,
                name: p.name.clone(),
                server_count: 0,
                support: ProfileSupport::Unsupported {
                    reason: reason.clone(),
                },
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_variants() {
        assert_eq!(decode_base64("SGVsbG8="), Some(b"Hello".to_vec()));
        assert_eq!(decode_base64("SGVsbG8"), Some(b"Hello".to_vec()));
        assert_eq!(decode_base64("a-_b"), decode_base64("a+/b"));
        assert_eq!(decode_base64("!!"), None);
    }
}
