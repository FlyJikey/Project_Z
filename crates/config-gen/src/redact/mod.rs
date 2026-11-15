//! Маскирование секретов для журнала (ТЗ §14) и очистка недоверенных строк.
//!
//! Чистые функции без зависимостей. Принцип: лучше замаскировать лишнее, чем
//! пропустить секрет. Основные случаи: `user:pass@` в URL, токен в пути и
//! параметрах адреса подписки, UUID, значения полей `password=`, `token=` и т.п.

use std::fmt::Write as _;

/// Подстановка вместо секрета.
pub const MASK: &str = "***";

/// Имена полей, значения которых считаются секретом (сравнение без регистра).
const SECRET_KEYS: &[&str] = &[
    "password",
    "passwd",
    "pass",
    "pwd",
    "token",
    "secret",
    "auth",
    "authorization",
    "apikey",
    "api_key",
    "api-key",
    "privatekey",
    "private_key",
    "publickey",
    "public_key",
    "uuid",
    "shortid",
    "sub",
    "key",
];

/// Логин в скрытом виде: первые два символа и `***` (`us***`).
pub fn mask_login(login: &str) -> String {
    let head: String = login.chars().take(2).collect();
    format!("{head}{MASK}")
}

/// Убирает управляющие символы (защита журнала от подмены строк) и обрезает
/// строку до `max_chars` символов.
pub fn sanitize_for_log(s: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (n, c) in s.chars().enumerate() {
        if n >= max_chars {
            out.push('…');
            break;
        }
        if c.is_control() {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

/// Идентификатор из недоверенного ввода (имя поля, тег, протокол) в безопасном
/// для журнала виде: только `[A-Za-z0-9_.:-]`, остальное — `?`, до 48 символов.
pub fn sanitize_ident(s: &str) -> String {
    let mut out = String::new();
    for (n, c) in s.chars().enumerate() {
        if n >= 48 {
            out.push('…');
            break;
        }
        if c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':' | '-') {
            out.push(c);
        } else {
            out.push('?');
        }
    }
    out
}

/// Маскирует UUID (8-4-4-4-12 hex) в тексте.
pub fn mask_uuids(text: &str) -> String {
    let b = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut last = 0;
    while i < b.len() {
        if i + 36 <= b.len() && is_uuid_at(b, i) {
            let before_ok = i == 0 || !b[i - 1].is_ascii_alphanumeric();
            let after_ok = i + 36 == b.len() || !b[i + 36].is_ascii_alphanumeric();
            if before_ok && after_ok {
                out.push_str(&text[last..i]);
                out.push_str(MASK);
                i += 36;
                last = i;
                continue;
            }
        }
        i += 1;
    }
    out.push_str(&text[last..]);
    out
}

fn is_uuid_at(b: &[u8], i: usize) -> bool {
    for k in 0..36 {
        let c = b[i + k];
        let ok = if matches!(k, 8 | 13 | 18 | 23) {
            c == b'-'
        } else {
            c.is_ascii_hexdigit()
        };
        if !ok {
            return false;
        }
    }
    true
}

/// Маскирует `user:pass@` в одном URL (схема сохраняется). Если разобрать не
/// удалось — возвращает безопасную заглушку.
pub fn redact_url_userinfo(url: &str) -> String {
    match find_userinfo(url) {
        Some((start, end)) => format!("{}{MASK}{}", &url[..start], &url[end..]),
        None => url.to_string(),
    }
}

/// Диапазон `userinfo` (без `@`) в URL вида `scheme://userinfo@host/...`.
fn find_userinfo(url: &str) -> Option<(usize, usize)> {
    let scheme_end = url.find("://")?;
    let auth_start = scheme_end + 3;
    let rest = &url[auth_start..];
    let auth_len = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let auth = &rest[..auth_len];
    let at = auth.rfind('@')?;
    Some((auth_start, auth_start + at))
}

/// Адрес подписки для журнала и UI: схема и хост остаются, `userinfo`, путь и
/// запрос скрыты (в них живёт токен): `https://host/***`.
pub fn redact_subscription_url(url: &str) -> String {
    let Some(scheme_end) = url.find("://") else {
        return MASK.to_string();
    };
    let auth_start = scheme_end + 3;
    let rest = &url[auth_start..];
    let auth_len = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let auth = &rest[..auth_len];
    let host = auth.rsplit('@').next().unwrap_or("");
    let tail = &rest[auth_len..];
    let mut out = String::new();
    out.push_str(&url[..auth_start]);
    if auth.contains('@') {
        out.push_str(MASK);
        out.push('@');
    }
    out.push_str(host);
    let has_path = tail.len() > 1 && tail.starts_with('/');
    if has_path || tail.starts_with('?') || tail.starts_with('#') {
        out.push_str("/***");
    }
    out
}

/// Маскирует секреты в произвольной строке журнала: URL (`userinfo`, путь и
/// запрос у http/https), UUID, значения полей из [`SECRET_KEYS`].
pub fn redact_text(text: &str) -> String {
    let step1 = redact_urls_in_text(text);
    let step2 = mask_uuids(&step1);
    mask_key_values(&step2)
}

fn redact_urls_in_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find("://") {
        // Начало схемы: буквы/цифры/+-. перед «://».
        let scheme_start = rest[..pos]
            .rfind(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')))
            .map_or(0, |p| p + 1);
        let after = pos + 3;
        let url_len = rest[after..]
            .find(|c: char| {
                c.is_whitespace() || matches!(c, '"' | '\'' | '<' | '>' | '`' | ')' | ',')
            })
            .unwrap_or(rest.len() - after);
        let url_end = after + url_len;
        out.push_str(&rest[..scheme_start]);
        let url = &rest[scheme_start..url_end];
        let scheme = &rest[scheme_start..pos];
        if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https") {
            out.push_str(&redact_subscription_url(url));
        } else {
            out.push_str(&redact_url_userinfo(url));
        }
        rest = &rest[url_end..];
    }
    out.push_str(rest);
    out
}

fn mask_key_values(text: &str) -> String {
    let b = text.as_bytes();
    let lower: Vec<u8> = b.iter().map(u8::to_ascii_lowercase).collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut copied = 0;
    while i < b.len() {
        let boundary = i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_');
        if boundary && let Some(klen) = key_at(&lower, i) {
            let mut j = i + klen;
            // Закрывающая кавычка ключа и пробелы до разделителя.
            if j < b.len() && matches!(b[j], b'"' | b'\'') {
                j += 1;
            }
            while j < b.len() && b[j] == b' ' {
                j += 1;
            }
            if j < b.len() && matches!(b[j], b'=' | b':') {
                j += 1;
                while j < b.len() && b[j] == b' ' {
                    j += 1;
                }
                let quoted = j < b.len() && matches!(b[j], b'"' | b'\'');
                if quoted {
                    j += 1;
                }
                let vstart = j;
                while j < b.len() {
                    let c = b[j];
                    let stop = if quoted {
                        matches!(c, b'"' | b'\'')
                    } else {
                        c.is_ascii_whitespace()
                            || matches!(c, b'&' | b',' | b';' | b'}' | b']' | b')')
                    };
                    if stop {
                        break;
                    }
                    j += 1;
                }
                if j > vstart && !text[vstart..j].starts_with(MASK) {
                    out.push_str(&text[copied..vstart]);
                    out.push_str(MASK);
                    copied = j;
                }
                i = j.max(i + 1);
                continue;
            }
        }
        i += 1;
    }
    out.push_str(&text[copied..]);
    out
}

/// Длина ключа из списка секретных, начинающегося в позиции `i`.
fn key_at(lower: &[u8], i: usize) -> Option<usize> {
    SECRET_KEYS
        .iter()
        .filter(|k| lower[i..].starts_with(k.as_bytes()))
        .map(|k| k.len())
        .max()
}

/// Короткий отпечаток для журнала: `abcd…` без раскрытия значения.
pub fn fingerprint_hint(secret: &str) -> String {
    let mut s = String::new();
    let _ = write!(s, "<{} симв.>", secret.chars().count());
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const UUID: &str = "4c56aecb-bdc2-4f74-930f-077140af72cb";

    #[test]
    fn masks_userinfo() {
        assert_eq!(
            redact_url_userinfo("socks5://USER:PASS@1.2.3.4:8000"),
            "socks5://***@1.2.3.4:8000"
        );
        assert_eq!(
            redact_url_userinfo("socks5://1.2.3.4:8000"),
            "socks5://1.2.3.4:8000"
        );
    }

    #[test]
    fn masks_subscription_token() {
        assert_eq!(
            redact_subscription_url("https://example.com/sub/TOKEN?x=1"),
            "https://example.com/***"
        );
        assert_eq!(
            redact_subscription_url("https://u:p@example.com:8443/a"),
            "https://***@example.com:8443/***"
        );
        assert_eq!(
            redact_subscription_url("https://example.com/"),
            "https://example.com"
        );
        assert_eq!(redact_subscription_url("garbage"), "***");
    }

    #[test]
    fn masks_uuid() {
        assert_eq!(mask_uuids(&format!("id={UUID}.")), "id=***.");
        assert_eq!(
            mask_uuids("4c56aecb-bdc2-4f74-930f-077140af72cbX"),
            "4c56aecb-bdc2-4f74-930f-077140af72cbX"
        );
    }

    #[test]
    fn redacts_text() {
        let line = format!(
            "fetch https://example.com/sub/SECRETTOKEN failed; proxy socks5://USER:PASS@1.2.3.4:8000 id {UUID} password=hunter2 \"token\":\"abc\""
        );
        let r = redact_text(&line);
        for leak in ["SECRETTOKEN", "USER", "PASS", UUID, "hunter2", "abc"] {
            assert!(!r.contains(leak), "{leak} leaked in {r}");
        }
        assert!(r.contains("example.com"));
    }

    #[test]
    fn sanitizes() {
        assert_eq!(sanitize_ident("a b\n/c"), "a?b??c");
        assert_eq!(sanitize_for_log("a\nb", 10), "a b");
        assert_eq!(mask_login("username"), "us***");
    }
}
