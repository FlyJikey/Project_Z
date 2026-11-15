//! Общие варианты входа для tests/singbox_*.rs (подключается через `#[path]`).
//! Все значения — плейсхолдеры и адреса из документационных диапазонов.
#![allow(dead_code, clippy::expect_used, clippy::unwrap_used)]

use std::net::IpAddr;

use config_gen::singbox::{
    CachePath, ClashApi, LogLevel, ProxyId, ProxyPool, SingboxInput, VpnServerIps, XraySocks,
};
use ipc_proto::{ProxyEndpoint, ProxyScheme, Secret};

pub const XRAY_PASSWORD: &str = "__XRAY_SOCKS_PASSWORD__";
pub const CLASH_SECRET: &str = "__CLASH_API_SECRET_0123456789__";
pub const PROXY1_PASSWORD: &str = "__PROXY1_PASSWORD__";
pub const PROXY2_PASSWORD: &str = "__PROXY2_PASSWORD__";

pub fn id(s: &str) -> ProxyId {
    ProxyId::new(s).expect("id")
}

pub fn socks(host: &str, port: u16, auth: Option<(&str, &str)>) -> ProxyEndpoint {
    ProxyEndpoint {
        scheme: ProxyScheme::Socks5,
        host: host.to_owned(),
        port,
        username: auth.map(|(u, _)| u.to_owned()),
        password: auth.map(|(_, p)| Secret::new(p)),
    }
}

pub fn http(host: &str, port: u16, auth: Option<(&str, &str)>) -> ProxyEndpoint {
    ProxyEndpoint {
        scheme: ProxyScheme::Http,
        ..socks(host, port, auth)
    }
}

pub fn ips(list: &[&str]) -> VpnServerIps {
    VpnServerIps::new(list.iter().map(|s| s.parse::<IpAddr>().expect("ip"))).expect("vpn ips")
}

/// Базовый вход: один socks5-прокси, LAN вкл, UDP выкл.
pub fn base() -> SingboxInput {
    SingboxInput {
        pool: ProxyPool::new(vec![(
            id("p1"),
            socks(
                "198.51.100.11",
                1080,
                Some(("__PROXY1_USER__", PROXY1_PASSWORD)),
            ),
        )])
        .expect("pool"),
        xray: XraySocks::new(10808, "__XRAY_SOCKS_USER__", Secret::new(XRAY_PASSWORD))
            .expect("xray"),
        vpn_server_ips: ips(&["203.0.113.10"]),
        allow_lan: true,
        udp_allowed: false,
        clash_api: ClashApi::new(9090, Secret::new(CLASH_SECRET)).expect("clash"),
        cache_path: CachePath::new("/var/lib/projectz/sing-box-cache.db").expect("path"),
        log_level: LogLevel::Info,
    }
}

pub fn with_pool(items: Vec<(ProxyId, ProxyEndpoint)>) -> SingboxInput {
    SingboxInput {
        pool: ProxyPool::new(items).expect("pool"),
        ..base()
    }
}

/// Именованные варианты для проверки ядром и структурных тестов.
pub fn variants() -> Vec<(&'static str, SingboxInput)> {
    let two = vec![
        (
            id("p1"),
            socks(
                "198.51.100.11",
                1080,
                Some(("__PROXY1_USER__", PROXY1_PASSWORD)),
            ),
        ),
        (
            id("p2"),
            http(
                "198.51.100.12",
                8080,
                Some(("__PROXY2_USER__", PROXY2_PASSWORD)),
            ),
        ),
    ];
    let many_ips: Vec<String> = (1..=60)
        .map(|i| format!("203.0.113.{i}"))
        .chain((1..=20).map(|i| format!("2001:db8:1::{i:x}")))
        .collect();
    let many_refs: Vec<&str> = many_ips.iter().map(String::as_str).collect();

    vec![
        ("one_socks5", base()),
        (
            "one_socks5_no_auth",
            with_pool(vec![(id("p1"), socks("198.51.100.11", 1080, None))]),
        ),
        ("two_socks5_http", with_pool(two)),
        (
            "domain_proxy",
            with_pool(vec![(
                id("p1"),
                socks("Proxy1.Example.NET", 1080, Some(("u", "pppppppp"))),
            )]),
        ),
        (
            "ipv6_proxy",
            with_pool(vec![(id("p1"), socks("2001:db8::11", 1080, None))]),
        ),
        (
            "lan_off",
            SingboxInput {
                allow_lan: false,
                ..base()
            },
        ),
        (
            "udp_on",
            SingboxInput {
                udp_allowed: true,
                ..base()
            },
        ),
        (
            "udp_on_lan_off_debug",
            SingboxInput {
                udp_allowed: true,
                allow_lan: false,
                log_level: LogLevel::Debug,
                ..base()
            },
        ),
        (
            "many_vpn_ips",
            SingboxInput {
                vpn_server_ips: ips(&many_refs),
                ..base()
            },
        ),
        (
            "windows_cache_path",
            SingboxInput {
                cache_path: CachePath::new("C:\\ProgramData\\ProjectZ\\sing-box-cache.db")
                    .expect("path"),
                ..base()
            },
        ),
    ]
}
