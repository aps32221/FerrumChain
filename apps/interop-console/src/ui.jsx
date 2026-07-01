// Reusable presentational primitives for the interop console.
import React, { useState } from 'react'
import { useChain } from './store.jsx'
import { CALLS, PALLET_INDEX } from './chain.js'

export function Card({ title, sub, badge, children, accent }) {
  return (
    <section className="card" style={accent ? { borderTopColor: accent, borderTopWidth: 2 } : undefined}>
      {(title || badge) && (
        <header className="card-head">
          <div>
            <h3>{title}</h3>
            {sub && <p className="muted small">{sub}</p>}
          </div>
          {badge}
        </header>
      )}
      {children}
    </section>
  )
}

export function Field({ label, hint, children }) {
  return (
    <label className="field">
      <span className="field-label">{label}</span>
      {children}
      {hint && <span className="field-hint">{hint}</span>}
    </label>
  )
}

export function Button({ children, kind = 'primary', ...rest }) {
  return (
    <button className={`btn btn-${kind}`} {...rest}>
      {children}
    </button>
  )
}

export function Pill({ children, tone = 'neutral' }) {
  return <span className={`pill pill-${tone}`}>{children}</span>
}

export function StatusPill({ status }) {
  const tone = { Pending: 'warn', FinalityVerified: 'info', Accepted: 'good', Rejected: 'bad' }[status] || 'neutral'
  return <Pill tone={tone}>{status}</Pill>
}

export function Mono({ children, title }) {
  return (
    <span className="mono" title={title}>
      {children}
    </span>
  )
}

export function Empty({ children }) {
  return <div className="empty muted small">{children}</div>
}

export function Table({ head, children }) {
  return (
    <div className="tablewrap">
      <table>
        <thead>
          <tr>{head.map((h, i) => <th key={i}>{h}</th>)}</tr>
        </thead>
        <tbody>{children}</tbody>
      </table>
    </div>
  )
}

// Shows the extrinsic that a given call would submit — the bridge between the
// form and the real `pallet-interop` call (module 15, indexed per Calls.cs).
export function CallTag({ name }) {
  const meta = CALLS[name]
  if (!meta) return null
  return (
    <Mono title="pallet.callIndex">
      Interop.{name} <span className="dim">· {PALLET_INDEX}/{meta.index}</span>
    </Mono>
  )
}

// A submit form that wraps children and shows the bound extrinsic + origin.
export function OpForm({ name, onSubmit, children, submitLabel }) {
  const { T } = useChain()
  const meta = CALLS[name]
  return (
    <form
      className="opform"
      onSubmit={(e) => {
        e.preventDefault()
        onSubmit()
      }}
    >
      <div className="opform-fields">{children}</div>
      <div className="opform-foot">
        <div className="opform-meta">
          <CallTag name={name} />
          <Pill tone="neutral">{originLabel(meta.origin, T)}</Pill>
        </div>
        <Button type="submit">{submitLabel || T('送出', 'Submit')}</Button>
      </div>
    </form>
  )
}

function originLabel(origin, T) {
  switch (origin) {
    case 'Federation':
      return T('條約理事會 (Root)', 'Treaty council (Root)')
    case 'Relayer':
      return T('中繼者 (Signed)', 'Relayer (Signed)')
    case 'Signed':
      return T('簽署帳戶', 'Signed account')
    default:
      return origin
  }
}

// Bilingual section heading used at the top of each view.
export function ViewHead({ tag, zh, en, lead }) {
  const { lang } = useChain()
  return (
    <div className="viewhead">
      <div className="viewhead-row">
        {tag && <span className="secnum">{tag}</span>}
        <h2>{lang === 'zh' ? zh : en}</h2>
      </div>
      {lead && <p className="muted">{lead}</p>}
    </div>
  )
}

// A copyable read-only hash value with a regenerate button.
export function HashInput({ value, onChange, onRoll }) {
  return (
    <div className="hashrow">
      <input className="input mono" value={value} onChange={(e) => onChange(e.target.value)} spellCheck={false} />
      {onRoll && (
        <button type="button" className="btn btn-ghost btn-sm" onClick={onRoll} title="randomize">
          ⟳
        </button>
      )}
    </div>
  )
}

export function useToggle(initial = false) {
  const [on, setOn] = useState(initial)
  return [on, () => setOn((v) => !v), setOn]
}
