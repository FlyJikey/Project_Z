// Небольшой набор базовых компонентов. Без UI-китов; тексты — только через props/ru.ts.

import { useEffect, useId, useRef, type ButtonHTMLAttributes, type ReactNode } from 'react'
import { t } from '../strings/ru.ts'

type Variant = 'primary' | 'secondary' | 'danger' | 'ghost'

const variants: Record<Variant, string> = {
  primary: 'bg-accent text-on-accent hover:brightness-110',
  secondary: 'bg-surface-2 text-fg border border-line hover:bg-line/50',
  danger: 'bg-bad text-on-accent hover:brightness-110',
  ghost: 'text-fg hover:bg-surface-2',
}

export function Button({
  variant = 'secondary',
  className = '',
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant }) {
  return (
    <button
      type="button"
      {...rest}
      className={`inline-flex items-center justify-center gap-2 rounded-lg px-3.5 py-2 text-sm font-medium transition disabled:cursor-not-allowed disabled:opacity-50 ${variants[variant]} ${className}`}
    />
  )
}

export function Card({
  title,
  children,
  actions,
  className = '',
  id,
}: {
  title?: ReactNode
  children: ReactNode
  actions?: ReactNode
  className?: string
  id?: string
}) {
  const headingId = useId()
  return (
    <section
      aria-labelledby={title ? headingId : undefined}
      id={id}
      className={`rounded-2xl border border-line bg-surface p-5 shadow-sm ${className}`}
    >
      {(title || actions) && (
        <div className="mb-3 flex items-center justify-between gap-3">
          {title && (
            <h2 id={headingId} className="text-base font-semibold">
              {title}
            </h2>
          )}
          {actions}
        </div>
      )}
      {children}
    </section>
  )
}

export function PageTitle({ children }: { children: ReactNode }) {
  return <h1 className="mb-5 text-2xl font-semibold tracking-tight">{children}</h1>
}

type Tone = 'ok' | 'warn' | 'bad' | 'neutral' | 'accent'
const toneClass: Record<Tone, string> = {
  ok: 'bg-ok-soft text-ok',
  warn: 'bg-warn-soft text-warn',
  bad: 'bg-bad-soft text-bad',
  neutral: 'bg-surface-2 text-muted',
  accent: 'bg-accent-soft text-accent',
}

export function Badge({ tone = 'neutral', children }: { tone?: Tone; children: ReactNode }) {
  return (
    <span className={`inline-flex items-center rounded-full px-2.5 py-0.5 text-xs font-medium ${toneClass[tone]}`}>
      {children}
    </span>
  )
}

export function Alert({
  tone,
  title,
  children,
  actions,
  role,
}: {
  tone: 'warn' | 'bad' | 'info'
  title?: ReactNode
  children?: ReactNode
  actions?: ReactNode
  role?: 'alert' | 'status'
}) {
  const cls =
    tone === 'bad'
      ? 'border-bad/40 bg-bad-soft'
      : tone === 'warn'
        ? 'border-warn/40 bg-warn-soft'
        : 'border-accent/40 bg-accent-soft'
  return (
    <div role={role} className={`flex items-start justify-between gap-4 rounded-xl border p-4 text-sm ${cls}`}>
      <div className="min-w-0">
        {title && <p className="font-semibold">{title}</p>}
        {children && <div className={title ? 'mt-1 text-fg/90' : ''}>{children}</div>}
      </div>
      {actions && <div className="shrink-0">{actions}</div>}
    </div>
  )
}

export function Spinner({ label }: { label?: string }) {
  return (
    <div role="status" className="flex items-center gap-3 py-6 text-sm text-muted">
      <span
        aria-hidden="true"
        className="inline-block size-4 animate-spin rounded-full border-2 border-line border-t-accent"
      />
      {label ?? t.common.loading}
    </div>
  )
}

export function EmptyState({ children }: { children: ReactNode }) {
  return (
    <p className="rounded-xl border border-dashed border-line px-4 py-8 text-center text-sm text-muted">
      {children}
    </p>
  )
}

export function ErrorState({
  title,
  detail,
  onRetry,
}: {
  title: string
  detail: string
  onRetry?: () => void
}) {
  return (
    <Alert
      role="alert"
      tone="bad"
      title={title}
      actions={onRetry && <Button onClick={onRetry}>{t.common.retry}</Button>}
    >
      {detail}
    </Alert>
  )
}

/** Переключатель-«свитч» с подписью. */
export function Toggle({
  checked,
  onChange,
  label,
  hint,
  disabled,
}: {
  checked: boolean
  onChange: (v: boolean) => void
  label: string
  hint?: string
  disabled?: boolean
}) {
  const id = useId()
  const hintId = useId()
  return (
    <div className="flex items-start justify-between gap-4 py-3">
      <div className="min-w-0">
        <label htmlFor={id} className="block text-sm font-medium">
          {label}
        </label>
        {hint && (
          <p id={hintId} className="mt-0.5 text-sm text-muted">
            {hint}
          </p>
        )}
      </div>
      <button
        id={id}
        type="button"
        role="switch"
        aria-checked={checked}
        aria-describedby={hint ? hintId : undefined}
        disabled={disabled}
        onClick={() => onChange(!checked)}
        className={`relative mt-0.5 h-6 w-11 shrink-0 rounded-full transition disabled:opacity-50 ${checked ? 'bg-accent' : 'bg-line'}`}
      >
        <span
          aria-hidden="true"
          className={`absolute top-0.5 left-0.5 size-5 rounded-full bg-white shadow transition-transform ${checked ? 'translate-x-5' : ''}`}
        />
      </button>
    </div>
  )
}

/** Модальный диалог на нативном <dialog>: фокус и Esc обрабатывает браузер. */
export function ConfirmDialog({
  open,
  title,
  body,
  confirmLabel,
  danger = true,
  busy,
  onConfirm,
  onCancel,
}: {
  open: boolean
  title: string
  body: string
  confirmLabel: string
  danger?: boolean
  busy?: boolean
  onConfirm: () => void
  onCancel: () => void
}) {
  const ref = useRef<HTMLDialogElement>(null)
  const titleId = useId()
  const bodyId = useId()
  useEffect(() => {
    const d = ref.current
    if (!d) return
    if (open && !d.open) d.showModal()
    if (!open && d.open) d.close()
  }, [open])
  return (
    <dialog
      ref={ref}
      aria-labelledby={titleId}
      aria-describedby={bodyId}
      onCancel={(e) => {
        e.preventDefault()
        onCancel()
      }}
      className="m-auto w-[min(28rem,90vw)] rounded-2xl border border-line bg-surface p-6 text-fg shadow-xl backdrop:bg-black/50"
    >
      <h2 id={titleId} className="text-lg font-semibold">
        {title}
      </h2>
      <p id={bodyId} className="mt-2 text-sm text-muted">
        {body}
      </p>
      <div className="mt-5 flex justify-end gap-2">
        <Button autoFocus onClick={onCancel}>
          {t.common.cancel}
        </Button>
        <Button variant={danger ? 'danger' : 'primary'} disabled={busy} onClick={onConfirm}>
          {confirmLabel}
        </Button>
      </div>
    </dialog>
  )
}
