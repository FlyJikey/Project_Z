//! Проверки входных значений. Чистые функции без обращения к сети.

use std::net::IpAddr;

/// Приводит IPv4-mapped IPv6 к IPv4, чтобы проверки не обходились записью `::ffff:a.b.c.d`.
pub(crate) fn canonical_ip(ip: IpAddr) -> IpAddr {
    ip.to_canonical()
}

/// IP, который не может быть адресом внешнего сервера.
pub(crate) fn is_unusable_ip(ip: IpAddr) -> bool {
    let ip = canonical_ip(ip);
    ip.is_unspecified() || ip.is_loopback() || ip.is_multicast()
}

/// Допустимый идентификатор записи пула: 1..=64 символов `[A-Za-z0-9_-]`.
/// Из него строится тег outbound, поэтому набор символов узкий.
pub(crate) fn is_valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// Проверяет и нормализует адрес прокси. Возвращает строку для поля `server`
/// (IP в каноническом виде без скобок, домен в нижнем регистре) либо `None`.
///
/// Домены только ASCII (IDN — в punycode), без точки в конце. Адрес вида
/// `localhost`, loopback, нулевой и multicast отклоняются: прокси должен быть
/// внешним и достигаться через цепочку.
pub(crate) fn normalize_host(raw: &str) -> Option<String> {
    let bare = match (raw.strip_prefix('['), raw.ends_with(']')) {
        (Some(inner), true) => {
            let inner = &inner[..inner.len() - 1];
            // В скобках допустим только IPv6.
            return match inner.parse::<IpAddr>() {
                Ok(ip @ IpAddr::V6(_)) if !is_unusable_ip(ip) => Some(ip.to_string()),
                _ => None,
            };
        }
        _ => raw,
    };

    if let Ok(ip) = bare.parse::<IpAddr>() {
        return if is_unusable_ip(ip) {
            None
        } else {
            Some(canonical_ip(ip).to_string())
        };
    }

    let domain = bare.to_ascii_lowercase();
    if domain.is_empty() || domain.len() > 253 {
        return None;
    }
    let mut last_label = "";
    for label in domain.split('.') {
        if label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return None;
        }
        last_label = label;
    }
    // Последняя метка из одних цифр — это кривой IP вроде `1.2.3` или `999.1.1.1`.
    if last_label.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if domain == "localhost" || domain.ends_with(".localhost") {
        return None;
    }
    Some(domain)
}

/// Логин/пароль: оба заданы и непусты, либо оба отсутствуют.
pub(crate) fn check_credentials(
    scheme_is_http: bool,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<(), &'static str> {
    match (username, password) {
        (None, None) => Ok(()),
        (Some(_), None) => Err("задан логин без пароля"),
        (None, Some(_)) => Err("задан пароль без логина"),
        (Some(u), Some(p)) => {
            if u.is_empty() {
                return Err("логин пустой");
            }
            if p.is_empty() {
                return Err("пароль пустой");
            }
            if u.len() > 255 || p.len() > 255 {
                return Err("логин и пароль не длиннее 255 байт");
            }
            if u.chars().any(char::is_control) || p.chars().any(char::is_control) {
                return Err("логин или пароль содержит управляющие символы");
            }
            if scheme_is_http && u.contains(':') {
                return Err("логин http-прокси не может содержать ':'");
            }
            Ok(())
        }
    }
}

/// Абсолютный путь в стиле POSIX или Windows. Проверка своя, а не
/// `Path::is_absolute`: та зависит от ОС, на которой запущена сборка.
pub(crate) fn check_absolute_path(path: &str) -> Result<(), &'static str> {
    if path.is_empty() {
        return Err("путь пустой");
    }
    if path.len() > 1024 {
        return Err("путь слишком длинный");
    }
    if path.chars().any(char::is_control) {
        return Err("путь содержит управляющие символы");
    }
    let b = path.as_bytes();
    let posix = b[0] == b'/';
    let drive =
        b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'\\' | b'/');
    let unc = path.starts_with("\\\\");
    if !(posix || drive || unc) {
        return Err("путь должен быть абсолютным");
    }
    if path.ends_with('/') || path.ends_with('\\') {
        return Err("путь указывает на каталог, нужен файл");
    }
    if path.split(['/', '\\']).any(|part| part == "..") {
        return Err("путь содержит '..'");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hosts() {
        assert_eq!(
            normalize_host("198.51.100.1").as_deref(),
            Some("198.51.100.1")
        );
        assert_eq!(
            normalize_host("[2001:db8::1]").as_deref(),
            Some("2001:db8::1")
        );
        assert_eq!(
            normalize_host("2001:db8::1").as_deref(),
            Some("2001:db8::1")
        );
        assert_eq!(
            normalize_host("Proxy.Example.NET").as_deref(),
            Some("proxy.example.net")
        );
        assert_eq!(
            normalize_host("xn--e1afmkfd.xn--p1ai").as_deref(),
            Some("xn--e1afmkfd.xn--p1ai")
        );
        for bad in [
            "",
            " ",
            "a b",
            "user:pass@host",
            "host:1080",
            "http://h",
            "h/p",
            "a..b",
            "a.b.",
            "-a.com",
            "a-.com",
            "пример.рф",
            "1.2.3",
            "999.1.1.1",
            "localhost",
            "x.localhost",
            "127.0.0.1",
            "0.0.0.0",
            "::1",
            "::",
            "224.0.0.1",
            "::ffff:127.0.0.1",
            "[1.2.3.4]",
            "[host]",
            "fe80::1%en0",
            "a_b.com",
            "a\u{0}b",
        ] {
            assert!(
                normalize_host(bad).is_none(),
                "должен быть отклонён: {bad:?}"
            );
        }
        assert!(normalize_host(&"a".repeat(64)).is_none());
        assert!(normalize_host(&format!("{}.com", "a".repeat(63))).is_some());
    }

    #[test]
    fn ids() {
        assert!(is_valid_id("a1_-Z"));
        for bad in ["", "a b", "а", "a/b", "a.b", &"x".repeat(65)] {
            assert!(!is_valid_id(bad));
        }
    }

    #[test]
    fn paths() {
        for ok in [
            "/var/lib/p/c.db",
            "C:\\ProgramData\\p\\c.db",
            "D:/p/c.db",
            "\\\\srv\\s\\c.db",
        ] {
            assert!(check_absolute_path(ok).is_ok(), "{ok}");
        }
        for bad in ["", "c.db", "./c.db", "/a/../b", "/a/", "C:c.db", "/a\nb"] {
            assert!(check_absolute_path(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn credentials() {
        assert!(check_credentials(false, None, None).is_ok());
        assert!(check_credentials(false, Some("u"), None).is_err());
        assert!(check_credentials(false, None, Some("p")).is_err());
        assert!(check_credentials(false, Some(""), Some("p")).is_err());
        assert!(check_credentials(false, Some("u"), Some("")).is_err());
        assert!(check_credentials(true, Some("a:b"), Some("p")).is_err());
        assert!(check_credentials(false, Some("a:b"), Some("p")).is_ok());
    }
}
