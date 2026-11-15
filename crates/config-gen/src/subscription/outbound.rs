//! Разбор одного outbound'а в строгие типы (белые списки полей).
//!
//! Всё, чего нет в белом списке, отбрасывается с предупреждением. Ошибка
//! (`Err`) означает «этот outbound не поддерживается»; решение о судьбе
//! профиля принимает вызывающий.

use std::collections::BTreeMap;

use ipc_proto::Secret;
use serde_json::{Map, Value};

use super::fields::{Fields, R, W, headers, lookup, norm_host, scalar, str_list};
use super::model::{
    Fragment, FreedomDialer, GrpcSettings, HttpUpgradeSettings, HysteriaSettings, OutboundSpec,
    ProxyOutbound, ProxyProtocol, RealitySettings, Security, Stream, TcpOpts, TlsSettings,
    Transport, WsSettings, XhttpSettings,
};
use crate::redact::sanitize_ident;

/// Результат разбора outbound'а, даже неудачного: тег нужен для ссылок.
pub(crate) struct ParsedOutbound {
    pub tag: Option<String>,
    /// Протокол из белого списка прокси (даже если разбор не удался).
    pub proxy_like: bool,
    /// Спецификация и тег `dialerProxy`.
    pub outcome: R<(OutboundSpec, Option<String>)>,
}

const PROXY_PROTOCOLS: &[&str] = &["vless", "vmess", "trojan", "shadowsocks", "hysteria"];

/// Причина для outbound'а с отключённой проверкой сертификата.
///
/// Решение: такой outbound помечается неподдерживаемым. Xray 26.11.9 вообще
/// отвергает `allowInsecure` (ошибка при сборке конфига), поэтому молча
/// включить проверку значит получить сервер, который, скорее всего, не
/// заработает (самоподписанный сертификат), а оставить поле — не запуститься.
/// Тихо ослаблять проверку сертификата мы не вправе (ТЗ §5.1, §14).
const ALLOW_INSECURE_REASON: &str = "требует allowInsecure (отключение проверки сертификата): Xray 26.11.9 и приложение этого не допускают";

pub(crate) fn parse_outbound(v: &Value, i: usize, w: &mut W<'_>) -> ParsedOutbound {
    let Some(map) = v.as_object() else {
        return ParsedOutbound {
            tag: None,
            proxy_like: false,
            outcome: Err("outbound не является объектом".into()),
        };
    };
    let mut f = Fields::map(map, &format!("outbounds[{i}]"));
    let tag = f.str("tag", 128).ok().flatten().map(str::to_string);
    f.label = match &tag {
        Some(t) => format!("outbound `{}`", sanitize_ident(t)),
        None => format!("outbound #{i}"),
    };
    let protocol = match f.str("protocol", 32) {
        Ok(Some(p)) => p.to_ascii_lowercase(),
        Ok(None) => {
            return ParsedOutbound {
                tag,
                proxy_like: false,
                outcome: Err("не указан protocol".into()),
            };
        }
        Err(e) => {
            return ParsedOutbound {
                tag,
                proxy_like: false,
                outcome: Err(e),
            };
        }
    };
    let proxy_like = PROXY_PROTOCOLS.contains(&protocol.as_str());
    let outcome = parse_inner(&mut f, &protocol, w);
    f.finish(w);
    ParsedOutbound {
        tag,
        proxy_like,
        outcome,
    }
}

fn parse_inner(
    f: &mut Fields<'_>,
    protocol: &str,
    w: &mut W<'_>,
) -> R<(OutboundSpec, Option<String>)> {
    let label = f.label.clone();
    f.drop_fields(
        &["sendThrough", "mux", "targetStrategy", "domainStrategy"],
        "привязка к адресу, мультиплексирование и стратегии задаёт приложение",
        w,
    );
    let settings = f.get("settings");
    let stream_v = f.get("streamSettings");
    let proxy_dialer = match f.get("proxySettings") {
        None => None,
        Some(v) => {
            // Устаревший способ цепочек: превращаем в зависимость dialerProxy.
            let mut pf = Fields::of(v, &format!("{label}.proxySettings"))?;
            let tag = pf.str("tag", 128)?.map(str::to_string);
            pf.drop_fields(&["transportLayer"], "не поддерживается", w);
            pf.finish(w);
            tag
        }
    };

    if protocol == "freedom" {
        let (dialer_spec, sock_dialer) = parse_freedom(settings, stream_v, &label, w)?;
        let dialer = merge_dialer(proxy_dialer, sock_dialer, &label)?;
        return Ok((OutboundSpec::Freedom(dialer_spec), dialer));
    }
    if !PROXY_PROTOCOLS.contains(&protocol) {
        return Err(format!(
            "протокол `{}` не поддерживается",
            sanitize_ident(protocol)
        ));
    }

    let (address, port, proto) = parse_proxy_settings(protocol, settings, &label, w)?;
    let is_hy = matches!(proto, ProxyProtocol::Hysteria);
    let (stream, sock_dialer) = parse_stream(stream_v, &label, is_hy, w)?;
    check_security_policy(&proto, &stream, &label)?;
    let dialer = merge_dialer(proxy_dialer, sock_dialer, &label)?;
    Ok((
        OutboundSpec::Proxy(Box::new(ProxyOutbound {
            address,
            port,
            protocol: proto,
            stream,
        })),
        dialer,
    ))
}

fn merge_dialer(a: Option<String>, b: Option<String>, label: &str) -> R<Option<String>> {
    match (a, b) {
        (Some(x), Some(y)) if x != y => Err(format!(
            "{label}: proxySettings.tag конфликтует с sockopt.dialerProxy"
        )),
        (Some(x), _) | (None, Some(x)) => Ok(Some(x)),
        (None, None) => Ok(None),
    }
}

/// Нельзя пускать трафик без шифрования: VLESS/Trojan без TLS/REALITY и без
/// VLESS Encryption отправили бы всё открытым текстом.
fn check_security_policy(p: &ProxyProtocol, s: &Stream, label: &str) -> R<()> {
    let plain = matches!(s.security, Security::None);
    match p {
        ProxyProtocol::Vless { encryption, .. } if plain && encryption == "none" => Err(format!(
            "{label}: VLESS без TLS/REALITY и без шифрования не поддерживается"
        )),
        ProxyProtocol::Trojan { .. } if plain => {
            Err(format!("{label}: Trojan без TLS не поддерживается"))
        }
        ProxyProtocol::Hysteria if plain => Err(format!("{label}: Hysteria требует TLS")),
        _ => Ok(()),
    }
}

// ---------------------------------------------------------------------------
// settings
// ---------------------------------------------------------------------------

/// Приводит `vnext`/`servers` (legacy) к плоскому виду.
fn flatten(
    settings: &Value,
    list_key: &str,
    with_users: bool,
    label: &str,
) -> R<Map<String, Value>> {
    let obj = settings
        .as_object()
        .ok_or_else(|| format!("{label}.settings: ожидался объект"))?;
    let mut flat = Map::new();
    for (k, v) in obj {
        if !k.eq_ignore_ascii_case(list_key) {
            flat.insert(k.clone(), v.clone());
        }
    }
    if let Some(list) = lookup(obj, list_key).filter(|v| !v.is_null()) {
        let arr = list
            .as_array()
            .ok_or_else(|| format!("{label}.settings.{list_key}: ожидался массив"))?;
        let [server] = arr.as_slice() else {
            return Err(format!(
                "{label}: в `{list_key}` должен быть ровно один сервер"
            ));
        };
        let srv = server
            .as_object()
            .ok_or_else(|| format!("{label}.settings.{list_key}: ожидался объект"))?;
        for (k, v) in srv {
            if !k.eq_ignore_ascii_case("users") {
                flat.insert(k.clone(), v.clone());
            }
        }
        if with_users && let Some(users) = lookup(srv, "users").filter(|v| !v.is_null()) {
            let arr = users
                .as_array()
                .ok_or_else(|| format!("{label}: users — не массив"))?;
            let [user] = arr.as_slice() else {
                return Err(format!("{label}: должен быть ровно один пользователь"));
            };
            let u = user
                .as_object()
                .ok_or_else(|| format!("{label}: пользователь — не объект"))?;
            for (k, v) in u {
                flat.insert(k.clone(), v.clone());
            }
        }
    }
    Ok(flat)
}

fn endpoint(f: &mut Fields<'_>) -> R<(String, u16)> {
    let raw = f.require_str("address", 256)?;
    let address = norm_host(raw)
        .ok_or_else(|| format!("{}: недопустимый адрес сервера (не домен и не IP)", f.label))?;
    let port = match f.get("port") {
        Some(v) => super::fields::as_port(v),
        None => None,
    }
    .ok_or_else(|| format!("{}: недопустимый порт", f.label))?;
    Ok((address, port))
}

const VMESS_SECURITY: &[&str] = &["auto", "aes-128-gcm", "chacha20-poly1305", "none", "zero"];
const SS_METHODS: &[&str] = &[
    "2022-blake3-aes-128-gcm",
    "2022-blake3-aes-256-gcm",
    "2022-blake3-chacha20-poly1305",
    "aes-256-gcm",
    "aes-128-gcm",
    "chacha20-poly1305",
    "chacha20-ietf-poly1305",
    "xchacha20-poly1305",
    "xchacha20-ietf-poly1305",
];

fn parse_proxy_settings(
    protocol: &str,
    settings: Option<&Value>,
    label: &str,
    w: &mut W<'_>,
) -> R<(String, u16, ProxyProtocol)> {
    let settings = settings.ok_or_else(|| format!("{label}: нет settings"))?;
    let (list_key, with_users) = match protocol {
        "vless" | "vmess" => ("vnext", true),
        _ => ("servers", false),
    };
    let flat = flatten(settings, list_key, with_users, label)?;
    let mut f = Fields::map(&flat, &format!("{label}.settings"));
    let (address, port) = endpoint(&mut f)?;
    f.ignore(&["level", "email"]);
    let proto = match protocol {
        "vless" => {
            let id = f.require_str("id", 64)?;
            let encryption = match f.str("encryption", 2048)? {
                Some(e) if !e.is_empty() => e,
                _ => {
                    w.warn(format!("{label}: encryption не указан, принят `none`"));
                    "none"
                }
            };
            if encryption != "none" && !valid_vlessenc(encryption) {
                return Err(format!("{label}: неподдерживаемое значение encryption"));
            }
            let flow = match f.str("flow", 64)? {
                None | Some("") => None,
                Some(x @ ("xtls-rprx-vision" | "xtls-rprx-vision-udp443")) => Some(x.to_string()),
                Some(_) => return Err(format!("{label}: неподдерживаемое значение flow")),
            };
            f.drop_fields(
                &["reverse", "seed", "testpre", "testseed"],
                "не поддерживается",
                w,
            );
            ProxyProtocol::Vless {
                id: Secret::new(id),
                encryption: encryption.to_string(),
                flow,
            }
        }
        "vmess" => {
            let id = f.require_str("id", 64)?;
            let security = f
                .str("security", 32)?
                .unwrap_or("auto")
                .to_ascii_lowercase();
            if !VMESS_SECURITY.contains(&security.as_str()) {
                return Err(format!("{label}: неподдерживаемое значение security"));
            }
            f.drop_fields(&["alterId", "experiments"], "не поддерживается", w);
            ProxyProtocol::Vmess {
                id: Secret::new(id),
                security,
            }
        }
        "trojan" => {
            let password = f.require_str("password", 256)?;
            f.drop_fields(&["flow"], "не поддерживается", w);
            ProxyProtocol::Trojan {
                password: Secret::new(password),
            }
        }
        "shadowsocks" => {
            let method = f.require_str("method", 64)?.to_ascii_lowercase();
            if !SS_METHODS.contains(&method.as_str()) {
                return Err(format!("{label}: неподдерживаемый метод shadowsocks"));
            }
            let password = f.require_str("password", 256)?;
            f.drop_fields(
                &["uot", "UoT", "uotVersion", "ivCheck"],
                "не поддерживается",
                w,
            );
            ProxyProtocol::Shadowsocks {
                method,
                password: Secret::new(password),
            }
        }
        _ => {
            // hysteria
            if let Some(v) = f.u32("version")?
                && v != 2
            {
                return Err(format!("{label}: поддерживается только Hysteria v2"));
            }
            ProxyProtocol::Hysteria
        }
    };
    f.finish(w);
    Ok((address, port, proto))
}

fn valid_vlessenc(s: &str) -> bool {
    s.starts_with("mlkem768x25519plus.")
        && s.len() <= 2048
        && s.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'.' | b'|' | b'-' | b'_' | b'+' | b'/' | b'=')
        })
}

// ---------------------------------------------------------------------------
// streamSettings
// ---------------------------------------------------------------------------

fn parse_stream(
    v: Option<&Value>,
    label: &str,
    is_hysteria: bool,
    w: &mut W<'_>,
) -> R<(Stream, Option<String>)> {
    let empty = Map::new();
    let slabel = format!("{label}.streamSettings");
    let mut f = match v {
        Some(v) => Fields::of(v, &slabel)?,
        None => Fields::map(&empty, &slabel),
    };
    let method = f.str("method", 32)?;
    let network = f.str("network", 32)?;
    let net = method.or(network).unwrap_or("raw").to_ascii_lowercase();
    let security = f
        .str("security", 16)?
        .unwrap_or("none")
        .to_ascii_lowercase();
    let tls_v = f.get("tlsSettings");
    let reality_v = f.get("realitySettings");
    let raw_v = f.get_any(&["rawSettings", "tcpSettings"]);
    let xhttp_v = f.get_any(&["xhttpSettings", "splithttpSettings"]);
    let ws_v = f.get("wsSettings");
    let grpc_v = f.get("grpcSettings");
    let hu_v = f.get("httpupgradeSettings");
    let hy_v = f.get("hysteriaSettings");
    let sockopt_v = f.get("sockopt");
    let finalmask_v = f.get("finalmask");
    f.drop_fields(
        &[
            "kcpSettings",
            "masqueSettings",
            "xdriveSettings",
            "xtlsSettings",
        ],
        "транспорт не поддерживается",
        w,
    );

    if (net == "hysteria") != is_hysteria {
        return Err(format!(
            "{label}: транспорт `{}` не подходит протоколу",
            sanitize_ident(&net)
        ));
    }

    let present = [raw_v, xhttp_v, ws_v, grpc_v, hu_v, hy_v]
        .iter()
        .filter(|x| x.is_some())
        .count();
    let used_transport_settings;
    let transport = match net.as_str() {
        "raw" | "tcp" => {
            used_transport_settings = usize::from(raw_v.is_some());
            parse_raw(raw_v, &slabel, w)?;
            Transport::Raw
        }
        "xhttp" | "splithttp" => {
            used_transport_settings = usize::from(xhttp_v.is_some());
            Transport::Xhttp(parse_xhttp(xhttp_v, &slabel, w)?)
        }
        "ws" | "websocket" => {
            used_transport_settings = usize::from(ws_v.is_some());
            Transport::Ws(parse_ws(ws_v, &slabel, w)?)
        }
        "grpc" => {
            used_transport_settings = usize::from(grpc_v.is_some());
            Transport::Grpc(parse_grpc(grpc_v, &slabel, w)?)
        }
        "httpupgrade" => {
            used_transport_settings = usize::from(hu_v.is_some());
            Transport::HttpUpgrade(parse_httpupgrade(hu_v, &slabel, w)?)
        }
        "hysteria" => {
            used_transport_settings = usize::from(hy_v.is_some());
            Transport::Hysteria(parse_hysteria(hy_v, &slabel, w)?)
        }
        other => {
            return Err(format!(
                "{label}: транспорт `{}` не поддерживается",
                sanitize_ident(other)
            ));
        }
    };
    if present > used_transport_settings {
        w.warn(format!(
            "{slabel}: настройки неиспользуемого транспорта отброшены"
        ));
    }

    let sec = match security.as_str() {
        "none" | "" => {
            if tls_v.is_some() || reality_v.is_some() {
                w.warn(format!(
                    "{slabel}: tls/reality-настройки при security=none отброшены"
                ));
            }
            Security::None
        }
        "tls" => Security::Tls(parse_tls(tls_v, &slabel, w)?),
        "reality" => {
            if !matches!(
                transport,
                Transport::Raw | Transport::Xhttp(_) | Transport::Grpc(_)
            ) {
                return Err(format!(
                    "{label}: REALITY допустим только с raw, xhttp и grpc"
                ));
            }
            Security::Reality(parse_reality(reality_v, &slabel, w)?)
        }
        other => {
            return Err(format!(
                "{label}: security `{}` не поддерживается",
                sanitize_ident(other)
            ));
        }
    };

    let (tcp, dialer) = parse_sockopt(sockopt_v, &slabel, w)?;
    if let Some(fm) = finalmask_v {
        check_finalmask_for_proxy(fm, &slabel, w)?;
    }
    f.finish(w);
    Ok((
        Stream {
            transport,
            security: sec,
            tcp,
        },
        dialer,
    ))
}

/// Для прокси-outbound'а финальная маска — часть протокола с сервером;
/// молча её отбросить значит получить нерабочий сервер. Принимаем только
/// отсутствие маски; `quicParams` (скорости Hysteria) отбрасываем: это
/// настройка производительности, а не совместимости.
fn check_finalmask_for_proxy(v: &Value, label: &str, w: &mut W<'_>) -> R<()> {
    let mut f = Fields::of(v, &format!("{label}.finalmask"))?;
    f.drop_fields(
        &["quicParams"],
        "скорости и параметры QUIC не поддерживаются",
        w,
    );
    let present: Vec<&str> = ["tcp", "udp"]
        .into_iter()
        .filter(|k| f.get(k).is_some_and(|x| !is_empty_json(x)))
        .collect();
    if !present.is_empty() {
        return Err(format!(
            "{label}: finalmask ({}) не поддерживается",
            present.join(", ")
        ));
    }
    f.finish(w);
    Ok(())
}

fn is_empty_json(v: &Value) -> bool {
    match v {
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
        Value::Null => true,
        _ => false,
    }
}

fn parse_sockopt(v: Option<&Value>, label: &str, w: &mut W<'_>) -> R<(TcpOpts, Option<String>)> {
    let Some(v) = v else {
        return Ok((TcpOpts::default(), None));
    };
    let mut f = Fields::of(v, &format!("{label}.sockopt"))?;
    let dialer = f
        .str("dialerProxy", 128)?
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let tcp_fast_open = match f.get("tcpFastOpen") {
        None => None,
        Some(Value::Bool(b)) => Some(*b),
        Some(Value::Number(n)) => Some(n.as_u64().is_some_and(|x| x > 0)),
        Some(_) => return Err(format!("{label}.sockopt.tcpFastOpen: ожидалось true/false")),
    };
    let tcp = TcpOpts {
        tcp_fast_open,
        keep_alive_idle: f.u32("tcpKeepAliveIdle")?,
        keep_alive_interval: f.u32("tcpKeepAliveInterval")?,
        user_timeout: f.u32("tcpUserTimeout")?,
    };
    f.drop_fields(
        &["interface", "mark"],
        "привязку к интерфейсу задаёт приложение; mark из подписки не принимается",
        w,
    );
    f.drop_fields(
        &[
            "domainStrategy",
            "tproxy",
            "happyEyeballs",
            "customSockopt",
            "tcpMptcp",
            "tcpcongestion",
            "tcpMaxSeg",
            "tcpWindowClamp",
            "V6Only",
            "addressPortStrategy",
            "acceptProxyProtocol",
            "trustedXForwardedFor",
        ],
        "не входит в белый список",
        w,
    );
    f.finish(w);
    Ok((tcp, dialer))
}

fn allow_insecure(v: Option<&Value>) -> bool {
    match v {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => {
            matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes")
        }
        Some(Value::Number(n)) => n.as_u64().is_some_and(|x| x != 0),
        _ => false,
    }
}

const FINGERPRINTS: &[&str] = &[
    "chrome",
    "firefox",
    "safari",
    "ios",
    "android",
    "edge",
    "360",
    "qq",
    "random",
    "randomized",
    "randomizednoalpn",
];

fn norm_fingerprint(v: Option<&str>, label: &str, w: &mut W<'_>) -> String {
    match v.map(str::to_ascii_lowercase) {
        None => "chrome".into(),
        Some(s) if s.is_empty() => "chrome".into(),
        Some(s) if FINGERPRINTS.contains(&s.as_str()) => s,
        Some(_) => {
            w.warn(format!(
                "{label}: неизвестный fingerprint заменён на chrome"
            ));
            "chrome".into()
        }
    }
}

fn opt_host(f: &mut Fields<'_>, key: &str) -> R<Option<String>> {
    match f.str(key, 256)? {
        None | Some("") => Ok(None),
        Some(s) => norm_host(s)
            .map(Some)
            .ok_or_else(|| format!("{}.{key}: недопустимое имя", f.label)),
    }
}

fn parse_tls(v: Option<&Value>, label: &str, w: &mut W<'_>) -> R<TlsSettings> {
    let empty = Map::new();
    let l = format!("{label}.tlsSettings");
    let mut f = match v {
        Some(v) => Fields::of(v, &l)?,
        None => Fields::map(&empty, &l),
    };
    if allow_insecure(f.get("allowInsecure")) {
        return Err(format!("{label}: {ALLOW_INSECURE_REASON}"));
    }
    let server_name = opt_host(&mut f, "serverName")?;
    let fingerprint = norm_fingerprint(f.str("fingerprint", 32)?, &l, w);
    let alpn = str_list(f.get("alpn"), &format!("{l}.alpn"), 8, 32)?;
    let verify = f
        .str("verifyPeerCertByName", 512)?
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let pinned = match f.str("pinnedPeerCertSha256", 1024)? {
        None | Some("") => None,
        Some(s)
            if s.bytes()
                .all(|b| b.is_ascii_hexdigit() || matches!(b, b',' | b':')) =>
        {
            Some(s.to_string())
        }
        Some(_) => return Err(format!("{l}.pinnedPeerCertSha256: ожидался hex")),
    };
    let ech = f
        .str("echConfigList", 4096)?
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let ver = |f: &mut Fields<'_>, key: &str| -> R<Option<String>> {
        match f.str(key, 8)? {
            None | Some("") => Ok(None),
            Some(s) if matches!(s, "1.0" | "1.1" | "1.2" | "1.3") => Ok(Some(s.to_string())),
            Some(_) => Err(format!("{l}.{key}: неподдерживаемая версия TLS")),
        }
    };
    let min_version = ver(&mut f, "minVersion")?;
    let max_version = ver(&mut f, "maxVersion")?;
    f.drop_fields(
        &["certificates", "masterKeyLog"],
        "содержит пути к файлам или клиентские сертификаты",
        w,
    );
    f.drop_fields(
        &[
            "useSystemCA",
            "disableSystemRoot",
            "cipherSuites",
            "curvePreferences",
            "enableSessionResumption",
            "echSockopt",
            "echForceQuery",
            "rejectUnknownSni",
            "echServerKeys",
        ],
        "не входит в белый список",
        w,
    );
    f.finish(w);
    Ok(TlsSettings {
        server_name,
        fingerprint,
        alpn,
        verify_peer_cert_by_name: verify,
        pinned_peer_cert_sha256: pinned,
        ech_config_list: ech,
        min_version,
        max_version,
    })
}

fn parse_reality(v: Option<&Value>, label: &str, w: &mut W<'_>) -> R<RealitySettings> {
    let l = format!("{label}.realitySettings");
    let v = v.ok_or_else(|| format!("{label}: нет realitySettings"))?;
    let mut f = Fields::of(v, &l)?;
    for server_only in [
        "target",
        "dest",
        "serverNames",
        "privateKey",
        "shortIds",
        "xver",
    ] {
        if f.has(server_only) {
            return Err(format!(
                "{label}: серверные поля REALITY в клиентском конфиге"
            ));
        }
    }
    let server_name = opt_host(&mut f, "serverName")?;
    let fingerprint = norm_fingerprint(f.str("fingerprint", 32)?, &l, w);
    let pk_new = f.str("password", 128)?;
    let pk_old = f.str("publicKey", 128)?;
    let public_key = pk_new.filter(|s| !s.is_empty()).or(pk_old).unwrap_or("");
    if !(32..=64).contains(&public_key.len())
        || !public_key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'+' | b'/' | b'='))
    {
        return Err(format!("{label}: недопустимый публичный ключ REALITY"));
    }
    let short_id = f.str("shortId", 32)?.unwrap_or("");
    if short_id.len() > 16
        || short_id.len() % 2 != 0
        || !short_id.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(format!("{label}: недопустимый shortId"));
    }
    let spider_x = f
        .str("spiderX", 256)?
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let mldsa = f
        .str("mldsa65Verify", 8192)?
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    f.drop_fields(&["show"], "отладочный вывод не нужен", w);
    f.finish(w);
    Ok(RealitySettings {
        server_name,
        fingerprint,
        public_key: Secret::new(public_key),
        short_id: Secret::new(short_id),
        spider_x,
        mldsa65_verify: mldsa,
    })
}

fn opt_path(f: &mut Fields<'_>) -> R<String> {
    Ok(f.str("path", 512)?.unwrap_or("").to_string())
}

fn opt_plain_host(f: &mut Fields<'_>) -> R<Option<String>> {
    match f.str("host", 256)? {
        None | Some("") => Ok(None),
        Some(s) if s.bytes().all(|b| b.is_ascii_graphic()) => Ok(Some(s.to_ascii_lowercase())),
        Some(_) => Err(format!("{}.host: недопустимое значение", f.label)),
    }
}

fn parse_raw(v: Option<&Value>, label: &str, w: &mut W<'_>) -> R<()> {
    let Some(v) = v else { return Ok(()) };
    let mut f = Fields::of(v, &format!("{label}.rawSettings"))?;
    if let Some(h) = f.get("header") {
        let mut hf = Fields::of(h, &format!("{label}.rawSettings.header"))?;
        let t = hf.str("type", 16)?.unwrap_or("none");
        if !t.eq_ignore_ascii_case("none") {
            return Err(format!("{label}: HTTP-обфускация raw не поддерживается"));
        }
        hf.finish(w);
    }
    f.drop_fields(&["acceptProxyProtocol"], "только для серверной стороны", w);
    f.finish(w);
    Ok(())
}

const XHTTP_MODES: &[&str] = &["auto", "packet-up", "stream-up", "stream-one"];
const XHTTP_PARAMS: &[&str] = &[
    "xPaddingBytes",
    "xPaddingObfsMode",
    "xPaddingKey",
    "xPaddingHeader",
    "xPaddingPlacement",
    "xPaddingMethod",
    "uplinkHTTPMethod",
    "sessionIDPlacement",
    "sessionIDKey",
    "sessionIDTable",
    "sessionIDLength",
    "seqPlacement",
    "seqKey",
    "uplinkDataPlacement",
    "uplinkDataKey",
    "uplinkChunkSize",
    "noGRPCHeader",
    "noSSEHeader",
    "scMaxEachPostBytes",
    "scMinPostsIntervalMs",
    "scMaxBufferedPosts",
    "scStreamUpServerSecs",
];
const XMUX_PARAMS: &[&str] = &[
    "maxConcurrency",
    "maxConnections",
    "cMaxReuseTimes",
    "hMaxRequestTimes",
    "hMaxReusableSecs",
    "hKeepAlivePeriod",
];

fn parse_xhttp(v: Option<&Value>, label: &str, w: &mut W<'_>) -> R<XhttpSettings> {
    let l = format!("{label}.xhttpSettings");
    let Some(v) = v else {
        return Ok(XhttpSettings::default());
    };
    let obj = v
        .as_object()
        .ok_or_else(|| format!("{l}: ожидался объект"))?;
    // `extra` — вложенный блок с теми же полями; внешние значения приоритетнее.
    let mut merged = Map::new();
    if let Some(extra) = lookup(obj, "extra").filter(|e| !e.is_null()) {
        let e = extra
            .as_object()
            .ok_or_else(|| format!("{l}.extra: ожидался объект"))?;
        for (k, val) in e {
            merged.insert(k.clone(), val.clone());
        }
    }
    for (k, val) in obj {
        if !k.eq_ignore_ascii_case("extra") {
            merged.insert(k.clone(), val.clone());
        }
    }
    let mut f = Fields::map(&merged, &l);
    if f.has("downloadSettings") && f.get("downloadSettings").is_some() {
        return Err(format!(
            "{label}: xhttp downloadSettings (отдельный сервер загрузки) не поддерживается"
        ));
    }
    let host = opt_plain_host(&mut f)?;
    let path = opt_path(&mut f)?;
    let mode = match f.str("mode", 16)? {
        None | Some("") => None,
        Some(m) if XHTTP_MODES.contains(&m) => Some(m.to_string()),
        Some(_) => return Err(format!("{label}: неподдерживаемый режим xhttp")),
    };
    let hdrs = headers(f.get("headers"), &l)?;
    let mut params = BTreeMap::new();
    for k in XHTTP_PARAMS {
        if let Some(val) = f.get(k) {
            let s = scalar(val).ok_or_else(|| format!("{l}.{k}: недопустимое значение"))?;
            params.insert((*k).to_string(), s);
        }
    }
    let mut xmux = BTreeMap::new();
    if let Some(xm) = f.get("xmux") {
        let mut xf = Fields::of(xm, &format!("{l}.xmux"))?;
        for k in XMUX_PARAMS {
            if let Some(val) = xf.get(k) {
                let s =
                    scalar(val).ok_or_else(|| format!("{l}.xmux.{k}: недопустимое значение"))?;
                xmux.insert((*k).to_string(), s);
            }
        }
        xf.finish(w);
    }
    f.drop_fields(&["h2Params"], "не поддерживается", w);
    f.finish(w);
    Ok(XhttpSettings {
        host,
        path,
        mode,
        headers: hdrs,
        params,
        xmux,
    })
}

fn parse_ws(v: Option<&Value>, label: &str, w: &mut W<'_>) -> R<WsSettings> {
    let l = format!("{label}.wsSettings");
    let Some(v) = v else {
        return Ok(WsSettings::default());
    };
    let mut f = Fields::of(v, &l)?;
    let host = opt_plain_host(&mut f)?;
    let path = opt_path(&mut f)?;
    let hdrs = headers(f.get("headers"), &l)?;
    let heartbeat_period = f.u32("heartbeatPeriod")?;
    f.drop_fields(&["acceptProxyProtocol"], "только для серверной стороны", w);
    f.finish(w);
    Ok(WsSettings {
        host,
        path,
        headers: hdrs,
        heartbeat_period,
    })
}

fn parse_httpupgrade(v: Option<&Value>, label: &str, w: &mut W<'_>) -> R<HttpUpgradeSettings> {
    let l = format!("{label}.httpupgradeSettings");
    let Some(v) = v else {
        return Ok(HttpUpgradeSettings::default());
    };
    let mut f = Fields::of(v, &l)?;
    let host = opt_plain_host(&mut f)?;
    let path = opt_path(&mut f)?;
    let hdrs = headers(f.get("headers"), &l)?;
    f.drop_fields(&["acceptProxyProtocol"], "только для серверной стороны", w);
    f.finish(w);
    Ok(HttpUpgradeSettings {
        host,
        path,
        headers: hdrs,
    })
}

fn parse_grpc(v: Option<&Value>, label: &str, w: &mut W<'_>) -> R<GrpcSettings> {
    let l = format!("{label}.grpcSettings");
    let Some(v) = v else {
        return Ok(GrpcSettings::default());
    };
    let mut f = Fields::of(v, &l)?;
    let g = GrpcSettings {
        service_name: f.str("serviceName", 256)?.unwrap_or("").to_string(),
        authority: f
            .str("authority", 256)?
            .filter(|s| !s.is_empty())
            .map(str::to_ascii_lowercase),
        multi_mode: f.bool("multiMode")?,
        idle_timeout: f.u32("idle_timeout")?,
        health_check_timeout: f.u32("health_check_timeout")?,
        permit_without_stream: f.bool("permit_without_stream")?,
        initial_windows_size: f.u32("initial_windows_size")?,
        user_agent: f
            .str("user_agent", 256)?
            .filter(|s| !s.is_empty())
            .map(str::to_string),
    };
    f.finish(w);
    Ok(g)
}

fn parse_hysteria(v: Option<&Value>, label: &str, w: &mut W<'_>) -> R<HysteriaSettings> {
    let l = format!("{label}.hysteriaSettings");
    let v = v.ok_or_else(|| format!("{label}: нет hysteriaSettings"))?;
    let mut f = Fields::of(v, &l)?;
    if let Some(ver) = f.u32("version")?
        && ver != 2
    {
        return Err(format!("{label}: поддерживается только Hysteria v2"));
    }
    let auth = f.require_str("auth", 256)?;
    let udp_idle_timeout = f.u32("udpIdleTimeout")?;
    f.drop_fields(&["masquerade"], "только для сервера", w);
    f.finish(w);
    Ok(HysteriaSettings {
        auth: Secret::new(auth),
        udp_idle_timeout,
    })
}

// ---------------------------------------------------------------------------
// freedom (только как dialerProxy)
// ---------------------------------------------------------------------------

fn valid_range(s: &str) -> Option<(u32, u32)> {
    let num = |p: &str| {
        (!p.is_empty() && p.len() <= 6 && p.bytes().all(|b| b.is_ascii_digit()))
            .then(|| p.parse::<u32>().ok())
            .flatten()
    };
    match s.split_once('-') {
        Some((a, b)) => {
            let (a, b) = (num(a)?, num(b)?);
            (a <= b).then_some((a, b))
        }
        None => num(s).map(|n| (n, n)),
    }
}

/// Число или `[ "a-b" ]` или `"a-b"` → строка диапазона.
fn range_value(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Array(a) => match a.first()? {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            _ => None,
        },
        _ => None,
    }
}

fn make_fragment(
    packets: Option<String>,
    length: Option<String>,
    interval: Option<String>,
    label: &str,
) -> R<Fragment> {
    let packets = packets.unwrap_or_else(|| "tlshello".into());
    let packets_ok = packets == "tlshello" || valid_range(&packets).is_some_and(|(a, _)| a >= 1);
    let length = length.ok_or_else(|| format!("{label}: у fragment нет length"))?;
    let interval = interval.ok_or_else(|| format!("{label}: у fragment нет interval/delay"))?;
    let length_ok = valid_range(&length).is_some_and(|(a, _)| a >= 1);
    let interval_ok = valid_range(&interval).is_some();
    if packets_ok && length_ok && interval_ok {
        Ok(Fragment {
            packets,
            length,
            interval,
        })
    } else {
        Err(format!("{label}: недопустимые параметры fragment"))
    }
}

fn parse_freedom(
    settings: Option<&Value>,
    stream: Option<&Value>,
    label: &str,
    w: &mut W<'_>,
) -> R<(FreedomDialer, Option<String>)> {
    let empty = Map::new();
    let sl = format!("{label}.settings");
    let mut sf = match settings {
        Some(v) => Fields::of(v, &sl)?,
        None => Fields::map(&empty, &sl),
    };
    let mut fragment = None;
    if let Some(fv) = sf.get("fragment") {
        let mut ff = Fields::of(fv, &format!("{sl}.fragment"))?;
        let packets = range_value(ff.get("packets"));
        let length = range_value(ff.get("length"));
        let interval = range_value(ff.get("interval"));
        ff.finish(w);
        fragment = Some(make_fragment(packets, length, interval, label)?);
    }
    sf.ignore(&["userLevel"]);
    sf.drop_fields(
        &[
            "redirect",
            "domainStrategy",
            "targetStrategy",
            "noises",
            "noise",
            "finalRules",
            "proxyProtocol",
            "ipsBlocked",
        ],
        "freedom используется только как диалер; перенаправления и правила не принимаются",
        w,
    );
    sf.finish(w);

    let stl = format!("{label}.streamSettings");
    let mut stf = match stream {
        Some(v) => Fields::of(v, &stl)?,
        None => Fields::map(&empty, &stl),
    };
    let sv = stf.get("sockopt");
    let (_, sock_dialer) = parse_sockopt(sv, &stl, w)?;
    if let Some(fm) = stf.get("finalmask") {
        let mut mf = Fields::of(fm, &format!("{stl}.finalmask"))?;
        if let Some(tcp) = mf.get("tcp") {
            let arr = tcp
                .as_array()
                .ok_or_else(|| format!("{label}: finalmask.tcp — не массив"))?;
            if arr.len() > 8 {
                return Err(format!("{label}: слишком много масок"));
            }
            for item in arr {
                let mut itf = Fields::of(item, &format!("{stl}.finalmask.tcp[]"))?;
                let ty = itf.str("type", 32)?.unwrap_or("").to_ascii_lowercase();
                if ty == "fragment" {
                    let sv = itf.get("settings");
                    let empty2 = Map::new();
                    let mut pf = match sv {
                        Some(s) => Fields::of(s, &format!("{stl}.finalmask.fragment"))?,
                        None => Fields::map(&empty2, &stl),
                    };
                    let packets = range_value(pf.get("packets"));
                    let length = range_value(pf.get_any(&["lengths", "length"]));
                    let interval = range_value(pf.get_any(&["delays", "delay"]));
                    pf.drop_fields(&["maxSplit"], "не поддерживается", w);
                    pf.finish(w);
                    if fragment.is_none() {
                        fragment = Some(make_fragment(packets, length, interval, label)?);
                    }
                } else {
                    w.warn(format!("{stl}: маска `{}` отброшена", sanitize_ident(&ty)));
                    itf.ignore(&["settings"]);
                }
                itf.finish(w);
            }
        }
        mf.drop_fields(&["udp", "quicParams"], "не поддерживается", w);
        mf.finish(w);
    }
    stf.drop_fields(
        &["network", "method", "security"],
        "у freedom-диалера только sockopt и finalmask",
        w,
    );
    stf.finish(w);
    Ok((FreedomDialer { fragment }, sock_dialer))
}
