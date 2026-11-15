//! Каждый вариант конфига прогоняется настоящим ядром:
//! `sing-box check --disable-color -c stdin` (конфиг в stdin, файлов нет).
//! Нужны код выхода 0 И пустой stderr (любое предупреждение — провал).
//! Путь к ядру: `PROJECTZ_CORES_DIR` или `<корень репозитория>/cores/darwin-amd64`.

#![allow(clippy::expect_used, clippy::unwrap_used)]

#[path = "fixtures/singbox/variants.rs"]
mod variants;

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use config_gen::singbox::render_config;

fn core_binary() -> PathBuf {
    let dir = match std::env::var_os("PROJECTZ_CORES_DIR") {
        Some(d) => PathBuf::from(d),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../cores/darwin-amd64"),
    };
    dir.join(if cfg!(windows) {
        "sing-box.exe"
    } else {
        "sing-box"
    })
}

struct Outcome {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn run_check(bin: &PathBuf, config: &str) -> Outcome {
    let mut cmd = Command::new(bin);
    cmd.args(["check", "--disable-color", "-c", "stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Устаревшие опции разрешать нельзя.
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("ENABLE_DEPRECATED_") {
            cmd.env_remove(&key);
        }
    }
    let mut child = cmd.spawn().expect("запуск sing-box");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(config.as_bytes())
        .expect("запись в stdin");
    let out = child.wait_with_output().expect("ожидание sing-box");
    Outcome {
        code: out.status.code(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

fn available() -> Option<PathBuf> {
    let bin = core_binary();
    if bin.is_file() {
        Some(bin)
    } else {
        eprintln!(
            "ПРОПУСК: sing-box не найден ({}). Положите ядро 1.14.4 туда или задайте PROJECTZ_CORES_DIR.",
            bin.display()
        );
        None
    }
}

#[test]
fn every_variant_passes_singbox_check() {
    let Some(bin) = available() else { return };
    for (name, input) in variants::variants() {
        let cfg = render_config(&input).expect(name);
        let out = run_check(&bin, &cfg);
        assert_eq!(
            out.code,
            Some(0),
            "{name}: код выхода {:?}, stderr: {}",
            out.code,
            out.stderr
        );
        assert!(
            out.stderr.is_empty(),
            "{name}: stderr не пуст: {}",
            out.stderr
        );
        assert!(
            out.stdout.is_empty(),
            "{name}: stdout не пуст: {}",
            out.stdout
        );
    }
}

/// Контроль самого стенда: битый конфиг обязан давать ненулевой код и stderr.
#[test]
fn checker_detects_broken_config() {
    let Some(bin) = available() else { return };
    let out = run_check(&bin, "{ \"outbounds\": [");
    assert_ne!(out.code, Some(0));
    assert!(!out.stderr.is_empty());

    // Неизвестное поле в сгенерированном конфиге — тоже провал.
    let mut value = config_gen::singbox::build_config(&variants::base()).expect("build");
    value["inbounds"][0]["foo"] = serde_json::json!(1);
    let out = run_check(&bin, &value.to_string());
    assert_ne!(out.code, Some(0));
}
