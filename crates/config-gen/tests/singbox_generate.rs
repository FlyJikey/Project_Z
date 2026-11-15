//! Структура сгенерированного конфига sing-box (без запуска ядра).
#![allow(clippy::expect_used, clippy::unwrap_used)]

#[path = "fixtures/singbox/variants.rs"]
mod variants;

use config_gen::singbox::{
    CachePath, ClashApi, LogLevel, ProxyId, ProxyPool, SELECTOR_TAG, SingboxError, SingboxInput,
    VpnServerIps, XRAY_OUTBOUND_TAG, XraySocks, build_config, proxy_delay_path, proxy_id_from_tag,
    proxy_tag, render_config, selector_path, selector_tag,
};
use ipc_proto::Secret;
use serde_json::Value;
use variants::*;

fn build(input: &SingboxInput) -> Value {
    build_config(input).expect("build")
}

fn outbound<'a>(cfg: &'a Value, tag: &str) -> &'a Value {
    cfg["outbounds"]
        .as_array()
        .expect("outbounds")
        .iter()
        .find(|o| o["tag"] == tag)
        .unwrap_or_else(|| panic!("нет outbound {tag}"))
}

fn exclude(cfg: &Value) -> Vec<&str> {
    cfg["inbounds"][0]["route_exclude_address"]
        .as_array()
        .expect("exclude")
        .iter()
        .map(|v| v.as_str().expect("str"))
        .collect()
}

fn rules(cfg: &Value) -> &Vec<Value> {
    cfg["route"]["rules"].as_array().expect("rules")
}

#[test]
fn one_socks5_structure() {
    let cfg = build(&base());

    let tun = &cfg["inbounds"][0];
    assert_eq!(cfg["inbounds"].as_array().unwrap().len(), 1);
    assert_eq!(tun["type"], "tun");
    assert_eq!(tun["auto_route"], true);
    assert_eq!(tun["strict_route"], true);
    assert_eq!(tun["stack"], "system");
    assert_eq!(tun["dns_mode"], "disabled");
    let addr = tun["address"].as_array().unwrap();
    assert!(
        addr.iter().any(|a| a.as_str().unwrap().contains('.')),
        "нет IPv4"
    );
    assert!(
        addr.iter().any(|a| a.as_str().unwrap().contains(':')),
        "нет IPv6"
    );

    let xray = outbound(&cfg, XRAY_OUTBOUND_TAG);
    assert_eq!(xray["type"], "socks");
    assert_eq!(xray["server"], "127.0.0.1");
    assert_eq!(xray["server_port"], 10808);
    assert_eq!(xray["version"], "5");
    assert_eq!(xray["username"], "__XRAY_SOCKS_USER__");
    assert_eq!(xray["password"], XRAY_PASSWORD);
    assert!(xray.get("detour").is_none());

    let p = outbound(&cfg, "proxy-p1");
    assert_eq!(p["type"], "socks");
    assert_eq!(p["server"], "198.51.100.11");
    assert_eq!(p["server_port"], 1080);
    assert_eq!(p["version"], "5", "version — строка");
    assert_eq!(p["detour"], "xray");
    assert_eq!(p["password"], PROXY1_PASSWORD);
    assert!(p.get("domain_resolver").is_none());

    let sel = outbound(&cfg, SELECTOR_TAG);
    assert_eq!(sel["type"], "selector");
    assert_eq!(sel["outbounds"], serde_json::json!(["proxy-p1"]));
    assert_eq!(sel["default"], "proxy-p1");
    assert_eq!(sel["interrupt_exist_connections"], true);
    assert_eq!(cfg["route"]["final"], SELECTOR_TAG);

    assert_eq!(
        cfg["experimental"]["clash_api"]["external_controller"],
        "127.0.0.1:9090"
    );
    assert_eq!(cfg["experimental"]["clash_api"]["secret"], CLASH_SECRET);
    assert_eq!(cfg["experimental"]["cache_file"]["store_fakeip"], true);
    assert_eq!(
        cfg["experimental"]["cache_file"]["path"],
        "/var/lib/projectz/sing-box-cache.db"
    );
}

#[test]
fn no_direct_no_resolvers_no_deprecated() {
    for (name, input) in variants() {
        let cfg = build(&input);
        for o in cfg["outbounds"].as_array().unwrap() {
            let t = o["type"].as_str().unwrap();
            assert!(
                !matches!(t, "direct" | "block" | "dns"),
                "{name}: outbound {t}"
            );
            if o["tag"] != XRAY_OUTBOUND_TAG && o["type"] != "selector" {
                assert_eq!(o["detour"], "xray", "{name}");
            }
            assert!(o.get("domain_resolver").is_none(), "{name}");
            assert!(o.get("domain_strategy").is_none(), "{name}");
        }
        assert!(
            cfg["route"].get("default_domain_resolver").is_none(),
            "{name}"
        );
        let text = render_config(&input).unwrap();
        for bad in [
            "independent_cache",
            "inet4_address",
            "inet6_address",
            "store_mode",
            "store_selected",
            "external_ui",
            "geoip",
            "geosite",
            "inet6_range",
        ] {
            assert!(!text.contains(bad), "{name}: найдено {bad}");
        }
        // Все детуры и final указывают на существующие теги.
        let tags: Vec<&str> = cfg["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["tag"].as_str().unwrap())
            .collect();
        for o in cfg["outbounds"].as_array().unwrap() {
            if let Some(d) = o["detour"].as_str() {
                assert!(tags.contains(&d), "{name}: detour {d}");
            }
        }
        assert!(tags.contains(&cfg["route"]["final"].as_str().unwrap()));
        assert!(tags.contains(&cfg["dns"]["servers"][0]["detour"].as_str().unwrap()));
    }
}

#[test]
fn dns_section() {
    let cfg = build(&base());
    let dns = &cfg["dns"];
    let servers = dns["servers"].as_array().unwrap();
    let doh = servers.iter().find(|s| s["tag"] == "doh").unwrap();
    assert_eq!(doh["type"], "https");
    assert_eq!(doh["detour"], "proxy");
    assert!(doh.get("domain_resolver").is_none());
    let fake = servers.iter().find(|s| s["tag"] == "fakeip").unwrap();
    assert_eq!(fake["type"], "fakeip");
    assert!(fake.get("inet6_range").is_none());
    assert_eq!(dns["final"], "doh");
    let r = dns["rules"].as_array().unwrap();
    assert_eq!(r[0]["query_type"], serde_json::json!(["AAAA"]));
    assert_eq!(r[0]["action"], "predefined");
    assert_eq!(r[0]["rcode"], "NOERROR");
    assert_eq!(r[1]["query_type"], serde_json::json!(["A"]));
    assert_eq!(r[1]["server"], "fakeip");
}

#[test]
fn route_rules_order() {
    let cfg = build(&base());
    let r = rules(&cfg);
    assert_eq!(r[0]["action"], "sniff");
    assert_eq!(r[1]["action"], "hijack-dns");
    assert_eq!(r[1]["protocol"], "dns");
    assert_eq!(r[2]["ip_version"], 6);
    assert_eq!(r[2]["action"], "reject");
    assert_eq!(cfg["route"]["auto_detect_interface"], false);
}

#[test]
fn udp_off_rejects_after_hijack_dns() {
    let cfg = build(&SingboxInput {
        udp_allowed: false,
        ..base()
    });
    let r = rules(&cfg);
    let hijack = r.iter().position(|x| x["action"] == "hijack-dns").unwrap();
    let udp = r
        .iter()
        .position(|x| x["network"] == "udp" && x["action"] == "reject")
        .expect("правило reject UDP");
    assert!(hijack < udp, "DNS должен перехватываться до отклонения UDP");
    assert_eq!(outbound(&cfg, "proxy-p1")["network"], "tcp");
    // DNS-перехват на месте.
    assert!(r.iter().any(|x| x["action"] == "hijack-dns"));
}

#[test]
fn udp_on_has_no_udp_reject() {
    let cfg = build(&SingboxInput {
        udp_allowed: true,
        ..base()
    });
    assert!(!rules(&cfg).iter().any(|x| x["network"] == "udp"));
    assert!(outbound(&cfg, "proxy-p1").get("network").is_none());
}

#[test]
fn udp_on_with_http_proxy_is_error() {
    let mut input = with_pool(vec![
        (id("a"), socks("198.51.100.11", 1080, None)),
        (id("b"), http("198.51.100.12", 8080, None)),
    ]);
    input.udp_allowed = true;
    assert_eq!(build_config(&input), Err(SingboxError::UdpWithHttpProxy));
}

#[test]
fn two_proxies_socks_and_http() {
    let input = variants()
        .into_iter()
        .find(|(n, _)| *n == "two_socks5_http")
        .unwrap()
        .1;
    let cfg = build(&input);
    let p1 = outbound(&cfg, "proxy-p1");
    let p2 = outbound(&cfg, "proxy-p2");
    assert_eq!(p1["type"], "socks");
    assert_eq!(p2["type"], "http");
    assert!(p2.get("version").is_none(), "у http в 1.14.4 нет version");
    assert_eq!(p2["detour"], "xray");
    assert_eq!(p2["username"], "__PROXY2_USER__");
    let sel = outbound(&cfg, SELECTOR_TAG);
    assert_eq!(
        sel["outbounds"],
        serde_json::json!(["proxy-p1", "proxy-p2"])
    );
    assert_eq!(
        sel["default"], "proxy-p1",
        "default — первый, порядок = приоритет"
    );
}

#[test]
fn priority_order_follows_input_and_default_is_first() {
    let cfg = build(&with_pool(vec![
        (id("z"), socks("198.51.100.1", 1, None)),
        (id("a"), socks("198.51.100.2", 1, None)),
    ]));
    let sel = outbound(&cfg, SELECTOR_TAG);
    assert_eq!(sel["outbounds"], serde_json::json!(["proxy-z", "proxy-a"]));
    assert_eq!(sel["default"], "proxy-z");
}

#[test]
fn domain_proxy_is_normalized_and_unresolved() {
    let cfg = build(&with_pool(vec![(
        id("d"),
        socks("Proxy1.Example.NET", 1080, None),
    )]));
    let p = outbound(&cfg, "proxy-d");
    assert_eq!(p["server"], "proxy1.example.net");
    assert!(p.get("domain_resolver").is_none());
    assert_eq!(p["detour"], "xray");
    assert!(p.get("username").is_none());
}

#[test]
fn ipv6_proxy_without_brackets() {
    let cfg = build(&with_pool(vec![(
        id("v6"),
        socks("[2001:DB8::11]", 1080, None),
    )]));
    assert_eq!(outbound(&cfg, "proxy-v6")["server"], "2001:db8::11");
}

#[test]
fn lan_on_off() {
    let on = build(&SingboxInput {
        allow_lan: true,
        ..base()
    });
    let off = build(&SingboxInput {
        allow_lan: false,
        ..base()
    });
    let on_ex = exclude(&on);
    for p in [
        "10.0.0.0/8",
        "172.16.0.0/12",
        "192.168.0.0/16",
        "169.254.0.0/16",
        "fc00::/7",
        "fe80::/10",
    ] {
        assert!(on_ex.contains(&p), "LAN вкл: нет {p}");
    }
    assert!(on_ex.contains(&"203.0.113.10/32"));
    assert_eq!(
        exclude(&off),
        vec!["203.0.113.10/32"],
        "LAN выкл: только IP VPN"
    );
}

#[test]
fn many_vpn_ips_sorted_deduped_with_prefixes() {
    let input = variants()
        .into_iter()
        .find(|(n, _)| *n == "many_vpn_ips")
        .unwrap()
        .1;
    let off = build(&SingboxInput {
        allow_lan: false,
        ..input
    });
    let ex = exclude(&off);
    assert_eq!(ex.len(), 80);
    assert!(ex.contains(&"203.0.113.60/32"));
    assert!(ex.contains(&"2001:db8:1::14/128"));

    // Порядок и повторы на входе не влияют на результат.
    let a = SingboxInput {
        vpn_server_ips: ips(&["203.0.113.2", "203.0.113.1", "203.0.113.2"]),
        ..base()
    };
    let b = SingboxInput {
        vpn_server_ips: ips(&["203.0.113.1", "203.0.113.2"]),
        ..base()
    };
    assert_eq!(render_config(&a).unwrap(), render_config(&b).unwrap());
}

#[test]
fn log_level_and_cache_path() {
    let cfg = build(&SingboxInput {
        log_level: LogLevel::Warn,
        ..base()
    });
    assert_eq!(cfg["log"]["level"], "warn");
    let win = variants()
        .into_iter()
        .find(|(n, _)| *n == "windows_cache_path")
        .unwrap()
        .1;
    assert_eq!(
        build(&win)["experimental"]["cache_file"]["path"],
        "C:\\ProgramData\\ProjectZ\\sing-box-cache.db"
    );
}

#[test]
fn deterministic_byte_for_byte() {
    for (name, input) in variants() {
        let a = render_config(&input).unwrap();
        let b = render_config(&input.clone()).unwrap();
        assert_eq!(a, b, "{name}");
        assert!(a.ends_with("}\n"));
        let parsed: Value = serde_json::from_str(&a).expect("валидный JSON");
        assert_eq!(parsed, build(&input), "{name}");
    }
}

#[test]
fn golden_two_proxies() {
    let input = variants()
        .into_iter()
        .find(|(n, _)| *n == "two_socks5_http")
        .unwrap()
        .1;
    let actual = render_config(&input).unwrap();
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/singbox/two_socks5_http.json"
        );
        std::fs::write(path, &actual).unwrap();
    }
    let expected = include_str!("fixtures/singbox/two_socks5_http.json");
    assert_eq!(
        actual, expected,
        "эталон tests/fixtures/singbox/two_socks5_http.json устарел"
    );
}

// ---------- валидация ----------

#[test]
fn empty_pool_rejected() {
    assert_eq!(ProxyPool::new(vec![]), Err(SingboxError::EmptyPool));
}

#[test]
fn duplicate_ids_rejected_case_insensitive() {
    let e = ProxyPool::new(vec![
        (id("a"), socks("198.51.100.1", 1, None)),
        (id("A"), socks("198.51.100.2", 1, None)),
    ])
    .unwrap_err();
    assert!(matches!(e, SingboxError::DuplicateProxyId { .. }));
}

#[test]
fn invalid_ids_rejected() {
    for bad in ["", "a b", "a/b", "прокси", "a.b", "a\"b", &"x".repeat(65)] {
        assert_eq!(
            ProxyId::new(bad),
            Err(SingboxError::InvalidProxyId),
            "{bad:?}"
        );
    }
}

#[test]
fn proxy_validation_errors() {
    let one = |ep| ProxyPool::new(vec![(id("a"), ep)]).unwrap_err();
    assert_eq!(
        one(socks("198.51.100.1", 0, None)),
        SingboxError::ProxyPortZero { position: 1 }
    );
    for bad in [
        "",
        "a b",
        "user:pass@host.com",
        "host.com:1080",
        "http://h.com",
        "a..b",
        "127.0.0.1",
        "localhost",
        "::",
        "-x.com",
        "пример.рф",
        "999.1.1.1",
    ] {
        assert_eq!(
            one(socks(bad, 1080, None)),
            SingboxError::InvalidProxyHost { position: 1 },
            "{bad:?}"
        );
    }
    let cred = |ep| {
        matches!(
            one(ep),
            SingboxError::InvalidProxyCredentials { position: 1, .. }
        )
    };
    let mut only_user = socks("198.51.100.1", 1, None);
    only_user.username = Some("u".into());
    assert!(cred(only_user));
    let mut only_pass = socks("198.51.100.1", 1, None);
    only_pass.password = Some(Secret::new("p"));
    assert!(cred(only_pass));
    assert!(cred(socks("198.51.100.1", 1, Some(("", "p")))));
    assert!(cred(socks("198.51.100.1", 1, Some(("u", "")))));
    assert!(cred(http("198.51.100.1", 1, Some(("a:b", "p")))));
    assert!(cred(socks("198.51.100.1", 1, Some(("u\n", "p")))));
}

#[test]
fn error_position_is_one_based_and_points_to_entry() {
    let e = ProxyPool::new(vec![
        (id("a"), socks("198.51.100.1", 1, None)),
        (id("b"), socks("bad host", 1, None)),
    ])
    .unwrap_err();
    assert_eq!(e, SingboxError::InvalidProxyHost { position: 2 });
}

#[test]
fn other_input_validation() {
    let s = || Secret::new("x");
    assert_eq!(
        XraySocks::new(0, "u", s()).unwrap_err(),
        SingboxError::XrayPortZero
    );
    assert!(XraySocks::new(1, "", s()).is_err());
    assert!(XraySocks::new(1, "u", Secret::new("")).is_err());
    assert_eq!(
        ClashApi::new(0, Secret::new(CLASH_SECRET)).unwrap_err(),
        SingboxError::ClashPortZero
    );
    assert!(matches!(
        ClashApi::new(1, Secret::new("short")).unwrap_err(),
        SingboxError::InvalidClashSecret { .. }
    ));
    assert!(CachePath::new("relative/cache.db").is_err());
    assert!(CachePath::new("").is_err());
    assert!(CachePath::new("/a/../b.db").is_err());
    assert_eq!(
        VpnServerIps::new([]).unwrap_err(),
        SingboxError::NoVpnServerIps
    );
    for bad in [
        "0.0.0.0",
        "127.0.0.1",
        "::1",
        "224.0.0.1",
        "::ffff:127.0.0.1",
    ] {
        assert_eq!(
            VpnServerIps::new([bad.parse().unwrap()]).unwrap_err(),
            SingboxError::InvalidVpnServerIp,
            "{bad}"
        );
    }
}

#[test]
fn port_conflict_rejected() {
    let input = SingboxInput {
        xray: XraySocks::new(9090, "u", Secret::new("p")).unwrap(),
        ..base()
    };
    assert_eq!(build_config(&input), Err(SingboxError::PortConflict));
}

// ---------- секреты ----------

#[test]
fn no_secrets_in_debug_and_errors() {
    let secrets = [
        XRAY_PASSWORD,
        CLASH_SECRET,
        PROXY1_PASSWORD,
        PROXY2_PASSWORD,
        "__PROXY1_USER__",
        "__XRAY_SOCKS_USER__",
    ];
    for (name, input) in variants() {
        let dbg = format!("{input:?} {:#?}", input);
        for s in secrets {
            assert!(!dbg.contains(s), "{name}: Debug содержит секрет {s}");
        }
        for e in input.pool.entries() {
            let d = format!("{e:?}");
            assert!(!d.contains("PASSWORD") && !d.contains("USER"));
        }
    }
    // Ошибки не содержат введённых значений, даже когда в поле хоста попал пароль.
    let leaked = "user:SuperSecretPw@host.com";
    let e = ProxyPool::new(vec![(
        id("a"),
        socks(leaked, 1, Some(("login-xyz", "SuperSecretPw"))),
    )])
    .unwrap_err();
    for text in [format!("{e}"), format!("{e:?}")] {
        assert!(
            !text.contains("SuperSecretPw") && !text.contains("login-xyz"),
            "{text}"
        );
    }
    let e = ProxyPool::new(vec![(
        id("a"),
        socks("198.51.100.1", 1, Some(("login-xyz", ""))),
    )])
    .unwrap_err();
    assert!(!format!("{e} {e:?}").contains("login-xyz"));
    let e = ClashApi::new(1, Secret::new("tiny-secret")).unwrap_err();
    assert!(!format!("{e} {e:?}").contains("tiny-secret"));
    let e = CachePath::new("relative-secret-dir/x").unwrap_err();
    assert!(!format!("{e} {e:?}").contains("secret-dir"));
}

// ---------- помощники для Clash API ----------

#[test]
fn tag_helpers() {
    let id = id("abc-1_X");
    assert_eq!(proxy_tag(&id), "proxy-abc-1_X");
    assert_eq!(proxy_id_from_tag("proxy-abc-1_X"), Some(id.clone()));
    assert_eq!(proxy_id_from_tag("xray"), None);
    assert_eq!(proxy_id_from_tag("proxy-"), None);
    assert_eq!(selector_tag(), SELECTOR_TAG);
    assert_eq!(selector_path(), "/proxies/proxy");
    assert_eq!(proxy_delay_path(&id), "/proxies/proxy-abc-1_X/delay");
    // Тег в конфиге совпадает с тем, что вернёт функция.
    let cfg = build(&with_pool(vec![(
        id.clone(),
        socks("198.51.100.1", 1, None),
    )]));
    assert_eq!(outbound(&cfg, &proxy_tag(&id))["server"], "198.51.100.1");
}
