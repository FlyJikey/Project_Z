import { useId, useMemo, useState, type DragEvent, type FormEvent } from 'react'
import { errorText, useApp } from '../state/app.tsx'
import { Badge, Button, Card, ConfirmDialog, EmptyState, ErrorState, PageTitle, Spinner } from '../components/ui.tsx'
import {
  endpointKey,
  hostPort,
  maskUsername,
  parseProxyList,
  reorder,
} from '../lib/proxyParse.ts'
import { countryName, flagEmoji } from '../lib/format.ts'
import type { ProxyEndpoint, ProxyInput, ProxyView } from '../ipc/types.ts'
import { t, type ErrorKey } from '../strings/ru.ts'

interface LineIssue {
  line: number
  text: string
}

interface Analysis {
  valid: { line: number; endpoint: ProxyEndpoint }[]
  issues: LineIssue[]
}

function analyze(text: string, existing: readonly ProxyView[]): Analysis {
  const seen = new Set(existing.map((p) => endpointKey(p.scheme, p.host, p.port)))
  const valid: Analysis['valid'] = []
  const issues: LineIssue[] = []
  for (const { line, result } of parseProxyList(text)) {
    if (!result.ok) {
      issues.push({ line, text: t.proxies.parseErrors[result.error] })
      continue
    }
    const e = result.endpoint
    const key = endpointKey(e.scheme, e.host, e.port)
    if (seen.has(key)) {
      issues.push({ line, text: t.proxies.duplicate })
      continue
    }
    seen.add(key)
    valid.push({ line, endpoint: e })
  }
  return { valid, issues }
}

function AddForm({ existing }: { existing: readonly ProxyView[] }) {
  const { actions } = useApp()
  const [mode, setMode] = useState<'one' | 'list'>('one')
  const [text, setText] = useState('')
  const [show, setShow] = useState(false)
  const [attempted, setAttempted] = useState(false)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<ErrorKey | null>(null)
  const [info, setInfo] = useState<string | null>(null)
  const fieldId = useId()
  const hintId = useId()
  const analysis = useMemo(() => analyze(text, existing), [text, existing])

  async function submit(e: FormEvent) {
    e.preventDefault()
    setInfo(null)
    setError(null)
    setAttempted(true)
    if (analysis.valid.length === 0) return
    setBusy(true)
    const inputs: ProxyInput[] = [
      ...existing.map((p): ProxyInput => ({ kind: 'existing', id: p.id })),
      ...analysis.valid.map((v): ProxyInput => ({ kind: 'new', endpoint: v.endpoint })),
    ]
    const r = await actions.call({ cmd: 'set_proxies', proxies: inputs })
    setBusy(false)
    if (!r.ok) return setError(r.error)
    // Пароли ушли помощнику: из поля убираем всё принятое, остаются только строки с ошибками.
    const failed = new Set(analysis.issues.map((i) => i.line))
    const rawLines = text.split(/\r?\n/)
    const remaining = rawLines.filter((_, i) => failed.has(i + 1)).join('\n')
    setText(remaining)
    setShow(false)
    setAttempted(remaining !== '')
    setInfo(t.proxies.addedCount(analysis.valid.length))
    await actions.reloadProxies()
  }

  const showIssues = attempted || (mode === 'list' && text.trim() !== '')
  const emptyAttempt = attempted && text.trim() === ''

  return (
    <Card title={t.proxies.addTitle}>
      <form onSubmit={(e) => void submit(e)} noValidate className="flex flex-col gap-3">
        <div role="group" className="inline-flex w-fit rounded-lg border border-line p-0.5">
          {(['one', 'list'] as const).map((m) => (
            <button
              key={m}
              type="button"
              aria-pressed={mode === m}
              onClick={() => setMode(m)}
              className={`rounded-md px-3 py-1.5 text-sm font-medium ${mode === m ? 'bg-accent-soft text-accent' : 'text-muted hover:text-fg'}`}
            >
              {m === 'one' ? t.proxies.modeOne : t.proxies.modeList}
            </button>
          ))}
        </div>

        <div>
          <label htmlFor={fieldId} className="mb-1 block text-sm font-medium">
            {mode === 'one' ? t.proxies.oneLabel : t.proxies.listLabel}
          </label>
          {mode === 'one' ? (
            <input
              id={fieldId}
              type={show ? 'text' : 'password'}
              value={text}
              onChange={(e) => setText(e.target.value)}
              placeholder={t.proxies.onePlaceholder}
              autoComplete="off"
              autoCapitalize="off"
              spellCheck={false}
              aria-describedby={hintId}
              className="w-full rounded-lg border border-line bg-bg px-3 py-2 font-mono text-sm"
            />
          ) : (
            <textarea
              id={fieldId}
              value={text}
              onChange={(e) => setText(e.target.value)}
              placeholder={t.proxies.listPlaceholder}
              rows={5}
              autoComplete="off"
              autoCapitalize="off"
              spellCheck={false}
              aria-describedby={hintId}
              className="w-full rounded-lg border border-line bg-bg px-3 py-2 font-mono text-sm"
            />
          )}
          <p id={hintId} className="mt-1 text-xs text-muted">
            {mode === 'one' ? t.proxies.oneHint : t.proxies.listHint} {t.proxies.passwordNote}
          </p>
          {mode === 'one' && (
            <label className="mt-2 inline-flex items-center gap-2 text-sm">
              <input type="checkbox" checked={show} onChange={(e) => setShow(e.target.checked)} />
              {t.proxies.showLine}
            </label>
          )}
        </div>

        {emptyAttempt && (
          <p role="alert" className="text-sm font-medium text-bad">
            {t.proxies.nothingToAdd}
          </p>
        )}

        {showIssues && text.trim() !== '' && (
          <div className="text-sm" aria-live="polite">
            <p className="mb-1 flex flex-wrap gap-2">
              <Badge tone={analysis.valid.length > 0 ? 'ok' : 'neutral'}>{t.proxies.previewOk(analysis.valid.length)}</Badge>
              {analysis.issues.length > 0 && <Badge tone="bad">{t.proxies.previewBad(analysis.issues.length)}</Badge>}
            </p>
            {mode === 'list' && analysis.valid.length > 0 && (
              <ul className="mb-1 space-y-0.5 text-muted">
                {analysis.valid.map((v) => (
                  <li key={v.line} className="font-mono text-xs">
                    {v.endpoint.scheme}://{hostPort(v.endpoint.host, v.endpoint.port)}
                    {v.endpoint.username !== null && ` · ${t.proxies.previewLogin} ${maskUsername(v.endpoint.username)}`}
                  </li>
                ))}
              </ul>
            )}
            {analysis.issues.length > 0 && (
              <ul className="space-y-0.5 text-bad">
                {analysis.issues.map((i) => (
                  <li key={i.line}>
                    {mode === 'list' ? t.proxies.lineError(i.line, i.text) : t.proxies.singleError(i.text)}
                  </li>
                ))}
              </ul>
            )}
          </div>
        )}

        {error && (
          <p role="alert" className="text-sm font-medium text-bad">
            {errorText(error)}
          </p>
        )}
        {info && (
          <p role="status" className="text-sm font-medium text-ok">
            {info}
          </p>
        )}
        <div>
          <Button type="submit" variant="primary" disabled={busy}>
            {busy ? t.proxies.adding : t.proxies.addButton}
          </Button>
        </div>
      </form>
    </Card>
  )
}

const healthTone = { unknown: 'neutral', up: 'ok', down: 'bad' } as const

function ProxyRow({
  proxy,
  index,
  total,
  busy,
  dragOver,
  onCheck,
  onMove,
  onRemove,
  onDragStart,
  onDragEnter,
  onDrop,
  checking,
}: {
  proxy: ProxyView
  index: number
  total: number
  busy: boolean
  dragOver: boolean
  checking: boolean
  onCheck: (p: ProxyView) => void
  onMove: (from: number, to: number) => void
  onRemove: (p: ProxyView) => void
  onDragStart: (i: number) => void
  onDragEnter: (i: number) => void
  onDrop: (i: number) => void
}) {
  const hp = hostPort(proxy.host, proxy.port)
  const exit = proxy.exit
  const place = exit ? [exit.city, exit.region, countryName(exit.country_code)].filter(Boolean).join(', ') : ''
  return (
    <li
      draggable
      onDragStart={() => onDragStart(index)}
      onDragEnter={() => onDragEnter(index)}
      onDragOver={(e: DragEvent) => e.preventDefault()}
      onDrop={(e: DragEvent) => {
        e.preventDefault()
        onDrop(index)
      }}
      aria-label={t.proxies.itemLabel(hp)}
      className={`rounded-xl border bg-surface p-4 ${dragOver ? 'border-accent' : 'border-line'}`}
    >
      <div className="flex flex-wrap items-center gap-2">
        <span aria-hidden="true" title={t.proxies.dragHandle} className="cursor-grab select-none text-muted">
          ⠿
        </span>
        <Badge tone={index === 0 ? 'accent' : 'neutral'}>{index === 0 ? `${t.proxies.position(1)} · ${t.proxies.primary}` : t.proxies.position(index + 1)}</Badge>
        <span className="font-mono text-sm font-medium">
          {proxy.scheme}://{hp}
        </span>
        <Badge tone={healthTone[proxy.health]}>{t.proxies.health[proxy.health]}</Badge>
      </div>
      <p className="mt-2 text-sm text-muted">
        {t.proxies.login}: {proxy.username_masked ?? t.proxies.noLogin}
        {' · '}
        {proxy.has_password ? t.proxies.passwordSet : t.proxies.passwordNone}
        {exit && (
          <>
            {' · '}
            {t.proxies.exit}: <span className="font-mono">{exit.ip}</span>
            {place && (
              <>
                {' '}
                <span aria-hidden="true">{flagEmoji(exit.country_code)}</span> {place}
              </>
            )}
          </>
        )}
      </p>
      <div className="mt-3 flex flex-wrap gap-2">
        <Button disabled={busy || checking} onClick={() => onCheck(proxy)}>
          {checking ? t.proxies.checking : t.proxies.check}
        </Button>
        {index > 0 && (
          <Button disabled={busy} onClick={() => onMove(index, 0)}>
            {t.proxies.makePrimary}
          </Button>
        )}
        <Button disabled={busy || index === 0} onClick={() => onMove(index, index - 1)} aria-label={`${t.proxies.moveUp}: ${hp}`}>
          ↑ {t.proxies.moveUp}
        </Button>
        <Button disabled={busy || index === total - 1} onClick={() => onMove(index, index + 1)} aria-label={`${t.proxies.moveDown}: ${hp}`}>
          ↓ {t.proxies.moveDown}
        </Button>
        <Button variant="ghost" disabled={busy} onClick={() => onRemove(proxy)}>
          {t.proxies.remove}
        </Button>
      </div>
    </li>
  )
}

export function Proxies() {
  const { state, actions } = useApp()
  const { data, loading, error } = state.proxies
  const [busy, setBusy] = useState(false)
  const [checking, setChecking] = useState<string | null>(null)
  const [actionError, setActionError] = useState<ErrorKey | null>(null)
  const [toRemove, setToRemove] = useState<ProxyView | null>(null)
  const [dragFrom, setDragFrom] = useState<number | null>(null)
  const [dragOver, setDragOver] = useState<number | null>(null)
  const [announce, setAnnounce] = useState('')

  async function setOrder(list: readonly ProxyView[]) {
    setBusy(true)
    setActionError(null)
    const r = await actions.call({
      cmd: 'set_proxies',
      proxies: list.map((p): ProxyInput => ({ kind: 'existing', id: p.id })),
    })
    if (!r.ok) setActionError(r.error)
    await actions.reloadProxies()
    setBusy(false)
  }

  async function move(from: number, to: number) {
    if (!data) return
    const next = reorder(data, from, to)
    const newIndex = next.findIndex((p) => p.id === data[from]?.id)
    setAnnounce(t.proxies.moved(newIndex + 1))
    await setOrder(next)
  }

  async function check(p: ProxyView) {
    setChecking(p.id)
    setActionError(null)
    const r = await actions.call({ cmd: 'check_proxy', id: p.id })
    if (!r.ok) setActionError(r.error)
    else if (r.response.kind === 'proxies') actions.applyProxies(r.response.proxies)
    else setActionError('internal')
    setChecking(null)
  }

  async function confirmRemove() {
    if (!toRemove || !data) return
    const id = toRemove.id
    setToRemove(null)
    await setOrder(data.filter((p) => p.id !== id))
  }

  function endDrag(to: number) {
    const from = dragFrom
    setDragFrom(null)
    setDragOver(null)
    if (from !== null && from !== to) void move(from, to)
  }

  return (
    <div className="flex flex-col gap-4">
      <PageTitle>{t.proxies.title}</PageTitle>
      <AddForm existing={data ?? []} />

      <h2 className="mt-2 text-lg font-semibold">{t.proxies.poolTitle}</h2>
      <p className="-mt-2 text-sm text-muted">
        {t.proxies.poolHint} {t.proxies.reorderHint}
      </p>
      <p role="status" aria-live="polite" className="sr-only">
        {announce}
      </p>
      {actionError && (
        <p role="alert" className="text-sm font-medium text-bad">
          {errorText(actionError)}
        </p>
      )}

      {!data && error && (
        <ErrorState title={t.proxies.loadFailed} detail={errorText(error)} onRetry={() => void actions.reloadProxies()} />
      )}
      {!data && !error && loading && <Spinner />}
      {data && data.length === 0 && <EmptyState>{t.proxies.empty}</EmptyState>}
      {data && data.length > 0 && (
        <ul className="flex flex-col gap-3">
          {data.map((p, i) => (
            <ProxyRow
              key={p.id}
              proxy={p}
              index={i}
              total={data.length}
              busy={busy}
              checking={checking === p.id}
              dragOver={dragFrom !== null && dragOver === i && dragFrom !== i}
              onCheck={(x) => void check(x)}
              onMove={(f, to) => void move(f, to)}
              onRemove={setToRemove}
              onDragStart={setDragFrom}
              onDragEnter={setDragOver}
              onDrop={endDrag}
            />
          ))}
        </ul>
      )}

      <ConfirmDialog
        open={toRemove !== null}
        title={t.proxies.removeConfirmTitle}
        body={toRemove ? t.proxies.removeConfirmBody(hostPort(toRemove.host, toRemove.port)) : ''}
        confirmLabel={t.proxies.removeConfirm}
        onCancel={() => setToRemove(null)}
        onConfirm={() => void confirmRemove()}
      />
    </div>
  )
}
