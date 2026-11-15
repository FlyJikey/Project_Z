// ВАЖНО: файл обязан в точности соответствовать crates/ipc-proto/src/lib.rs
// и версии PROTOCOL_VERSION ниже. Любое изменение типов в ipc-proto
// (имена полей, serde-теги, rename_all) требует правки здесь и повышения
// версии протокола. Источник истины — Rust-крейт, не этот файл.
//
// Соответствие serde:
//  - Option<T>            -> T | null
//  - Secret               -> string (только в направлении UI -> помощник;
//                            в ответах помощника секретов нет, ТЗ §14)
//  - #[serde(tag = "x")]  -> размеченное объединение по полю x
//  - rename_all           -> значения snake_case / lowercase как на проводе

export const PROTOCOL_VERSION = 1

/** Секрет. Только UI -> помощник; в состоянии UI после отправки не хранится. */
export type Secret = string

// --- Прокси (ТЗ §6) ---

export type ProxyScheme = 'socks5' | 'http'

export interface ProxyEndpoint {
  scheme: ProxyScheme
  host: string
  port: number
  username: string | null
  password: Secret | null
}

export type PoolMode = 'primary_only' | 'with_fallback'

export type LinkHealth = 'unknown' | 'up' | 'down'

export interface ExitInfo {
  ip: string
  country_code: string | null
  region: string | null
  city: string | null
  asn: string | null
  org: string | null
  timezone: string | null
}

export interface ProxyView {
  id: string
  scheme: ProxyScheme
  host: string
  port: number
  username_masked: string | null
  has_password: boolean
  health: LinkHealth
  exit: ExitInfo | null
}

// --- Подписки (ТЗ §5) ---

export interface SubscriptionUsage {
  upload: number | null
  download: number | null
  total: number | null
  /** Unix-секунды; null — бессрочно. */
  expire: number | null
}

export type ProfileSupport =
  | { status: 'supported' }
  | { status: 'unsupported'; reason: string }

export interface ProfileView {
  index: number
  name: string
  server_count: number
  support: ProfileSupport
}

export interface SubscriptionView {
  id: string
  title: string
  host: string
  usage: SubscriptionUsage | null
  update_interval_hours: number | null
  support_url: string | null
  /** Unix-секунды. */
  updated_at: number | null
  last_error: string | null
  profiles: ProfileView[]
}

export type ProfileChoice =
  | { kind: 'auto' }
  | { kind: 'manual'; subscription_id: string; index: number }

// --- Состояние (ТЗ §4.1) ---

export type StartStep =
  | 'snapshot'
  | 'firewall'
  | 'xray'
  | 'check_vpn'
  | 'sing_box'
  | 'check_proxy'

export type BlockReason =
  | 'vpn_down'
  | 'all_servers_down'
  | 'proxy_down'
  | 'all_proxies_down'
  | 'network_changed'
  | 'core_crashed'
  | 'firewall_rules_missing'

export type ChainState =
  | { state: 'off' }
  | { state: 'starting'; step: StartStep }
  | { state: 'protected' }
  | { state: 'blocked'; reason: BlockReason }
  | { state: 'stopping' }
  | { state: 'rolling_back' }
  | { state: 'wifi_login'; until: number; real_ip_exposed: boolean }
  | { state: 'protection_disabled' }

export type ChangedKind =
  | 'firewall'
  | 'tun'
  | 'timezone'
  | 'timezone_auto'
  | 'language'
  | 'firefox'

export interface ChangedSetting {
  kind: ChangedKind
  original: string | null
  current: string
}

export type Conflict =
  | { kind: 'other_vpn'; interface: string }
  | { kind: 'private_relay' }
  | { kind: 'helper_disabled_by_user' }

export interface Status {
  chain: ChainState
  vpn: LinkHealth
  proxy: LinkHealth
  kill_switch_active: boolean
  exit: ExitInfo | null
  active_profile: string | null
  active_proxy_id: string | null
  changed: ChangedSetting[]
  pending_restore: boolean
  conflicts: Conflict[]
  /** Причина последнего неудачного запуска/отката; сбрасывается при успешном включении. */
  last_error: IpcError | null
}

export interface Settings {
  pool_mode: PoolMode
  allow_lan: boolean
  start_chain_on_boot: boolean
  sync_timezone: boolean
  profile: ProfileChoice
}

// --- Запросы и ответы ---

export interface Hello {
  protocol_version: number
  app_version: string
  token: Secret
}

export interface HelloReply {
  protocol_version: number
  helper_version: string
}

export type ProxyInput =
  | { kind: 'new'; endpoint: ProxyEndpoint }
  | { kind: 'existing'; id: string }

export interface HeaderPair {
  name: string
  value: string
}

export type Request =
  | { cmd: 'get_status' }
  | { cmd: 'subscribe' }
  | { cmd: 'enable' }
  | { cmd: 'disable' }
  | { cmd: 'disable_protection'; confirmed: boolean }
  | { cmd: 'reset_all' }
  | { cmd: 'restore_settings' }
  | { cmd: 'get_settings' }
  | { cmd: 'set_settings'; settings: Settings }
  | { cmd: 'list_proxies' }
  | { cmd: 'set_proxies'; proxies: ProxyInput[] }
  | { cmd: 'check_proxy'; id: string }
  | { cmd: 'list_subscriptions' }
  | { cmd: 'add_subscription'; url: Secret; headers: HeaderPair[] }
  | { cmd: 'refresh_subscription'; id: string }
  | { cmd: 'remove_subscription'; id: string }
  | { cmd: 'start_wifi_login' }
  | { cmd: 'stop_wifi_login' }
  | { cmd: 'get_log'; max_lines: number }

export type ErrorCode =
  | 'unauthorized'
  | 'version_mismatch'
  | 'invalid_request'
  | 'invalid_state'
  | 'confirmation_required'
  | 'not_found'
  | 'no_subscription'
  | 'no_proxy'
  | 'subscription_fetch_failed'
  | 'subscription_unsupported'
  | 'conflict'
  | 'internal'

export interface IpcError {
  code: ErrorCode
  /** Для журнала; пользователю показывается строка по code. */
  message: string
}

export type Response =
  | { kind: 'ok' }
  | { kind: 'status'; status: Status }
  | { kind: 'settings'; settings: Settings }
  | { kind: 'proxies'; proxies: ProxyView[] }
  | { kind: 'subscriptions'; subscriptions: SubscriptionView[] }
  | { kind: 'subscription'; subscription: SubscriptionView }
  | { kind: 'log'; lines: string[] }
  | { kind: 'error'; error: IpcError }

export type Event =
  | { event: 'status_changed'; status: Status }
  | { event: 'proxy_switched'; proxy_id: string; exit: ExitInfo | null }
  | { event: 'subscription_expiring'; id: string; days_left: number }
  | { event: 'setting_kept_as_is'; kind: ChangedKind }

// Конверты провода (на стороне UI используются только адаптером Tauri).
export type ServerMessage =
  | { type: 'reply'; id: number; response: Response }
  | { type: 'event'; event: Event }

export interface ClientMessage {
  id: number
  request: Request
}
