//! Структура сгенерированного конфига Xray. Xray молча игнорирует неизвестные
//! поля, поэтому форму JSON проверяем сами (docs/reference/xray.md §3.7, §10).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;
use std::net::{IpAddr, Ipv4Addr};

use config_gen::subscription::*;
use config_gen::xray::*;
use ipc_proto::Secret;
use serde_json::Value;

fn load_profiles(name: &str) -> ParsedSubscription {
    let b = std::fs::read(format!(
        "{}/tests/fixtures/subscription/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    parse_subscription(&b).unwrap()
}

fn profile(sub: &ParsedSubscription, i: usize) -> &ProfileConfig {
    match &sub.profiles[i].outcome {
        ProfileOutcome::Supported(c) => c,
        ProfileOutcome::Unsupported { reason } => panic!("{reason}"),
    }
}

fn ip4(s: &str) -> Ipv4Addr {
    s.parse().unwrap()
}

fn table(entries: &[(&str, &[&str])]) -> ResolvedHosts {
    entries
        .iter()
        .map(|(d, ips)| ((*d).to_string(), ips.iter().map(|i| ip4(i)).collect()))
        .collect()
}

const PASS: &str = "socks-PASS-PLACEHOLDER";

fn build(p: &ProfileConfig, resolved: &ResolvedHosts) -> Result<(BuiltConfig, Value), XrayError> {
    let pw = Secret::new(PASS);
    let params = XrayParams {
        socks_port: 10808,
        socks_user: "pz_user",
        socks_password: &pw,
        interface: "en0",
        resolved,
        probe_url: None,
    };
    let b = build_config(p, &params)?;
    let v: Value = serde_json::from_str(b.json.expose()).unwrap();
    Ok((b, v))
}

fn germany_table() -> ResolvedHosts {
    table(&[
        ("de1.example.com", &["203.0.113.10", "203.0.113.11"]),
        ("cdn-de.example.com", &["198.51.100.20"]),
        ("frag.example.com", &["203.0.113.30"]),
    ])
}

fn outbound<'a>(v: &'a Value, tag: &str) -> &'a Value {
    v["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["tag"] == tag)
        .unwrap_or_else(|| panic!("нет outbound {tag}"))
}

/// Все ключи объектов на любой глубине.
fn all_keys(v: &Value, out: &mut BTreeSet<String>) {
    match v {
        Value::Object(m) => {
            for (k, x) in m {
                out.insert(k.clone());
                all_keys(x, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| all_keys(x, out)),
        _ => {}
    }
}

#[test]
fn skeleton_inbound_routing_log() {
    let sub = load_profiles("happ_full.json");
    let (_, v) = build(profile(&sub, 0), &germany_table()).unwrap();

    let inb = v["inbounds"].as_array().unwrap();
    assert_eq!(inb.len(), 1);
    assert_eq!(inb[0]["listen"], "127.0.0.1");
    assert_eq!(inb[0]["port"], 10808);
    assert_eq!(inb[0]["protocol"], "socks");
    assert_eq!(inb[0]["settings"]["auth"], "password");
    assert_eq!(inb[0]["settings"]["udp"], true);
    assert_eq!(inb[0]["settings"]["users"][0]["user"], "pz_user");
    assert_eq!(inb[0]["settings"]["users"][0]["pass"], PASS);

    let rules = v["routing"]["rules"].as_array().unwrap();
    assert_eq!(rules.len(), 1, "единственное правило");
    assert_eq!(rules[0]["inboundTag"][0], "pz-in");
    assert_eq!(rules[0]["balancerTag"], "pz-bal");
    assert!(rules[0].get("outboundTag").is_none());
    assert_eq!(v["routing"]["domainStrategy"], "AsIs");
    let bal = &v["routing"]["balancers"][0];
    assert_eq!(bal["tag"], "pz-bal");
    assert_eq!(bal["selector"], serde_json::json!(["pz-srv-"]));
    assert_eq!(bal["strategy"]["type"], "leastPing");
    assert_eq!(
        bal["fallbackTag"], v["outbounds"][0]["tag"],
        "fallback — первый сервер"
    );

    assert_eq!(v["log"]["access"], "none");
    assert_eq!(v["log"]["maskAddress"], "full");
    assert_eq!(v["log"]["logLevel"], "warning");

    let bo = &v["burstObservatory"];
    assert_eq!(bo["subjectSelector"], serde_json::json!(["pz-srv-"]));
    assert_eq!(bo["pingConfig"]["interval"], "30s");
    assert_eq!(bo["pingConfig"]["sampling"], 5);
    assert_eq!(bo["pingConfig"]["timeout"], "4s");
    assert_eq!(
        bo["pingConfig"]["destination"], DEFAULT_PROBE_URL,
        "адрес проб из подписки не берётся"
    );
}

#[test]
fn forbidden_keys_never_appear() {
    let sub = load_profiles("happ_full.json");
    for i in 0..4 {
        let resolved = table(&[
            ("de1.example.com", &["203.0.113.10"]),
            ("cdn-de.example.com", &["198.51.100.20"]),
            ("frag.example.com", &["203.0.113.30"]),
            ("bridge.example.com", &["203.0.113.40"]),
            ("exit.example.com", &["203.0.113.41"]),
            ("trojan.example.com", &["203.0.113.42"]),
            ("ws.example.com", &["203.0.113.43"]),
        ]);
        let (_, v) = build(profile(&sub, i), &resolved).unwrap();
        let mut keys = BTreeSet::new();
        all_keys(&v, &mut keys);
        for bad in [
            "dns",
            "api",
            "policy",
            "stats",
            "proxySettings",
            "allowInsecure",
            "vnext",
            "servers",
            "mux",
            "sendThrough",
            "mark",
            "publicKey",
            "tcpSettings",
            "extra",
            "downloadSettings",
            "remarks",
        ] {
            assert!(
                !keys.contains(bad),
                "профиль {i}: ключ {bad} не должен попадать в конфиг"
            );
        }
        assert!(!v.get("inbounds").unwrap().to_string().contains("0.0.0.0"));
    }
}

#[test]
fn addresses_are_ips_and_domains_move_to_server_name_and_host() {
    let sub = load_profiles("happ_full.json");
    let (b, v) = build(profile(&sub, 0), &germany_table()).unwrap();
    let text = b.json.expose();
    assert!(
        !text.contains("de1.example.com"),
        "домен сервера не должен остаться в конфиге"
    );
    assert!(!text.contains("cdn-de.example.com\",\"port"), "адрес — IP");

    for o in v["outbounds"].as_array().unwrap() {
        if o["protocol"] != "freedom" {
            let a: IpAddr = o["settings"]["address"]
                .as_str()
                .unwrap()
                .parse()
                .expect("address должен быть IP");
            assert!(!a.is_unspecified());
        }
    }
    // Reality: serverName из подписки сохранён; публичный ключ — в `password`.
    let r = outbound(&v, "pz-srv-0-0");
    assert_eq!(r["settings"]["address"], "203.0.113.10");
    assert_eq!(r["settings"]["flow"], "xtls-rprx-vision");
    assert_eq!(r["settings"]["encryption"], "none");
    assert_eq!(r["streamSettings"]["network"], "raw");
    assert_eq!(r["streamSettings"]["security"], "reality");
    let rs = &r["streamSettings"]["realitySettings"];
    assert_eq!(rs["serverName"], "www.example.com");
    assert_eq!(rs["fingerprint"], "chrome");
    assert_eq!(
        rs["password"],
        "22N5CfHXOd5uVoEPntAVNd47bq75-YyX9tha_PMPfxg"
    );
    assert_eq!(rs["shortId"], "0123456789abcdef");
    // Второй IP того же домена — отдельный outbound с теми же настройками.
    let r2 = outbound(&v, "pz-srv-0-1");
    assert_eq!(r2["settings"]["address"], "203.0.113.11");
    assert_eq!(r2["streamSettings"]["realitySettings"], *rs);

    // xhttp: в подписке нет ни serverName, ни host — оба равны исходному домену.
    let x = outbound(&v, "pz-srv-1-0");
    assert_eq!(x["settings"]["address"], "198.51.100.20");
    assert_eq!(x["streamSettings"]["network"], "xhttp");
    assert_eq!(
        x["streamSettings"]["tlsSettings"]["serverName"],
        "cdn-de.example.com"
    );
    assert_eq!(x["streamSettings"]["tlsSettings"]["fingerprint"], "chrome");
    let xs = &x["streamSettings"]["xhttpSettings"];
    assert_eq!(xs["host"], "cdn-de.example.com");
    assert_eq!(xs["path"], "/xh");
    assert_eq!(xs["mode"], "auto");
    assert_eq!(xs["xPaddingBytes"], "100-1000");
    assert_eq!(xs["xmux"]["maxConcurrency"], "16-32");
    assert!(xs.get("extra").is_none());

    // Hysteria: IP-литерал, serverName из подписки, alpn h3.
    let h = outbound(&v, "pz-srv-2-0");
    assert_eq!(h["protocol"], "hysteria");
    assert_eq!(h["settings"]["version"], 2);
    assert_eq!(h["settings"]["address"], "203.0.113.50");
    assert_eq!(h["streamSettings"]["network"], "hysteria");
    assert_eq!(h["streamSettings"]["hysteriaSettings"]["version"], 2);
    assert_eq!(
        h["streamSettings"]["hysteriaSettings"]["auth"],
        "hy-auth-PLACEHOLDER-1"
    );
    assert_eq!(
        h["streamSettings"]["tlsSettings"]["serverName"],
        "hy.example.com"
    );
    assert_eq!(
        h["streamSettings"]["tlsSettings"]["alpn"],
        serde_json::json!(["h3"])
    );
}

#[test]
fn interface_is_on_every_dialing_outbound_and_not_on_dialer_proxy_users() {
    let sub = load_profiles("happ_full.json");
    let (_, v) = build(profile(&sub, 0), &germany_table()).unwrap();
    let outs = v["outbounds"].as_array().unwrap();
    let mut with_dialer = 0;
    for o in outs {
        let so = &o["streamSettings"]["sockopt"];
        if so.get("dialerProxy").is_some() {
            with_dialer += 1;
            assert!(
                so.get("interface").is_none(),
                "{}: interface не нужен при dialerProxy",
                o["tag"]
            );
        } else {
            assert_eq!(
                so["interface"], "en0",
                "{}: нет привязки к интерфейсу",
                o["tag"]
            );
        }
    }
    assert_eq!(with_dialer, 1);
    let frag_user = outbound(&v, "pz-srv-3-0");
    assert_eq!(
        frag_user["streamSettings"]["sockopt"]["dialerProxy"],
        "pz-dial-4"
    );
    let dialer = outbound(&v, "pz-dial-4");
    assert_eq!(dialer["protocol"], "freedom");
    assert_eq!(dialer["streamSettings"]["sockopt"]["interface"], "en0");
    assert_eq!(dialer["settings"]["fragment"]["packets"], "tlshello");
    assert_eq!(dialer["settings"]["fragment"]["length"], "100-200");
    assert_eq!(dialer["settings"]["fragment"]["interval"], "10-20");
    // Безопасное TCP-поле из подписки перенесено, `mark`/чужой `interface` — нет.
    assert_eq!(
        outbound(&v, "pz-srv-0-0")["streamSettings"]["sockopt"]["tcpFastOpen"],
        true
    );
    assert!(!v.to_string().contains("eth0"));
    // fingerprint всегда явный.
    for o in outs {
        let ss = &o["streamSettings"];
        for k in ["tlsSettings", "realitySettings"] {
            if let Some(s) = ss.get(k) {
                assert!(
                    s["fingerprint"].as_str().is_some_and(|f| !f.is_empty()),
                    "{}: нет fingerprint",
                    o["tag"]
                );
            }
        }
    }
}

#[test]
fn tags_are_own_and_selector_does_not_catch_helpers() {
    let sub = load_profiles("happ_full.json");
    let (_, v) = build(profile(&sub, 0), &germany_table()).unwrap();
    let outs = v["outbounds"].as_array().unwrap();
    let tags: Vec<&str> = outs.iter().map(|o| o["tag"].as_str().unwrap()).collect();
    let unique: BTreeSet<_> = tags.iter().collect();
    assert_eq!(unique.len(), tags.len(), "теги уникальны");
    assert_eq!(tags[0], "pz-srv-0-0", "первым идёт сервер подписки");
    for t in &tags {
        assert!(t.starts_with("pz-"), "тег {t} не наш");
        if t.starts_with(SERVER_TAG_PREFIX) {
            assert_ne!(outbound(&v, t)["protocol"], "freedom");
        }
    }
    // Служебные теги под префикс серверов не попадают.
    assert!(
        tags.iter()
            .filter(|t| !t.starts_with(SERVER_TAG_PREFIX))
            .all(|t| t.starts_with("pz-dial-") || t.starts_with("pz-hop-"))
    );
    assert_eq!(
        tags.iter()
            .filter(|t| t.starts_with(SERVER_TAG_PREFIX))
            .count(),
        5
    );
    assert_eq!(tags.len(), 6);
}

#[test]
fn server_ips_match_what_is_in_the_config() {
    let sub = load_profiles("happ_full.json");
    let p = profile(&sub, 0);
    let r = germany_table();
    let (b, v) = build(p, &r).unwrap();
    let expected: Vec<IpAddr> = [
        "198.51.100.20",
        "203.0.113.10",
        "203.0.113.11",
        "203.0.113.30",
        "203.0.113.50",
    ]
    .iter()
    .map(|s| s.parse().unwrap())
    .collect();
    assert_eq!(b.server_ips, expected);
    assert_eq!(vpn_server_ips(p, &r), expected);
    assert_eq!(b.server_outbounds, 5);
    assert!(b.skipped.is_empty());
    // Каждый IP из набора действительно встречается как address.
    let in_cfg: BTreeSet<IpAddr> = v["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|o| {
            o["settings"]["address"]
                .as_str()
                .and_then(|s| s.parse().ok())
        })
        .collect();
    assert_eq!(in_cfg, expected.iter().copied().collect());
}

#[test]
fn server_without_ip_is_skipped_and_all_missing_is_an_error() {
    let sub = load_profiles("happ_full.json");
    let p = profile(&sub, 0);
    let r = table(&[
        ("de1.example.com", &["203.0.113.10"]),
        ("cdn-de.example.com", &[]),
    ]);
    let (b, v) = build(p, &r).unwrap();
    // de-xhttp (пустой список) и de-frag (нет записи) пропущены; fragment не нужен.
    assert_eq!(b.skipped, ["cdn-de.example.com", "frag.example.com"]);
    let tags: Vec<&str> = v["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["tag"].as_str().unwrap())
        .collect();
    assert_eq!(tags, ["pz-srv-0-0", "pz-srv-2-0"]);
    assert!(
        !tags.iter().any(|t| t.starts_with("pz-dial-")),
        "диалер без пользователей не нужен"
    );
    assert_eq!(vpn_server_ips(p, &r).len(), 2);

    // Ни одного IP — ошибка, кроме IP-литерала hysteria: его пропустить нельзя.
    let (b, _) = build(p, &ResolvedHosts::new()).unwrap();
    assert_eq!(b.server_outbounds, 1);

    let ws_only = profile(&sub, 3);
    let err = build(ws_only, &table(&[("ws.example.com", &[])]))
        .map(|_| ())
        .unwrap_err();
    assert_eq!(err, XrayError::NoServers);
    let mobile = profile(&sub, 2);
    // IP-литерал сервера (ss) остаётся рабочим без таблицы.
    let (b, _) = build(mobile, &ResolvedHosts::new()).unwrap();
    assert_eq!(b.server_outbounds, 1);
}

#[test]
fn bridge_chain_uses_first_ip_for_hop_and_all_for_exit() {
    let sub = load_profiles("happ_full.json");
    let r = table(&[
        ("bridge.example.com", &["203.0.113.40", "203.0.113.44"]),
        ("exit.example.com", &["203.0.113.41", "203.0.113.45"]),
    ]);
    let (b, v) = build(profile(&sub, 1), &r).unwrap();
    let tags: Vec<&str> = v["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["tag"].as_str().unwrap())
        .collect();
    assert_eq!(tags, ["pz-srv-0-0", "pz-srv-0-1", "pz-hop-0"]);
    for t in ["pz-srv-0-0", "pz-srv-0-1"] {
        let o = outbound(&v, t);
        assert_eq!(o["streamSettings"]["sockopt"]["dialerProxy"], "pz-hop-0");
        assert!(o["streamSettings"]["sockopt"].get("interface").is_none());
        assert_eq!(
            o["streamSettings"]["realitySettings"]["serverName"],
            "exit-front.example.com"
        );
        assert_eq!(o["streamSettings"]["realitySettings"]["shortId"], "");
    }
    let hop = outbound(&v, "pz-hop-0");
    assert_eq!(hop["settings"]["address"], "203.0.113.40");
    assert_eq!(hop["streamSettings"]["sockopt"]["interface"], "en0");
    assert_eq!(
        hop["streamSettings"]["realitySettings"]["serverName"],
        "bridge-front.example.com"
    );
    assert_eq!(b.server_ips.len(), 3);
    // Если у моста нет IP — выход без моста работать не может.
    let r2 = table(&[("exit.example.com", &["203.0.113.41"])]);
    assert_eq!(
        build(profile(&sub, 1), &r2).map(|_| ()).unwrap_err(),
        XrayError::NoServers
    );
}

#[test]
fn grpc_ws_ss_trojan_defaults_to_original_domain() {
    let sub = load_profiles("happ_full.json");
    let (_, v) = build(
        profile(&sub, 2),
        &table(&[("trojan.example.com", &["203.0.113.42"])]),
    )
    .unwrap();
    let t = outbound(&v, "pz-srv-0-0");
    assert_eq!(t["protocol"], "trojan");
    assert_eq!(t["settings"]["password"], "TROJAN-PASSWORD-PLACEHOLDER");
    assert_eq!(t["streamSettings"]["network"], "grpc");
    assert_eq!(t["streamSettings"]["grpcSettings"]["serviceName"], "tr");
    assert_eq!(
        t["streamSettings"]["grpcSettings"]["authority"],
        "trojan.example.com"
    );
    assert_eq!(t["streamSettings"]["grpcSettings"]["multiMode"], true);
    let ss = outbound(&v, "pz-srv-1-0");
    assert_eq!(ss["protocol"], "shadowsocks");
    assert_eq!(ss["settings"]["method"], "2022-blake3-aes-128-gcm");
    assert_eq!(ss["streamSettings"]["security"], "none");

    let (_, v) = build(
        profile(&sub, 3),
        &table(&[("ws.example.com", &["203.0.113.43"])]),
    )
    .unwrap();
    let w = outbound(&v, "pz-srv-0-0");
    assert_eq!(w["streamSettings"]["wsSettings"]["host"], "ws.example.com");
    assert_eq!(w["streamSettings"]["wsSettings"]["path"], "/ws");
    assert_eq!(
        w["streamSettings"]["tlsSettings"]["serverName"],
        "ws.example.com"
    );
}

#[test]
fn explicit_host_and_server_name_from_subscription_win() {
    let body = br#"[{"outbounds":[{"tag":"a","protocol":"vless","settings":{"address":"edge.example.com","port":443,"id":"49e24ff0-cdfc-417e-9a01-f1687647e767","encryption":"none"},
      "streamSettings":{"network":"ws","security":"tls","tlsSettings":{"serverName":"front.example.com"},"wsSettings":{"path":"/p","host":"Real.Example.com"}}}]}]"#;
    let sub = parse_subscription(body).unwrap();
    let (_, v) = build(
        profile(&sub, 0),
        &table(&[("edge.example.com", &["203.0.113.60"])]),
    )
    .unwrap();
    let o = outbound(&v, "pz-srv-0-0");
    assert_eq!(
        o["streamSettings"]["tlsSettings"]["serverName"],
        "front.example.com"
    );
    assert_eq!(
        o["streamSettings"]["wsSettings"]["host"],
        "real.example.com"
    );

    // host пуст, serverName задан (фронтинг): host = serverName, как сделал бы сам Xray.
    let body = br#"[{"outbounds":[{"tag":"a","protocol":"vless","settings":{"address":"edge.example.com","port":443,"id":"49e24ff0-cdfc-417e-9a01-f1687647e767","encryption":"none"},
      "streamSettings":{"network":"xhttp","security":"tls","tlsSettings":{"serverName":"front.example.com"},"xhttpSettings":{"path":"/p"}}}]}]"#;
    let sub = parse_subscription(body).unwrap();
    let (_, v) = build(
        profile(&sub, 0),
        &table(&[("edge.example.com", &["203.0.113.60"])]),
    )
    .unwrap();
    assert_eq!(
        outbound(&v, "pz-srv-0-0")["streamSettings"]["xhttpSettings"]["host"],
        "front.example.com"
    );
}

#[test]
fn unusable_addresses_are_never_put_into_config() {
    let body = br#"[{"outbounds":[
        {"tag":"loop","protocol":"trojan","settings":{"address":"127.0.0.1","port":443,"password":"p"},"streamSettings":{"security":"tls"}},
        {"tag":"dom","protocol":"trojan","settings":{"address":"evil.example.com","port":443,"password":"p"},"streamSettings":{"security":"tls"}}]}]"#;
    let sub = parse_subscription(body).unwrap();
    let p = profile(&sub, 0);
    // Резолвер вернул 0.0.0.0, loopback и multicast: пригодных адресов нет.
    let r = table(&[("evil.example.com", &["0.0.0.0", "127.0.0.1", "224.0.0.1"])]);
    assert_eq!(build(p, &r).map(|_| ()).unwrap_err(), XrayError::NoServers);
    // Дубликаты и лимит на число IP.
    let many: Vec<String> = (1..=20).map(|i| format!("203.0.113.{i}")).collect();
    let refs: Vec<&str> = many
        .iter()
        .map(String::as_str)
        .chain(["203.0.113.1"])
        .collect();
    let r = table(&[("evil.example.com", &refs)]);
    let (b, _) = build(p, &r).unwrap();
    assert_eq!(b.server_outbounds, MAX_IPS_PER_DOMAIN);
}

#[test]
fn params_are_validated() {
    let sub = load_profiles("happ_full.json");
    let p = profile(&sub, 0);
    let r = germany_table();
    let pw = Secret::new("p");
    let empty = Secret::new("");
    let mk = |port: u16,
              user: &'static str,
              pw: &Secret,
              iface: &'static str,
              probe: Option<&'static str>| {
        build_config(
            p,
            &XrayParams {
                socks_port: port,
                socks_user: user,
                socks_password: pw,
                interface: iface,
                resolved: &r,
                probe_url: probe,
            },
        )
        .map(|_| ())
    };
    assert_eq!(mk(0, "u", &pw, "en0", None), Err(XrayError::InvalidPort));
    assert_eq!(
        mk(1, "", &pw, "en0", None),
        Err(XrayError::InvalidCredentials)
    );
    assert_eq!(
        mk(1, "u", &empty, "en0", None),
        Err(XrayError::InvalidCredentials)
    );
    assert_eq!(mk(1, "u", &pw, "", None), Err(XrayError::InvalidInterface));
    assert_eq!(
        mk(1, "u", &pw, "en\n0", None),
        Err(XrayError::InvalidInterface)
    );
    assert_eq!(
        mk(1, "u", &pw, "en0", Some("http://x.example.com/204")),
        Err(XrayError::InvalidProbeUrl)
    );
    assert_eq!(
        mk(1, "u", &pw, "en0", Some("https://u:p@x.example.com/204")),
        Err(XrayError::InvalidProbeUrl)
    );
    assert!(
        mk(
            1,
            "u",
            &pw,
            "Wi-Fi 2",
            Some("https://probe.example.net/204")
        )
        .is_ok()
    );
}

#[test]
fn build_is_deterministic_and_debug_hides_secrets() {
    let sub = load_profiles("happ_full.json");
    let p = profile(&sub, 0);
    let r = germany_table();
    let (a, _) = build(p, &r).unwrap();
    let (b, _) = build(p, &r).unwrap();
    assert_eq!(a.json, b.json);
    let dbg = format!("{a:?}");
    for s in [
        PASS,
        "49e24ff0-cdfc-417e-9a01-f1687647e767",
        "hy-auth-PLACEHOLDER-1",
        "22N5CfHXOd5uVoEPntAVNd47bq75",
    ] {
        assert!(!dbg.contains(s), "{s} в Debug");
    }
    assert!(a.json.expose().contains(PASS));
}
