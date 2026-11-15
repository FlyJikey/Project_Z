// Единственная точка открытия внешних ссылок. Позже заменить реализацию
// на плагин opener в Tauri. Разрешены только https-адреса.
import { safeHttpsUrl } from './format.ts'

export function openExternal(url: string): boolean {
  const safe = safeHttpsUrl(url)
  if (!safe) return false
  window.open(safe, '_blank', 'noopener,noreferrer')
  return true
}
