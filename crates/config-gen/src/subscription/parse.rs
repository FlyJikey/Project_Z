//! Разбор тела подписки: массив Xray-конфигов → профили.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use super::fields::{Fields, R, Sink, W, as_u32, str_list};
use super::model::{
    BalancerStrategy, Node, OutboundSpec, ParsedSubscription, ProbeConfig, Profile, ProfileConfig,
    ProfileOutcome,
};
use super::outbound::{ParsedOutbound, parse_outbound};
use crate::redact::{sanitize_for_log, sanitize_ident};

/// Предельные размеры (защита от вредоносной подписки).
pub mod limits {
    /// Тело ответа, байт.
    pub const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;
    /// Профилей в подписке.
    pub const MAX_PROFILES: usize = 200;
    /// Outbound'ов в профиле.
    pub const MAX_OUTBOUNDS: usize = 64;
    /// Балансировщиков в профиле.
    pub const MAX_BALANCERS: usize = 16;
    /// Префиксов в селекторе.
    pub const MAX_SELECTORS: usize = 64;
    /// Длина `remarks`, символов.
    pub const MAX_NAME_CHARS: usize = 128;
    /// Длина цепочки `dialerProxy`.
    pub const MAX_DIALER_DEPTH: usize = 8;
    // Глубину JSON ограничивает serde_json (128 уровней) — глубже вложенное
    // тело отвергается как `NotJson`.
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SubscriptionError {
    #[error("тело подписки слишком большое")]
    TooLarge,
    #[error("зашифрованные подписки Happ (happ://crypt) не поддерживаются")]
    HappEncrypted,
    #[error("тело подписки не является JSON ({0}); другие форматы пока не поддерживаются")]
    NotJson(String),
    #[error("ожидался JSON-массив профилей")]
    NotArray,
    #[error("в подписке нет профилей")]
    Empty,
    #[error("в подписке слишком много профилей")]
    TooManyProfiles,
}

/// Разбирает тело подписки Happ (JSON-массив Xray-конфигов).
///
/// Подписка недоверенная: результат содержит только то, что прошло белые списки.
pub fn parse_subscription(body: &[u8]) -> Result<ParsedSubscription, SubscriptionError> {
    if body.len() > limits::MAX_BODY_BYTES {
        return Err(SubscriptionError::TooLarge);
    }
    let text = body
        .strip_prefix(b"\xEF\xBB\xBF")
        .unwrap_or(body)
        .trim_ascii_start();
    if text.len() >= 7 && text[..7].eq_ignore_ascii_case(b"happ://") {
        return Err(SubscriptionError::HappEncrypted);
    }
    let value: Value = serde_json::from_slice(text).map_err(|e| {
        SubscriptionError::NotJson(format!("строка {}, столбец {}", e.line(), e.column()))
    })?;
    let items: Vec<&Value> = match &value {
        Value::Array(a) => a.iter().collect(),
        // Одиночный конфиг вместо массива.
        Value::Object(_) => vec![&value],
        _ => return Err(SubscriptionError::NotArray),
    };
    if items.is_empty() {
        return Err(SubscriptionError::Empty);
    }
    if items.len() > limits::MAX_PROFILES {
        return Err(SubscriptionError::TooManyProfiles);
    }
    let mut sink = Sink::default();
    let profiles = items
        .iter()
        .enumerate()
        .map(|(i, v)| parse_profile(u32::try_from(i).unwrap_or(u32::MAX), v, &mut sink))
        .collect();
    if sink.overflowed {
        sink.items.push(super::model::Warning {
            profile: None,
            message: "слишком много предупреждений, остальные опущены".into(),
        });
    }
    Ok(ParsedSubscription {
        profiles,
        warnings: sink.items,
    })
}

fn parse_profile(index: u32, v: &Value, sink: &mut Sink) -> Profile {
    let mut w = W {
        sink,
        profile: index,
    };
    let mut name = format!("Профиль {}", index + 1);
    let outcome = match parse_profile_inner(v, &mut name, &mut w) {
        Ok(cfg) => ProfileOutcome::Supported(Box::new(cfg)),
        Err(reason) => ProfileOutcome::Unsupported { reason },
    };
    Profile {
        index,
        name,
        outcome,
    }
}

struct BalancerInfo {
    selectors: Vec<String>,
    strategy: BalancerStrategy,
}

fn parse_profile_inner(v: &Value, name: &mut String, w: &mut W<'_>) -> R<ProfileConfig> {
    let mut f = Fields::of(v, "профиль")
        .map_err(|_| "элемент подписки не является объектом".to_string())?;
    if let Some(Value::String(r)) = f.get("remarks") {
        let clean = sanitize_for_log(r.trim(), limits::MAX_NAME_CHARS);
        if !clean.is_empty() {
            *name = clean;
        }
    }
    f.drop_fields(
        &[
            "dns",
            "inbounds",
            "api",
            "policy",
            "stats",
            "metrics",
            "reverse",
            "fakedns",
            "transport",
            "log",
            "env",
            "version",
        ],
        "своё значение задаёт приложение",
        w,
    );
    let outbounds_v = f.get("outbounds");
    let routing_v = f.get("routing");
    let burst_v = f.get("burstObservatory");
    let obs_v = f.get("observatory");
    f.finish(w);

    let arr = outbounds_v
        .and_then(Value::as_array)
        .ok_or_else(|| "в профиле нет списка outbounds".to_string())?;
    if arr.len() > limits::MAX_OUTBOUNDS {
        return Err("в профиле слишком много outbound'ов".into());
    }
    let outs: Vec<ParsedOutbound> = arr
        .iter()
        .enumerate()
        .map(|(i, o)| parse_outbound(o, i, w))
        .collect();
    let balancers = parse_routing(routing_v, w)?;
    let (obs_selectors, probe) = parse_observatories(burst_v, obs_v, w);
    assemble(outs, &balancers, &obs_selectors, probe, w)
}

fn parse_routing(v: Option<&Value>, w: &mut W<'_>) -> R<Vec<BalancerInfo>> {
    let Some(v) = v else { return Ok(Vec::new()) };
    let mut f = Fields::of(v, "routing")?;
    if let Some(rules) = f.get("rules") {
        let n = rules.as_array().map_or(0, Vec::len);
        w.warn(format!(
            "routing.rules ({n}) отброшены: правила маршрутизации задаёт приложение"
        ));
    }
    f.drop_fields(
        &["domainStrategy", "domainMatcher", "settings", "script"],
        "задаёт приложение",
        w,
    );
    let mut out = Vec::new();
    if let Some(b) = f.get("balancers") {
        let arr = b
            .as_array()
            .ok_or_else(|| "routing.balancers: ожидался массив".to_string())?;
        if arr.len() > limits::MAX_BALANCERS {
            return Err("слишком много балансировщиков".into());
        }
        for item in arr {
            let mut bf = Fields::of(item, "routing.balancers[]")?;
            bf.ignore(&["tag"]);
            let selectors = str_list(
                bf.get("selector"),
                "routing.balancers[].selector",
                limits::MAX_SELECTORS,
                128,
            )?;
            let mut strategy = BalancerStrategy::Random;
            if let Some(sv) = bf.get("strategy") {
                let mut sf = Fields::of(sv, "routing.balancers[].strategy")?;
                strategy = match sf.str("type", 32)?.map(str::to_ascii_lowercase).as_deref() {
                    None | Some("random") => BalancerStrategy::Random,
                    Some("roundrobin") => BalancerStrategy::RoundRobin,
                    Some("leastping") => BalancerStrategy::LeastPing,
                    Some("leastload") => BalancerStrategy::LeastLoad,
                    Some(other) => {
                        w.warn(format!(
                            "стратегия балансировщика `{}` неизвестна, принята random",
                            sanitize_ident(other)
                        ));
                        BalancerStrategy::Random
                    }
                };
                sf.drop_fields(&["settings"], "параметры стратегии не переносятся", w);
                sf.finish(w);
            }
            bf.drop_fields(&["fallbackTag"], "запасной сервер задаёт приложение", w);
            bf.finish(w);
            out.push(BalancerInfo {
                selectors,
                strategy,
            });
        }
    }
    f.finish(w);
    Ok(out)
}

/// `1m`, `30s`, `2h` → секунды.
fn parse_duration(s: &str) -> Option<u32> {
    let s = s.trim();
    let (num, unit) = s.split_at(s.len().checked_sub(1)?);
    let n: u32 = num.parse().ok()?;
    let mul = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3600,
        _ => return None,
    };
    n.checked_mul(mul)
}

/// Селекторы наблюдателя и параметры проб. Ошибки здесь не делают профиль
/// неподдерживаемым: параметры проб заменяются значениями по умолчанию.
fn parse_observatories(
    burst: Option<&Value>,
    obs: Option<&Value>,
    w: &mut W<'_>,
) -> (Vec<String>, ProbeConfig) {
    let mut selectors = Vec::new();
    let mut probe = ProbeConfig::default();
    let mut any = false;
    if let Some(b) = burst {
        let res: R<()> = (|| {
            let mut f = Fields::of(b, "burstObservatory")?;
            selectors.extend(str_list(
                f.get("subjectSelector"),
                "burstObservatory.subjectSelector",
                limits::MAX_SELECTORS,
                128,
            )?);
            if let Some(pc) = f.get("pingConfig") {
                let mut pf = Fields::of(pc, "burstObservatory.pingConfig")?;
                if let Some(i) = pf.str("interval", 16)?.and_then(parse_duration) {
                    probe.interval_secs = i.clamp(10, 3600);
                }
                if let Some(t) = pf.str("timeout", 16)?.and_then(parse_duration) {
                    probe.timeout_secs = t.clamp(1, 60);
                }
                if let Some(s) = pf.get("sampling").and_then(as_u32) {
                    probe.sampling = s.clamp(1, 20);
                }
                pf.drop_fields(
                    &["destination", "connectivity", "httpMethod"],
                    "адрес проб задаёт приложение",
                    w,
                );
                pf.finish(w);
            }
            f.finish(w);
            Ok(())
        })();
        any = true;
        if let Err(e) = res {
            w.warn(format!(
                "burstObservatory не разобран ({e}), приняты значения по умолчанию"
            ));
        }
    }
    if let Some(o) = obs {
        let res: R<()> = (|| {
            let mut f = Fields::of(o, "observatory")?;
            let sel = str_list(
                f.get("subjectSelector"),
                "observatory.subjectSelector",
                limits::MAX_SELECTORS,
                128,
            )?;
            if !any && let Some(i) = f.str("probeInterval", 16)?.and_then(parse_duration) {
                probe.interval_secs = i.clamp(10, 3600);
            }
            selectors.extend(sel);
            f.drop_fields(
                &["probeUrl", "enableConcurrency", "probeInterval"],
                "параметры проб задаёт приложение",
                w,
            );
            f.finish(w);
            Ok(())
        })();
        if let Err(e) = res {
            w.warn(format!(
                "observatory не разобран ({e}), приняты значения по умолчанию"
            ));
        }
    }
    (selectors, probe)
}

fn matches_any(tag: &str, prefixes: &[String]) -> bool {
    prefixes.iter().any(|p| tag.starts_with(p.as_str()))
}

/// Цепочка зависимостей `dialerProxy` от outbound'а `i` (без него самого).
fn collect_deps(
    i: usize,
    outs: &[ParsedOutbound],
    tag_idx: &HashMap<String, usize>,
) -> R<Vec<usize>> {
    let mut deps = Vec::new();
    let mut seen = HashSet::from([i]);
    let mut cur = i;
    for _ in 0..=limits::MAX_DIALER_DEPTH {
        let Ok((_, dialer)) = &outs[cur].outcome else {
            return Err("внутренняя ошибка: зависимость без разбора".into());
        };
        let Some(dtag) = dialer else { return Ok(deps) };
        let Some(&d) = tag_idx.get(dtag) else {
            return Err(format!(
                "зависимость dialerProxy `{}` не найдена",
                sanitize_ident(dtag)
            ));
        };
        if !seen.insert(d) {
            return Err("циклическая ссылка dialerProxy".into());
        }
        if let Err(r) = &outs[d].outcome {
            return Err(format!(
                "зависимость `{}` не поддерживается: {r}",
                sanitize_ident(dtag)
            ));
        }
        deps.push(d);
        cur = d;
    }
    Err("цепочка dialerProxy слишком длинная".into())
}

/// Выбор серверов и сборка профиля.
///
/// Правило выбора серверов («входов»):
/// 1. есть `routing.balancers` — серверы это прокси-outbound'ы, чей тег
///    начинается с одного из префиксов селекторов (так выбирает и сам Xray;
///    префиксы разрешаются здесь, в явный список);
/// 2. иначе, если есть observatory-селекторы — по ним;
/// 3. иначе — все прокси-outbound'ы, на которые никто не ссылается через
///    `dialerProxy` (профиль без балансировщика эквивалентен «балансировщику
///    по всем серверам»; цепочки вроде bridge → exit дают один сервер).
///
/// Наблюдатель (observatory) НЕ расширяет набор серверов при наличии
/// балансировщика: пробуются ровно те серверы, между которыми выбирает Xray.
/// Freedom, попавший под селектор, пропускается: это прямой выход в сеть.
fn assemble(
    outs: Vec<ParsedOutbound>,
    balancers: &[BalancerInfo],
    obs_selectors: &[String],
    probe: ProbeConfig,
    w: &mut W<'_>,
) -> R<ProfileConfig> {
    let mut tag_idx: HashMap<String, usize> = HashMap::new();
    for (i, o) in outs.iter().enumerate() {
        if let Some(t) = &o.tag
            && tag_idx.insert(t.clone(), i).is_some()
        {
            return Err("в профиле повторяются теги outbound'ов".into());
        }
    }

    let candidates: Vec<usize> = if !balancers.is_empty() {
        let prefixes: Vec<String> = balancers
            .iter()
            .flat_map(|b| b.selectors.iter().cloned())
            .collect();
        by_prefix(&outs, &prefixes)
    } else if !obs_selectors.is_empty() {
        by_prefix(&outs, obs_selectors)
    } else {
        let dialed: HashSet<&str> = outs
            .iter()
            .filter_map(|o| o.outcome.as_ref().ok().and_then(|(_, d)| d.as_deref()))
            .collect();
        let heads: Vec<usize> = (0..outs.len())
            .filter(|&i| {
                outs[i].proxy_like && !outs[i].tag.as_deref().is_some_and(|t| dialed.contains(t))
            })
            .collect();
        if heads.is_empty() {
            // Все прокси ссылаются друг на друга (цикл): отдаём на проверку цепочек,
            // чтобы причина была названа точно.
            (0..outs.len()).filter(|&i| outs[i].proxy_like).collect()
        } else {
            heads
        }
    };

    let mut entries: Vec<(usize, Vec<usize>)> = Vec::new();
    let mut first_failure: Option<String> = None;
    for i in candidates {
        let label = outs[i]
            .tag
            .as_deref()
            .map_or_else(|| format!("#{i}"), sanitize_ident);
        let res = match &outs[i].outcome {
            Err(r) => Err(r.clone()),
            Ok((OutboundSpec::Freedom(_), _)) => {
                Err("freedom не может быть сервером (прямой выход)".into())
            }
            Ok((OutboundSpec::Proxy(_), _)) => collect_deps(i, &outs, &tag_idx),
        };
        match res {
            Ok(deps) => entries.push((i, deps)),
            Err(r) => {
                w.warn(format!("сервер `{label}` отброшен: {r}"));
                first_failure.get_or_insert(r);
            }
        }
    }
    if entries.is_empty() {
        return Err(match first_failure {
            Some(r) => format!("нет поддерживаемых серверов: {r}"),
            None if balancers.is_empty() && obs_selectors.is_empty() => {
                "в профиле нет поддерживаемых прокси-outbound'ов".into()
            }
            None => "балансировщик не выбрал ни одного прокси-outbound'а".into(),
        });
    }

    let mut keep: Vec<usize> = entries
        .iter()
        .flat_map(|(i, d)| std::iter::once(*i).chain(d.iter().copied()))
        .collect();
    keep.sort_unstable();
    keep.dedup();
    let new_index: HashMap<usize, usize> =
        keep.iter().enumerate().map(|(n, &old)| (old, n)).collect();
    let mut nodes = Vec::with_capacity(keep.len());
    for &old in &keep {
        let o = &outs[old];
        let Ok((spec, dialer)) = &o.outcome else {
            return Err("внутренняя ошибка: неразобранный узел".into());
        };
        let dialer = dialer
            .as_ref()
            .and_then(|t| tag_idx.get(t))
            .and_then(|d| new_index.get(d))
            .copied();
        nodes.push(Node {
            source_tag: o.tag.clone(),
            spec: spec.clone(),
            dialer,
        });
    }
    let discarded = outs.len() - keep.len();
    if discarded > 0 {
        w.warn(format!("отброшено outbound'ов, на которые никто не ссылается или не поддерживаемых: {discarded}"));
    }
    if balancers.len() > 1 {
        w.warn("несколько балансировщиков: серверы объединены, стратегия первого");
    }
    let strategy = balancers
        .first()
        .map_or(BalancerStrategy::Random, |b| b.strategy);
    let entry_nodes = entries
        .iter()
        .filter_map(|(i, _)| new_index.get(i).copied())
        .collect();
    Ok(ProfileConfig {
        nodes,
        entries: entry_nodes,
        strategy,
        probe,
    })
}

fn by_prefix(outs: &[ParsedOutbound], prefixes: &[String]) -> Vec<usize> {
    (0..outs.len())
        .filter(|&i| {
            outs[i]
                .tag
                .as_deref()
                .is_some_and(|t| matches_any(t, prefixes))
        })
        .collect()
}
