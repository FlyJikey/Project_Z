// Mock-помощник в памяти: задержки, события StatusChanged, подписка.
// Логика — в machine.ts (чистая, покрыта тестами); здесь только таймеры.

import type { Backend } from '../backend.ts'
import type { Event, Request, Response } from '../types.ts'
import {
  advance,
  createWorld,
  handleRequest,
  nextDelayMs,
  simulateFailure,
  simulatePendingRestore,
  simulateStartFailure,
  simulateRecovery,
  toggleConflict,
  type World,
} from './machine.ts'

const LATENCY_MS = 180

export function createMockBackend(opts: { speed?: number } = {}): Backend {
  const speed = opts.speed ?? 1
  const nowSec = () => Math.floor(Date.now() / 1000)
  let world: World = createWorld(nowSec())
  const listeners = new Set<(e: Event) => void>()
  let timer: ReturnType<typeof setTimeout> | null = null

  const emit = (e: Event) => listeners.forEach((l) => l(e))

  function schedule() {
    if (timer !== null) {
      clearTimeout(timer)
      timer = null
    }
    const d = nextDelayMs(world)
    if (d === null) return
    timer = setTimeout(() => {
      timer = null
      commit(advance(world))
    }, d / speed)
  }

  function commit(next: World) {
    const prev = world
    world = next
    if (next.status !== prev.status) emit({ event: 'status_changed', status: next.status })
    schedule()
  }

  return {
    kind: 'mock',

    request(req: Request): Promise<Response> {
      return new Promise((resolve) => {
        setTimeout(() => {
          const r = handleRequest(world, req, nowSec())
          commit(r.world)
          resolve(r.response)
        }, LATENCY_MS / speed)
      })
    },

    subscribe(cb) {
      listeners.add(cb)
      // Как у настоящего помощника: сообщаем про скорое окончание подписок (ТЗ §5.3).
      const t = setTimeout(() => {
        const now = nowSec()
        for (const s of world.subscriptions) {
          const exp = s.usage?.expire
          if (exp != null && exp > now && exp - now < 3 * 86400) {
            cb({ event: 'subscription_expiring', id: s.id, days_left: Math.floor((exp - now) / 86400) })
          }
        }
      }, 600 / speed)
      return () => {
        clearTimeout(t)
        listeners.delete(cb)
      }
    },

    demo: {
      fail: (reason) => commit(simulateFailure(world, reason)),
      recover: () => commit(simulateRecovery(world)),
      toggleConflict: (c) => commit(toggleConflict(world, c)),
      simulatePendingRestore: () => commit(simulatePendingRestore(world)),
      toggleStartFailure: () => commit(simulateStartFailure(world)),
    },
  }
}
