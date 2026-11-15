// Состояние UI: React context + reducer, без внешних библиотек.
// Помощник — единственный владелец состояния; UI только отображает ответы
// и события. Секреты (пароли прокси, адрес подписки) в состоянии не хранятся.

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useReducer,
  useRef,
  type ReactNode,
} from 'react'
import { getBackend, type Backend } from '../ipc/backend.ts'
import type {
  ErrorCode,
  Event,
  ProxyView,
  Request,
  Response,
  Settings,
  Status,
  SubscriptionView,
} from '../ipc/types.ts'
import { t, type ErrorKey } from '../strings/ru.ts'
import { daysLeft } from '../lib/format.ts'

export interface Slice<T> {
  data: T | null
  loading: boolean
  error: ErrorKey | null
}

export interface Notice {
  id: number
  tone: 'info' | 'warn'
  text: string
}

export interface AppState {
  ready: boolean
  backendKind: 'tauri' | 'mock' | null
  status: Slice<Status>
  settings: Slice<Settings>
  proxies: Slice<ProxyView[]>
  subscriptions: Slice<SubscriptionView[]>
  notices: Notice[]
}

type SliceName = 'status' | 'settings' | 'proxies' | 'subscriptions'

type Action =
  | { type: 'ready'; kind: 'tauri' | 'mock' }
  | { type: 'loading'; slice: SliceName }
  | { type: 'loaded'; slice: SliceName; data: unknown }
  | { type: 'failed'; slice: SliceName; error: ErrorKey }
  | { type: 'notice'; notice: Notice }
  | { type: 'dismiss'; id: number }

const emptySlice = <T,>(): Slice<T> => ({ data: null, loading: false, error: null })

export const initialState: AppState = {
  ready: false,
  backendKind: null,
  status: emptySlice(),
  settings: emptySlice(),
  proxies: emptySlice(),
  subscriptions: emptySlice(),
  notices: [],
}

export function reducer(state: AppState, a: Action): AppState {
  switch (a.type) {
    case 'ready':
      return { ...state, ready: true, backendKind: a.kind }
    case 'loading':
      return { ...state, [a.slice]: { ...state[a.slice], loading: true, error: null } }
    case 'loaded':
      return { ...state, [a.slice]: { data: a.data, loading: false, error: null } }
    case 'failed':
      return { ...state, [a.slice]: { ...state[a.slice], loading: false, error: a.error } }
    case 'notice':
      return { ...state, notices: [...state.notices, a.notice].slice(-4) }
    case 'dismiss':
      return { ...state, notices: state.notices.filter((n) => n.id !== a.id) }
  }
}

export type CallResult =
  | { ok: true; response: Response }
  | { ok: false; error: ErrorKey }

export interface Actions {
  call(req: Request): Promise<CallResult>
  reloadStatus(): Promise<void>
  reloadSettings(): Promise<void>
  reloadProxies(): Promise<void>
  reloadSubscriptions(): Promise<void>
  applyProxies(list: ProxyView[]): void
  dismissNotice(id: number): void
  notify(tone: Notice['tone'], text: string): void
}

interface Ctx {
  state: AppState
  actions: Actions
  backend: Backend | null
}

const AppContext = createContext<Ctx | null>(null)

export function useApp(): Ctx {
  const c = useContext(AppContext)
  if (!c) throw new Error('useApp вне AppProvider')
  return c
}

export function errorText(e: ErrorKey): string {
  return t.errors[e]
}

function describeBlock(reason: string): string {
  return reason in t.home.block ? t.home.block[reason as keyof typeof t.home.block] : reason
}

export function AppProvider({ children }: { children: ReactNode }) {
  const [state, dispatch] = useReducer(reducer, initialState)
  const backendRef = useRef<Backend | null>(null)
  const stateRef = useRef(state)
  stateRef.current = state
  const noticeId = useRef(0)

  const notify = useCallback((tone: Notice['tone'], text: string) => {
    noticeId.current += 1
    dispatch({ type: 'notice', notice: { id: noticeId.current, tone, text } })
  }, [])

  const call = useCallback(async (req: Request): Promise<CallResult> => {
    const b = backendRef.current
    if (!b) return { ok: false, error: 'unavailable' }
    try {
      const response = await b.request(req)
      if (response.kind === 'error') return { ok: false, error: response.error.code satisfies ErrorCode }
      return { ok: true, response }
    } catch {
      // Тексты исключений не показываем и не логируем: в них могут быть данные запроса.
      return { ok: false, error: 'unavailable' }
    }
  }, [])

  const load = useCallback(
    async <K extends Response['kind']>(
      slice: SliceName,
      req: Request,
      kind: K,
      pick: (r: Extract<Response, { kind: K }>) => unknown,
    ) => {
      dispatch({ type: 'loading', slice })
      const r = await call(req)
      if (!r.ok) return dispatch({ type: 'failed', slice, error: r.error })
      if (r.response.kind !== kind) return dispatch({ type: 'failed', slice, error: 'internal' })
      dispatch({ type: 'loaded', slice, data: pick(r.response as Extract<Response, { kind: K }>) })
    },
    [call],
  )

  const actions = useMemo<Actions>(
    () => ({
      call,
      reloadStatus: () => load('status', { cmd: 'get_status' }, 'status', (r) => r.status),
      reloadSettings: () => load('settings', { cmd: 'get_settings' }, 'settings', (r) => r.settings),
      reloadProxies: () => load('proxies', { cmd: 'list_proxies' }, 'proxies', (r) => r.proxies),
      reloadSubscriptions: () =>
        load('subscriptions', { cmd: 'list_subscriptions' }, 'subscriptions', (r) => r.subscriptions),
      applyProxies: (list) => dispatch({ type: 'loaded', slice: 'proxies', data: list }),
      dismissNotice: (id) => dispatch({ type: 'dismiss', id }),
      notify,
    }),
    [call, load, notify],
  )

  // Обработка событий помощника.
  const onEvent = useCallback(
    (e: Event) => {
      switch (e.event) {
        case 'status_changed': {
          const prev = stateRef.current.status.data?.chain.state
          const next = e.status.chain
          if (prev === 'starting' && next.state === 'rolling_back') {
            notify('warn', t.events.startAborted)
          } else if (prev === 'protected' && next.state === 'blocked') {
            notify('warn', t.events.toBlocked(describeBlock(next.reason)))
          } else if (prev === 'blocked' && next.state === 'protected') {
            notify('info', t.events.recovered)
          }
          dispatch({ type: 'loaded', slice: 'status', data: e.status })
          break
        }
        case 'proxy_switched':
          notify('info', e.exit ? t.events.proxySwitched(e.exit.ip) : t.events.proxySwitchedNoIp)
          void actions.reloadProxies()
          break
        case 'subscription_expiring': {
          const sub = stateRef.current.subscriptions.data?.find((s) => s.id === e.id)
          notify('warn', t.events.expiring(sub?.title ?? sub?.host ?? e.id, e.days_left))
          break
        }
        case 'setting_kept_as_is':
          notify('info', t.events.keptAsIs(t.home.changedKind[e.kind]))
          break
      }
    },
    [actions, notify],
  )

  useEffect(() => {
    let cancelled = false
    let unsubscribe: (() => void) | null = null
    void getBackend().then((b) => {
      if (cancelled) return
      backendRef.current = b
      dispatch({ type: 'ready', kind: b.kind })
      unsubscribe = b.subscribe(onEvent)
      void actions.reloadStatus()
      void actions.reloadSettings()
      void actions.reloadProxies()
      void actions.reloadSubscriptions()
    })
    return () => {
      cancelled = true
      unsubscribe?.()
    }
    // Подписка создаётся один раз на жизнь приложения.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const value = useMemo<Ctx>(
    () => ({ state, actions, backend: backendRef.current }),
    [state, actions],
  )
  return <AppContext.Provider value={value}>{children}</AppContext.Provider>
}

/** Сколько дней осталось у подписки (null — бессрочно или неизвестно). */
export function subscriptionDaysLeft(s: SubscriptionView, nowSec: number): number | null {
  const exp = s.usage?.expire
  return exp == null ? null : daysLeft(exp, nowSec)
}
