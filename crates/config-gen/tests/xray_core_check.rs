//! Каждый сгенерированный конфиг проходит настоящее ядро: `xray run -test -c stdin:`.
//!
//! Путь к ядру: `PROJECTZ_CORES_DIR` или `<корень репозитория>/cores/darwin-amd64`
//! (бинарник `xray`). Нет бинарника — тест сообщает и пропускается. Конфиг идёт
//! через stdin, файлы не создаются.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use config_gen::subscription::*;
use config_gen::xray::*;
use ipc_proto::Secret;

fn xray_binary() -> Option<PathBuf> {
    let dir = match std::env::var_os("PROJECTZ_CORES_DIR") {
        Some(d) => PathBuf::from(d),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../cores/darwin-amd64"),
    };
    let bin = dir.join(if cfg!(windows) { "xray.exe" } else { "xray" });
    bin.is_file().then_some(bin)
}

/// `Ok(вывод)` при коде 0, иначе `Err(вывод)`.
fn run_test(bin: &PathBuf, config: &str) -> Result<String, String> {
    let mut child = Command::new(bin)
        .args(["run", "-test", "-c", "stdin:"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("запуск xray");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(config.as_bytes())
        .expect("запись в stdin");
    let out = child.wait_with_output().expect("ожидание xray");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    if out.status.success() && text.contains("Configuration OK") {
        Ok(text)
    } else {
        Err(text)
    }
}

fn fixture(name: &str) -> ParsedSubscription {
    let b = std::fs::read(format!(
        "{}/tests/fixtures/subscription/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    parse_subscription(&b).unwrap()
}

/// Для каждого домена профиля — два IP из документационных блоков.
fn synthetic_table(p: &ProfileConfig) -> ResolvedHosts {
    p.server_domains()
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let n = u8::try_from(i + 1).unwrap();
            (
                d.clone(),
                vec![
                    format!("203.0.113.{n}").parse().unwrap(),
                    format!("198.51.100.{n}").parse().unwrap(),
                ],
            )
        })
        .collect()
}

fn build_for(p: &ProfileConfig, probe: Option<&str>) -> BuiltConfig {
    let pw = Secret::new("core-check-PASSWORD");
    let table = synthetic_table(p);
    build_config(
        p,
        &XrayParams {
            socks_port: 10808,
            socks_user: "pz_user",
            socks_password: &pw,
            interface: "en0",
            resolved: &table,
            probe_url: probe,
        },
    )
    .unwrap()
}

macro_rules! need_core {
    () => {
        match xray_binary() {
            Some(b) => b,
            None => {
                eprintln!("xray_core_check: бинарник xray не найден (PROJECTZ_CORES_DIR или cores/darwin-amd64) — тест пропущен");
                return;
            }
        }
    };
}

#[test]
fn core_version_is_pinned() {
    let bin = need_core!();
    let out = Command::new(&bin)
        .arg("version")
        .output()
        .expect("xray version");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.starts_with("Xray 26.11.9"),
        "ожидалось ядро 26.11.9, получено: {}",
        text.lines().next().unwrap_or("")
    );
}

#[test]
fn every_supported_profile_of_every_fixture_passes_the_real_core() {
    let bin = need_core!();
    let mut checked = 0;
    for name in ["happ_full.json", "hostile.json"] {
        for prof in &fixture(name).profiles {
            let ProfileOutcome::Supported(cfg) = &prof.outcome else {
                continue;
            };
            let built = build_for(cfg, None);
            if let Err(e) = run_test(&bin, built.json.expose()) {
                panic!("{name} / {}: xray -test отклонил конфиг:\n{e}", prof.name);
            }
            checked += 1;
        }
    }
    assert!(checked >= 11, "проверено только {checked} профилей");
}

#[test]
fn every_balancer_strategy_and_custom_probe_url_pass() {
    let bin = need_core!();
    let sub = fixture("happ_full.json");
    let ProfileOutcome::Supported(base) = &sub.profiles[0].outcome else {
        panic!()
    };
    for strategy in [
        BalancerStrategy::Random,
        BalancerStrategy::RoundRobin,
        BalancerStrategy::LeastPing,
        BalancerStrategy::LeastLoad,
    ] {
        let mut p = (**base).clone();
        p.strategy = strategy;
        let built = build_for(&p, Some("https://probe.example.net/generate_204"));
        run_test(&bin, built.json.expose()).unwrap_or_else(|e| panic!("{strategy:?}: {e}"));
    }
}

#[test]
fn hostile_fragment_forms_pass() {
    // Фрагмент из finalmask-формы подписки собирается в форму settings.fragment.
    let bin = need_core!();
    let sub = fixture("hostile.json");
    let prof = sub
        .profiles
        .iter()
        .find(|p| p.name.starts_with("11 "))
        .unwrap();
    let ProfileOutcome::Supported(cfg) = &prof.outcome else {
        panic!()
    };
    let built = build_for(cfg, None);
    assert!(
        built
            .json
            .expose()
            .contains(r#""fragment":{"interval":"10-20","length":"3-5","packets":"tlshello"}"#)
    );
    run_test(&bin, built.json.expose()).unwrap();
}

#[test]
fn harness_detects_a_broken_config() {
    // Контроль: проверка не «вечно зелёная» — неизвестный fingerprint ядро отвергает.
    let bin = need_core!();
    let sub = fixture("happ_full.json");
    let ProfileOutcome::Supported(cfg) = &sub.profiles[0].outcome else {
        panic!()
    };
    let good = build_for(cfg, None).json.expose().to_string();
    assert!(run_test(&bin, &good).is_ok());
    let bad = good.replacen(
        r#""fingerprint":"chrome""#,
        r#""fingerprint":"no-such-fingerprint""#,
        1,
    );
    assert_ne!(bad, good);
    let err = run_test(&bin, &bad).unwrap_err();
    assert!(!err.contains("Configuration OK"), "{err}");
}
