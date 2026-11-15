//! Строгие типы разобранной подписки. Исходный JSON дальше не используется.
//!
//! Секреты (UUID, пароли, ключи) хранятся в [`Secret`], поэтому `Debug` не
//! печатает их. Адреса серверов — не секрет с точки зрения этого крейта.

use std::collections::BTreeMap;
use std::net::IpAddr;

use ipc_proto::Secret;
use serde_json::Value;

/// Результат разбора тела подписки.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedSubscription {
    pub profiles: Vec<Profile>,
    /// Что было отброшено и почему. Без значений полей и без секретов.
    pub warnings: Vec<Warning>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    /// Индекс профиля или `None` для подписки в целом.
    pub profile: Option<u32>,
    pub message: String,
}

impl std::fmt::Display for Warning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.profile {
            Some(i) => write!(f, "профиль {i}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    /// Индекс в массиве подписки.
    pub index: u32,
    /// `remarks`, очищенное от управляющих символов.
    pub name: String,
    pub outcome: ProfileOutcome,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProfileOutcome {
    Supported(Box<ProfileConfig>),
    /// Профиль не прошёл проверку (ТЗ §5.1). Причина — без секретов.
    Unsupported {
        reason: String,
    },
}

/// Выбранная часть профиля: серверы-«входы» балансировщика и зависимости.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileConfig {
    /// Оставленные outbound'ы в исходном порядке.
    pub nodes: Vec<Node>,
    /// Индексы в `nodes`: серверы, между которыми балансирует Xray.
    pub entries: Vec<usize>,
    pub strategy: BalancerStrategy,
    pub probe: ProbeConfig,
}

impl ProfileConfig {
    pub fn server_count(&self) -> usize {
        self.entries.len()
    }

    /// Доменные имена серверов, которые помощник должен зарезолвить заранее
    /// (ТЗ §4, bootstrap-DNS): адреса всех оставленных узлов, кроме IP-литералов.
    /// Нижний регистр, без повторов, порядок стабильный.
    pub fn server_domains(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for n in &self.nodes {
            if let Some(a) = n.spec.address()
                && a.parse::<IpAddr>().is_err()
                && !out.iter().any(|d| d == a)
            {
                out.push(a.to_string());
            }
        }
        out
    }
}

/// Один оставленный outbound.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// Тег из подписки (только для диагностики; в конфиг Xray не попадает).
    pub source_tag: Option<String>,
    pub spec: OutboundSpec,
    /// Индекс в `ProfileConfig::nodes` outbound'а из `sockopt.dialerProxy`
    /// (или конвертированного `proxySettings.tag`).
    pub dialer: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OutboundSpec {
    Proxy(Box<ProxyOutbound>),
    /// Freedom — только как цель `dialerProxy`.
    Freedom(FreedomDialer),
}

impl OutboundSpec {
    pub fn address(&self) -> Option<&str> {
        match self {
            OutboundSpec::Proxy(p) => Some(&p.address),
            OutboundSpec::Freedom(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProxyOutbound {
    /// Нормализованный адрес: домен в нижнем регистре или IP-литерал.
    pub address: String,
    pub port: u16,
    pub protocol: ProxyProtocol,
    pub stream: Stream,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProxyProtocol {
    Vless {
        id: Secret,
        encryption: String,
        flow: Option<String>,
    },
    Vmess {
        id: Secret,
        security: String,
    },
    Trojan {
        password: Secret,
    },
    Shadowsocks {
        method: String,
        password: Secret,
    },
    Hysteria,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stream {
    pub transport: Transport,
    pub security: Security,
    pub tcp: TcpOpts,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Transport {
    Raw,
    Xhttp(XhttpSettings),
    Ws(WsSettings),
    Grpc(GrpcSettings),
    HttpUpgrade(HttpUpgradeSettings),
    Hysteria(HysteriaSettings),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Security {
    None,
    Tls(TlsSettings),
    Reality(RealitySettings),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TlsSettings {
    pub server_name: Option<String>,
    /// Всегда задан (по умолчанию `chrome`).
    pub fingerprint: String,
    pub alpn: Vec<String>,
    pub verify_peer_cert_by_name: Option<String>,
    pub pinned_peer_cert_sha256: Option<String>,
    pub ech_config_list: Option<String>,
    pub min_version: Option<String>,
    pub max_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RealitySettings {
    pub server_name: Option<String>,
    pub fingerprint: String,
    /// Публичный ключ сервера (`password`, он же старый `publicKey`).
    pub public_key: Secret,
    pub short_id: Secret,
    pub spider_x: Option<String>,
    pub mldsa65_verify: Option<String>,
}

/// Безопасные TCP-поля `sockopt`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TcpOpts {
    pub tcp_fast_open: Option<bool>,
    pub keep_alive_idle: Option<u32>,
    pub keep_alive_interval: Option<u32>,
    pub user_timeout: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct XhttpSettings {
    pub host: Option<String>,
    pub path: String,
    pub mode: Option<String>,
    pub headers: BTreeMap<String, String>,
    /// Скалярные поля из белого списка (`xPaddingBytes`, `noGRPCHeader`, ...).
    pub params: BTreeMap<String, Value>,
    /// Скалярные поля `xmux` из белого списка.
    pub xmux: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WsSettings {
    pub host: Option<String>,
    pub path: String,
    pub headers: BTreeMap<String, String>,
    pub heartbeat_period: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HttpUpgradeSettings {
    pub host: Option<String>,
    pub path: String,
    pub headers: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GrpcSettings {
    pub service_name: String,
    pub authority: Option<String>,
    pub multi_mode: Option<bool>,
    pub idle_timeout: Option<u32>,
    pub health_check_timeout: Option<u32>,
    pub permit_without_stream: Option<bool>,
    pub initial_windows_size: Option<u32>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HysteriaSettings {
    pub auth: Secret,
    pub udp_idle_timeout: Option<u32>,
}

/// Freedom-диалер (цель `dialerProxy`), например `fragment`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FreedomDialer {
    pub fragment: Option<Fragment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fragment {
    /// `tlshello` или диапазон/число (`1-3`).
    pub packets: String,
    /// `100-200`.
    pub length: String,
    /// `10-20`, мс.
    pub interval: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BalancerStrategy {
    #[default]
    Random,
    RoundRobin,
    LeastPing,
    LeastLoad,
}

impl BalancerStrategy {
    pub fn as_xray(self) -> &'static str {
        match self {
            BalancerStrategy::Random => "random",
            BalancerStrategy::RoundRobin => "roundRobin",
            BalancerStrategy::LeastPing => "leastPing",
            BalancerStrategy::LeastLoad => "leastLoad",
        }
    }
}

/// Параметры проб `burstObservatory` (URL проб задаёт сама программа).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeConfig {
    pub interval_secs: u32,
    pub sampling: u32,
    pub timeout_secs: u32,
}

impl Default for ProbeConfig {
    fn default() -> Self {
        Self {
            interval_secs: 60,
            sampling: 3,
            timeout_secs: 5,
        }
    }
}
