//! Вспомогательные средства строгого разбора недоверенного JSON.

use std::collections::{BTreeMap, HashSet};
use std::net::IpAddr;

use serde_json::{Map, Value};

use super::model::Warning;
use crate::redact::sanitize_ident;

/// Причина отказа без секретов и без значений из подписки.
pub(crate) type R<T> = Result<T, String>;

pub(crate) const MAX_WARNINGS: usize = 256;

/// Накопитель предупреждений с верхней границей.
#[derive(Default)]
pub(crate) struct Sink {
    pub items: Vec<Warning>,
    pub overflowed: bool,
}

impl Sink {
    pub fn push(&mut self, profile: Option<u32>, message: String) {
        if self.items.len() >= MAX_WARNINGS {
            self.overflowed = true;
        } else {
            self.items.push(Warning { profile, message });
        }
    }
}

/// Предупреждения в контексте одного профиля.
pub(crate) struct W<'s> {
    pub sink: &'s mut Sink,
    pub profile: u32,
}

impl W<'_> {
    pub fn warn(&mut self, message: impl Into<String>) {
        self.sink.push(Some(self.profile), message.into());
    }
}

/// Строка допустима: не длиннее `max` байт, без управляющих символов.
pub(crate) fn clean(s: &str, max: usize) -> bool {
    s.len() <= max && !s.chars().any(char::is_control)
}

/// Домен (нижний регистр) или IP-литерал. Любое другое значение (путь к
/// сокету, пробелы, `unix:`, числовые «IP» вроде `0x7f.1`) — `None`.
pub(crate) fn norm_host(s: &str) -> Option<String> {
    let t = s.trim().trim_start_matches('[').trim_end_matches(']');
    if let Ok(ip) = t.parse::<IpAddr>() {
        return Some(ip.to_string());
    }
    let d = t.trim_end_matches('.').to_ascii_lowercase();
    if d.is_empty() || d.len() > 253 {
        return None;
    }
    let labels_ok = d.split('.').all(|l| {
        !l.is_empty()
            && l.len() <= 63
            && l.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    });
    let numeric_tld = d
        .rsplit('.')
        .next()
        .is_some_and(|l| l.bytes().all(|b| b.is_ascii_digit()));
    (labels_ok && !numeric_tld).then_some(d)
}

/// Объект с учётом использованных ключей. Имена ищутся без учёта регистра
/// (как у Go-разбора Xray), чтобы `AllowInsecure` не проскальзывало мимо проверок.
pub(crate) struct Fields<'a> {
    map: &'a Map<String, Value>,
    used: HashSet<&'a str>,
    pub label: String,
}

impl<'a> Fields<'a> {
    pub fn of(v: &'a Value, label: &str) -> R<Self> {
        match v.as_object() {
            Some(m) => Ok(Self::map(m, label)),
            None => Err(format!("{label}: ожидался объект")),
        }
    }

    pub fn map(map: &'a Map<String, Value>, label: &str) -> Self {
        Self {
            map,
            used: HashSet::new(),
            label: label.to_string(),
        }
    }

    /// Значение поля; `null` считается отсутствием.
    pub fn get(&mut self, key: &str) -> Option<&'a Value> {
        let (k, v) = match self.map.get_key_value(key) {
            Some(kv) => kv,
            None => self.map.iter().find(|(k, _)| k.eq_ignore_ascii_case(key))?,
        };
        self.used.insert(k.as_str());
        if v.is_null() { None } else { Some(v) }
    }

    pub fn get_any(&mut self, keys: &[&str]) -> Option<&'a Value> {
        let mut found = None;
        for k in keys {
            let v = self.get(k);
            if found.is_none() {
                found = v;
            }
        }
        found
    }

    pub fn str(&mut self, key: &str, max: usize) -> R<Option<&'a str>> {
        match self.get(key) {
            None => Ok(None),
            Some(Value::String(s)) if clean(s, max) => Ok(Some(s.as_str())),
            Some(Value::String(_)) => Err(format!("{}.{key}: недопустимая строка", self.label)),
            Some(_) => Err(format!("{}.{key}: ожидалась строка", self.label)),
        }
    }

    pub fn require_str(&mut self, key: &str, max: usize) -> R<&'a str> {
        match self.str(key, max)? {
            Some(s) if !s.is_empty() => Ok(s),
            _ => Err(format!(
                "{}.{key}: обязательное поле отсутствует",
                self.label
            )),
        }
    }

    pub fn u32(&mut self, key: &str) -> R<Option<u32>> {
        match self.get(key) {
            None => Ok(None),
            Some(v) => as_u32(v)
                .map(Some)
                .ok_or_else(|| format!("{}.{key}: ожидалось целое число", self.label)),
        }
    }

    pub fn bool(&mut self, key: &str) -> R<Option<bool>> {
        match self.get(key) {
            None => Ok(None),
            Some(Value::Bool(b)) => Ok(Some(*b)),
            Some(_) => Err(format!("{}.{key}: ожидалось true/false", self.label)),
        }
    }

    /// Пометить поля использованными без предупреждения (безвредный шум).
    pub fn ignore(&mut self, keys: &[&str]) {
        for k in keys {
            self.get(k);
        }
    }

    /// Поля, которые мы сознательно не принимаем: отбрасываем и записываем
    /// одно предупреждение со списком имён (значения не печатаются).
    pub fn drop_fields(&mut self, keys: &[&str], why: &str, w: &mut W<'_>) {
        let present: Vec<&str> = keys.iter().copied().filter(|k| self.has(k)).collect();
        for k in &present {
            self.get(k);
        }
        if !present.is_empty() {
            w.warn(format!(
                "{}: поля [{}] отброшены ({why})",
                self.label,
                present.join(", ")
            ));
        }
    }

    pub fn has(&self, key: &str) -> bool {
        self.map.keys().any(|k| k.eq_ignore_ascii_case(key))
    }

    /// Всё, что не было запрошено, — неизвестные поля.
    pub fn finish(self, w: &mut W<'_>) {
        let mut unknown: Vec<String> = self
            .map
            .keys()
            .filter(|k| !self.used.contains(k.as_str()))
            .take(8)
            .map(|k| sanitize_ident(k))
            .collect();
        if !unknown.is_empty() {
            unknown.sort();
            w.warn(format!(
                "{}: неизвестные поля [{}] отброшены",
                self.label,
                unknown.join(", ")
            ));
        }
    }
}

pub(crate) fn as_u32(v: &Value) -> Option<u32> {
    match v {
        Value::Number(n) => n.as_u64().and_then(|x| u32::try_from(x).ok()),
        Value::String(s) if s.len() <= 10 => s.parse().ok(),
        _ => None,
    }
}

pub(crate) fn as_port(v: &Value) -> Option<u16> {
    as_u32(v)
        .and_then(|p| u16::try_from(p).ok())
        .filter(|p| *p != 0)
}

/// Список строк (не более `max_items`, каждая не длиннее `max_len`).
pub(crate) fn str_list(
    v: Option<&Value>,
    label: &str,
    max_items: usize,
    max_len: usize,
) -> R<Vec<String>> {
    let Some(v) = v else { return Ok(Vec::new()) };
    let Some(arr) = v.as_array() else {
        return Err(format!("{label}: ожидался массив строк"));
    };
    if arr.len() > max_items {
        return Err(format!("{label}: слишком много элементов"));
    }
    arr.iter()
        .map(|x| match x {
            Value::String(s) if clean(s, max_len) => Ok(s.clone()),
            _ => Err(format!("{label}: недопустимый элемент")),
        })
        .collect()
}

/// Заголовки HTTP: имя → значение, не более 16 штук.
pub(crate) fn headers(v: Option<&Value>, label: &str) -> R<BTreeMap<String, String>> {
    let Some(v) = v else {
        return Ok(BTreeMap::new());
    };
    let Some(obj) = v.as_object() else {
        return Err(format!("{label}.headers: ожидался объект"));
    };
    if obj.len() > 16 {
        return Err(format!("{label}.headers: слишком много заголовков"));
    }
    let mut out = BTreeMap::new();
    for (k, val) in obj {
        let name_ok = !k.is_empty()
            && k.len() <= 64
            && k.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
        match val {
            Value::String(s) if name_ok && clean(s, 512) => {
                out.insert(k.clone(), s.clone());
            }
            _ => return Err(format!("{label}.headers: недопустимый заголовок")),
        }
    }
    Ok(out)
}

/// Скаляр (строка до 128 байт, число, bool) — для полей, значения которых мы
/// переносим как есть.
pub(crate) fn scalar(v: &Value) -> Option<Value> {
    match v {
        Value::String(s) if clean(s, 128) => Some(v.clone()),
        Value::Number(_) | Value::Bool(_) => Some(v.clone()),
        _ => None,
    }
}

/// `lookup` без учёта регистра в «сыром» объекте.
pub(crate) fn lookup<'a>(m: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
    m.get(key).or_else(|| {
        m.iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v)
    })
}
