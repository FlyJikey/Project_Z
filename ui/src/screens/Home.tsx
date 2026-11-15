import { useEffect, useState } from 'react'
import { useApp, errorText } from '../state/app.tsx'
import { Alert, Badge, Button, Card, ConfirmDialog, ErrorState, PageTitle, Spinner } from '../components/ui.tsx'
import { chainTones, STEP_ORDER, type LinkTone } from '../lib/chain.ts'
import { countryName, flagEmoji } from '../lib/format.ts'
import { hostPort } from '../lib/proxyParse.ts'
import type { ChainState, Status } from '../ipc/types.ts'
import type { ErrorKey } from '../strings/ru.ts'
import { t } from '../strings/ru.ts'

const toneBox: Record<LinkTone, string> = {
  ok: 'border-ok bg-ok-soft text-ok',
  pending: 'border-warn bg-warn-soft text-warn animate-pulse',
  bad: 'border-bad bg-bad-soft text-bad',
  idle: 'border-line bg-surface-2 text-muted',
  blocked: 'border-bad bg-bad-soft text-bad',
}
const toneLine: Record<LinkTone, string> = {
  ok: 'bg-ok',
  pending: 'bg-warn',
  bad: 'bg-bad',
  idle: 'bg-line',
  blocked: 'bg-bad',
}
const toneIcon: Record<LinkTone, string> = { ok: '✓', pending: '…', bad: '✕', idle: '–', blocked: '⛔' }

function ChainView({ status }: { status: Status }) {
  const tones = chainTones(status)
  const proxyCode = status.exit?.country_code ?? null
  const nodes: { key: keyof typeof tones; label: string; extra: string | null }[] = [
    { key: 'you', label: t.home.nodeYou, extra: null },
    { key: 'vpn', label: t.home.nodeVpn, extra: status.active_profile },
    {
      key: 'proxy',
      label: t.home.nodeProxy,
      extra: proxyCode ? t.home.nodeCountry(flagEmoji(proxyCode), proxyCode.toUpperCase()) : null,
    },
    { key: 'internet', label: t.home.nodeInternet, extra: null },
  ]
  const lineCls = (tone: LinkTone) => `absolute top-[22px] h-1 ${toneLine[tone]} ${tone === 'blocked' ? 'opacity-40' : ''}`
  return (
    <ol aria-label={t.home.chainLabel} className="grid grid-cols-4">
      {nodes.map((n, i) => {
        const tone = tones[n.key]
        const next = i < nodes.length - 1 ? tones[nodes[i + 1]!.key] : null
        return (
          <li key={n.key} className="relative flex flex-col items-center gap-1.5 text-center">
            {i > 0 && <span aria-hidden="true" className={`${lineCls(tone)} left-0 w-1/2`} />}
            {next && <span aria-hidden="true" className={`${lineCls(next)} left-1/2 w-1/2`} />}
            <span
              aria-hidden="true"
              className={`relative grid size-12 place-items-center rounded-full border-2 bg-surface text-lg font-semibold ${toneBox[tone]}`}
            >
              {toneIcon[tone]}
            </span>
            <span className="text-sm font-medium">{n.label}</span>
            {n.extra && <span className="max-w-full truncate px-1 text-xs" title={n.extra}>{n.extra}</span>}
            <span className={`text-xs ${tone === 'idle' ? 'text-muted' : toneBox[tone].split(' ').find((c) => c.startsWith('text-'))}`}>
              {t.home.tone[tone]}
            </span>
          </li>
        )
      })}
    </ol>
  )
}

function useCountdown(untilSec: number | null): string | null {
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    if (untilSec === null) return
    const id = setInterval(() => setNow(Date.now()), 1000)
    return () => clearInterval(id)
  }, [untilSec])
  if (untilSec === null) return null
  const left = Math.max(0, Math.ceil(untilSec - now / 1000))
  return `${Math.floor(left / 60)}:${String(left % 60).padStart(2, '0')}`
}

function stateTitle(c: ChainState): string {
  return t.home.state[c.state]
}

function stateDetail(c: ChainState): string | null {
  switch (c.state) {
    case 'starting': {
      const i = STEP_ORDER.indexOf(c.step)
      return `${t.home.step[c.step]} · ${t.home.stepOf(i + 1, STEP_ORDER.length)}`
    }
    case 'blocked':
      return t.home.block[c.reason]
    default:
      return null
  }
}

function BigSwitch({ status }: { status: Status }) {
  const { actions } = useApp()
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<ErrorKey | null>(null)
  const chain = status.chain
  const on = chain.state !== 'off' && chain.state !== 'stopping' && chain.state !== 'rolling_back'
  const transitional = chain.state === 'stopping' || chain.state === 'rolling_back'
  const disabled = busy || transitional

  async function toggle() {
    setBusy(true)
    setError(null)
    const r = await actions.call({ cmd: on ? 'disable' : 'enable' })
    if (!r.ok) setError(r.error)
    await actions.reloadStatus()
    setBusy(false)
  }

  const detail = stateDetail(chain)
  const countdown = useCountdown(chain.state === 'wifi_login' ? chain.until : null)
  return (
    <div className="flex items-center gap-6">
      <button
        type="button"
        role="switch"
        aria-checked={on}
        aria-label={t.home.switchLabel}
        disabled={disabled}
        onClick={() => void toggle()}
        className={`relative h-20 w-36 shrink-0 rounded-full transition disabled:opacity-60 ${on ? 'bg-ok' : 'bg-line'}`}
      >
        <span
          aria-hidden="true"
          className={`absolute top-2 left-2 size-16 rounded-full bg-white shadow-md transition-transform ${on ? 'translate-x-16' : ''}`}
        />
      </button>
      <div className="min-w-0">
        <h2 className="text-xl font-semibold">{stateTitle(chain)}</h2>
        {detail && <p className="mt-0.5 text-sm font-medium">{detail}</p>}
        <p className="mt-1 text-sm text-muted">{t.home.stateHint[chain.state]}</p>
        {countdown !== null && <p className="mt-0.5 text-sm font-medium tabular-nums">{t.home.wifiLeft(countdown)}</p>}
        {chain.state === 'wifi_login' && chain.real_ip_exposed && (
          <p className="mt-1 text-sm font-medium text-bad">{t.home.wifiExposed}</p>
        )}
        {error && (
          <p role="alert" className="mt-2 text-sm font-medium text-bad">
            {errorText(error)}
          </p>
        )}
      </div>
    </div>
  )
}

function ExitCard({ status }: { status: Status }) {
  const { state } = useApp()
  const exit = status.exit
  const proxy = state.proxies.data?.find((p) => p.id === status.active_proxy_id)
  const country = countryName(exit?.country_code ?? null)
  const place = exit ? [exit.city, exit.region, country].filter(Boolean).join(', ') : ''
  return (
    <Card title={t.home.exitTitle}>
      {exit ? (
        <dl className="grid grid-cols-[auto_1fr] gap-x-6 gap-y-1.5 text-sm">
          <dt className="text-muted">{t.home.exitIp}</dt>
          <dd className="font-mono font-medium">{exit.ip}</dd>
          <dt className="text-muted">{t.home.exitPlace}</dt>
          <dd>
            <span aria-hidden="true">{flagEmoji(exit.country_code)} </span>
            {place || t.common.unknown}
          </dd>
          {exit.org && (
            <>
              <dt className="text-muted">{t.home.exitOrg}</dt>
              <dd>{exit.org}</dd>
            </>
          )}
          {exit.timezone && (
            <>
              <dt className="text-muted">{t.home.exitTimezone}</dt>
              <dd>{exit.timezone}</dd>
            </>
          )}
          {status.active_profile && (
            <>
              <dt className="text-muted">{t.home.activeProfile}</dt>
              <dd>{status.active_profile}</dd>
            </>
          )}
          {proxy && (
            <>
              <dt className="text-muted">{t.home.activeProxy}</dt>
              <dd className="font-mono">{hostPort(proxy.host, proxy.port)}</dd>
            </>
          )}
        </dl>
      ) : (
        <p className="text-sm text-muted">{t.home.exitNone}</p>
      )}
    </Card>
  )
}

function KillSwitchCard({ status }: { status: Status }) {
  return (
    <Card title={t.home.killSwitchTitle}>
      <div className="flex items-center gap-3">
        <Badge tone={status.kill_switch_active ? 'ok' : 'neutral'}>
          {status.kill_switch_active ? t.home.killSwitchOn : t.home.killSwitchOff}
        </Badge>
        {status.kill_switch_active && <span className="text-sm text-muted">{t.home.killSwitchHint}</span>}
      </div>
    </Card>
  )
}

function ChangedCard({ status }: { status: Status }) {
  return (
    <Card title={t.home.changedTitle}>
      {status.changed.length === 0 ? (
        <p className="text-sm text-muted">{t.home.changedEmpty}</p>
      ) : (
        <ul className="divide-y divide-line text-sm">
          {status.changed.map((c) => (
            <li key={c.kind} className="flex flex-wrap items-baseline gap-x-3 py-2 first:pt-0 last:pb-0">
              <span className="font-medium">{t.home.changedKind[c.kind]}</span>
              <span className="text-muted">
                {c.original !== null && (
                  <>
                    {t.home.changedWas}: <span className="font-mono">{c.original}</span> →{' '}
                  </>
                )}
                {t.home.changedNow}: <span className="font-mono">{c.current}</span>
              </span>
            </li>
          ))}
        </ul>
      )}
    </Card>
  )
}

function Warnings({ status }: { status: Status }) {
  if (status.conflicts.length === 0) return null
  return (
    <Alert tone="warn" title={t.home.conflictTitle}>
      <ul className="list-disc space-y-1 pl-5">
        {status.conflicts.map((c) => (
          <li key={c.kind}>
            {c.kind === 'other_vpn'
              ? t.home.conflictOtherVpn(c.interface)
              : c.kind === 'private_relay'
                ? t.home.conflictPrivateRelay
                : t.home.conflictHelperDisabled}
          </li>
        ))}
      </ul>
    </Alert>
  )
}

function Demo() {
  const { backend } = useApp()
  const d = backend?.demo
  if (!d) return null
  return (
    <details className="rounded-xl border border-dashed border-line p-3 text-sm">
      <summary className="cursor-pointer font-medium text-muted">{t.demo.title}</summary>
      <div className="mt-3 flex flex-wrap gap-2">
        <Button onClick={() => d.fail('vpn_down')}>{t.demo.vpnDown}</Button>
        <Button onClick={() => d.fail('proxy_down')}>{t.demo.proxyDown}</Button>
        <Button onClick={() => d.recover()}>{t.demo.recover}</Button>
        <Button onClick={() => d.toggleConflict({ kind: 'other_vpn', interface: 'utun3' })}>{t.demo.conflict}</Button>
        <Button onClick={() => d.simulatePendingRestore()}>{t.demo.pendingRestore}</Button>
        <Button onClick={() => d.toggleStartFailure()}>{t.demo.startFailure}</Button>
      </div>
    </details>
  )
}

export function Home() {
  const { state, actions } = useApp()
  const { data: status, loading, error } = state.status
  const [confirmOpen, setConfirmOpen] = useState(false)
  const [busy, setBusy] = useState(false)
  const [actionError, setActionError] = useState<ErrorKey | null>(null)

  if (!status) {
    return (
      <>
        <PageTitle>{t.home.title}</PageTitle>
        {loading || !error ? (
          <Spinner />
        ) : (
          <ErrorState title={t.home.stateLoadFailed} detail={errorText(error)} onRetry={() => void actions.reloadStatus()} />
        )}
      </>
    )
  }

  async function run(req: Parameters<typeof actions.call>[0]) {
    setBusy(true)
    setActionError(null)
    const r = await actions.call(req)
    if (!r.ok) setActionError(r.error)
    await actions.reloadStatus()
    setBusy(false)
  }

  const blocked = status.chain.state === 'blocked'

  return (
    <div className="flex flex-col gap-4">
      <PageTitle>{t.home.title}</PageTitle>

      {status.pending_restore && (
        <Alert
          tone="warn"
          title={t.home.restoreTitle}
          actions={
            <Button variant="primary" disabled={busy} onClick={() => void run({ cmd: 'restore_settings' })}>
              {t.home.restoreButton}
            </Button>
          }
        >
          {t.home.restoreHint}
        </Alert>
      )}

      <Warnings status={status} />

      {status.last_error && status.chain.state === 'off' && (
        <Alert tone="bad" role="alert" title={t.home.lastErrorTitle}>
          {errorText(status.last_error.code)} {t.home.lastErrorHint}
        </Alert>
      )}

      <Card>
        <div aria-live="polite" className="flex flex-col gap-6">
          <BigSwitch status={status} />
          <ChainView status={status} />
        </div>
        {(blocked || status.chain.state === 'wifi_login') && (
          <div className="mt-5 flex flex-wrap gap-2 border-t border-line pt-4">
            {blocked && (
              <Button variant="danger" disabled={busy} onClick={() => setConfirmOpen(true)}>
                {t.home.disableProtection}
              </Button>
            )}
            {status.chain.state === 'wifi_login' && (
              <Button disabled={busy} onClick={() => void run({ cmd: 'stop_wifi_login' })}>
                {t.home.stopWifi}
              </Button>
            )}
          </div>
        )}
        {actionError && (
          <p role="alert" className="mt-3 text-sm font-medium text-bad">
            {errorText(actionError)}
          </p>
        )}
      </Card>

      <div className="grid gap-4 md:grid-cols-2">
        <ExitCard status={status} />
        <KillSwitchCard status={status} />
      </div>
      <ChangedCard status={status} />
      <Demo />

      <ConfirmDialog
        open={confirmOpen}
        title={t.home.disableDialogTitle}
        body={t.home.disableDialogBody}
        confirmLabel={t.home.disableDialogConfirm}
        busy={busy}
        onCancel={() => setConfirmOpen(false)}
        onConfirm={() => {
          setConfirmOpen(false)
          void run({ cmd: 'disable_protection', confirmed: true })
        }}
      />
    </div>
  )
}
