// Разбор и проверка строки прокси на клиенте (ТЗ §6).
// Формат: scheme://[user[:pass]@]host:port, scheme = socks5 | http.
// Логин и пароль допускают percent-encoding (спецсимволы: @ : / % и т.д.).
// IPv6 — только в квадратных скобках: socks5://[2001:db8::1]:1080.
// Хост в результате хранится БЕЗ скобок и в нижнем регистре.

import type { ProxyEndpoint, ProxyScheme } from '../ipc/types.ts'

export type ProxyParseError =
  | 'empty'
  | 'whitespace'
  | 'scheme'
  | 'path'
  | 'userinfo'
  | 'encoding'
  | 'host'
  | 'ipv6_brackets'
  | 'ipv6_invalid'
  | 'port'

export type ProxyParseResult =
  | { ok: true; endpoint: ProxyEndpoint }
  | { ok: false; error: ProxyParseError }

const fail = (error: ProxyParseError): ProxyParseResult => ({ ok: false, error })

export function isValidIPv4(s: string): boolean {
  const parts = s.split('.')
  if (parts.length !== 4) return false
  return parts.every((p) => /^\d{1,3}$/.test(p) && Number(p) <= 255 && (p === '0' || !p.startsWith('0')))
}

export function isValidIPv6(s: string): boolean {
  if (!/^[0-9a-f:.]+$/i.test(s) || !s.includes(':')) return false
  const dbl = s.split('::')
  if (dbl.length > 2) return false
  let groups: string[] = []
  let tail4 = false
  const parseSide = (side: string): string[] | null => (side === '' ? [] : side.split(':'))
  const left = parseSide(dbl[0] ?? '')
  const right = dbl.length === 2 ? parseSide(dbl[1] ?? '') : []
  if (!left || !right) return false
  groups = [...left, ...right]
  const last = groups[groups.length - 1]
  if (last !== undefined && last.includes('.')) {
    if (!isValidIPv4(last)) return false
    tail4 = true
    groups = groups.slice(0, -1)
  }
  if (!groups.every((g) => /^[0-9a-f]{1,4}$/i.test(g))) return false
  const count = groups.length + (tail4 ? 2 : 0)
  return dbl.length === 2 ? count <= 7 : count === 8
}

export function isValidHostname(s: string): boolean {
  if (s.length === 0 || s.length > 253) return false
  const labels = s.split('.')
  return labels.every(
    (l) => l.length >= 1 && l.length <= 63 && /^[a-z0-9]([a-z0-9-]*[a-z0-9])?$/i.test(l),
  )
}

function decode(s: string): string | null {
  try {
    return decodeURIComponent(s)
  } catch {
    return null
  }
}

export function parseProxyLine(input: string): ProxyParseResult {
  const line = input.trim()
  if (line === '') return fail('empty')
  // Управляющие символы и пробелы внутри строки не допускаются.
  if (/[\s\u0000-\u001f\u007f]/.test(line)) return fail('whitespace')

  const m = /^([a-z0-9]+):\/\/(.*)$/i.exec(line)
  if (!m) return fail('scheme')
  const scheme = (m[1] ?? '').toLowerCase()
  if (scheme !== 'socks5' && scheme !== 'http') return fail('scheme')
  const rest = m[2] ?? ''

  // Авторитет — до первого / ? #. Всё после (кроме одиночного «/») — ошибка.
  const cut = rest.search(/[/?#]/)
  const authority = cut === -1 ? rest : rest.slice(0, cut)
  const tail = cut === -1 ? '' : rest.slice(cut)
  if (tail !== '' && tail !== '/') return fail('path')

  let username: string | null = null
  let password: string | null = null
  let hostport = authority
  const at = authority.lastIndexOf('@')
  if (at !== -1) {
    const userinfo = authority.slice(0, at)
    hostport = authority.slice(at + 1)
    const colon = userinfo.indexOf(':')
    const rawUser = colon === -1 ? userinfo : userinfo.slice(0, colon)
    const rawPass = colon === -1 ? '' : userinfo.slice(colon + 1)
    if (rawUser === '') return fail('userinfo')
    const u = decode(rawUser)
    const p = decode(rawPass)
    if (u === null || p === null) return fail('encoding')
    username = u
    password = p === '' ? null : p
  }

  let host: string
  let portStr: string
  if (hostport.startsWith('[')) {
    const close = hostport.indexOf(']')
    if (close === -1) return fail('ipv6_invalid')
    host = hostport.slice(1, close)
    const after = hostport.slice(close + 1)
    if (!after.startsWith(':')) return fail('port')
    portStr = after.slice(1)
    if (!isValidIPv6(host)) return fail('ipv6_invalid')
  } else {
    const colon = hostport.lastIndexOf(':')
    if (colon === -1) return fail('port')
    host = hostport.slice(0, colon)
    portStr = hostport.slice(colon + 1)
    if (host.includes(':')) return fail('ipv6_brackets')
    if (host === '') return fail('host')
    if (/^[0-9.]+$/.test(host)) {
      if (!isValidIPv4(host)) return fail('host')
    } else if (!isValidHostname(host)) {
      return fail('host')
    }
  }

  if (!/^\d{1,5}$/.test(portStr)) return fail('port')
  const port = Number(portStr)
  if (port < 1 || port > 65535) return fail('port')

  return {
    ok: true,
    endpoint: {
      scheme: scheme as ProxyScheme,
      host: host.toLowerCase(),
      port,
      username,
      password,
    },
  }
}

export interface ParsedLine {
  /** Номер строки, с 1. */
  line: number
  result: ProxyParseResult
}

/** Разбор списка: пустые строки и строки-комментарии (#) пропускаются. */
export function parseProxyList(text: string): ParsedLine[] {
  const out: ParsedLine[] = []
  text.split(/\r?\n/).forEach((raw, i) => {
    const s = raw.trim()
    if (s === '' || s.startsWith('#')) return
    out.push({ line: i + 1, result: parseProxyLine(s) })
  })
  return out
}

/** host:port для показа и сравнения (IPv6 — в скобках). */
export function hostPort(host: string, port: number): string {
  return `${host.includes(':') ? `[${host}]` : host}:${port}`
}

/** Ключ для поиска дублей. */
export function endpointKey(scheme: ProxyScheme, host: string, port: number): string {
  return `${scheme}://${hostPort(host.toLowerCase(), port)}`
}

/** Скрытый логин для предпросмотра до отправки: первые 2 символа + ***. */
export function maskUsername(u: string | null): string | null {
  if (u === null) return null
  return `${Array.from(u).slice(0, 2).join('')}***`
}

/** Перестановка элемента списка: возвращает новый массив. */
export function reorder<T>(items: readonly T[], from: number, to: number): T[] {
  if (from === to || from < 0 || from >= items.length) return [...items]
  const clamped = Math.max(0, Math.min(items.length - 1, to))
  const next = [...items]
  const [moved] = next.splice(from, 1)
  if (moved === undefined) return [...items]
  next.splice(clamped, 0, moved)
  return next
}
