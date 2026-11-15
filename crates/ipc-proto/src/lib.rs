//! Типы сообщений UI ↔ помощник (ТЗ §3, §4.1, §14).
//!
//! Помощник — граница привилегий: API узкий и типизированный. Здесь нет и не
//! должно появляться готовых конфигов ядер, путей к файлам, имён бинарников и
//! команд оболочки — только намерения пользователя и состояние для показа.
//!
//! Кадр на проводе: 4 байта длины (big-endian) + JSON. См. [`frame`].

use std::fmt;

use serde::{Deserialize, Serialize};

pub mod frame;

/// Версия протокола. UI и помощник сверяют её при подключении (ТЗ §13).
pub const PROTOCOL_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Секреты
// ---------------------------------------------------------------------------

/// Строка-секрет: не попадает в логи через `Debug`/`Display` (ТЗ §14).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Явный доступ к значению. Вызывать только там, где секрет действительно
    /// нужен (генерация конфига ядра, запрос подписки).
    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(***)")
    }
}

// ---------------------------------------------------------------------------
// Прокси (ТЗ §6)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProxyScheme {
    Socks5,
    Http,
}

/// Прокси пользователя с секретами. Идёт только в направлении UI → помощник
/// (один раз при вводе) и внутри помощника.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyEndpoint {
    pub scheme: ProxyScheme,
    /// IP или доменное имя. Домен локально не резолвится (ТЗ §4).
    /// IPv6 — без квадратных скобок (`2001:db8::1`).
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<Secret>,
}

/// Режим пула прокси (ТЗ §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PoolMode {
    /// Только №1; упал → блок, IP не меняется никогда.
    #[default]
    PrimaryOnly,
    /// Упал №1 → следующий живой; ожил №1 → возврат к нему.
    WithFallback,
}

/// Прокси для показа в UI: без пароля.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyView {
    /// Стабильный идентификатор записи пула, выдаёт помощник.
    pub id: String,
    pub scheme: ProxyScheme,
    pub host: String,
    pub port: u16,
    /// Логин в скрытом виде, например `us***`.
    pub username_masked: Option<String>,
    pub has_password: bool,
    pub health: LinkHealth,
    pub exit: Option<ExitInfo>,
}

/// Сведения о выходном IP (ТЗ §6, §10.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExitInfo {
    pub ip: String,
    pub country_code: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
    pub asn: Option<String>,
    pub org: Option<String>,
    pub timezone: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkHealth {
    #[default]
    Unknown,
    Up,
    Down,
}

// ---------------------------------------------------------------------------
// Подписки (ТЗ §5)
// ---------------------------------------------------------------------------

/// Подписка для показа в UI: без токена в адресе.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionView {
    pub id: String,
    /// `Profile-Title` или хост адреса.
    pub title: String,
    /// Хост адреса подписки (путь и токен не показываются).
    pub host: String,
    pub usage: Option<SubscriptionUsage>,
    /// `Profile-Update-Interval`, часы.
    pub update_interval_hours: Option<u32>,
    /// `Profile-Web-Page-Url`.
    pub support_url: Option<String>,
    /// Время последнего удачного обновления, Unix-секунды.
    pub updated_at: Option<u64>,
    pub last_error: Option<String>,
    pub profiles: Vec<ProfileView>,
}

/// `Subscription-Userinfo` (ТЗ §5.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SubscriptionUsage {
    pub upload: Option<u64>,
    pub download: Option<u64>,
    pub total: Option<u64>,
    /// Unix-секунды; `None` — бессрочно.
    pub expire: Option<u64>,
}

/// Профиль подписки (один элемент массива Xray-конфигов).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileView {
    /// Индекс в массиве подписки.
    pub index: u32,
    /// `remarks`.
    pub name: String,
    pub server_count: u32,
    pub support: ProfileSupport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ProfileSupport {
    Supported,
    /// Профиль не прошёл проверку; остальные работают (ТЗ §5.1).
    Unsupported {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProfileChoice {
    Auto,
    Manual { subscription_id: String, index: u32 },
}

// ---------------------------------------------------------------------------
// Состояние (ТЗ §4.1)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ChainState {
    Off,
    Starting {
        step: StartStep,
    },
    Protected,
    /// Цепочка не работает, kill switch держит.
    Blocked {
        reason: BlockReason,
    },
    Stopping,
    RollingBack,
    /// «Вход в Wi-Fi», ограничено по времени (ТЗ §7).
    WifiLogin {
        until: u64,
        real_ip_exposed: bool,
    },
    /// Защита снята явной кнопкой при неработающей цепочке (ТЗ §7).
    ProtectionDisabled,
}

/// Шаги запуска в порядке выполнения (ТЗ §4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartStep {
    Snapshot,
    Firewall,
    Xray,
    CheckVpn,
    SingBox,
    CheckProxy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockReason {
    VpnDown,
    AllServersDown,
    ProxyDown,
    AllProxiesDown,
    NetworkChanged,
    CoreCrashed,
    FirewallRulesMissing,
}

/// Одна запись блока «Сейчас изменено» (ТЗ §9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangedSetting {
    pub kind: ChangedKind,
    /// Значение до включения, для показа.
    pub original: Option<String>,
    /// Текущее значение, установленное приложением.
    pub current: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangedKind {
    Firewall,
    Tun,
    Timezone,
    TimezoneAuto,
    Language,
    Firefox,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    pub chain: ChainState,
    pub vpn: LinkHealth,
    pub proxy: LinkHealth,
    pub kill_switch_active: bool,
    pub exit: Option<ExitInfo>,
    /// Имя активного профиля подписки.
    pub active_profile: Option<String>,
    /// Идентификатор активного прокси из пула.
    pub active_proxy_id: Option<String>,
    pub changed: Vec<ChangedSetting>,
    /// Незакрытый снимок после аварии — предложить «Восстановить настройки».
    pub pending_restore: bool,
    pub conflicts: Vec<Conflict>,
    /// Причина последнего неудачного запуска или отката; сбрасывается при
    /// следующем успешном включении.
    pub last_error: Option<IpcError>,
}

/// Конфликты, о которых предупреждаем до запуска (ТЗ §4.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Conflict {
    OtherVpn { interface: String },
    PrivateRelay,
    HelperDisabledByUser,
}

/// Настройки, которые хранит помощник.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub pool_mode: PoolMode,
    /// «Разрешить локальную сеть», по умолчанию включено (ТЗ §7).
    pub allow_lan: bool,
    /// «Запуск цепочки при старте» (ТЗ §7).
    pub start_chain_on_boot: bool,
    /// Согласование часового пояса, по умолчанию включено (ТЗ §10.1).
    pub sync_timezone: bool,
    pub profile: ProfileChoice,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            pool_mode: PoolMode::PrimaryOnly,
            allow_lan: true,
            start_chain_on_boot: false,
            sync_timezone: true,
            profile: ProfileChoice::Auto,
        }
    }
}

// ---------------------------------------------------------------------------
// Запросы и ответы
// ---------------------------------------------------------------------------

/// Первое сообщение клиента после подключения.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    pub protocol_version: u32,
    pub app_version: String,
    /// Токен — дополнение к проверке uid подключившегося (ТЗ §14).
    pub token: Secret,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelloReply {
    pub protocol_version: u32,
    pub helper_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    GetStatus,
    /// Подписаться на события; дальше помощник шлёт [`Event`] в этот канал.
    Subscribe,
    Enable,
    Disable,
    /// «Отключить защиту» при неработающей цепочке — только с подтверждением.
    DisableProtection {
        confirmed: bool,
    },
    /// Аварийный «Сбросить всё» (ТЗ §9).
    ResetAll,
    RestoreSettings,

    GetSettings,
    SetSettings {
        settings: Settings,
    },

    ListProxies,
    /// Задать пул целиком; порядок = приоритет. Запись без пароля с `keep_id`
    /// сохраняет прежний секрет.
    SetProxies {
        proxies: Vec<ProxyInput>,
    },
    /// Ответ — [`Response::Proxies`] с обновлённым пулом.
    CheckProxy {
        id: String,
    },

    ListSubscriptions,
    /// `headers` — дополнительные заголовки запроса; пустой список означает
    /// «заголовки клиента Happ по умолчанию», их подставляет помощник.
    /// Ответ — [`Response::Subscription`].
    AddSubscription {
        url: Secret,
        headers: Vec<HeaderPair>,
    },
    RefreshSubscription {
        id: String,
    },
    RemoveSubscription {
        id: String,
    },

    /// «Войти в сеть Wi-Fi» (ТЗ §7).
    StartWifiLogin,
    StopWifiLogin,

    /// Последние строки журнала (без секретов).
    GetLog {
        max_lines: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProxyInput {
    /// Новый прокси, введённый пользователем.
    New { endpoint: ProxyEndpoint },
    /// Уже известный помощнику прокси (перестановка в пуле).
    Existing { id: String },
}

/// Заголовок запроса подписки (ТЗ §5.1: заголовки как у клиента Happ).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeaderPair {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Response {
    Ok,
    Status {
        status: Status,
    },
    Settings {
        settings: Settings,
    },
    Proxies {
        proxies: Vec<ProxyView>,
    },
    Subscriptions {
        subscriptions: Vec<SubscriptionView>,
    },
    Subscription {
        subscription: SubscriptionView,
    },
    Log {
        lines: Vec<String>,
    },
    Error {
        error: IpcError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{code:?}: {message}")]
pub struct IpcError {
    pub code: ErrorCode,
    /// Текст для журнала; пользователю UI показывает свою строку по `code`.
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Unauthorized,
    VersionMismatch,
    InvalidRequest,
    InvalidState,
    ConfirmationRequired,
    NotFound,
    NoSubscription,
    NoProxy,
    SubscriptionFetchFailed,
    SubscriptionUnsupported,
    Conflict,
    Internal,
}

/// Событие помощника для подписавшегося клиента.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    StatusChanged {
        status: Status,
    },
    /// Прокси переключён (режим «С резервом») — уведомление с новым IP.
    ProxySwitched {
        proxy_id: String,
        exit: Option<ExitInfo>,
    },
    /// До окончания подписки осталось мало дней (ТЗ §5.3).
    SubscriptionExpiring {
        id: String,
        days_left: u32,
    },
    /// Значение изменено пользователем во время работы и не восстановлено.
    SettingKeptAsIs {
        kind: ChangedKind,
    },
}

/// Конверт: всё, что помощник шлёт клиенту после рукопожатия.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Ответ на запрос с тем же `id`.
    Reply {
        id: u64,
        response: Response,
    },
    Event {
        event: Event,
    },
}

/// Конверт запроса клиента.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientMessage {
    pub id: u64,
    pub request: Request,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_is_not_printed() {
        let s = Secret::new("hunter2");
        assert_eq!(format!("{s:?}"), "Secret(***)");
        let p = ProxyEndpoint {
            scheme: ProxyScheme::Socks5,
            host: "1.2.3.4".into(),
            port: 8000,
            username: Some("USER".into()),
            password: Some(s),
        };
        assert!(!format!("{p:?}").contains("hunter2"));
    }

    #[test]
    fn request_wire_format_is_tagged() {
        let m = ClientMessage {
            id: 7,
            request: Request::DisableProtection { confirmed: true },
        };
        let json = serde_json::to_string(&m).unwrap_or_default();
        assert_eq!(
            json,
            r#"{"id":7,"request":{"cmd":"disable_protection","confirmed":true}}"#
        );
        let back: ClientMessage = serde_json::from_str(&json).unwrap_or(ClientMessage {
            id: 0,
            request: Request::GetStatus,
        });
        assert_eq!(back, m);
    }

    #[test]
    fn unknown_command_is_rejected() {
        let r: Result<ClientMessage, _> =
            serde_json::from_str(r#"{"id":1,"request":{"cmd":"run_shell","command":"id"}}"#);
        assert!(r.is_err());
    }
}
