//! Разбор правдоподобной подписки Happ (ТЗ §5.1, §15).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use config_gen::subscription::*;
use ipc_proto::ProfileSupport;

fn load(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/fixtures/subscription/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn cfg(sub: &ParsedSubscription, i: usize) -> &ProfileConfig {
    match &sub.profiles[i].outcome {
        ProfileOutcome::Supported(c) => c,
        ProfileOutcome::Unsupported { reason } => panic!("профиль {i} не поддерживается: {reason}"),
    }
}

fn proxy(node: &Node) -> &ProxyOutbound {
    match &node.spec {
        OutboundSpec::Proxy(p) => p,
        OutboundSpec::Freedom(_) => panic!("ожидался прокси"),
    }
}

/// Все секреты фикстуры: не должны попадать ни в предупреждения, ни в Debug.
const SECRETS: &[&str] = &[
    "49e24ff0-cdfc-417e-9a01-f1687647e767",
    "c32c8fdc-face-4527-9c7d-86d1cfbfda94",
    "a87fcb69-7705-4a6e-bd81-e6d572208210",
    "9bee537c-a1e0-4ac6-bf36-b88ce7f4d112",
    "2e9a133c-1ace-46c6-a2ae-883ce30f52cb",
    "b8cea2f8-32c0-402e-a98c-4910b21e6b20",
    "hy-auth-PLACEHOLDER-1",
    "TROJAN-PASSWORD-PLACEHOLDER",
    "AAECAwQFBgcICQoLDA0ODw==",
    "22N5CfHXOd5uVoEPntAVNd47bq75-YyX9tha_PMPfxg",
    "8G1ip5xc6YHbWk-S0EpR5DyFAKTpD_Hai4a0q1XxvC0",
    "0123456789abcdef",
];

#[test]
fn full_subscription_parses_into_four_supported_profiles() {
    let sub = parse_subscription(&load("happ_full.json")).unwrap();
    assert_eq!(sub.profiles.len(), 4);
    for i in 0..4 {
        cfg(&sub, i);
    }
    let names: Vec<&str> = sub.profiles.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "🇩🇪 Germany #1",
            "🇺🇸 USA (bridge)",
            "📱 Mobile (no balancer)",
            "Observatory only"
        ]
    );
}

#[test]
fn germany_keeps_balancer_servers_and_fragment_dialer_only() {
    let sub = parse_subscription(&load("happ_full.json")).unwrap();
    let c = cfg(&sub, 0);
    let tags: Vec<_> = c
        .nodes
        .iter()
        .map(|n| n.source_tag.clone().unwrap())
        .collect();
    // direct, block, dns-out отброшены; fragment оставлен как зависимость de-frag.
    assert_eq!(
        tags,
        ["de-reality", "de-xhttp", "de-hy", "de-frag", "fragment"]
    );
    assert_eq!(c.entries, [0, 1, 2, 3]);
    assert_eq!(c.server_count(), 4);
    assert_eq!(c.strategy, BalancerStrategy::LeastPing);
    assert_eq!(
        c.probe,
        ProbeConfig {
            interval_secs: 30,
            sampling: 5,
            timeout_secs: 4
        }
    );

    // de-frag ссылается на fragment (legacy vnext прочитан).
    assert_eq!(c.nodes[3].dialer, Some(4));
    match &c.nodes[4].spec {
        OutboundSpec::Freedom(f) => {
            let fr = f.fragment.as_ref().unwrap();
            assert_eq!(
                (
                    fr.packets.as_str(),
                    fr.length.as_str(),
                    fr.interval.as_str()
                ),
                ("tlshello", "100-200", "10-20")
            );
        }
        OutboundSpec::Proxy(_) => panic!("fragment должен быть freedom"),
    }
    let frag = proxy(&c.nodes[3]);
    assert_eq!(frag.address, "frag.example.com");
    assert!(matches!(
        &frag.protocol,
        ProxyProtocol::Vless { flow: None, .. }
    ));
    // Домены для резолва: IP-литерал (hysteria) не включается, порядок стабильный.
    assert_eq!(
        c.server_domains(),
        ["de1.example.com", "cdn-de.example.com", "frag.example.com"]
    );
}

#[test]
fn reality_xhttp_hysteria_fields_are_kept_strictly() {
    let sub = parse_subscription(&load("happ_full.json")).unwrap();
    let c = cfg(&sub, 0);

    let r = proxy(&c.nodes[0]);
    assert_eq!(r.port, 443);
    match (&r.protocol, &r.stream.security, &r.stream.transport) {
        (
            ProxyProtocol::Vless {
                flow, encryption, ..
            },
            Security::Reality(rs),
            Transport::Raw,
        ) => {
            assert_eq!(flow.as_deref(), Some("xtls-rprx-vision"));
            assert_eq!(encryption, "none");
            // Старое имя publicKey принято.
            assert_eq!(
                rs.public_key.expose(),
                "22N5CfHXOd5uVoEPntAVNd47bq75-YyX9tha_PMPfxg"
            );
            assert_eq!(rs.server_name.as_deref(), Some("www.example.com"));
            assert_eq!(rs.fingerprint, "chrome");
        }
        other => panic!("неожиданно: {other:?}"),
    }
    // sockopt.interface/mark/domainStrategy отброшены, безопасное поле осталось.
    assert_eq!(
        r.stream.tcp,
        TcpOpts {
            tcp_fast_open: Some(true),
            ..TcpOpts::default()
        }
    );

    let x = proxy(&c.nodes[1]);
    match (&x.stream.security, &x.stream.transport) {
        (Security::Tls(t), Transport::Xhttp(xs)) => {
            assert_eq!(
                t.server_name, None,
                "подписка не задаёт serverName: подставит генератор"
            );
            assert_eq!(t.alpn, ["h2"]);
            assert_eq!(xs.host, None);
            assert_eq!(xs.path, "/xh");
            assert_eq!(xs.mode.as_deref(), Some("auto"));
            // Поля из `extra` поднялись, xmux разобран.
            assert_eq!(xs.params["xPaddingBytes"], "100-1000");
            assert_eq!(xs.xmux["maxConcurrency"], "16-32");
        }
        other => panic!("неожиданно: {other:?}"),
    }

    let h = proxy(&c.nodes[2]);
    assert_eq!(h.address, "203.0.113.50");
    assert!(matches!(h.protocol, ProxyProtocol::Hysteria));
    match (&h.stream.security, &h.stream.transport) {
        (Security::Tls(t), Transport::Hysteria(hs)) => {
            assert_eq!(t.server_name.as_deref(), Some("hy.example.com"));
            assert_eq!(hs.auth.expose(), "hy-auth-PLACEHOLDER-1");
            assert_eq!(hs.udp_idle_timeout, Some(60));
        }
        other => panic!("неожиданно: {other:?}"),
    }
}

#[test]
fn bridge_profile_keeps_dependency_chain() {
    let sub = parse_subscription(&load("happ_full.json")).unwrap();
    let c = cfg(&sub, 1);
    // Селектор `us-exit` выбирает только выход; мост остался как зависимость.
    assert_eq!(c.nodes.len(), 2);
    assert_eq!(c.entries, [1]);
    assert_eq!(c.nodes[0].source_tag.as_deref(), Some("us-bridge-1"));
    assert_eq!(c.nodes[1].dialer, Some(0));
    assert_eq!(c.nodes[0].dialer, None);
    assert_eq!(c.strategy, BalancerStrategy::Random);
    assert_eq!(c.probe.interval_secs, 60);
    assert_eq!(
        c.server_domains(),
        ["bridge.example.com", "exit.example.com"]
    );
}

#[test]
fn profile_without_balancer_uses_all_servers() {
    let sub = parse_subscription(&load("happ_full.json")).unwrap();
    let c = cfg(&sub, 2);
    assert_eq!(c.nodes.len(), 2, "direct и block отброшены");
    assert_eq!(c.entries, [0, 1]);
    let t = proxy(&c.nodes[0]);
    assert!(
        matches!(t.stream.transport, Transport::Grpc(ref g) if g.service_name == "tr" && g.multi_mode == Some(true))
    );
    assert!(matches!(
        proxy(&c.nodes[1]).protocol,
        ProxyProtocol::Shadowsocks { .. }
    ));
    assert_eq!(c.server_domains(), ["trojan.example.com"]);
}

#[test]
fn observatory_selector_picks_servers_when_no_balancer() {
    let sub = parse_subscription(&load("happ_full.json")).unwrap();
    let c = cfg(&sub, 3);
    assert_eq!(c.entries, [0]);
    assert_eq!(c.probe.interval_secs, 30);
    let w = proxy(&c.nodes[0]);
    assert!(matches!(&w.protocol, ProxyProtocol::Vmess { security, .. } if security == "auto"));
    assert!(
        matches!(&w.stream.transport, Transport::Ws(ws) if ws.path == "/ws" && ws.headers.contains_key("User-Agent"))
    );
}

#[test]
fn dropped_things_are_reported_without_values_or_secrets() {
    let sub = parse_subscription(&load("happ_full.json")).unwrap();
    let text: String = sub.warnings.iter().map(|w| format!("{w}\n")).collect();
    for needle in [
        "routing.rules (3)",
        "dns",
        "inbounds",
        "log",
        "mark",
        "interface",
        "mux",
        "fragment",
        "quicParams",
        "domainStrategy",
    ] {
        assert!(
            text.contains(needle),
            "нет предупреждения про {needle}:\n{text}"
        );
    }
    for s in SECRETS.iter().chain(&[
        "/var/log/xray",
        "geosite",
        "yandex",
        "0.0.0.0",
        "eth0",
        "utun",
    ]) {
        assert!(
            !text.contains(s),
            "значение {s} попало в предупреждения:\n{text}"
        );
    }
}

#[test]
fn debug_output_has_no_secrets() {
    let sub = parse_subscription(&load("happ_full.json")).unwrap();
    let dbg = format!("{sub:?}");
    for s in SECRETS {
        assert!(!dbg.contains(s), "секрет {s} напечатан в Debug");
    }
    assert!(dbg.contains("Secret(***)"));
}

#[test]
fn profile_views_for_ui() {
    let sub = parse_subscription(&load("happ_full.json")).unwrap();
    let v = profile_views(&sub);
    assert_eq!(v.len(), 4);
    assert_eq!((v[0].index, v[0].server_count), (0, 4));
    assert_eq!(v[1].server_count, 1);
    assert!(v.iter().all(|p| p.support == ProfileSupport::Supported));
}

#[test]
fn single_config_object_is_accepted_as_one_profile() {
    let body = br#"{"remarks":"solo","outbounds":[{"tag":"a","protocol":"trojan","settings":{"address":"a.example.com","port":443,"password":"p"},"streamSettings":{"security":"tls"}}]}"#;
    let sub = parse_subscription(body).unwrap();
    assert_eq!(sub.profiles.len(), 1);
    cfg(&sub, 0);
}
