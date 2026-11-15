import { useEffect, useState } from 'react'
import { AppProvider, useApp } from './state/app.tsx'
import { Button, Spinner } from './components/ui.tsx'
import { Home } from './screens/Home.tsx'
import { Subscriptions } from './screens/Subscriptions.tsx'
import { Proxies } from './screens/Proxies.tsx'
import { Checks } from './screens/Checks.tsx'
import { SettingsScreen } from './screens/Settings.tsx'
import { t } from './strings/ru.ts'

type Tab = 'home' | 'subscriptions' | 'proxies' | 'checks' | 'settings'
const TABS: readonly Tab[] = ['home', 'subscriptions', 'proxies', 'checks', 'settings']

function NoticeItem({ id, tone, text }: { id: number; tone: 'info' | 'warn'; text: string }) {
  const { actions } = useApp()
  const { dismissNotice } = actions
  useEffect(() => {
    const timer = setTimeout(() => dismissNotice(id), tone === 'warn' ? 15000 : 8000)
    return () => clearTimeout(timer)
  }, [id, tone, dismissNotice])
  return (
    <div
      className={`pointer-events-auto flex items-start gap-3 rounded-xl border p-3 text-sm shadow-lg ${
        tone === 'warn' ? 'border-warn/40 bg-warn-soft' : 'border-accent/40 bg-surface'
      }`}
    >
      <p className="min-w-0 flex-1">{text}</p>
      <Button variant="ghost" className="!px-2 !py-1" aria-label={t.common.dismiss} onClick={() => dismissNotice(id)}>
        ✕
      </Button>
    </div>
  )
}

function Notices() {
  const { state } = useApp()
  return (
    <div
      role="status"
      aria-live="polite"
      aria-label={t.common.notice}
      className="pointer-events-none fixed right-4 bottom-4 z-50 flex w-80 flex-col gap-2"
    >
      {state.notices.map((n) => (
        <NoticeItem key={n.id} {...n} />
      ))}
    </div>
  )
}

function Shell() {
  const { state } = useApp()
  const [tab, setTab] = useState<Tab>('home')

  if (!state.ready) {
    return (
      <div className="grid h-full place-items-center">
        <Spinner label={t.app.startingUp} />
      </div>
    )
  }

  return (
    <div className="flex h-full">
      <nav aria-label={t.nav.label} className="flex w-52 shrink-0 flex-col gap-1 border-r border-line bg-surface p-3">
        <div className="px-3 pt-2 pb-4 text-lg font-semibold tracking-tight">{t.app.name}</div>
        {TABS.map((id) => (
          <button
            key={id}
            type="button"
            aria-current={tab === id ? 'page' : undefined}
            onClick={() => setTab(id)}
            className={`rounded-lg px-3 py-2 text-left text-sm font-medium transition ${
              tab === id ? 'bg-accent-soft text-accent' : 'text-fg hover:bg-surface-2'
            }`}
          >
            {t.nav[id]}
          </button>
        ))}
        {state.backendKind === 'mock' && (
          <p className="mt-auto rounded-lg bg-warn-soft px-3 py-2 text-xs text-warn">{t.app.demoBadge}</p>
        )}
      </nav>
      <main className="min-w-0 flex-1 overflow-y-auto p-6">
        <div className="mx-auto max-w-3xl">
          {tab === 'home' && <Home />}
          {tab === 'subscriptions' && <Subscriptions />}
          {tab === 'proxies' && <Proxies />}
          {tab === 'checks' && <Checks />}
          {tab === 'settings' && <SettingsScreen />}
        </div>
      </main>
      <Notices />
    </div>
  )
}

export default function App() {
  return (
    <AppProvider>
      <Shell />
    </AppProvider>
  )
}
