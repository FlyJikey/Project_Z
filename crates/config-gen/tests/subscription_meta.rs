//! Заголовки ответа, адрес подписки и предикат «локальный адрес».
#![allow(clippy::unwrap_used, clippy::expect_used)]

use config_gen::redact::{redact_subscription_url, redact_text};
use config_gen::subscription::*;
use ipc_proto::SubscriptionUsage;
use url::Url;

#[test]
fn headers_are_parsed_case_insensitively() {
    let m = parse_response_headers([
        ("profile-title", "My VPN"),
        (
            "SUBSCRIPTION-USERINFO",
            "upload=1; download=2; total=300; expire=1893456000",
        ),
        ("Profile-Update-Interval", "12"),
        ("Profile-Web-Page-Url", "https://support.example.com/help"),
        ("X-Other", "ignored"),
    ]);
    assert_eq!(m.title.as_deref(), Some("My VPN"));
    assert_eq!(
        m.usage,
        Some(SubscriptionUsage {
            upload: Some(1),
            download: Some(2),
            total: Some(300),
            expire: Some(1_893_456_000)
        })
    );
    assert_eq!(m.update_interval_hours, Some(12));
    assert_eq!(
        m.support_url.as_deref(),
        Some("https://support.example.com/help")
    );
}

#[test]
fn title_base64_variant_and_cleaning() {
    // "Мой VPN" в base64 (UTF-8).
    let m = parse_response_headers([("Profile-Title", "base64:0JzQvtC5IFZQTg==")]);
    assert_eq!(m.title.as_deref(), Some("Мой VPN"));
    let m = parse_response_headers([("Profile-Title", "base64:0JzQvtC5IFZQTg")]);
    assert_eq!(m.title.as_deref(), Some("Мой VPN"));
    let m = parse_response_headers([("Profile-Title", "base64:!!!")]);
    assert_eq!(m.title, None);
    let m = parse_response_headers([("Profile-Title", "a\u{1b}[31mb")]);
    assert!(!m.title.unwrap().contains('\u{1b}'));
    let long = "x".repeat(5000);
    let m = parse_response_headers([("Profile-Title", long.as_str())]);
    assert!(m.title.unwrap().chars().count() <= 129);
}

#[test]
fn userinfo_edge_cases() {
    let m = parse_response_headers([(
        "Subscription-Userinfo",
        "upload=0; download=5; total=0; expire=0",
    )]);
    let u = m.usage.unwrap();
    assert_eq!(
        (u.upload, u.download, u.total, u.expire),
        (Some(0), Some(5), None, None),
        "0 = без ограничений"
    );
    let m = parse_response_headers([(
        "Subscription-Userinfo",
        "garbage; upload=abc; download=-1; expire=1.7e9",
    )]);
    assert_eq!(m.usage.map(|u| u.expire), Some(Some(1_700_000_000)));
    let m = parse_response_headers([("Subscription-Userinfo", "nothing here")]);
    assert_eq!(m.usage, None);
}

#[test]
fn interval_and_support_url_are_validated() {
    let m = parse_response_headers([
        ("Profile-Update-Interval", "0"),
        ("Profile-Web-Page-Url", "javascript:alert(1)"),
    ]);
    assert_eq!((m.update_interval_hours, m.support_url), (None, None));
    let m = parse_response_headers([
        ("Profile-Update-Interval", "99999999999"),
        ("Profile-Web-Page-Url", "https://u:p@example.com/"),
    ]);
    assert_eq!((m.update_interval_hours, m.support_url), (Some(8760), None));
    let m = parse_response_headers([("Profile-Update-Interval", "abc")]);
    assert_eq!(m.update_interval_hours, None);
}

#[test]
fn subscription_url_rules() {
    assert!(parse_subscription_url("https://example.com/sub/TOKEN").is_ok());
    assert_eq!(
        parse_subscription_url("http://example.com/sub").unwrap_err(),
        UrlError::NotHttps
    );
    assert_eq!(
        parse_subscription_url("ftp://example.com/sub").unwrap_err(),
        UrlError::NotHttps
    );
    assert_eq!(
        parse_subscription_url("https://user:pass@example.com/sub").unwrap_err(),
        UrlError::HasCredentials
    );
    assert_eq!(
        parse_subscription_url("https://user@example.com/sub").unwrap_err(),
        UrlError::HasCredentials
    );
    assert_eq!(
        parse_subscription_url("not a url").unwrap_err(),
        UrlError::Invalid
    );
    assert_eq!(
        parse_subscription_url("file:///etc/passwd").unwrap_err(),
        UrlError::NotHttps
    );
}

#[test]
fn happ_crypt_links_get_their_own_error() {
    for s in [
        "happ://crypt/abcdef",
        "happ://crypt2/abcdef",
        "HAPP://CRYPT3/x",
        "  happ://crypt4/x",
    ] {
        let e = parse_subscription_url(s).unwrap_err();
        assert_eq!(e, UrlError::HappEncrypted, "{s}");
        assert!(e.to_string().contains("happ://crypt"));
    }
    assert_ne!(
        parse_subscription_url("happ://add/x").unwrap_err(),
        UrlError::HappEncrypted
    );
}

#[test]
fn redirect_targets_must_be_public_https() {
    let ok = |s: &str| check_redirect_target(&Url::parse(s).unwrap());
    assert!(ok("https://example.com/x").is_ok());
    assert!(ok("https://8.8.8.8/x").is_ok());
    for bad in [
        "https://127.0.0.1/x",
        "https://localhost/x",
        "https://foo.localhost/x",
        "https://10.0.0.5/x",
        "https://192.168.1.1/x",
        "https://172.16.0.1/",
        "https://169.254.169.254/latest/meta-data",
        "https://[::1]/x",
        "https://[fd00::1]/x",
        "https://[::ffff:127.0.0.1]/x",
        "https://2130706433/x",
        "https://0x7f.1/x",
        "https://router.local/",
        "https://intranet/",
        "https://198.18.0.1/",
    ] {
        assert_eq!(ok(bad), Err(UrlError::LocalTarget), "{bad}");
    }
    assert_eq!(ok("http://example.com/x"), Err(UrlError::NotHttps));
    assert_eq!(
        ok("https://u:p@example.com/x"),
        Err(UrlError::HasCredentials)
    );
}

#[test]
fn log_redaction_of_subscription_data() {
    assert_eq!(
        redact_subscription_url("https://example.com/subhapp/SECRETTOKEN"),
        "https://example.com/***"
    );
    let line =
        redact_text("update failed for https://example.com/subhapp/SECRETTOKEN?x=y: timeout");
    assert!(!line.contains("SECRETTOKEN"));
    assert!(line.contains("example.com"));
}
