//! Проверка адреса подписки и перенаправлений (ТЗ §5.1).

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use ipnet::{Ipv4Net, Ipv6Net};
use url::{Host, Url};

const MAX_URL_LEN: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum UrlError {
    /// `happ://crypt…` — зашифрованные ссылки Happ не поддерживаются (ТЗ §5.2).
    #[error(
        "зашифрованные ссылки Happ (happ://crypt) не поддерживаются: используйте обычную ссылку подписки https://"
    )]
    HappEncrypted,
    #[error("адрес подписки не разобран")]
    Invalid,
    #[error("адрес подписки должен начинаться с https://")]
    NotHttps,
    #[error("адрес подписки не должен содержать логин и пароль")]
    HasCredentials,
    #[error("в адресе подписки нет хоста")]
    NoHost,
    #[error("перенаправление на локальный или частный адрес запрещено")]
    LocalTarget,
}

/// Проверяет адрес подписки: только `https`, без `userinfo`, с хостом.
pub fn parse_subscription_url(s: &str) -> Result<Url, UrlError> {
    let t = s.trim();
    if t.len() >= 12 && t[..12].eq_ignore_ascii_case("happ://crypt") {
        return Err(UrlError::HappEncrypted);
    }
    if t.len() > MAX_URL_LEN {
        return Err(UrlError::Invalid);
    }
    let u = Url::parse(t).map_err(|_| UrlError::Invalid)?;
    check_scheme_and_host(&u)?;
    Ok(u)
}

fn check_scheme_and_host(u: &Url) -> Result<(), UrlError> {
    if u.scheme() != "https" {
        return Err(UrlError::NotHttps);
    }
    if !u.username().is_empty() || u.password().is_some() {
        return Err(UrlError::HasCredentials);
    }
    if u.host().is_none() {
        return Err(UrlError::NoHost);
    }
    Ok(())
}

/// Проверка цели перенаправления: `https`, без `userinfo`, хост не локальный и
/// не частный. Для доменных имён проверяются только имена-маркеры; адреса,
/// в которые домен резолвится, вызывающий обязан проверить [`is_non_public_ip`].
pub fn check_redirect_target(u: &Url) -> Result<(), UrlError> {
    check_scheme_and_host(u)?;
    match u.host() {
        Some(h) if is_local_or_private_host(&h) => Err(UrlError::LocalTarget),
        Some(_) => Ok(()),
        None => Err(UrlError::NoHost),
    }
}

/// Хост локальный или частный: IP-литерал из непубличного диапазона либо имя
/// вида `localhost`, `*.localhost`, `*.local`, `*.internal`, `*.lan`,
/// `*.home.arpa` или имя без точки.
pub fn is_local_or_private_host(host: &Host<&str>) -> bool {
    match host {
        Host::Ipv4(a) => is_non_public_ip(IpAddr::V4(*a)),
        Host::Ipv6(a) => is_non_public_ip(IpAddr::V6(*a)),
        Host::Domain(d) => {
            let d = d.trim_end_matches('.').to_ascii_lowercase();
            d.is_empty()
                || !d.contains('.')
                || d == "localhost"
                || [
                    ".localhost",
                    ".local",
                    ".internal",
                    ".lan",
                    ".home.arpa",
                    ".localdomain",
                ]
                .iter()
                .any(|s| d.ends_with(s))
        }
    }
}

fn v4_nets() -> Vec<Ipv4Net> {
    [
        "0.0.0.0/8",
        "10.0.0.0/8",
        "100.64.0.0/10",
        "127.0.0.0/8",
        "169.254.0.0/16",
        "172.16.0.0/12",
        "192.0.0.0/24",
        "192.0.2.0/24",
        "192.168.0.0/16",
        "198.18.0.0/15",
        "198.51.100.0/24",
        "203.0.113.0/24",
        "224.0.0.0/4",
        "240.0.0.0/4",
    ]
    .iter()
    .filter_map(|s| s.parse().ok())
    .collect()
}

fn v6_nets() -> Vec<Ipv6Net> {
    [
        "::/128",
        "::1/128",
        "fc00::/7",
        "fe80::/10",
        "ff00::/8",
        "2001:db8::/32",
        "100::/64",
    ]
    .iter()
    .filter_map(|s| s.parse().ok())
    .collect()
}

/// Адрес не является публичным: loopback, частный, link-local, CGNAT,
/// документационный, multicast, зарезервированный, `198.18.0.0/15` (пул TUN).
/// IPv4, вложенный в IPv6 (`::ffff:a.b.c.d`, `64:ff9b::/96`), проверяется как IPv4.
pub fn is_non_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(a) => v4_nets().iter().any(|n| n.contains(&a)),
        IpAddr::V6(a) => {
            if let Some(v4) = embedded_v4(a) {
                return is_non_public_ip(IpAddr::V4(v4));
            }
            v6_nets().iter().any(|n| n.contains(&a))
        }
    }
}

fn embedded_v4(a: Ipv6Addr) -> Option<Ipv4Addr> {
    let o = a.octets();
    let mapped = o[..10].iter().all(|b| *b == 0) && o[10] == 0xff && o[11] == 0xff;
    let nat64 = o[..4] == [0x00, 0x64, 0xff, 0x9b] && o[4..12].iter().all(|b| *b == 0);
    (mapped || nat64).then(|| Ipv4Addr::new(o[12], o[13], o[14], o[15]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_ips() {
        for s in [
            "127.0.0.1",
            "10.1.2.3",
            "192.168.0.1",
            "172.20.0.1",
            "169.254.1.1",
            "100.64.0.1",
            "198.18.0.5",
            "::1",
            "fe80::1",
            "fd00::1",
            "::ffff:10.0.0.1",
            "0.0.0.0",
        ] {
            assert!(
                is_non_public_ip(s.parse().unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))),
                "{s}"
            );
        }
        for s in ["8.8.8.8", "1.1.1.1", "2606:4700::1111", "172.32.0.1"] {
            assert!(
                !is_non_public_ip(s.parse().unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))),
                "{s}"
            );
        }
    }
}
