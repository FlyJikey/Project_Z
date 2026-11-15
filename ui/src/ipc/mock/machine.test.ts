import { describe, expect, it } from 'vitest'
import {
  advance,
  createWorld,
  handleRequest,
  nextDelayMs,
  simulateFailure,
  simulatePendingRestore,
  simulateStartFailure,
  simulateRecovery,
  START_STEPS,
  type World,
} from './machine.ts'
import type { Request, Response } from '../types.ts'

const NOW = 1_800_000_000

function req(w: World, r: Request): { w: World; res: Response } {
  const h = handleRequest(w, r, NOW)
  return { w: h.world, res: h.response }
}

/** Прогон таймера до устойчивого состояния; возвращает пройденные состояния. */
function settle(w: World): { w: World; trail: string[] } {
  const trail: string[] = []
  let cur = w
  for (let i = 0; i < 50 && nextDelayMs(cur) !== null; i++) {
    const c = cur.status.chain
    trail.push(c.state === 'starting' ? `starting:${c.step}` : c.state)
    cur = advance(cur)
  }
  trail.push(cur.status.chain.state)
  return { w: cur, trail }
}

const errCode = (r: Response) => (r.kind === 'error' ? r.error.code : null)

describe('mock-машина: полный цикл', () => {
  it('Выключено → запуск по шагам в порядке ТЗ → Защищено', () => {
    const { w: started } = req(createWorld(NOW), { cmd: 'enable' })
    expect(started.status.chain).toEqual({ state: 'starting', step: 'snapshot' })
    const { w, trail } = settle(started)
    expect(trail.slice(0, 6)).toEqual(START_STEPS.map((s) => `starting:${s}`))
    expect(w.status.chain.state).toBe('protected')
    expect(w.status.kill_switch_active).toBe(true)
    expect(w.status.vpn).toBe('up')
    expect(w.status.proxy).toBe('up')
    expect(w.status.exit?.ip).toBe('203.0.113.10')
    expect(w.status.active_proxy_id).toBe('px-1')
    expect(w.status.active_profile).toBe('Германия 1')
    const kinds = w.status.changed.map((c) => c.kind)
    expect(kinds).toContain('firewall')
    expect(kinds).toContain('tun')
    expect(kinds).toContain('timezone')
  })

  it('Защищено → Остановка → Откат → Выключено, блок «изменено» пуст', () => {
    const on = settle(req(createWorld(NOW), { cmd: 'enable' }).w).w
    const { w: stopping } = req(on, { cmd: 'disable' })
    expect(stopping.status.chain.state).toBe('stopping')
    const { w: off, trail } = settle(stopping)
    expect(trail).toEqual(['stopping', 'rolling_back', 'off'])
    expect(off.status.changed).toEqual([])
    expect(off.status.exit).toBeNull()
    expect(off.status.kill_switch_active).toBe(false)
    expect(off.status.vpn).toBe('unknown')
  })

  it('синхронизация часового пояса выключена — пояс не меняется', () => {
    let w = createWorld(NOW)
    w = req(w, { cmd: 'set_settings', settings: { ...w.settings, sync_timezone: false } }).w
    const on = settle(req(w, { cmd: 'enable' }).w).w
    expect(on.status.changed.map((c) => c.kind)).not.toContain('timezone')
  })

  it('disable в состоянии «Выключено» — идемпотентно', () => {
    const { w, res } = req(createWorld(NOW), { cmd: 'disable' })
    expect(res.kind).toBe('ok')
    expect(w.status.chain.state).toBe('off')
  })
})

describe('mock-машина: блок и восстановление', () => {
  const protectedWorld = () => settle(req(createWorld(NOW), { cmd: 'enable' }).w).w

  it('падение VPN → Блок, kill switch держит, IP скрыт, прокси не считается упавшим', () => {
    const b = simulateFailure(protectedWorld(), 'vpn_down')
    expect(b.status.chain).toEqual({ state: 'blocked', reason: 'vpn_down' })
    expect(b.status.vpn).toBe('down')
    expect(b.status.proxy).toBe('unknown')
    expect(b.status.kill_switch_active).toBe(true)
    expect(b.status.exit).toBeNull()
  })

  it('падение прокси → VPN остаётся up', () => {
    const b = simulateFailure(protectedWorld(), 'proxy_down')
    expect(b.status.vpn).toBe('up')
    expect(b.status.proxy).toBe('down')
  })

  it('восстановление возвращает Защищено с IP', () => {
    const r = simulateRecovery(simulateFailure(protectedWorld(), 'vpn_down'))
    expect(r.status.chain.state).toBe('protected')
    expect(r.status.exit).not.toBeNull()
  })

  it('сбой вне «Защищено» игнорируется', () => {
    const off = createWorld(NOW)
    expect(simulateFailure(off, 'vpn_down')).toBe(off)
  })

  it('«Отключить защиту»: без подтверждения — ошибка, с подтверждением — только из Блока', () => {
    const blocked = simulateFailure(protectedWorld(), 'proxy_down')
    expect(errCode(req(blocked, { cmd: 'disable_protection', confirmed: false }).res)).toBe('confirmation_required')
    expect(errCode(req(protectedWorld(), { cmd: 'disable_protection', confirmed: true }).res)).toBe('invalid_state')
    const { w, res } = req(blocked, { cmd: 'disable_protection', confirmed: true })
    expect(res.kind).toBe('ok')
    expect(w.status.chain.state).toBe('protection_disabled')
    expect(w.status.kill_switch_active).toBe(false)
    // из этого состояния обычное выключение возвращает всё в исходное
    expect(settle(req(w, { cmd: 'disable' }).w).w.status.chain.state).toBe('off')
  })

  it('«Сбросить всё» из Блока доводит до Выключено', () => {
    const blocked = simulateFailure(protectedWorld(), 'vpn_down')
    const { w } = req(blocked, { cmd: 'reset_all' })
    expect(settle(w).w.status.chain.state).toBe('off')
  })
})

describe('mock-машина: ошибки включения', () => {
  it('повторное включение — invalid_state', () => {
    const on = req(createWorld(NOW), { cmd: 'enable' }).w
    expect(errCode(req(on, { cmd: 'enable' }).res)).toBe('invalid_state')
  })
  it('нет прокси — no_proxy', () => {
    const w = req(createWorld(NOW), { cmd: 'set_proxies', proxies: [] }).w
    expect(errCode(req(w, { cmd: 'enable' }).res)).toBe('no_proxy')
  })
  it('нет подписок — no_subscription', () => {
    let w = createWorld(NOW)
    w = req(w, { cmd: 'remove_subscription', id: 'sub-1' }).w
    expect(errCode(req(w, { cmd: 'enable' }).res)).toBe('no_subscription')
  })
  it('мёртвый основной прокси: запуск не удаётся → Откат → Выключено', () => {
    let w = createWorld(NOW)
    w = req(w, {
      cmd: 'set_proxies',
      proxies: [{ kind: 'new', endpoint: { scheme: 'socks5', host: '203.0.113.99', port: 1080, username: null, password: null } }],
    }).w
    const { w: end, trail } = settle(req(w, { cmd: 'enable' }).w)
    expect(trail).toContain('rolling_back')
    expect(end.status.chain.state).toBe('off')
    expect(end.status.changed).toEqual([])
  })
  it('аварийный запуск: Starting → RollingBack → Off, last_error заполнен и сбрасывается при успехе', () => {
    const armed = simulateStartFailure(createWorld(NOW))
    const { w: end, trail } = settle(req(armed, { cmd: 'enable' }).w)
    expect(trail).toContain('starting:check_proxy')
    expect(trail.slice(-2)).toEqual(['rolling_back', 'off'])
    expect(end.status.last_error).toEqual({ code: 'internal', message: 'sing-box exited during start' })
    expect(end.status.changed).toEqual([])
    expect(end.failNextStart).toBe(false)
    const ok = settle(req(end, { cmd: 'enable' }).w).w
    expect(ok.status.chain.state).toBe('protected')
    expect(ok.status.last_error).toBeNull()
  })
  it('мёртвый прокси при запуске даёт last_error no_proxy', () => {
    let w = createWorld(NOW)
    w = req(w, { cmd: 'set_proxies', proxies: [{ kind: 'new', endpoint: { scheme: 'http', host: '203.0.113.99', port: 80, username: null, password: null } }] }).w
    expect(settle(req(w, { cmd: 'enable' }).w).w.status.last_error?.code).toBe('no_proxy')
  })
  it('незакрытый снимок блокирует включение, пока не восстановлен', () => {
    const w = simulatePendingRestore(createWorld(NOW))
    expect(w.status.pending_restore).toBe(true)
    expect(errCode(req(w, { cmd: 'enable' }).res)).toBe('invalid_state')
    const r = req(w, { cmd: 'restore_settings' })
    expect(r.w.status.pending_restore).toBe(false)
    expect(r.w.status.changed).toEqual([])
    expect(errCode(req(r.w, { cmd: 'restore_settings' }).res)).toBe('invalid_state')
  })
})

describe('mock-машина: пул прокси', () => {
  const newP = (host: string, pw: string | null = 'SECRET') => ({
    kind: 'new' as const,
    endpoint: { scheme: 'socks5' as const, host, port: 1080, username: 'alice', password: pw },
  })

  it('новый прокси: логин скрыт, пароль в ответе и состоянии не появляется', () => {
    const w0 = createWorld(NOW)
    const { w, res } = req(w0, {
      cmd: 'set_proxies',
      proxies: [...w0.proxies.map((p) => ({ kind: 'existing' as const, id: p.id })), newP('203.0.113.20')],
    })
    expect(res.kind).toBe('proxies')
    const added = w.proxies[2]!
    expect(added.username_masked).toBe('al***')
    expect(added.has_password).toBe(true)
    expect(JSON.stringify(w)).not.toContain('SECRET')
    expect(JSON.stringify(res)).not.toContain('SECRET')
  })

  it('перестановка сохраняет id и меняет порядок (основной — первый)', () => {
    const w0 = createWorld(NOW)
    const { w } = req(w0, {
      cmd: 'set_proxies',
      proxies: [{ kind: 'existing', id: 'px-2' }, { kind: 'existing', id: 'px-1' }],
    })
    expect(w.proxies.map((p) => p.id)).toEqual(['px-2', 'px-1'])
  })

  it('неизвестный id и неверный порт', () => {
    const w0 = createWorld(NOW)
    expect(errCode(req(w0, { cmd: 'set_proxies', proxies: [{ kind: 'existing', id: 'nope' }] }).res)).toBe('not_found')
    const badPort = { kind: 'new' as const, endpoint: { scheme: 'http' as const, host: 'h.example.com', port: 0, username: null, password: null } }
    expect(errCode(req(w0, { cmd: 'set_proxies', proxies: [badPort] }).res)).toBe('invalid_request')
  })

  it('check_proxy: живой и мёртвый', () => {
    const w0 = createWorld(NOW)
    const a = req(w0, { cmd: 'check_proxy', id: 'px-1' }).w
    expect(a.proxies[0]!.health).toBe('up')
    expect(a.proxies[0]!.exit?.country_code).toBe('US')
    const dead = req(w0, { cmd: 'set_proxies', proxies: [newP('203.0.113.99')] }).w
    const b = req(dead, { cmd: 'check_proxy', id: dead.proxies[0]!.id }).w
    expect(b.proxies[0]!.health).toBe('down')
    expect(errCode(req(w0, { cmd: 'check_proxy', id: 'x' }).res)).toBe('not_found')
  })

  it('режим «С резервом»: берётся следующий живой прокси', () => {
    let w = createWorld(NOW)
    w = req(w, { cmd: 'set_settings', settings: { ...w.settings, pool_mode: 'with_fallback' } }).w
    w = req(w, {
      cmd: 'set_proxies',
      proxies: [newP('203.0.113.99'), { kind: 'existing', id: 'px-2' }],
    }).w
    w = req(w, { cmd: 'check_proxy', id: w.proxies[0]!.id }).w
    const on = settle(req(w, { cmd: 'enable' }).w).w
    expect(on.status.chain.state).toBe('protected')
    expect(on.status.active_proxy_id).toBe('px-2')
  })

  it('«Только основной»: мёртвый основной не заменяется резервным', () => {
    let w = createWorld(NOW)
    w = req(w, { cmd: 'set_proxies', proxies: [newP('203.0.113.99'), { kind: 'existing', id: 'px-2' }] }).w
    expect(settle(req(w, { cmd: 'enable' }).w).w.status.chain.state).toBe('off')
  })
})

describe('mock-машина: подписки и настройки', () => {
  it('добавление: только https, виден хост, адрес с токеном не сохраняется', () => {
    const w0 = createWorld(NOW)
    expect(errCode(req(w0, { cmd: 'add_subscription', url: 'http://new.example.com/sub/TOKEN', headers: [] }).res)).toBe('invalid_request')
    expect(errCode(req(w0, { cmd: 'add_subscription', url: 'не адрес', headers: [] }).res)).toBe('invalid_request')
    const { w, res } = req(w0, { cmd: 'add_subscription', url: 'https://new.example.com/sub/TOKEN123', headers: [] })
    expect(res.kind).toBe('subscription')
    expect(w.subscriptions[1]!.host).toBe('new.example.com')
    expect(JSON.stringify(w)).not.toContain('TOKEN123')
    expect(JSON.stringify(res)).not.toContain('TOKEN123')
    expect(errCode(req(w, { cmd: 'add_subscription', url: 'https://new.example.com/other', headers: [] }).res)).toBe('conflict')
  })

  it('ошибки загрузки и неподдерживаемого формата', () => {
    const w0 = createWorld(NOW)
    expect(errCode(req(w0, { cmd: 'add_subscription', url: 'https://fail.example.com/x', headers: [] }).res)).toBe('subscription_fetch_failed')
    expect(errCode(req(w0, { cmd: 'add_subscription', url: 'https://unsupported.example.com/x', headers: [] }).res)).toBe('subscription_unsupported')
  })

  it('ручной выбор профиля: неподдерживаемый отклоняется; удаление подписки сбрасывает выбор в «авто»', () => {
    const w0 = createWorld(NOW)
    const manual = (index: number) => ({ ...w0.settings, profile: { kind: 'manual' as const, subscription_id: 'sub-1', index } })
    expect(errCode(req(w0, { cmd: 'set_settings', settings: manual(3) }).res)).toBe('subscription_unsupported')
    expect(errCode(req(w0, { cmd: 'set_settings', settings: manual(42) }).res)).toBe('not_found')
    const w1 = req(w0, { cmd: 'set_settings', settings: manual(2) }).w
    expect(settle(req(w1, { cmd: 'enable' }).w).w.status.active_profile).toBe('США')
    const w2 = req(w1, { cmd: 'remove_subscription', id: 'sub-1' }).w
    expect(w2.settings.profile).toEqual({ kind: 'auto' })
  })

  it('обновление подписки', () => {
    const w0 = createWorld(NOW)
    const { w } = req(w0, { cmd: 'refresh_subscription', id: 'sub-1' })
    expect(w.subscriptions[0]!.updated_at).toBe(NOW)
    expect(errCode(req(w0, { cmd: 'refresh_subscription', id: 'zzz' }).res)).toBe('not_found')
  })

  it('журнал: возвращает последние строки и не содержит секретов', () => {
    let w = createWorld(NOW)
    w = req(w, { cmd: 'add_subscription', url: 'https://x.example.com/TOKEN9', headers: [] }).w
    w = settle(req(w, { cmd: 'enable' }).w).w
    const res = req(w, { cmd: 'get_log', max_lines: 3 }).res
    expect(res.kind === 'log' && res.lines.length).toBe(3)
    expect(JSON.stringify(w.log)).not.toContain('TOKEN9')
  })
})
