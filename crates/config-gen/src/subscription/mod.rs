//! Разбор подписки формата Happ / Xray JSON (ТЗ §5.1).
//!
//! Подписка — недоверенный ввод. Тело разбирается в строгие типы
//! ([`model`]) по белым спискам протоколов и полей; всё остальное
//! отбрасывается с предупреждением ([`Warning`], без значений и секретов).
//! Конфиг для Xray собирается заново модулем [`crate::xray`]; исходный JSON в
//! Xray не передаётся никогда.
//!
//! Что отбрасывается: неизвестные протоколы и поля; `routing.rules`, `dns`,
//! `inbounds`, `api`, `policy`, `loopback`/`dns`/`blackhole`-outbound'ы; пути
//! к файлам (сертификаты, `masterKeyLog`, unix-сокеты); `sockopt.interface`,
//! `mark`; `mux`; `allowInsecure` (см. [`outbound`]: сервер помечается
//! неподдерживаемым).
//!
//! Что сохраняется: серверы балансировщика и вся цепочка их зависимостей
//! (`dialerProxy`, в том числе freedom `fragment`).

mod fields;
mod meta;
pub mod model;
mod outbound;
mod parse;
mod url_check;

pub use meta::{SubscriptionMeta, parse_response_headers, profile_views};
pub use model::*;
pub use parse::{SubscriptionError, limits, parse_subscription};
pub use url_check::{
    UrlError, check_redirect_target, is_local_or_private_host, is_non_public_ip,
    parse_subscription_url,
};
