//! Генерация конфига sing-box 1.14.4 (ТЗ §6–§8, справка `docs/reference/sing-box.md`).
//!
//! Чистые функции: порты, секреты, пути и IP приходят аргументами.
//!
//! Ключевые решения:
//! - **`direct` outbound не создаётся** (справка §8.1): иначе ядро требует
//!   `default_domain_resolver`, а у sing-box появляется путь мимо цепочки.
//!   Каждый outbound, кроме `xray` (loopback), имеет `detour`. LAN отдаётся ОС
//!   через `route_exclude_address` TUN.
//! - **Прокси без `domain_resolver`**: домен прокси уходит в Xray как есть
//!   (справка §4.5, §8.2).
//! - **`auto_detect_interface: false`**: все исходящие соединения sing-box идут
//!   только на 127.0.0.1 (Xray) — через `detour`; привязка к физическому
//!   интерфейсу не нужна, а привязка loopback-соединений подтверждена лишь
//!   чтением исходников (справка §4.5). Петлю Xray→TUN рвёт `sockopt.interface`
//!   Xray и исключения `route_exclude_address` (ТЗ §4). Если что-то всё же
//!   попытается выйти не через detour, оно упрётся в TUN (отказ), а не утечёт.
//! - **`stack: "system"`** задан явно, как в проверенном ядром примере (справка §8.3).
//! - **AAAA** — пустой NOERROR, `inet6_range` у fake-ip нет; IPv6-литералы
//!   отклоняются правилом маршрута (справка §5.3).
//! - UDP разрешён вместе с http-прокси — ошибка (CONNECT не несёт UDP).

mod build;
mod clash;
mod error;
mod input;
mod validate;

pub use build::{build_config, render_config};
pub use clash::{
    CLASH_AUTH_HEADER, CLASH_DELAY_SUFFIX, CLASH_DELAY_TIMEOUT_PARAM, CLASH_DELAY_URL_PARAM,
    CLASH_PROXIES_PATH, CLASH_VERSION_PATH, PROXY_TAG_PREFIX, SELECTOR_TAG, TUN_INBOUND_TAG,
    XRAY_OUTBOUND_TAG, proxy_delay_path, proxy_id_from_tag, proxy_tag, selector_path, selector_tag,
};
pub use error::SingboxError;
pub use input::{
    CLASH_SECRET_MIN_LEN, CachePath, ClashApi, LogLevel, PoolEntry, ProxyId, ProxyPool,
    SingboxInput, VpnServerIps, XraySocks,
};
