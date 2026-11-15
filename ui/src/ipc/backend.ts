// Тонкая граница между UI и помощником. Реализация выбирается один раз:
// Tauri, если доступен window.__TAURI_INTERNALS__, иначе mock в памяти.

import type { BlockReason, Conflict, Event, Request, Response } from './types.ts'

export interface Backend {
  readonly kind: 'tauri' | 'mock'
  request(req: Request): Promise<Response>
  /** Подписка на события помощника; возвращает функцию отписки. */
  subscribe(cb: (e: Event) => void): () => void
  /** Только у mock: управление имитацией для проверки интерфейса. */
  readonly demo?: DemoControls
}

export interface DemoControls {
  fail(reason: BlockReason): void
  recover(): void
  toggleConflict(c: Conflict): void
  simulatePendingRestore(): void
  /** Следующий запуск завершится ошибкой (Starting → RollingBack → Off с last_error). */
  toggleStartFailure(): void
}

export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

let cached: Promise<Backend> | null = null

/** Ленивая загрузка: код Tauri и mock не попадают друг к другу в начальный бандл. */
export function getBackend(): Promise<Backend> {
  cached ??= isTauri()
    ? import('./tauriBackend.ts').then((m) => m.tauriBackend)
    : import('./mock/mockBackend.ts').then((m) => m.createMockBackend())
  return cached
}
