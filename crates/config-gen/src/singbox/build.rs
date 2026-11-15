//! Сборка конфига sing-box 1.14.4 (схема — справка §8, ТЗ §6–§8).

use std::net::IpAddr;

use ipc_proto::ProxyScheme;
use serde_json::{Map, Value, json};

use super::clash::{SELECTOR_TAG, TUN_INBOUND_TAG, XRAY_OUTBOUND_TAG, proxy_tag};
use super::error::SingboxError;
use super::input::{PoolEntry, SingboxInput};

const DNS_DOH_TAG: &str = "doh";
const DNS_FAKEIP_TAG: &str = "fakeip";

/// Адреса интерфейса TUN. IPv6-адрес нужен, чтобы TUN забрал весь IPv6 (ТЗ §8).
const TUN_ADDRESS_V4: &str = "172.19.0.1/30";
const TUN_ADDRESS_V6: &str = "fdfe:dcba:9876::1/126";
const TUN_MTU: u16 = 1500;
/// Диапазон fake-ip (только IPv4: AAAA отвечается пустым NOERROR).
const FAKEIP_RANGE_V4: &str = "198.18.0.0/15";
/// Запасной DoH-резолвер (IP, чтобы не нужен `domain_resolver`); идёт через цепочку.
const DOH_SERVER: &str = "8.8.8.8";

/// Частные префиксы, исключаемые из TUN при «разрешить локальную сеть» (ТЗ §7).
const LAN_PREFIXES: [&str; 6] = [
    "10.0.0.0/8",
    "172.16.0.0/12",
    "192.168.0.0/16",
    "169.254.0.0/16",
    "fc00::/7",
    "fe80::/10",
];

/// Собирает конфиг sing-box как `serde_json::Value`.
///
/// Ошибки перекрёстных проверок: совпадение портов Xray и Clash API;
/// разрешённый UDP при наличии http-прокси в пуле.
pub fn build_config(input: &SingboxInput) -> Result<Value, SingboxError> {
    if input.xray.port() == input.clash_api.port() {
        return Err(SingboxError::PortConflict);
    }
    if input.udp_allowed && input.pool.has_http() {
        return Err(SingboxError::UdpWithHttpProxy);
    }

    let mut route_rules = vec![
        json!({ "action": "sniff" }),
        json!({ "protocol": "dns", "action": "hijack-dns" }),
        json!({ "ip_version": 6, "action": "reject" }),
    ];
    if !input.udp_allowed {
        // После hijack-dns: иначе отклонился бы и DNS по UDP/53.
        route_rules.push(json!({ "network": "udp", "action": "reject" }));
    }

    Ok(json!({
        "log": { "level": input.log_level.as_str(), "timestamp": true },
        "dns": dns_section(),
        "inbounds": [tun_inbound(input)],
        "outbounds": outbounds(input),
        "route": {
            "rules": route_rules,
            "final": SELECTOR_TAG,
            // См. документацию к модулю: outbound'ы sing-box ходят только на loopback.
            "auto_detect_interface": false,
        },
        "experimental": {
            "cache_file": {
                "enabled": true,
                "path": input.cache_path.as_str(),
                "store_fakeip": true,
            },
            "clash_api": {
                "external_controller": format!("127.0.0.1:{}", input.clash_api.port()),
                "secret": input.clash_api.secret(),
            },
        },
    }))
}

/// Конфиг строкой (JSON с отступами, перевод строки в конце). Содержит секреты:
/// в журнал не писать.
pub fn render_config(input: &SingboxInput) -> Result<String, SingboxError> {
    let value = build_config(input)?;
    // Сериализация Value не может упасть (ключи — строки); на всякий случай
    // без паники отдаём компактный вид.
    let mut text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
    text.push('\n');
    Ok(text)
}

fn dns_section() -> Value {
    json!({
        "servers": [
            // Запасной DoH обязательно через detour: без него сервер ходил бы напрямую.
            { "type": "https", "tag": DNS_DOH_TAG, "server": DOH_SERVER, "detour": SELECTOR_TAG },
            { "type": "fakeip", "tag": DNS_FAKEIP_TAG, "inet4_range": FAKEIP_RANGE_V4 },
        ],
        "rules": [
            { "query_type": ["AAAA"], "action": "predefined", "rcode": "NOERROR" },
            { "query_type": ["A"], "action": "route", "server": DNS_FAKEIP_TAG },
        ],
        "final": DNS_DOH_TAG,
        "strategy": "ipv4_only",
    })
}

fn cidr(ip: &IpAddr) -> String {
    match ip {
        IpAddr::V4(_) => format!("{ip}/32"),
        IpAddr::V6(_) => format!("{ip}/128"),
    }
}

fn tun_inbound(input: &SingboxInput) -> Value {
    let mut exclude: Vec<String> = input.vpn_server_ips.as_slice().iter().map(cidr).collect();
    if input.allow_lan {
        exclude.extend(LAN_PREFIXES.iter().map(|s| (*s).to_owned()));
    }
    json!({
        "type": "tun",
        "tag": TUN_INBOUND_TAG,
        "address": [TUN_ADDRESS_V4, TUN_ADDRESS_V6],
        "mtu": TUN_MTU,
        "auto_route": true,
        "strict_route": true,
        "stack": "system",
        // Системный DNS не меняем (ТЗ §8): DNS перехватывает правило hijack-dns.
        "dns_mode": "disabled",
        "route_exclude_address": exclude,
    })
}

fn outbounds(input: &SingboxInput) -> Value {
    let mut list = vec![json!({
        "type": "socks",
        "tag": XRAY_OUTBOUND_TAG,
        "server": "127.0.0.1",
        "server_port": input.xray.port(),
        "version": "5",
        "username": input.xray.username(),
        "password": input.xray.password(),
    })];
    let entries = input.pool.entries();
    list.extend(entries.iter().map(|e| proxy_outbound(e, input.udp_allowed)));
    let tags: Vec<String> = entries.iter().map(|e| proxy_tag(e.id())).collect();
    let default = tags.first().cloned().unwrap_or_default();
    list.push(json!({
        "type": "selector",
        "tag": SELECTOR_TAG,
        "outbounds": tags,
        "default": default,
        "interrupt_exist_connections": true,
    }));
    Value::Array(list)
}

/// Прокси пользователя: всегда с `detour` на Xray и без `domain_resolver`
/// (имя уходит в цепочку как есть, справка §4.5, §8.2).
fn proxy_outbound(e: &PoolEntry, udp_allowed: bool) -> Value {
    let mut o = Map::new();
    let kind = match e.scheme() {
        ProxyScheme::Socks5 => "socks",
        ProxyScheme::Http => "http",
    };
    o.insert("type".into(), json!(kind));
    o.insert("tag".into(), json!(proxy_tag(e.id())));
    o.insert("server".into(), json!(e.host()));
    o.insert("server_port".into(), json!(e.port()));
    if e.scheme() == ProxyScheme::Socks5 {
        // `version` у socks — строка (справка §8.3).
        o.insert("version".into(), json!("5"));
        if !udp_allowed {
            // Подстраховка к правилу reject UDP в route.
            o.insert("network".into(), json!("tcp"));
        }
    }
    if let Some((user, pass)) = e.auth() {
        o.insert("username".into(), json!(user));
        o.insert("password".into(), json!(pass));
    }
    o.insert("detour".into(), json!(XRAY_OUTBOUND_TAG));
    Value::Object(o)
}
