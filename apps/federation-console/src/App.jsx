import React, { useState, useEffect } from 'react'
import { useChain } from './store.jsx'
import { MODULE_NAME } from './chain.js'
import Overview from './views/Overview.jsx'
import Council from './views/Council.jsx'
import Proposals from './views/Proposals.jsx'
import Treasury from './views/Treasury.jsx'
import Clearing from './views/Clearing.jsx'
import EventLog from './views/EventLog.jsx'

const NAV = [
  { key: 'overview', zh: '總覽', en: 'Overview', icon: '◎', el: Overview },
  { key: 'council', zh: '11.1 條約理事會', en: '11.1 Council', icon: '⊚', el: Council },
  { key: 'proposals', zh: '11.2 治理表決', en: '11.2 Governance', icon: '⊞', el: Proposals },
  { key: 'treasury', zh: 'XSU 籃子與準備', en: 'XSU basket & reserve', icon: '∑', el: Treasury },
  { key: 'clearing', zh: '清算與儲備證明', en: 'Clearing & PoR', icon: '⇄', el: Clearing },
  { key: 'events', zh: '事件記錄', en: 'Event log', icon: '≣', el: EventLog },
]

export default function App() {
  const { lang, setLang, T, state, toast, dismissToast, reset, live } = useChain()
  const [tab, setTab] = useState('overview')
  const Active = NAV.find((n) => n.key === tab).el

  useEffect(() => {
    if (!toast) return
    const t = setTimeout(dismissToast, 4200)
    return () => clearTimeout(t)
  }, [toast, dismissToast])

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <span className="brand-mark">Fe</span>
          <div className="brand-text">
            <strong>{T('聯邦治理主控台', 'Federation Governance Console')}</strong>
            <span className="muted small">{T('白皮書第 11 章 · pallet-federation', 'Whitepaper §11 · pallet-federation')}</span>
          </div>
        </div>
        <div className="topbar-right">
          <span className={`chainbadge ${live ? 'is-live' : ''}`} title="council seat">
            <span className="dot-live" /> {live ? T('連線', 'live') : T('模擬', 'sim')} · <strong>{state.localMember}</strong> {T('席', 'seat')}
          </span>
          <span className="blockbadge">#{state.block}</span>
          <button className="btn btn-ghost btn-sm" onClick={reset} title={T('重置狀態', 'reset state')}>{T('重置', 'Reset')}</button>
          <button className="btn btn-ghost btn-sm" onClick={() => setLang(lang === 'zh' ? 'en' : 'zh')}>
            {lang === 'zh' ? 'EN' : '中'}
          </button>
        </div>
      </header>

      <ConnectionBar />

      <div className="shell">
        <nav className="sidebar">
          {NAV.map((n) => (
            <button key={n.key} className={`navitem ${tab === n.key ? 'active' : ''}`} onClick={() => setTab(n.key)}>
              <span className="navicon">{n.icon}</span>
              <span>{lang === 'zh' ? n.zh : n.en}</span>
            </button>
          ))}
          <div className="sidebar-foot muted small">
            {T('原型主控台 · 忠實模擬鏈上行為,離線可操作。', 'Prototype console — faithfully simulates on-chain behavior, runs offline.')}
          </div>
        </nav>
        <main className="content">
          <Active />
        </main>
      </div>

      {toast && (
        <div className={`toast ${toast.ok ? 'toast-ok' : 'toast-bad'}`} onClick={dismissToast}>
          <div className="toast-msg">{lang === 'zh' ? toast.zh : toast.en}</div>
          {toast.call && <div className="toast-call mono">{MODULE_NAME}.{toast.call.name} · {toast.call.pallet}/{toast.call.index}</div>}
          {toast.hash && <div className="toast-call mono">xt: {toast.hash.slice(0, 18)}…{toast.hash.slice(-8)}</div>}
        </div>
      )}
    </div>
  )
}

const DEV_ACCOUNTS = ['//Alice', '//Bob', '//Charlie', '//Dave']

function ConnectionBar() {
  const { T, live, account, conn, connecting, connect, disconnect } = useChain()
  const [endpoint, setEndpoint] = useState('ws://127.0.0.1:9944')
  const [uri, setUri] = useState('//Alice')

  if (live) {
    return (
      <div className="connbar connbar-live">
        <span className="conn-dot" />
        <span className="small"><strong>{T('連線中', 'Connected')}</strong> · {conn.chainName} · spec {conn.specVersion}/tx {conn.transactionVersion}</span>
        <span className="mono small conn-acct" title={account}>{account?.slice(0, 8)}…{account?.slice(-6)}</span>
        <span className="muted small conn-note">{T('呼叫將以 author_submitExtrinsic 真實上鏈。', 'Calls submit real extrinsics via author_submitExtrinsic.')}</span>
        <button className="btn btn-ghost btn-sm" onClick={disconnect}>{T('離線', 'Disconnect')}</button>
      </div>
    )
  }
  return (
    <div className="connbar">
      <span className="muted small">{T('模擬模式', 'Simulation mode')} ·</span>
      <input className="input conn-in" value={endpoint} onChange={(e) => setEndpoint(e.target.value)} placeholder="ws://127.0.0.1:9944" spellCheck={false} />
      <select className="input conn-acct-sel" value={uri} onChange={(e) => setUri(e.target.value)}>
        {DEV_ACCOUNTS.map((a) => <option key={a}>{a}</option>)}
      </select>
      <button className="btn btn-primary btn-sm" disabled={connecting} onClick={() => connect(endpoint, uri)}>
        {connecting ? T('連線中…', 'Connecting…') : T('連線節點', 'Connect node')}
      </button>
      <span className="muted small conn-note">{T('未連線時於本機忠實模擬;連線後改以真實 author_submitExtrinsic 送出。', 'Faithful local simulation until connected; then real author_submitExtrinsic.')}</span>
    </div>
  )
}
