// Чистая машина состояний помощника для mock-бэкенда (ТЗ §4.1).
// Выключено -> Запуск по шагам -> Защищено <-> Блок -> Остановка -> Откат -> Выключено.
// Никаких таймеров и побочных эффектов: время приходит параметром, продвижение
// по шагам — явным вызовом advance(). Данные — только плейсхолдеры.

import type {
  BlockReason,
  ChangedSetting,
  Conflict,
  ErrorCode,
  ExitInfo,
  LinkHealth,
  ProfileView,
  ProxyEndpoint,
  ProxyView,
  Request,
  Response,
  Settings,
  StartStep,
  Status,
  SubscriptionView,
} from '../types.ts'

export const START_STEPS: readonly StartStep[] = [
  'snapshot',
  'firewall',
  'xray',
  'check_vpn',
  'sing_box',
  'check_proxy',
]

export const WIFI_LOGIN_SECONDS = 300
const DAY = 86400

export interface World {
  status: Status
  settings: Settings
  proxies: ProxyView[]
  subscriptions: SubscriptionView[]
  log: string[]
  seq: number
  /** Демо: следующий запуск провалится на проверке прокси. */
  failNextStart: boolean
}

export interface Handled {
  world: World
  response: Response
}

const err = (code: ErrorCode, message: string): Response => ({
  kind: 'error',
  error: { code, message },
})
const OK: Response = { kind: 'ok' }

// --- Начальное состояние ---------------------------------------------------

export function createWorld(now: number): World {
  const profiles: ProfileView[] = [
    { index: 0, name: 'Германия 1', server_count: 3, support: { status: 'supported' } },
    { index: 1, name: 'Германия 2 (фрагментация)', server_count: 2, support: { status: 'supported' } },
    { index: 2, name: 'США', server_count: 2, support: { status: 'supported' } },
    {
      index: 3,
      name: 'Мобильный',
      server_count: 1,
      support: { status: 'unsupported', reason: 'протокол вне белого списка' },
    },
  ]
  return {
    status: {
      chain: { state: 'off' },
      vpn: 'unknown',
      proxy: 'unknown',
      kill_switch_active: false,
      exit: null,
      active_profile: null,
      active_proxy_id: null,
      changed: [],
      pending_restore: false,
      conflicts: [],
      last_error: null,
    },
    settings: {
      pool_mode: 'primary_only',
      allow_lan: true,
      start_chain_on_boot: false,
      sync_timezone: true,
      profile: { kind: 'auto' },
    },
    proxies: [
      {
        id: 'px-1',
        scheme: 'socks5',
        host: '203.0.113.10',
        port: 1080,
        username_masked: 'us***',
        has_password: true,
        health: 'unknown',
        exit: null,
      },
      {
        id: 'px-2',
        scheme: 'http',
        host: '203.0.113.11',
        port: 8080,
        username_masked: 'us***',
        has_password: true,
        health: 'unknown',
        exit: null,
      },
    ],
    subscriptions: [
      {
        id: 'sub-1',
        title: 'Example VPN',
        host: 'sub.example.com',
        usage: {
          upload: 3 * 1024 ** 3,
          download: 41 * 1024 ** 3,
          total: 200 * 1024 ** 3,
          expire: now + 2 * DAY + 3600,
        },
        update_interval_hours: 12,
        support_url: 'https://example.com/support',
        updated_at: now - 3600,
        last_error: null,
        profiles,
      },
    ],
    log: ['helper started (mock)'],
    seq: 2,
    failNextStart: false,
  }
}

// --- Вспомогательное --------------------------------------------------------

function log(w: World, line: string): World {
  return { ...w, log: [...w.log, line].slice(-500) }
}

function withStatus(w: World, patch: Partial<Status>): World {
  return { ...w, status: { ...w.status, ...patch } }
}

function hash(s: string): number {
  let h = 0
  for (const c of s) h = (h * 31 + c.charCodeAt(0)) >>> 0
  return h
}

const US_PLACES = [
  { region: 'Florida', city: 'Miami', tz: 'America/New_York' },
  { region: 'Georgia', city: 'Atlanta', tz: 'America/New_York' },
  { region: 'Illinois', city: 'Chicago', tz: 'America/Chicago' },
]

export function exitFor(p: Pick<ProxyView, 'host'>): ExitInfo {
  const h = hash(p.host)
  const place = US_PLACES[h % US_PLACES.length] ?? US_PLACES[0]!
  const isV4 = /^\d+\.\d+\.\d+\.\d+$/.test(p.host)
  return {
    ip: isV4 ? p.host : `203.0.113.${(h % 253) + 1}`,
    country_code: 'US',
    region: place.region,
    city: place.city,
    asn: 'AS64496',
    org: 'Example ISP',
    timezone: place.tz,
  }
}

export function proxyLooksDead(host: string): boolean {
  return host.endsWith('.99') || host.startsWith('down.')
}

/** Какой профиль будет использован (ТЗ §5.3). null — нет подходящего. */
export function resolveProfile(w: World): string | null {
  const c = w.settings.profile
  if (c.kind === 'manual') {
    const sub = w.subscriptions.find((s) => s.id === c.subscription_id)
    const p = sub?.profiles.find((x) => x.index === c.index)
    return p && p.support.status === 'supported' ? p.name : null
  }
  for (const s of w.subscriptions) {
    const p = s.profiles.find((x) => x.support.status === 'supported')
    if (p) return p.name
  }
  return null
}

/** Активный прокси по режиму пула. */
export function pickProxy(w: World): ProxyView | null {
  if (w.proxies.length === 0) return null
  if (w.settings.pool_mode === 'with_fallback') {
    return w.proxies.find((p) => p.health !== 'down') ?? null
  }
  return w.proxies[0] ?? null
}

function addChanged(w: World, c: ChangedSetting): World {
  if (w.status.changed.some((x) => x.kind === c.kind)) return w
  return withStatus(w, { changed: [...w.status.changed, c] })
}

function protectedStatus(w: World): World {
  const px = pickProxy(w)
  return withStatus(w, {
    chain: { state: 'protected' },
    vpn: 'up',
    proxy: 'up',
    kill_switch_active: true,
    exit: px ? (px.exit ?? exitFor(px)) : null,
    active_proxy_id: px?.id ?? null,
  })
}

export function isActiveChain(s: Status): boolean {
  const st = s.chain.state
  return st !== 'off' && st !== 'stopping' && st !== 'rolling_back'
}

// --- Продвижение по шагам --------------------------------------------------

/** Через сколько мс нужен следующий advance(); null — состояние устойчивое. */
export function nextDelayMs(w: World): number | null {
  switch (w.status.chain.state) {
    case 'starting':
      return 650
    case 'stopping':
      return 600
    case 'rolling_back':
      return 800
    default:
      return null
  }
}

export function advance(w: World): World {
  const chain = w.status.chain
  if (chain.state === 'starting') {
    const step = chain.step
    const idx = START_STEPS.indexOf(step)
    let n = w
    // Эффект завершённого шага.
    switch (step) {
      case 'snapshot':
        n = log(n, 'snapshot written')
        break
      case 'firewall':
        n = addChanged(n, { kind: 'firewall', original: null, current: 'firewall rules' })
        n = log(n, 'firewall installed')
        break
      case 'xray':
        n = withStatus(n, { active_profile: resolveProfile(n) })
        n = log(n, 'xray started')
        break
      case 'check_vpn':
        n = withStatus(n, { vpn: 'up' })
        n = log(n, 'vpn check ok')
        break
      case 'sing_box':
        n = addChanged(n, { kind: 'tun', original: null, current: 'utun9' })
        n = log(n, 'sing-box started')
        break
      case 'check_proxy': {
        const px = pickProxy(n)
        if (n.failNextStart) {
          n = { ...n, failNextStart: false }
          n = withStatus(n, {
            chain: { state: 'rolling_back' },
            last_error: { code: 'internal', message: 'sing-box exited during start' },
          })
          return log(n, 'start failed: sing-box exited; rolling back')
        }
        if (!px || px.health === 'down' || proxyLooksDead(px.host)) {
          n = withStatus(n, {
            proxy: 'down',
            chain: { state: 'rolling_back' },
            last_error: { code: 'no_proxy', message: 'proxy check failed' },
          })
          return log(n, 'proxy check failed: start aborted')
        }
        n = withStatus(protectedStatus(n), { last_error: null })
        if (n.settings.sync_timezone) {
          n = addChanged(n, { kind: 'timezone_auto', original: 'on', current: 'off' })
          n = addChanged(n, {
            kind: 'timezone',
            original: 'Europe/Berlin',
            current: n.status.exit?.timezone ?? 'America/New_York',
          })
        }
        return log(n, 'state: protected')
      }
    }
    const next = START_STEPS[idx + 1]
    return next ? withStatus(n, { chain: { state: 'starting', step: next } }) : n
  }
  if (chain.state === 'stopping') {
    return log(withStatus(w, { chain: { state: 'rolling_back' } }), 'cores stopped; restoring settings')
  }
  if (chain.state === 'rolling_back') {
    return log(
      withStatus(w, {
        chain: { state: 'off' },
        vpn: 'unknown',
        proxy: 'unknown',
        kill_switch_active: false,
        exit: null,
        active_profile: null,
        active_proxy_id: null,
        changed: [],
      }),
      'state: off',
    )
  }
  return w
}

// --- Внешние события (демо) -------------------------------------------------

export function simulateFailure(w: World, reason: BlockReason): World {
  if (w.status.chain.state !== 'protected') return w
  let vpn: LinkHealth = 'unknown'
  let proxy: LinkHealth = 'unknown'
  if (reason === 'vpn_down' || reason === 'all_servers_down') vpn = 'down'
  else if (reason === 'proxy_down' || reason === 'all_proxies_down') {
    vpn = 'up'
    proxy = 'down'
  }
  return log(
    withStatus(w, {
      chain: { state: 'blocked', reason },
      vpn,
      proxy,
      exit: null,
      kill_switch_active: true,
    }),
    `state: blocked (${reason})`,
  )
}

export function simulateRecovery(w: World): World {
  if (w.status.chain.state !== 'blocked') return w
  return log(protectedStatus(w), 'state: protected (recovered)')
}

export function toggleConflict(w: World, c: Conflict): World {
  const has = w.status.conflicts.some((x) => x.kind === c.kind)
  return withStatus(w, {
    conflicts: has ? w.status.conflicts.filter((x) => x.kind !== c.kind) : [...w.status.conflicts, c],
  })
}

export function simulateStartFailure(w: World): World {
  return { ...w, failNextStart: !w.failNextStart }
}

export function simulatePendingRestore(w: World): World {
  if (w.status.chain.state !== 'off') return w
  return withStatus(w, {
    pending_restore: true,
    changed: [{ kind: 'timezone', original: 'Europe/Berlin', current: 'America/New_York' }],
  })
}

// --- Запросы ----------------------------------------------------------------

function beginStop(w: World): World {
  return log(withStatus(w, { chain: { state: 'stopping' } }), 'stop requested')
}

function validEndpoint(e: ProxyEndpoint): boolean {
  return (
    (e.scheme === 'socks5' || e.scheme === 'http') &&
    e.host.length > 0 &&
    Number.isInteger(e.port) &&
    e.port >= 1 &&
    e.port <= 65535
  )
}

const mask = (u: string | null): string | null =>
  u === null ? null : `${Array.from(u).slice(0, 2).join('')}***`

export function handleRequest(w: World, req: Request, now: number): Handled {
  const ret = (world: World, response: Response = OK): Handled => ({ world, response })
  const st = w.status.chain.state

  switch (req.cmd) {
    case 'get_status':
      return ret(w, { kind: 'status', status: w.status })
    case 'subscribe':
      return ret(w)

    case 'enable': {
      if (st !== 'off') return ret(w, err('invalid_state', `cannot enable from ${st}`))
      if (w.status.pending_restore) return ret(w, err('invalid_state', 'restore settings first'))
      if (resolveProfile(w) === null) return ret(w, err('no_subscription', 'no usable profile'))
      if (w.proxies.length === 0) return ret(w, err('no_proxy', 'proxy pool is empty'))
      return ret(log(withStatus(w, { chain: { state: 'starting', step: 'snapshot' } }), 'start requested'))
    }

    case 'disable':
      if (st === 'off' || st === 'stopping' || st === 'rolling_back') return ret(w)
      return ret(beginStop(w))

    case 'disable_protection': {
      if (st !== 'blocked') return ret(w, err('invalid_state', 'protection can be disabled only in block'))
      if (!req.confirmed) return ret(w, err('confirmation_required', 'confirmation required'))
      return ret(
        log(
          withStatus(w, { chain: { state: 'protection_disabled' }, kill_switch_active: false }),
          'protection disabled by user',
        ),
      )
    }

    case 'reset_all': {
      let n = withStatus(w, { pending_restore: false })
      if (st === 'off') return ret(withStatus(n, { changed: [] }))
      if (st !== 'stopping' && st !== 'rolling_back') n = beginStop(n)
      return ret(log(n, 'reset all'))
    }

    case 'restore_settings':
      if (!w.status.pending_restore) return ret(w, err('invalid_state', 'nothing to restore'))
      return ret(log(withStatus(w, { pending_restore: false, changed: [] }), 'settings restored from snapshot'))

    case 'get_settings':
      return ret(w, { kind: 'settings', settings: w.settings })

    case 'set_settings': {
      const c = req.settings.profile
      if (c.kind === 'manual') {
        const p = w.subscriptions
          .find((s) => s.id === c.subscription_id)
          ?.profiles.find((x) => x.index === c.index)
        if (!p) return ret(w, err('not_found', 'profile not found'))
        if (p.support.status !== 'supported') return ret(w, err('subscription_unsupported', 'profile unsupported'))
      }
      return ret({ ...w, settings: req.settings }, { kind: 'settings', settings: req.settings })
    }

    case 'list_proxies':
      return ret(w, { kind: 'proxies', proxies: w.proxies })

    case 'set_proxies': {
      let seq = w.seq
      const next: ProxyView[] = []
      for (const input of req.proxies) {
        if (input.kind === 'existing') {
          const found = w.proxies.find((p) => p.id === input.id)
          if (!found) return ret(w, err('not_found', `proxy ${input.id}`))
          next.push(found)
        } else {
          const e = input.endpoint
          if (!validEndpoint(e)) return ret(w, err('invalid_request', 'invalid endpoint'))
          seq += 1
          next.push({
            id: `px-${seq}`,
            scheme: e.scheme,
            host: e.host,
            port: e.port,
            username_masked: mask(e.username),
            has_password: e.password !== null && e.password !== '',
            health: 'unknown',
            exit: null,
          })
        }
      }
      if (next.length === 0 && isActiveChain(w.status))
        return ret(w, err('invalid_state', 'cannot empty the pool while running'))
      let n: World = { ...w, proxies: next, seq }
      if (isActiveChain(n.status) && n.status.active_proxy_id !== null) {
        n = withStatus(n, { active_proxy_id: pickProxy(n)?.id ?? null })
      }
      return ret(log(n, `proxy pool updated (${next.length})`), { kind: 'proxies', proxies: next })
    }

    case 'check_proxy': {
      const p = w.proxies.find((x) => x.id === req.id)
      if (!p) return ret(w, err('not_found', `proxy ${req.id}`))
      const updated: ProxyView = proxyLooksDead(p.host)
        ? { ...p, health: 'down', exit: null }
        : { ...p, health: 'up', exit: exitFor(p) }
      const proxies = w.proxies.map((x) => (x.id === p.id ? updated : x))
      return ret(log({ ...w, proxies }, `proxy ${p.id} checked: ${updated.health}`), {
        kind: 'proxies',
        proxies,
      })
    }

    case 'list_subscriptions':
      return ret(w, { kind: 'subscriptions', subscriptions: w.subscriptions })

    case 'add_subscription': {
      let url: URL
      try {
        url = new URL(req.url)
      } catch {
        return ret(w, err('invalid_request', 'invalid url'))
      }
      if (url.protocol !== 'https:') return ret(w, err('invalid_request', 'https required'))
      if (w.subscriptions.some((s) => s.host === url.hostname))
        return ret(w, err('conflict', 'subscription with this host exists'))
      if (url.hostname.includes('fail')) return ret(w, err('subscription_fetch_failed', 'fetch failed'))
      if (url.hostname.includes('unsupported'))
        return ret(w, err('subscription_unsupported', 'no supported profiles'))
      const seq = w.seq + 1
      const sub: SubscriptionView = {
        id: `sub-${seq}`,
        title: url.hostname,
        host: url.hostname,
        usage: {
          upload: 1024 ** 3,
          download: 5 * 1024 ** 3,
          total: 100 * 1024 ** 3,
          expire: now + 30 * DAY,
        },
        update_interval_hours: 24,
        support_url: null,
        updated_at: now,
        last_error: null,
        profiles: [
          { index: 0, name: 'США', server_count: 2, support: { status: 'supported' } },
          { index: 1, name: 'Нидерланды', server_count: 1, support: { status: 'supported' } },
        ],
      }
      return ret(
        log({ ...w, seq, subscriptions: [...w.subscriptions, sub] }, `subscription added: ${sub.host}`),
        { kind: 'subscription', subscription: sub },
      )
    }

    case 'refresh_subscription': {
      const s = w.subscriptions.find((x) => x.id === req.id)
      if (!s) return ret(w, err('not_found', `subscription ${req.id}`))
      const upd: SubscriptionView = { ...s, updated_at: now, last_error: null }
      return ret(
        log(
          { ...w, subscriptions: w.subscriptions.map((x) => (x.id === s.id ? upd : x)) },
          `subscription ${s.id} refreshed`,
        ),
        { kind: 'subscription', subscription: upd },
      )
    }

    case 'remove_subscription': {
      if (!w.subscriptions.some((x) => x.id === req.id))
        return ret(w, err('not_found', `subscription ${req.id}`))
      let n: World = { ...w, subscriptions: w.subscriptions.filter((x) => x.id !== req.id) }
      const c = n.settings.profile
      if (c.kind === 'manual' && c.subscription_id === req.id) {
        n = { ...n, settings: { ...n.settings, profile: { kind: 'auto' } } }
      }
      return ret(log(n, `subscription ${req.id} removed`))
    }

    case 'start_wifi_login':
      if (st !== 'blocked' && st !== 'protected') return ret(w, err('invalid_state', 'wifi login unavailable'))
      return ret(
        log(
          withStatus(w, {
            chain: { state: 'wifi_login', until: now + WIFI_LOGIN_SECONDS, real_ip_exposed: false },
            exit: null,
          }),
          'wifi login started',
        ),
      )

    case 'stop_wifi_login':
      if (st !== 'wifi_login') return ret(w, err('invalid_state', 'not in wifi login'))
      return ret(
        log(withStatus(w, { chain: { state: 'blocked', reason: 'network_changed' } }), 'wifi login stopped'),
      )

    case 'get_log':
      return ret(w, { kind: 'log', lines: w.log.slice(-Math.max(0, req.max_lines)) })
  }
}
