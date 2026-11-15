//! Мусорные и вредоносные подписки (ТЗ §5.1, §15).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use config_gen::subscription::*;

fn load(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/subscription/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn hostile() -> ParsedSubscription {
    parse_subscription(&load("hostile.json")).unwrap()
}

fn by_name<'a>(sub: &'a ParsedSubscription, prefix: &str) -> &'a Profile {
    sub.profiles
        .iter()
        .find(|p| p.name.starts_with(prefix))
        .unwrap_or_else(|| panic!("нет профиля {prefix}"))
}

fn supported<'a>(sub: &'a ParsedSubscription, prefix: &str) -> &'a ProfileConfig {
    match &by_name(sub, prefix).outcome {
        ProfileOutcome::Supported(c) => c,
        ProfileOutcome::Unsupported { reason } => panic!("{prefix}: не поддерживается: {reason}"),
    }
}

fn reason<'a>(sub: &'a ParsedSubscription, prefix: &str) -> &'a str {
    match &by_name(sub, prefix).outcome {
        ProfileOutcome::Unsupported { reason } => reason,
        ProfileOutcome::Supported(_) => panic!("{prefix}: ожидался отказ"),
    }
}

fn warnings_of(sub: &ParsedSubscription, profile: u32) -> String {
    sub.warnings
        .iter()
        .filter(|w| w.profile == Some(profile))
        .map(|w| format!("{}\n", w.message))
        .collect()
}

fn index_of(sub: &ParsedSubscription, prefix: &str) -> u32 {
    by_name(sub, prefix).index
}

#[test]
fn file_paths_api_policy_loopback_dns_are_dropped() {
    let sub = hostile();
    let c = supported(&sub, "00 ");
    // Остался только ok-1: loopback, dns, wireguard, unix-сокет и freedom не серверы.
    assert_eq!(c.nodes.len(), 1);
    assert_eq!(c.entries, [0]);
    let w = warnings_of(&sub, index_of(&sub, "00 "));
    for needle in [
        "api",
        "policy",
        "stats",
        "unknownTopLevel",
        "sendThrough",
        "certificates",
        "masterKeyLog",
        "interface",
        "mark",
        "customSockopt",
        "redirect",
        "loopback",
        "wireguard",
        "evilField",
        "bogusStream",
    ] {
        assert!(w.contains(needle), "нет предупреждения про {needle}:\n{w}");
    }
    // Значения (пути, адреса, секреты) в предупреждениях и в Debug не появляются.
    let dbg = format!("{sub:?}");
    for leak in [
        "/etc/ssl/evil",
        "/tmp/keys.log",
        "/var/run/xray.sock",
        "127.0.0.1:22",
        "WG-SECRET",
        "utun9",
        "10.0.0.1",
    ] {
        assert!(
            !w.contains(leak) && !dbg.contains(leak),
            "{leak} просочилось"
        );
    }
    let all: String = sub.warnings.iter().map(|w| w.message.clone()).collect();
    for leak in [
        "/etc/ssl/evil",
        "/tmp/keys.log",
        "/var/run/xray.sock",
        "127.0.0.1:22",
        "WG-SECRET",
        "52526adb",
    ] {
        assert!(!all.contains(leak), "{leak} просочилось в предупреждения");
    }
    // Безопасные поля сохранены, небезопасные — нет.
    let OutboundSpec::Proxy(p) = &c.nodes[0].spec else {
        panic!()
    };
    match &p.stream.security {
        Security::Tls(t) => assert_eq!(t.server_name.as_deref(), Some("files.example.com")),
        other => panic!("{other:?}"),
    }
}

#[test]
fn allow_insecure_marks_only_that_outbound_unsupported() {
    let sub = hostile();
    let c = supported(&sub, "01 ");
    assert_eq!(c.entries.len(), 1);
    let OutboundSpec::Proxy(p) = &c.nodes[0].spec else {
        panic!()
    };
    assert_eq!(
        p.address, "secure.example.com",
        "allowInsecure:false не мешает"
    );
    let w = warnings_of(&sub, index_of(&sub, "01 "));
    assert!(w.contains("ai-1") && w.contains("allowInsecure"), "{w}");
    assert!(!w.contains("INSECURE-PASSWORD"));
}

#[test]
fn allow_insecure_in_any_case_and_form_makes_profile_unsupported() {
    let sub = hostile();
    let r = reason(&sub, "02 ");
    assert!(r.contains("allowInsecure"), "{r}");
    assert!(!r.contains("INSECURE-PASSWORD"));
}

#[test]
fn dialer_proxy_cycles_do_not_hang_and_are_rejected() {
    let sub = hostile();
    let r = reason(&sub, "03 ");
    assert!(r.contains("циклическая"), "{r}");
}

#[test]
fn proxy_settings_become_dialer_dependency() {
    let sub = hostile();
    let c = supported(&sub, "04 ");
    assert_eq!(c.nodes.len(), 2);
    let tags: Vec<_> = c
        .nodes
        .iter()
        .map(|n| n.source_tag.clone().unwrap())
        .collect();
    assert_eq!(tags, ["ps-exit", "ps-hop"]);
    assert_eq!(c.nodes[0].dialer, Some(1));
    let w = warnings_of(&sub, index_of(&sub, "04 "));
    assert!(
        w.contains("ps-conflict") && w.contains("proxySettings"),
        "{w}"
    );
}

#[test]
fn legacy_servers_and_vnext_forms_are_read() {
    let sub = hostile();
    let c = supported(&sub, "05 ");
    assert_eq!(c.entries.len(), 2);
    let OutboundSpec::Proxy(t) = &c.nodes[0].spec else {
        panic!()
    };
    assert_eq!(
        (t.address.as_str(), t.port),
        ("legacy.example.com", 443),
        "домен нормализован, порт-строка принят"
    );
    let OutboundSpec::Proxy(v) = &c.nodes[1].spec else {
        panic!()
    };
    assert!(
        matches!(&v.protocol, ProxyProtocol::Vmess { security, .. } if security == "aes-128-gcm")
    );
    assert_eq!(c.server_domains(), ["legacy.example.com"]);
    let w = warnings_of(&sub, index_of(&sub, "05 "));
    assert!(
        w.contains("lg-two"),
        "несколько серверов в одном outbound: {w}"
    );
}

#[test]
fn broken_profiles_are_unsupported_but_do_not_break_others() {
    let sub = hostile();
    assert_eq!(sub.profiles.len(), 13);
    assert!(reason(&sub, "06 ").contains("нет"));
    assert!(reason(&sub, "08 ").contains("outbounds"));
    for name in ["Профиль 8", "Профиль 9"] {
        assert!(reason(&sub, name).contains("не является объектом"));
    }
    // Остальные остались рабочими.
    assert!(matches!(
        by_name(&sub, "00 ").outcome,
        ProfileOutcome::Supported(_)
    ));
}

#[test]
fn freedom_matched_by_selector_is_never_a_server() {
    let sub = hostile();
    let c = supported(&sub, "09 ");
    assert_eq!(c.nodes.len(), 1);
    let OutboundSpec::Proxy(p) = &c.nodes[0].spec else {
        panic!()
    };
    assert_eq!(p.address, "fz.example.com");
    assert!(warnings_of(&sub, index_of(&sub, "09 ")).contains("freedom"));
}

#[test]
fn plaintext_and_unsupported_transports_are_rejected_per_outbound() {
    let sub = hostile();
    let c = supported(&sub, "10 ");
    assert_eq!(c.entries.len(), 1);
    assert_eq!(c.server_domains(), ["ok.example.com"]);
    let w = warnings_of(&sub, index_of(&sub, "10 "));
    for needle in [
        "pt-vless",
        "pt-ws-reality",
        "pt-kcp",
        "pt-dl",
        "pt-mask",
        "downloadSettings",
        "finalmask",
    ] {
        assert!(w.contains(needle), "{needle}:\n{w}");
    }
}

#[test]
fn freedom_fragment_in_finalmask_form_with_arrays_is_read() {
    let sub = hostile();
    let c = supported(&sub, "11 ");
    assert_eq!(c.nodes.len(), 2);
    let OutboundSpec::Freedom(f) = &c.nodes[1].spec else {
        panic!()
    };
    let fr = f.fragment.as_ref().unwrap();
    assert_eq!(
        (
            fr.packets.as_str(),
            fr.length.as_str(),
            fr.interval.as_str()
        ),
        ("tlshello", "3-5", "10-20")
    );
    assert_eq!(c.nodes[0].dialer, Some(1));
    assert!(warnings_of(&sub, index_of(&sub, "11 ")).contains("noise"));
}

// ---------------------------------------------------------------------------
// Ограничения размеров
// ---------------------------------------------------------------------------

fn trojan(tag: &str, n: usize) -> String {
    format!(
        r#"{{"tag":"{tag}{n}","protocol":"trojan","settings":{{"address":"h{n}.example.com","port":443,"password":"p"}},"streamSettings":{{"security":"tls"}}}}"#
    )
}

#[test]
fn too_many_outbounds_make_profile_unsupported() {
    let outs: Vec<String> = (0..=limits::MAX_OUTBOUNDS)
        .map(|n| trojan("s", n))
        .collect();
    let body = format!(
        r#"[{{"remarks":"big","outbounds":[{}]}},{{"remarks":"fine","outbounds":[{}]}}]"#,
        outs.join(","),
        trojan("ok", 1)
    );
    let sub = parse_subscription(body.as_bytes()).unwrap();
    assert!(matches!(
        sub.profiles[0].outcome,
        ProfileOutcome::Unsupported { .. }
    ));
    assert!(matches!(
        sub.profiles[1].outcome,
        ProfileOutcome::Supported(_)
    ));
}

#[test]
fn exactly_max_outbounds_is_fine() {
    let outs: Vec<String> = (0..limits::MAX_OUTBOUNDS).map(|n| trojan("s", n)).collect();
    let body = format!(r#"[{{"outbounds":[{}]}}]"#, outs.join(","));
    let sub = parse_subscription(body.as_bytes()).unwrap();
    assert!(
        matches!(&sub.profiles[0].outcome, ProfileOutcome::Supported(c) if c.entries.len() == limits::MAX_OUTBOUNDS)
    );
}

#[test]
fn too_many_profiles_is_an_error() {
    let body = format!("[{}]", vec!["{}"; limits::MAX_PROFILES + 1].join(","));
    assert_eq!(
        parse_subscription(body.as_bytes()).unwrap_err(),
        SubscriptionError::TooManyProfiles
    );
}

#[test]
fn oversized_body_is_rejected_without_parsing() {
    let body = vec![b' '; limits::MAX_BODY_BYTES + 1];
    assert_eq!(
        parse_subscription(&body).unwrap_err(),
        SubscriptionError::TooLarge
    );
}

#[test]
fn deeply_nested_json_is_rejected() {
    let body = format!("{}{}", "[".repeat(5000), "]".repeat(5000));
    assert!(matches!(
        parse_subscription(body.as_bytes()),
        Err(SubscriptionError::NotJson(_))
    ));
    let obj = format!(
        r#"[{{"remarks":"d","outbounds":[],"x":{}0{}}}]"#,
        "[".repeat(300),
        "]".repeat(300)
    );
    assert!(matches!(
        parse_subscription(obj.as_bytes()),
        Err(SubscriptionError::NotJson(_))
    ));
}

#[test]
fn huge_selector_array_and_long_strings_are_bounded() {
    let sel: Vec<String> = (0..1000).map(|n| format!("\"p{n}\"")).collect();
    let body = format!(
        r#"[{{"outbounds":[{}],"routing":{{"balancers":[{{"selector":[{}]}}]}}}}]"#,
        trojan("p", 1),
        sel.join(",")
    );
    let sub = parse_subscription(body.as_bytes()).unwrap();
    assert!(matches!(
        sub.profiles[0].outcome,
        ProfileOutcome::Unsupported { .. }
    ));

    let long = "я".repeat(100_000);
    let body = format!(
        r#"[{{"remarks":"{long}","outbounds":[{}]}}]"#,
        trojan("t", 1)
    );
    let sub = parse_subscription(body.as_bytes()).unwrap();
    assert!(sub.profiles[0].name.chars().count() <= limits::MAX_NAME_CHARS + 1);

    let long_pw = "x".repeat(100_000);
    let body = format!(
        r#"[{{"outbounds":[{{"tag":"a","protocol":"trojan","settings":{{"address":"a.example.com","port":443,"password":"{long_pw}"}},"streamSettings":{{"security":"tls"}}}}]}}]"#
    );
    let sub = parse_subscription(body.as_bytes()).unwrap();
    assert!(matches!(
        sub.profiles[0].outcome,
        ProfileOutcome::Unsupported { .. }
    ));
}

#[test]
fn control_characters_in_names_are_neutralised() {
    let body = format!(
        r#"[{{"remarks":"a\nb\u001b[31mred","outbounds":[{}]}}]"#,
        trojan("t", 1)
    );
    let sub = parse_subscription(body.as_bytes()).unwrap();
    assert!(!sub.profiles[0].name.chars().any(char::is_control));
}

#[test]
fn warning_count_is_capped() {
    let junk: Vec<String> = (0..200)
        .map(|n| format!(r#"{{"remarks":"j{n}","a{n}":1,"b{n}":2,"c{n}":3,"outbounds":[]}}"#))
        .collect();
    let body = format!("[{}]", junk.join(","));
    let sub = parse_subscription(body.as_bytes()).unwrap();
    assert!(sub.warnings.len() <= 257, "{}", sub.warnings.len());
}

#[test]
fn happ_crypt_and_garbage_bodies() {
    assert_eq!(
        parse_subscription(b"happ://crypt3/AAAA").unwrap_err(),
        SubscriptionError::HappEncrypted
    );
    assert!(matches!(
        parse_subscription(b"dmxlc3M6Ly8uLi4=").unwrap_err(),
        SubscriptionError::NotJson(_)
    ));
    assert_eq!(
        parse_subscription(b"\"str\"").unwrap_err(),
        SubscriptionError::NotArray
    );
    assert_eq!(
        parse_subscription(b"[]").unwrap_err(),
        SubscriptionError::Empty
    );
    assert!(
        parse_subscription(b"\xEF\xBB\xBF[{}]").is_ok(),
        "BOM допустим"
    );
}
