//! Сборка конфига Xray 26.11.9 из разобранного профиля (ТЗ §4, §5.1;
//! справка `docs/reference/xray.md`, §9–§10).
//!
//! Конфиг строится заново из строгих типов [`crate::subscription::model`];
//! ничего из исходного JSON подписки в него не попадает.
//!
//! Решения:
//! - **Адреса → IP.** Домен сервера заменяется IP из таблицы помощника
//!   ([`ResolvedHosts`]); исходный домен явно пишется в `serverName` (TLS и
//!   REALITY) и в `host`/`authority` транспорта, если в подписке они пусты.
//! - **Несколько IP у домена.** Один outbound на каждый IP (не больше
//!   [`MAX_IPS_PER_DOMAIN`]), все — члены балансировщика. Так Xray сам
//!   отсеивает недоступный адрес по пробам observatory, а IP-набор для
//!   брандмауэра и исключений TUN совпадает с тем, что реально используется.
//!   Промежуточные звенья цепочки (`dialerProxy` на прокси-outbound) берут
//!   только первый IP: размножать цепочки декартовым произведением нельзя.
//! - **Свои теги.** `pz-srv-<k>-<j>` (серверы), `pz-hop-<n>` (звенья
//!   цепочки), `pz-dial-<n>` (freedom-диалеры). Селектор `pz-srv-` не
//!   зацепляет служебные outbound'ы. Первым в списке идёт первый сервер.
//! - **`sockopt.interface`** — на каждом outbound'е, который сам открывает
//!   соединение, и не на тех, у кого есть `dialerProxy`.
//! - Блока `dns`, `api`, `policy` нет; `proxySettings`/`allowInsecure` не
//!   пишутся; `fingerprint` всегда явный; лог без адресов пользователя.

use std::collections::{BTreeMap, BTreeSet};
use std::net::{IpAddr, Ipv4Addr};

use ipc_proto::Secret;
use serde_json::{Map, Value, json};

use crate::subscription::{
    FreedomDialer, GrpcSettings, HttpUpgradeSettings, Node, OutboundSpec, ProfileConfig,
    ProxyOutbound, ProxyProtocol, Security, Stream, Transport, WsSettings, XhttpSettings,
};

/// Таблица «домен (нижний регистр) → IPv4» от резолвера помощника.
pub type ResolvedHosts = BTreeMap<String, Vec<Ipv4Addr>>;

/// Не больше стольких IP на один домен.
pub const MAX_IPS_PER_DOMAIN: usize = 8;
/// Адрес проб по умолчанию (ответ 204).
pub const DEFAULT_PROBE_URL: &str = "https://connectivitycheck.gstatic.com/generate_204";

pub const INBOUND_TAG: &str = "pz-in";
pub const BALANCER_TAG: &str = "pz-bal";
/// Префикс тегов серверов-членов балансировщика.
pub const SERVER_TAG_PREFIX: &str = "pz-srv-";

/// Параметры, которые приходят от помощника, а не из подписки.
pub struct XrayParams<'a> {
    /// Порт socks-inbound на 127.0.0.1.
    pub socks_port: u16,
    pub socks_user: &'a str,
    pub socks_password: &'a Secret,
    /// Имя физического интерфейса (`en0`, `Wi-Fi`).
    pub interface: &'a str,
    pub resolved: &'a ResolvedHosts,
    /// Адрес проб `burstObservatory`; `None` — [`DEFAULT_PROBE_URL`].
    pub probe_url: Option<&'a str>,
}

/// Готовый конфиг. Содержит секреты, поэтому текст лежит в [`Secret`].
#[derive(Debug, Clone)]
pub struct BuiltConfig {
    /// JSON для `xray run -c stdin:`.
    pub json: Secret,
    /// IP VPN-серверов, которые попали в конфиг (для брандмауэра и TUN).
    pub server_ips: Vec<IpAddr>,
    /// Серверы, пропущенные из-за отсутствия IP в таблице.
    pub skipped: Vec<String>,
    /// Сколько серверных outbound'ов (с учётом размножения по IP).
    pub server_outbounds: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum XrayError {
    #[error("ни у одного сервера профиля нет известного IP")]
    NoServers,
    #[error("недопустимый порт socks-inbound")]
    InvalidPort,
    #[error("недопустимый логин или пароль socks-inbound")]
    InvalidCredentials,
    #[error("недопустимое имя интерфейса")]
    InvalidInterface,
    #[error("недопустимый адрес проб (нужен https:// без логина)")]
    InvalidProbeUrl,
}

/// IP, пригодный как адрес VPN-сервера: не нулевой, не loopback, не multicast.
fn usable_ip(ip: &IpAddr) -> bool {
    !(ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_multicast()
        || matches!(ip, IpAddr::V4(a) if a.is_broadcast()))
}

struct PlannedServer {
    node: usize,
    ip: IpAddr,
    tag: String,
}

struct PlannedDep {
    node: usize,
    /// `None` у freedom.
    ip: Option<IpAddr>,
}

struct Plan {
    servers: Vec<PlannedServer>,
    deps: Vec<PlannedDep>,
    ips: BTreeSet<IpAddr>,
    skipped: Vec<String>,
}

fn node_ips(node: &Node, resolved: &ResolvedHosts) -> Vec<IpAddr> {
    let OutboundSpec::Proxy(p) = &node.spec else {
        return Vec::new();
    };
    let candidates: Vec<IpAddr> = match p.address.parse::<IpAddr>() {
        Ok(ip) => vec![ip],
        Err(_) => resolved
            .get(&p.address.to_ascii_lowercase())
            .map(|v| v.iter().map(|a| IpAddr::V4(*a)).collect())
            .unwrap_or_default(),
    };
    let mut out: Vec<IpAddr> = Vec::new();
    for ip in candidates {
        if usable_ip(&ip) && !out.contains(&ip) && out.len() < MAX_IPS_PER_DOMAIN {
            out.push(ip);
        }
    }
    out
}

/// Звено цепочки пригодно: у прокси есть IP, у freedom всегда; выше по цепочке то же.
fn dep_usable(profile: &ProfileConfig, resolved: &ResolvedHosts, mut node: usize) -> bool {
    for _ in 0..32 {
        let n = &profile.nodes[node];
        if matches!(n.spec, OutboundSpec::Proxy(_)) && node_ips(n, resolved).is_empty() {
            return false;
        }
        match n.dialer {
            Some(d) => node = d,
            None => return true,
        }
    }
    false
}

fn plan(profile: &ProfileConfig, resolved: &ResolvedHosts) -> Plan {
    let mut servers = Vec::new();
    let mut needed: BTreeSet<usize> = BTreeSet::new();
    let mut skipped: Vec<String> = Vec::new();
    let mut note_skip = |s: &str| {
        if !skipped.iter().any(|x| x == s) {
            skipped.push(s.to_string());
        }
    };
    for (k, &e) in profile.entries.iter().enumerate() {
        let node = &profile.nodes[e];
        let name = node.spec.address().unwrap_or("?");
        if let Some(d) = node.dialer
            && !dep_usable(profile, resolved, d)
        {
            note_skip(name);
            continue;
        }
        let ips = node_ips(node, resolved);
        if ips.is_empty() {
            note_skip(name);
            continue;
        }
        for (j, ip) in ips.into_iter().enumerate() {
            servers.push(PlannedServer {
                node: e,
                ip,
                tag: format!("{SERVER_TAG_PREFIX}{k}-{j}"),
            });
        }
        let mut cur = node.dialer;
        while let Some(d) = cur {
            if !needed.insert(d) {
                break;
            }
            cur = profile.nodes[d].dialer;
        }
    }
    let deps: Vec<PlannedDep> = needed
        .into_iter()
        .map(|n| PlannedDep {
            node: n,
            ip: node_ips(&profile.nodes[n], resolved).first().copied(),
        })
        .collect();
    let mut ips: BTreeSet<IpAddr> = servers.iter().map(|s| s.ip).collect();
    ips.extend(deps.iter().filter_map(|d| d.ip));
    Plan {
        servers,
        deps,
        ips,
        skipped,
    }
}

/// Итоговый набор IP VPN-серверов профиля (для брандмауэра и исключений TUN):
/// ровно те адреса, которые попадут в конфиг [`build_config`].
pub fn vpn_server_ips(profile: &ProfileConfig, resolved: &ResolvedHosts) -> Vec<IpAddr> {
    plan(profile, resolved).ips.into_iter().collect()
}

fn dep_tag(profile: &ProfileConfig, node: usize) -> String {
    match profile.nodes[node].spec {
        OutboundSpec::Proxy(_) => format!("pz-hop-{node}"),
        OutboundSpec::Freedom(_) => format!("pz-dial-{node}"),
    }
}

/// Собирает конфиг Xray.
pub fn build_config(
    profile: &ProfileConfig,
    params: &XrayParams<'_>,
) -> Result<BuiltConfig, XrayError> {
    validate_params(params)?;
    let plan = plan(profile, params.resolved);
    let Some(first) = plan.servers.first() else {
        return Err(XrayError::NoServers);
    };
    let iface = params.interface;

    let mut outbounds: Vec<Value> = Vec::new();
    for s in &plan.servers {
        let node = &profile.nodes[s.node];
        let OutboundSpec::Proxy(p) = &node.spec else {
            continue;
        };
        let dialer = node.dialer.map(|d| dep_tag(profile, d));
        outbounds.push(proxy_outbound(&s.tag, p, s.ip, iface, dialer.as_deref()));
    }
    for d in &plan.deps {
        let node = &profile.nodes[d.node];
        let tag = dep_tag(profile, d.node);
        let dialer = node.dialer.map(|x| dep_tag(profile, x));
        match (&node.spec, d.ip) {
            (OutboundSpec::Proxy(p), Some(ip)) => {
                outbounds.push(proxy_outbound(&tag, p, ip, iface, dialer.as_deref()))
            }
            (OutboundSpec::Freedom(fd), _) => {
                outbounds.push(freedom_outbound(&tag, fd, iface, dialer.as_deref()))
            }
            _ => {}
        }
    }

    let probe_url = params.probe_url.unwrap_or(DEFAULT_PROBE_URL);
    let strategy = profile.strategy.as_xray();
    let probe = profile.probe;
    let cfg = json!({
        "log": { "logLevel": "warning", "access": "none", "maskAddress": "full" },
        "inbounds": [{
            "tag": INBOUND_TAG,
            "listen": "127.0.0.1",
            "port": params.socks_port,
            "protocol": "socks",
            "settings": {
                "auth": "password",
                "users": [{ "user": params.socks_user, "pass": params.socks_password.expose() }],
                "udp": true,
                "ip": "127.0.0.1"
            }
        }],
        "outbounds": outbounds,
        "routing": {
            "domainStrategy": "AsIs",
            "rules": [{ "inboundTag": [INBOUND_TAG], "balancerTag": BALANCER_TAG }],
            "balancers": [{
                "tag": BALANCER_TAG,
                "selector": [SERVER_TAG_PREFIX],
                "strategy": { "type": strategy },
                "fallbackTag": first.tag
            }]
        },
        "burstObservatory": {
            "subjectSelector": [SERVER_TAG_PREFIX],
            "pingConfig": {
                "destination": probe_url,
                "connectivity": "",
                "interval": format!("{}s", probe.interval_secs),
                "sampling": probe.sampling,
                "timeout": format!("{}s", probe.timeout_secs),
                "httpMethod": "HEAD"
            }
        }
    });
    let text = serde_json::to_string(&cfg).map_err(|_| XrayError::NoServers)?;
    Ok(BuiltConfig {
        json: Secret::new(text),
        server_ips: plan.ips.iter().copied().collect(),
        skipped: plan.skipped,
        server_outbounds: plan.servers.len(),
    })
}

fn validate_params(p: &XrayParams<'_>) -> Result<(), XrayError> {
    if p.socks_port == 0 {
        return Err(XrayError::InvalidPort);
    }
    let bad =
        |s: &str, max: usize| s.is_empty() || s.len() > max || s.chars().any(char::is_control);
    if bad(p.socks_user, 128) || bad(p.socks_password.expose(), 256) {
        return Err(XrayError::InvalidCredentials);
    }
    if bad(p.interface, 64) {
        return Err(XrayError::InvalidInterface);
    }
    if let Some(u) = p.probe_url {
        let ok = url::Url::parse(u).is_ok_and(|u| {
            u.scheme() == "https"
                && u.host().is_some()
                && u.username().is_empty()
                && u.password().is_none()
        });
        if !ok {
            return Err(XrayError::InvalidProbeUrl);
        }
    }
    Ok(())
}

fn sockopt(tcp: &crate::subscription::TcpOpts, iface: &str, dialer: Option<&str>) -> Value {
    let mut m = Map::new();
    match dialer {
        // У outbound'а с dialerProxy соединение открывает диалер: interface не нужен.
        Some(d) => {
            m.insert("dialerProxy".into(), json!(d));
        }
        None => {
            m.insert("interface".into(), json!(iface));
        }
    }
    if let Some(v) = tcp.tcp_fast_open {
        m.insert("tcpFastOpen".into(), json!(v));
    }
    if let Some(v) = tcp.keep_alive_idle {
        m.insert("tcpKeepAliveIdle".into(), json!(v));
    }
    if let Some(v) = tcp.keep_alive_interval {
        m.insert("tcpKeepAliveInterval".into(), json!(v));
    }
    if let Some(v) = tcp.user_timeout {
        m.insert("tcpUserTimeout".into(), json!(v));
    }
    Value::Object(m)
}

fn freedom_outbound(tag: &str, fd: &FreedomDialer, iface: &str, dialer: Option<&str>) -> Value {
    let mut settings = Map::new();
    if let Some(f) = &fd.fragment {
        // Форма `settings.fragment` помечена устаревшей в 26.11.9, но работает;
        // см. docs/reference/xray.md §4.3. При смене ядра перепроверить.
        settings.insert(
            "fragment".into(),
            json!({ "packets": f.packets, "length": f.length, "interval": f.interval }),
        );
    }
    json!({
        "tag": tag,
        "protocol": "freedom",
        "settings": settings,
        "streamSettings": { "sockopt": sockopt(&Default::default(), iface, dialer) }
    })
}

fn proxy_outbound(
    tag: &str,
    p: &ProxyOutbound,
    ip: IpAddr,
    iface: &str,
    dialer: Option<&str>,
) -> Value {
    let mut s = Map::new();
    s.insert("address".into(), json!(ip.to_string()));
    s.insert("port".into(), json!(p.port));
    let protocol = match &p.protocol {
        ProxyProtocol::Vless {
            id,
            encryption,
            flow,
        } => {
            s.insert("id".into(), json!(id.expose()));
            s.insert("encryption".into(), json!(encryption));
            if let Some(f) = flow {
                s.insert("flow".into(), json!(f));
            }
            "vless"
        }
        ProxyProtocol::Vmess { id, security } => {
            s.insert("id".into(), json!(id.expose()));
            s.insert("security".into(), json!(security));
            "vmess"
        }
        ProxyProtocol::Trojan { password } => {
            s.insert("password".into(), json!(password.expose()));
            "trojan"
        }
        ProxyProtocol::Shadowsocks { method, password } => {
            s.insert("method".into(), json!(method));
            s.insert("password".into(), json!(password.expose()));
            "shadowsocks"
        }
        ProxyProtocol::Hysteria => {
            s.insert("version".into(), json!(2));
            "hysteria"
        }
    };
    let orig_domain = p
        .address
        .parse::<IpAddr>()
        .is_err()
        .then_some(p.address.as_str());
    json!({
        "tag": tag,
        "protocol": protocol,
        "settings": s,
        "streamSettings": stream_settings(&p.stream, orig_domain, iface, dialer)
    })
}

fn put<T: serde::Serialize>(m: &mut Map<String, Value>, key: &str, v: &Option<T>) {
    if let Some(v) = v
        && let Ok(val) = serde_json::to_value(v)
    {
        m.insert(key.into(), val);
    }
}

fn stream_settings(
    st: &Stream,
    orig_domain: Option<&str>,
    iface: &str,
    dialer: Option<&str>,
) -> Value {
    let mut m = Map::new();
    // Имя для SNI: из подписки, иначе исходный домен сервера.
    let sni: Option<String> = match &st.security {
        Security::Tls(t) => t.server_name.clone(),
        Security::Reality(r) => r.server_name.clone(),
        Security::None => None,
    }
    .or_else(|| orig_domain.map(str::to_string));

    let net = match &st.transport {
        Transport::Raw => "raw",
        Transport::Xhttp(_) => "xhttp",
        Transport::Ws(_) => "ws",
        Transport::Grpc(_) => "grpc",
        Transport::HttpUpgrade(_) => "httpupgrade",
        Transport::Hysteria(_) => "hysteria",
    };
    m.insert("network".into(), json!(net));
    match &st.security {
        Security::None => {
            m.insert("security".into(), json!("none"));
        }
        Security::Tls(t) => {
            m.insert("security".into(), json!("tls"));
            let mut tm = Map::new();
            put(&mut tm, "serverName", &sni);
            tm.insert("fingerprint".into(), json!(t.fingerprint));
            let alpn = if t.alpn.is_empty() && matches!(st.transport, Transport::Hysteria(_)) {
                vec!["h3".to_string()]
            } else {
                t.alpn.clone()
            };
            if !alpn.is_empty() {
                tm.insert("alpn".into(), json!(alpn));
            }
            put(&mut tm, "verifyPeerCertByName", &t.verify_peer_cert_by_name);
            put(&mut tm, "pinnedPeerCertSha256", &t.pinned_peer_cert_sha256);
            put(&mut tm, "echConfigList", &t.ech_config_list);
            put(&mut tm, "minVersion", &t.min_version);
            put(&mut tm, "maxVersion", &t.max_version);
            m.insert("tlsSettings".into(), Value::Object(tm));
        }
        Security::Reality(r) => {
            m.insert("security".into(), json!("reality"));
            let mut rm = Map::new();
            put(&mut rm, "serverName", &sni);
            rm.insert("fingerprint".into(), json!(r.fingerprint));
            rm.insert("password".into(), json!(r.public_key.expose()));
            rm.insert("shortId".into(), json!(r.short_id.expose()));
            put(&mut rm, "spiderX", &r.spider_x);
            put(&mut rm, "mldsa65Verify", &r.mldsa65_verify);
            m.insert("realitySettings".into(), Value::Object(rm));
        }
    }
    match &st.transport {
        Transport::Raw => {}
        Transport::Xhttp(x) => {
            m.insert("xhttpSettings".into(), xhttp(x, sni.as_deref()));
        }
        Transport::Ws(w) => {
            m.insert("wsSettings".into(), ws(w, sni.as_deref()));
        }
        Transport::Grpc(g) => {
            m.insert("grpcSettings".into(), grpc(g, sni.as_deref()));
        }
        Transport::HttpUpgrade(h) => {
            m.insert("httpupgradeSettings".into(), httpupgrade(h, sni.as_deref()));
        }
        Transport::Hysteria(h) => {
            let mut hm = Map::new();
            hm.insert("version".into(), json!(2));
            hm.insert("auth".into(), json!(h.auth.expose()));
            put(&mut hm, "udpIdleTimeout", &h.udp_idle_timeout);
            m.insert("hysteriaSettings".into(), Value::Object(hm));
        }
    }
    m.insert("sockopt".into(), sockopt(&st.tcp, iface, dialer));
    Value::Object(m)
}

fn host_or(host: &Option<String>, sni: Option<&str>) -> Option<String> {
    host.clone().or_else(|| sni.map(str::to_string))
}

fn xhttp(x: &XhttpSettings, sni: Option<&str>) -> Value {
    let mut m = Map::new();
    put(&mut m, "host", &host_or(&x.host, sni));
    if !x.path.is_empty() {
        m.insert("path".into(), json!(x.path));
    }
    put(&mut m, "mode", &x.mode);
    if !x.headers.is_empty() {
        m.insert("headers".into(), json!(x.headers));
    }
    for (k, v) in &x.params {
        m.insert(k.clone(), v.clone());
    }
    if !x.xmux.is_empty() {
        m.insert("xmux".into(), json!(x.xmux));
    }
    Value::Object(m)
}

fn ws(w: &WsSettings, sni: Option<&str>) -> Value {
    let mut m = Map::new();
    put(&mut m, "host", &host_or(&w.host, sni));
    if !w.path.is_empty() {
        m.insert("path".into(), json!(w.path));
    }
    if !w.headers.is_empty() {
        m.insert("headers".into(), json!(w.headers));
    }
    put(&mut m, "heartbeatPeriod", &w.heartbeat_period);
    Value::Object(m)
}

fn httpupgrade(h: &HttpUpgradeSettings, sni: Option<&str>) -> Value {
    let mut m = Map::new();
    put(&mut m, "host", &host_or(&h.host, sni));
    if !h.path.is_empty() {
        m.insert("path".into(), json!(h.path));
    }
    if !h.headers.is_empty() {
        m.insert("headers".into(), json!(h.headers));
    }
    Value::Object(m)
}

fn grpc(g: &GrpcSettings, sni: Option<&str>) -> Value {
    let mut m = Map::new();
    m.insert("serviceName".into(), json!(g.service_name));
    put(&mut m, "authority", &host_or(&g.authority, sni));
    put(&mut m, "multiMode", &g.multi_mode);
    put(&mut m, "idle_timeout", &g.idle_timeout);
    put(&mut m, "health_check_timeout", &g.health_check_timeout);
    put(&mut m, "permit_without_stream", &g.permit_without_stream);
    put(&mut m, "initial_windows_size", &g.initial_windows_size);
    put(&mut m, "user_agent", &g.user_agent);
    Value::Object(m)
}
