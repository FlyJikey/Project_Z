// Адаптер к Tauri. Команда и событие появятся в crates/app (этап 1, позже);
// имена зафиксированы константами.

import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { Backend } from './backend.ts'
import type { Event, Request, Response } from './types.ts'

export const IPC_COMMAND = 'ipc_request'
export const IPC_EVENT = 'ipc_event'

export const tauriBackend: Backend = {
  kind: 'tauri',

  request(request: Request): Promise<Response> {
    return invoke<Response>(IPC_COMMAND, { request })
  },

  subscribe(cb: (e: Event) => void): () => void {
    let disposed = false
    let unlisten: (() => void) | null = null
    void listen<Event>(IPC_EVENT, (e) => cb(e.payload)).then((fn) => {
      if (disposed) fn()
      else unlisten = fn
    })
    // Просим помощника слать события в этот канал.
    void invoke<Response>(IPC_COMMAND, { request: { cmd: 'subscribe' } satisfies Request }).catch(() => {})
    return () => {
      disposed = true
      unlisten?.()
    }
  },
}
