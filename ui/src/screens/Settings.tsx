import { useId, useState } from 'react'
import { errorText, useApp } from '../state/app.tsx'
import { Button, Card, ConfirmDialog, ErrorState, PageTitle, Spinner, Toggle } from '../components/ui.tsx'
import type { PoolMode, Settings } from '../ipc/types.ts'
import { t, type ErrorKey } from '../strings/ru.ts'

const POOL_MODES: readonly PoolMode[] = ['primary_only', 'with_fallback']

function LogCard() {
  const { actions } = useApp()
  const [lines, setLines] = useState<string[] | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<ErrorKey | null>(null)

  async function load() {
    setLoading(true)
    setError(null)
    const r = await actions.call({ cmd: 'get_log', max_lines: 200 })
    setLoading(false)
    if (!r.ok) return setError(r.error)
    setLines(r.response.kind === 'log' ? r.response.lines : [])
  }

  return (
    <Card
      title={t.settings.logTitle}
      actions={
        <Button disabled={loading} onClick={() => void load()}>
          {lines === null ? t.settings.logShow : t.settings.logRefresh}
        </Button>
      }
    >
      <p className="mb-3 text-sm text-muted">{t.settings.logHint}</p>
      {loading && <Spinner label={t.settings.logLoading} />}
      {error && (
        <p role="alert" className="text-sm font-medium text-bad">
          {t.settings.logFailed}: {errorText(error)}
        </p>
      )}
      {lines !== null && !loading && (
        lines.length === 0 ? (
          <p className="text-sm text-muted">{t.settings.logEmpty}</p>
        ) : (
          <pre
            tabIndex={0}
            aria-label={t.settings.logLabel}
            className="max-h-72 overflow-auto rounded-lg bg-surface-2 p-3 font-mono text-xs leading-relaxed whitespace-pre-wrap"
          >
            {lines.join('\n')}
          </pre>
        )
      )}
    </Card>
  )
}

export function SettingsScreen() {
  const { state, actions } = useApp()
  const { data: settings, loading, error } = state.settings
  const [saveError, setSaveError] = useState<ErrorKey | null>(null)
  const [saving, setSaving] = useState(false)
  const [resetOpen, setResetOpen] = useState(false)
  const [resetMsg, setResetMsg] = useState<string | null>(null)
  const [resetError, setResetError] = useState<ErrorKey | null>(null)
  const group = useId()
  const [wifiOpen, setWifiOpen] = useState(false)
  const [wifiMsg, setWifiMsg] = useState<string | null>(null)
  const [wifiError, setWifiError] = useState<ErrorKey | null>(null)
  const chainState = state.status.data?.chain.state
  const wifiAvailable = chainState === 'protected' || chainState === 'blocked'

  async function startWifi() {
    setWifiOpen(false)
    setWifiMsg(null)
    setWifiError(null)
    const r = await actions.call({ cmd: 'start_wifi_login' })
    if (r.ok) setWifiMsg(t.settings.wifiStarted)
    else setWifiError(r.error)
    await actions.reloadStatus()
  }

  async function save(patch: Partial<Settings>) {
    if (!settings) return
    setSaving(true)
    setSaveError(null)
    const r = await actions.call({ cmd: 'set_settings', settings: { ...settings, ...patch } })
    if (!r.ok) setSaveError(r.error)
    await actions.reloadSettings()
    setSaving(false)
  }

  async function resetAll() {
    setResetOpen(false)
    setResetMsg(null)
    setResetError(null)
    const r = await actions.call({ cmd: 'reset_all' })
    if (r.ok) setResetMsg(t.settings.resetDone)
    else setResetError(r.error)
    await actions.reloadStatus()
  }

  return (
    <div className="flex flex-col gap-4">
      <PageTitle>{t.settings.title}</PageTitle>

      {!settings && error && (
        <ErrorState title={t.settings.loadFailed} detail={errorText(error)} onRetry={() => void actions.reloadSettings()} />
      )}
      {!settings && !error && loading && <Spinner />}

      {settings && (
        <>
          {saveError && (
            <p role="alert" className="text-sm font-medium text-bad">
              {t.settings.saveFailed}: {errorText(saveError)}
            </p>
          )}

          <Card title={t.settings.poolTitle}>
            <fieldset disabled={saving} className="flex flex-col gap-3">
              <legend className="sr-only">{t.settings.poolTitle}</legend>
              {POOL_MODES.map((m) => (
                <label key={m} className="flex items-start gap-3 text-sm">
                  <input
                    type="radio"
                    name={`pool-${group}`}
                    className="mt-1"
                    checked={settings.pool_mode === m}
                    onChange={() => void save({ pool_mode: m })}
                  />
                  <span>
                    <span className="font-medium">{t.settings.poolMode[m]}</span>
                    <span className="mt-0.5 block text-muted">{t.settings.poolModeHint[m]}</span>
                  </span>
                </label>
              ))}
            </fieldset>
          </Card>

          <Card title={t.settings.generalTitle}>
            <div className="divide-y divide-line">
              <Toggle
                label={t.settings.allowLan}
                hint={t.settings.allowLanHint}
                checked={settings.allow_lan}
                disabled={saving}
                onChange={(v) => void save({ allow_lan: v })}
              />
              <Toggle
                label={t.settings.startOnBoot}
                hint={t.settings.startOnBootHint}
                checked={settings.start_chain_on_boot}
                disabled={saving}
                onChange={(v) => void save({ start_chain_on_boot: v })}
              />
              <Toggle
                label={t.settings.syncTimezone}
                hint={t.settings.syncTimezoneHint}
                checked={settings.sync_timezone}
                disabled={saving}
                onChange={(v) => void save({ sync_timezone: v })}
              />
            </div>
          </Card>
        </>
      )}

      <Card title={t.settings.wifiTitle}>
        <p className="text-sm text-muted">{t.settings.wifiHint}</p>
        <p className="mt-2 text-sm font-medium text-warn">{t.settings.wifiWarning}</p>
        <div className="mt-3 flex flex-wrap items-center gap-3">
          <Button disabled={!wifiAvailable} onClick={() => setWifiOpen(true)}>
            {t.settings.wifiButton}
          </Button>
          {!wifiAvailable && <span className="text-sm text-muted">{t.settings.wifiUnavailable}</span>}
        </div>
        {wifiMsg && (
          <p role="status" className="mt-3 text-sm font-medium text-ok">
            {wifiMsg}
          </p>
        )}
        {wifiError && (
          <p role="alert" className="mt-3 text-sm font-medium text-bad">
            {errorText(wifiError)}
          </p>
        )}
      </Card>

      <Card title={t.settings.dangerTitle}>
        <p className="mb-3 text-sm text-muted">{t.settings.resetHint}</p>
        <Button variant="danger" onClick={() => setResetOpen(true)}>
          {t.settings.resetAll}
        </Button>
        {resetMsg && (
          <p role="status" className="mt-3 text-sm font-medium text-ok">
            {resetMsg}
          </p>
        )}
        {resetError && (
          <p role="alert" className="mt-3 text-sm font-medium text-bad">
            {errorText(resetError)}
          </p>
        )}
      </Card>

      <LogCard />

      <ConfirmDialog
        open={wifiOpen}
        danger={false}
        title={t.settings.wifiDialogTitle}
        body={t.settings.wifiDialogBody}
        confirmLabel={t.settings.wifiConfirm}
        onCancel={() => setWifiOpen(false)}
        onConfirm={() => void startWifi()}
      />
      <ConfirmDialog
        open={resetOpen}
        title={t.settings.resetDialogTitle}
        body={t.settings.resetDialogBody}
        confirmLabel={t.settings.resetConfirm}
        onCancel={() => setResetOpen(false)}
        onConfirm={() => void resetAll()}
      />
    </div>
  )
}
