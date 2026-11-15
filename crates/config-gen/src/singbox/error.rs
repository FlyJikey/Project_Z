//! Ошибки сборки конфига sing-box.
//!
//! Правило (ТЗ §14): в сообщениях нет значений, пришедших от пользователя
//! (хост, логин, пароль, секрет, путь): в поле «хост» могла быть вставлена вся
//! строка `user:pass@host`. Только номера записей, теги и названия полей.

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SingboxError {
    #[error("пул прокси пуст: нужен хотя бы один прокси")]
    EmptyPool,
    #[error("недопустимый идентификатор записи пула: допустимы 1..=64 символов из [A-Za-z0-9_-]")]
    InvalidProxyId,
    #[error("идентификатор записи пула повторяется: {id}")]
    DuplicateProxyId { id: String },
    #[error("прокси №{position}: недопустимый адрес (нужен IP или доменное имя)")]
    InvalidProxyHost { position: usize },
    #[error("прокси №{position}: порт 0 недопустим")]
    ProxyPortZero { position: usize },
    #[error("прокси №{position}: {reason}")]
    InvalidProxyCredentials {
        position: usize,
        reason: &'static str,
    },
    #[error("socks-inbound Xray: порт 0 недопустим")]
    XrayPortZero,
    #[error("socks-inbound Xray: {reason}")]
    InvalidXrayCredentials { reason: &'static str },
    #[error("Clash API: порт 0 недопустим")]
    ClashPortZero,
    #[error(
        "Clash API: секрет слишком короткий (нужно не менее {min} символов) или содержит управляющие символы"
    )]
    InvalidClashSecret { min: usize },
    #[error("порты socks-inbound Xray и Clash API совпадают")]
    PortConflict,
    #[error("список IP VPN-серверов пуст")]
    NoVpnServerIps,
    #[error("недопустимый IP VPN-сервера (нулевой, loopback или multicast)")]
    InvalidVpnServerIp,
    #[error("недопустимый путь cache_file: {reason}")]
    InvalidCachePath { reason: &'static str },
    #[error(
        "UDP разрешён, но в пуле есть http-прокси (CONNECT не передаёт UDP): \
         выключите UDP или уберите http-прокси"
    )]
    UdpWithHttpProxy,
}
