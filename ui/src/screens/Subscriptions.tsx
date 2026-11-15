import { useId, useState, type FormEvent } from 'react'
import { errorText, subscriptionDaysLeft, useApp } from '../state/app.tsx'
import { Alert, Badge, Button, Card, ConfirmDialog, EmptyState, ErrorState, PageTitle, Spinner } from '../components/ui.tsx'
import { openExternal } from '../lib/openExternal.ts'
import { formatBytes, formatDate, formatDateTime, safeHttpsUrl, validateSubscriptionUrl } from '../lib/format.ts'
import type { Settings, SubscriptionView } from '../ipc/types.ts'
import { t, type ErrorKey } from '../strings/ru.ts'

function AddForm() {
  const { actions } = useApp()
  const [url, setUrl] = useState('')
  const [show, setShow] = useState(false)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [info, setInfo] = useState<string | null>(null)
  const inputId = useId()
  const hintId = useId()
  const errId = useId()

  async function submit(e: FormEvent) {
    e.preventDefault()
    setInfo(null)
    const v = validateSubscriptionUrl(url)
    if (!v.ok) return setError(t.subs.urlErrors[v.error])
    setError(null)
    setBusy(true)
    const r = await actions.call({ cmd: 'add_subscription', url: url.trim(), headers: [] })
    setBusy(false)
    if (!r.ok) return setError(errorText(r.error))
    // Адрес с токеном больше нигде не хранится: поле очищается.
    setUrl('')
    setShow(false)
    setInfo(t.subs.added(v.host))
    await actions.reloadSubscriptions()
  }

  return (
    <Card title={t.subs.addTitle}>
      <form onSubmit={(e) => void submit(e)} noValidate className="flex flex-col gap-3">
        <div>
          <label htmlFor={inputId} className="mb-1 block text-sm font-medium">
            {t.subs.urlLabel}
          </label>
          <input
            id={inputId}
            type={show ? 'text' : 'password'}
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder={t.subs.urlPlaceholder}
            autoComplete="off"
            autoCapitalize="off"
            spellCheck={false}
            aria-describedby={`${hintId} ${error ? errId : ''}`}
            aria-invalid={error ? true : undefined}
            className="w-full rounded-lg border border-line bg-bg px-3 py-2 font-mono text-sm"
          />
          <p id={hintId} className="mt-1 text-xs text-muted">
            {t.subs.urlHint}
          </p>
          <label className="mt-2 inline-flex items-center gap-2 text-sm">
            <input type="checkbox" checked={show} onChange={(e) => setShow(e.target.checked)} />
            {t.subs.showUrl}
          </label>
        </div>
        {error && (
          <p id={errId} role="alert" className="text-sm font-medium text-bad">
            {error}
          </p>
        )}
        {info && (
          <p role="status" className="text-sm font-medium text-ok">
            {info}
          </p>
        )}
        <div>
          <Button type="submit" variant="primary" disabled={busy}>
            {busy ? t.subs.adding : t.subs.addButton}
          </Button>
        </div>
      </form>
    </Card>
  )
}

function Usage({ s }: { s: SubscriptionView }) {
  const u = s.usage
  if (!u) return null
  const used = (u.upload ?? 0) + (u.download ?? 0)
  const pct = u.total ? Math.min(100, Math.round((used / u.total) * 100)) : null
  return (
    <div className="mt-3 grid gap-3 text-sm sm:grid-cols-2">
      <div>
        <p className="text-muted">{t.subs.traffic}</p>
        <p className="font-medium">
          {u.total ? t.subs.trafficOf(formatBytes(used), formatBytes(u.total)) : t.subs.trafficUsed(formatBytes(used))}
        </p>
        {pct !== null && (
          <div
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={pct}
            aria-label={t.subs.traffic}
            className="mt-1.5 h-1.5 overflow-hidden rounded-full bg-line"
          >
            <div className={`h-full ${pct >= 90 ? 'bg-bad' : 'bg-accent'}`} style={{ width: `${pct}%` }} />
          </div>
        )}
      </div>
      <div>
        <p className="font-medium">
          {u.expire == null ? t.subs.expireNever : t.subs.expire(formatDate(u.expire))}
        </p>
      </div>
    </div>
  )
}

function ProfileList({ sub, settings }: { sub: SubscriptionView; settings: Settings | null }) {
  const { actions } = useApp()
  const [error, setError] = useState<ErrorKey | null>(null)
  const group = useId()

  async function choose(index: number) {
    if (!settings) return
    setError(null)
    const r = await actions.call({
      cmd: 'set_settings',
      settings: { ...settings, profile: { kind: 'manual', subscription_id: sub.id, index } },
    })
    if (!r.ok) setError(r.error)
    await actions.reloadSettings()
  }

  if (sub.profiles.length === 0) return <p className="mt-3 text-sm text-muted">{t.subs.noProfiles}</p>
  return (
    <fieldset className="mt-4">
      <legend className="mb-1 text-sm font-medium">{t.subs.profilesLegend}</legend>
      <ul className="divide-y divide-line rounded-xl border border-line">
        {sub.profiles.map((p) => {
          const supported = p.support.status === 'supported'
          const checked =
            settings?.profile.kind === 'manual' &&
            settings.profile.subscription_id === sub.id &&
            settings.profile.index === p.index
          const rid = `${group}-${p.index}`
          return (
            <li key={p.index} className="flex items-start gap-3 px-3 py-2.5 text-sm">
              <input
                id={rid}
                type="radio"
                name={`profile-${group}`}
                className="mt-1"
                disabled={!supported || !settings}
                checked={checked}
                onChange={() => void choose(p.index)}
              />
              <label htmlFor={rid} className={`min-w-0 flex-1 ${supported ? '' : 'text-muted'}`}>
                <span className="font-medium">{p.name}</span>
                <span className="ml-2 text-muted">{t.subs.profileServers(p.server_count)}</span>
                {p.support.status === 'unsupported' && (
                  <span className="mt-0.5 block text-xs">{t.subs.unsupportedReason(p.support.reason)}</span>
                )}
              </label>
              {p.support.status === 'unsupported' && <Badge tone="warn">{t.subs.unsupported}</Badge>}
            </li>
          )
        })}
      </ul>
      {error && (
        <p role="alert" className="mt-2 text-sm font-medium text-bad">
          {errorText(error)}
        </p>
      )}
    </fieldset>
  )
}

function SubscriptionCard({
  sub,
  settings,
  nowSec,
  onRemove,
}: {
  sub: SubscriptionView
  settings: Settings | null
  nowSec: number
  onRemove: (s: SubscriptionView) => void
}) {
  const { actions } = useApp()
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<ErrorKey | null>(null)
  const left = subscriptionDaysLeft(sub, nowSec)
  const support = safeHttpsUrl(sub.support_url)

  async function refresh() {
    setBusy(true)
    setError(null)
    const r = await actions.call({ cmd: 'refresh_subscription', id: sub.id })
    if (!r.ok) setError(r.error)
    await actions.reloadSubscriptions()
    setBusy(false)
  }

  return (
    <Card
      title={sub.title}
      actions={
        <div className="flex gap-2">
          <Button disabled={busy} onClick={() => void refresh()}>
            {busy ? t.subs.refreshing : t.subs.refresh}
          </Button>
          <Button variant="ghost" onClick={() => onRemove(sub)}>
            {t.subs.remove}
          </Button>
        </div>
      }
    >
      <p className="text-sm text-muted">
        {t.subs.host}: <span className="font-mono">{sub.host}</span>
        {' · '}
        {sub.updated_at != null ? t.subs.updatedAt(formatDateTime(sub.updated_at)) : t.subs.neverUpdated}
        {sub.update_interval_hours != null && <> · {t.subs.interval(sub.update_interval_hours)}</>}
        {support && (
          <>
            {' · '}
            <a
              href={support}
              onClick={(e) => {
                e.preventDefault()
                openExternal(support)
              }}
              className="text-accent underline"
            >
              {t.subs.support}
            </a>
          </>
        )}
      </p>
      {left !== null && left <= 3 && (
        <div className="mt-3">
          <Alert tone={left < 0 ? 'bad' : 'warn'} role="status">
            {left < 0 ? t.subs.expired : t.subs.expireWarning(left)}
          </Alert>
        </div>
      )}
      <Usage s={sub} />
      {sub.last_error && (
        <p role="alert" className="mt-3 text-sm text-bad">
          {t.subs.lastError}: {sub.last_error}
        </p>
      )}
      {error && (
        <p role="alert" className="mt-3 text-sm font-medium text-bad">
          {errorText(error)}
        </p>
      )}
      <ProfileList sub={sub} settings={settings} />
    </Card>
  )
}

export function Subscriptions() {
  const { state, actions } = useApp()
  const { data, loading, error } = state.subscriptions
  const settings = state.settings.data
  const [toRemove, setToRemove] = useState<SubscriptionView | null>(null)
  const [removeError, setRemoveError] = useState<ErrorKey | null>(null)
  const [busy, setBusy] = useState(false)
  const [nowSec] = useState(() => Math.floor(Date.now() / 1000))
  const auto = settings?.profile.kind === 'auto'
  const autoId = useId()

  async function setAuto() {
    if (!settings) return
    await actions.call({ cmd: 'set_settings', settings: { ...settings, profile: { kind: 'auto' } } })
    await actions.reloadSettings()
  }

  async function confirmRemove() {
    if (!toRemove) return
    setBusy(true)
    const r = await actions.call({ cmd: 'remove_subscription', id: toRemove.id })
    setRemoveError(r.ok ? null : r.error)
    setToRemove(null)
    await Promise.all([actions.reloadSubscriptions(), actions.reloadSettings()])
    setBusy(false)
  }

  return (
    <div className="flex flex-col gap-4">
      <PageTitle>{t.subs.title}</PageTitle>
      <AddForm />

      <h2 className="mt-2 text-lg font-semibold">{t.subs.listTitle}</h2>
      {removeError && (
        <p role="alert" className="text-sm font-medium text-bad">
          {errorText(removeError)}
        </p>
      )}

      {!data && error && (
        <ErrorState title={t.subs.loadFailed} detail={errorText(error)} onRetry={() => void actions.reloadSubscriptions()} />
      )}
      {!data && !error && loading && <Spinner />}
      {data && data.length === 0 && <EmptyState>{t.subs.empty}</EmptyState>}
      {data && data.length > 0 && (
        <>
          <div className="flex items-start gap-3 rounded-xl border border-line bg-surface px-4 py-3 text-sm">
            <input
              id={autoId}
              type="radio"
              name="profile-auto"
              className="mt-1"
              checked={auto}
              disabled={!settings}
              onChange={() => void setAuto()}
            />
            <label htmlFor={autoId}>
              <span className="font-medium">{t.subs.profileAuto}</span>
              <span className="mt-0.5 block text-muted">{t.subs.profileAutoHint}</span>
            </label>
          </div>
          {data.map((s) => (
            <SubscriptionCard key={s.id} sub={s} settings={settings} nowSec={nowSec} onRemove={setToRemove} />
          ))}
        </>
      )}

      <ConfirmDialog
        open={toRemove !== null}
        title={t.subs.removeConfirmTitle}
        body={toRemove ? t.subs.removeConfirmBody(toRemove.host) : ''}
        confirmLabel={t.subs.removeConfirm}
        busy={busy}
        onCancel={() => setToRemove(null)}
        onConfirm={() => void confirmRemove()}
      />
    </div>
  )
}
