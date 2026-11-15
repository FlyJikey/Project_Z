//! Входные типы генератора. Каждый строится через конструктор с проверкой, так
//! что собранный из них [`SingboxInput`] заведомо корректен по отдельности;
//! перекрёстные проверки (порты, UDP и http) делает `build_config`.
//!
//! `Debug` везде ручной: пароли, секреты и логины не печатаются.

use std::collections::BTreeSet;
use std::fmt;
use std::net::IpAddr;

use ipc_proto::{ProxyEndpoint, ProxyScheme, Secret};

use super::error::SingboxError;
use super::validate::{
    canonical_ip, check_absolute_path, check_credentials, is_unusable_ip, is_valid_id,
    normalize_host,
};

/// Минимальная длина секрета Clash API: API на loopback доступен любому
/// локальному процессу, секрет — единственная защита.
pub const CLASH_SECRET_MIN_LEN: usize = 16;

/// Стабильный идентификатор записи пула (выдаёт помощник). Допустимы
/// 1..=64 символов `[A-Za-z0-9_-]`: из него строится тег outbound.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProxyId(String);

impl ProxyId {
    pub fn new(id: impl Into<String>) -> Result<Self, SingboxError> {
        let id = id.into();
        if is_valid_id(&id) {
            Ok(Self(id))
        } else {
            Err(SingboxError::InvalidProxyId)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Запись пула: идентификатор + проверенный прокси. Порядок в пуле = приоритет.
#[derive(Clone, PartialEq, Eq)]
pub struct PoolEntry {
    id: ProxyId,
    scheme: ProxyScheme,
    /// Нормализованный адрес для поля `server`.
    host: String,
    port: u16,
    auth: Option<(String, Secret)>,
}

impl PoolEntry {
    pub fn id(&self) -> &ProxyId {
        &self.id
    }

    pub fn scheme(&self) -> ProxyScheme {
        self.scheme
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn has_auth(&self) -> bool {
        self.auth.is_some()
    }

    pub(crate) fn auth(&self) -> Option<(&str, &str)> {
        self.auth.as_ref().map(|(u, p)| (u.as_str(), p.expose()))
    }
}

impl fmt::Debug for PoolEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PoolEntry")
            .field("id", &self.id)
            .field("scheme", &self.scheme)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("has_auth", &self.has_auth())
            .finish()
    }
}

/// Упорядоченный непустой пул прокси без повторов идентификаторов.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyPool {
    entries: Vec<PoolEntry>,
}

impl ProxyPool {
    pub fn new(items: Vec<(ProxyId, ProxyEndpoint)>) -> Result<Self, SingboxError> {
        if items.is_empty() {
            return Err(SingboxError::EmptyPool);
        }
        // Повторы ищем без учёта регистра: теги в Clash API передаются в URL.
        let mut seen = BTreeSet::new();
        let mut entries = Vec::with_capacity(items.len());
        for (index, (id, ep)) in items.into_iter().enumerate() {
            let position = index + 1;
            if !seen.insert(id.as_str().to_ascii_lowercase()) {
                return Err(SingboxError::DuplicateProxyId {
                    id: id.as_str().to_owned(),
                });
            }
            if ep.port == 0 {
                return Err(SingboxError::ProxyPortZero { position });
            }
            let host =
                normalize_host(&ep.host).ok_or(SingboxError::InvalidProxyHost { position })?;
            check_credentials(
                ep.scheme == ProxyScheme::Http,
                ep.username.as_deref(),
                ep.password.as_ref().map(Secret::expose),
            )
            .map_err(|reason| SingboxError::InvalidProxyCredentials { position, reason })?;
            let auth = match (ep.username, ep.password) {
                (Some(u), Some(p)) => Some((u, p)),
                _ => None,
            };
            entries.push(PoolEntry {
                id,
                scheme: ep.scheme,
                host,
                port: ep.port,
                auth,
            });
        }
        Ok(Self { entries })
    }

    pub fn entries(&self) -> &[PoolEntry] {
        &self.entries
    }

    pub fn has_http(&self) -> bool {
        self.entries.iter().any(|e| e.scheme == ProxyScheme::Http)
    }
}

/// Локальный socks-inbound Xray (127.0.0.1) с обязательной авторизацией.
#[derive(Clone, PartialEq, Eq)]
pub struct XraySocks {
    port: u16,
    username: String,
    password: Secret,
}

impl XraySocks {
    pub fn new(
        port: u16,
        username: impl Into<String>,
        password: Secret,
    ) -> Result<Self, SingboxError> {
        let username = username.into();
        if port == 0 {
            return Err(SingboxError::XrayPortZero);
        }
        check_credentials(false, Some(&username), Some(password.expose()))
            .map_err(|reason| SingboxError::InvalidXrayCredentials { reason })?;
        Ok(Self {
            port,
            username,
            password,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub(crate) fn username(&self) -> &str {
        &self.username
    }

    pub(crate) fn password(&self) -> &str {
        self.password.expose()
    }
}

impl fmt::Debug for XraySocks {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("XraySocks")
            .field("port", &self.port)
            .field("credentials", &"***")
            .finish()
    }
}

/// Clash API на 127.0.0.1.
#[derive(Clone, PartialEq, Eq)]
pub struct ClashApi {
    port: u16,
    secret: Secret,
}

impl ClashApi {
    pub fn new(port: u16, secret: Secret) -> Result<Self, SingboxError> {
        if port == 0 {
            return Err(SingboxError::ClashPortZero);
        }
        let s = secret.expose();
        if s.chars().count() < CLASH_SECRET_MIN_LEN || s.chars().any(char::is_control) {
            return Err(SingboxError::InvalidClashSecret {
                min: CLASH_SECRET_MIN_LEN,
            });
        }
        Ok(Self { port, secret })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub(crate) fn secret(&self) -> &str {
        self.secret.expose()
    }
}

impl fmt::Debug for ClashApi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClashApi")
            .field("port", &self.port)
            .field("secret", &"***")
            .finish()
    }
}

/// Абсолютный путь файла `cache_file` (POSIX или Windows).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachePath(String);

impl CachePath {
    pub fn new(path: impl Into<String>) -> Result<Self, SingboxError> {
        let path = path.into();
        check_absolute_path(&path).map_err(|reason| SingboxError::InvalidCachePath { reason })?;
        Ok(Self(path))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// IP VPN-серверов: исключения маршрутов TUN (ТЗ §4). Непустой, без повторов,
/// отсортирован (результат не зависит от порядка на входе).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VpnServerIps(Vec<IpAddr>);

impl VpnServerIps {
    pub fn new(ips: impl IntoIterator<Item = IpAddr>) -> Result<Self, SingboxError> {
        let mut set = BTreeSet::new();
        for ip in ips {
            if is_unusable_ip(ip) {
                return Err(SingboxError::InvalidVpnServerIp);
            }
            set.insert(canonical_ip(ip));
        }
        if set.is_empty() {
            return Err(SingboxError::NoVpnServerIps);
        }
        Ok(Self(set.into_iter().collect()))
    }

    pub fn as_slice(&self) -> &[IpAddr] {
        &self.0
    }
}

/// Уровень журнала sing-box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
        }
    }
}

/// Всё, что нужно для конфига sing-box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SingboxInput {
    pub pool: ProxyPool,
    pub xray: XraySocks,
    pub vpn_server_ips: VpnServerIps,
    /// «Разрешить локальную сеть» (ТЗ §7): частные префиксы исключаются из TUN.
    pub allow_lan: bool,
    /// UDP через прокси разрешён (ТЗ §6). Нет — UDP отклоняется (DNS перехватывается раньше).
    pub udp_allowed: bool,
    pub clash_api: ClashApi,
    pub cache_path: CachePath,
    pub log_level: LogLevel,
}
