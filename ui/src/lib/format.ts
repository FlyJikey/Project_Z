// Чистые функции форматирования (без строк интерфейса, кроме единиц через ru.ts).
import { t } from '../strings/ru.ts'

export function formatBytes(n: number): string {
  const units = t.units.bytes
  let v = n
  let i = 0
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024
    i++
  }
  const digits = v >= 100 || i === 0 ? 0 : v >= 10 ? 1 : 2
  return `${v.toFixed(digits)} ${units[i] ?? ''}`
}

export function formatDate(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleDateString('ru-RU', {
    day: 'numeric',
    month: 'long',
    year: 'numeric',
  })
}

export function formatDateTime(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleString('ru-RU', {
    day: 'numeric',
    month: 'short',
    hour: '2-digit',
    minute: '2-digit',
  })
}

/** Сколько полных дней осталось до unix-секунды (может быть < 0). */
export function daysLeft(expire: number, nowSeconds: number): number {
  return Math.floor((expire - nowSeconds) / 86400)
}

/** Эмодзи-флаг по коду страны ISO 3166-1 alpha-2. */
export function flagEmoji(code: string | null): string {
  if (!code || !/^[a-z]{2}$/i.test(code)) return ''
  return String.fromCodePoint(
    ...Array.from(code.toUpperCase()).map((c) => 0x1f1e6 + c.charCodeAt(0) - 65),
  )
}

export function countryName(code: string | null): string | null {
  if (!code) return null
  try {
    return new Intl.DisplayNames(['ru'], { type: 'region' }).of(code.toUpperCase()) ?? null
  } catch {
    return null
  }
}

/** Показывать ссылку только для https. */
export function safeHttpsUrl(u: string | null): string | null {
  if (!u) return null
  try {
    const url = new URL(u)
    return url.protocol === 'https:' ? url.toString() : null
  } catch {
    return null
  }
}

/** Проверка адреса подписки: только https, с хостом, без логина в адресе. */
export type SubscriptionUrlError = 'empty' | 'invalid' | 'not_https'
export function validateSubscriptionUrl(
  raw: string,
): { ok: true; host: string } | { ok: false; error: SubscriptionUrlError } {
  const s = raw.trim()
  if (s === '') return { ok: false, error: 'empty' }
  let url: URL
  try {
    url = new URL(s)
  } catch {
    return { ok: false, error: 'invalid' }
  }
  if (url.protocol !== 'https:') {
    return { ok: false, error: /^[a-z][a-z0-9+.-]*:/i.test(s) ? 'not_https' : 'invalid' }
  }
  if (url.hostname === '' || url.username !== '' || url.password !== '') {
    return { ok: false, error: 'invalid' }
  }
  return { ok: true, host: url.hostname }
}
