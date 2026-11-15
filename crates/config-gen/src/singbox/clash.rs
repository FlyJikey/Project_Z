//! Теги и пути Clash API: то, чем пользуется помощник для переключения.
//! Пути эндпоинтов взяты из исходников sing-box 1.14.4 (справка §6.1);
//! в документации sing-box их нет.

use super::input::ProxyId;

/// Тег socks-outbound на локальный Xray.
pub const XRAY_OUTBOUND_TAG: &str = "xray";
/// Тег selector'а (он же `route.final`).
pub const SELECTOR_TAG: &str = "proxy";
/// Тег TUN-inbound.
pub const TUN_INBOUND_TAG: &str = "tun-in";
/// Префикс тегов прокси пользователя.
pub const PROXY_TAG_PREFIX: &str = "proxy-";

/// Заголовок авторизации: `Authorization: Bearer <secret>`.
pub const CLASH_AUTH_HEADER: &str = "Authorization";
/// `GET /version`.
pub const CLASH_VERSION_PATH: &str = "/version";
/// `GET /proxies` — список; `GET/PUT /proxies/{name}` — информация / переключение selector
/// (тело PUT: `{"name":"<тег прокси>"}`, успех — 204).
pub const CLASH_PROXIES_PATH: &str = "/proxies";
/// `GET /proxies/{name}/delay?url=<https-URL>&timeout=<мс>`.
pub const CLASH_DELAY_SUFFIX: &str = "/delay";
/// Имя query-параметра проверки задержки.
pub const CLASH_DELAY_URL_PARAM: &str = "url";
pub const CLASH_DELAY_TIMEOUT_PARAM: &str = "timeout";

/// Тег outbound прокси по стабильному id. Символы id ограничены `[A-Za-z0-9_-]`,
/// поэтому тег безопасен в URL без кодирования.
pub fn proxy_tag(id: &ProxyId) -> String {
    format!("{PROXY_TAG_PREFIX}{}", id.as_str())
}

/// Обратное преобразование: id из тега (для сообщений Clash API).
pub fn proxy_id_from_tag(tag: &str) -> Option<ProxyId> {
    ProxyId::new(tag.strip_prefix(PROXY_TAG_PREFIX)?).ok()
}

/// Тег selector'а.
pub fn selector_tag() -> &'static str {
    SELECTOR_TAG
}

/// Путь переключения selector'а: `/proxies/proxy` (метод PUT).
pub fn selector_path() -> String {
    format!("{CLASH_PROXIES_PATH}/{SELECTOR_TAG}")
}

/// Путь проверки задержки прокси: `/proxies/proxy-<id>/delay` (метод GET).
pub fn proxy_delay_path(id: &ProxyId) -> String {
    format!("{CLASH_PROXIES_PATH}/{}{CLASH_DELAY_SUFFIX}", proxy_tag(id))
}
