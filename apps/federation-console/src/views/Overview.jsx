import React from 'react'
import { useChain } from '../store.jsx'
import { Card, Table, Mono, Pill, Empty, ViewHead } from '../ui.jsx'
import { actionLabel, actionDomain, DOMAINS } from '../chain.js'
import { fmtXsu, fmtPct, memberLabel, memberName, MEMBERS, ALL_MEMBERS } from '../format.js'

const STATUS_TONE = { Open: 'warn', Queued: 'info', Enacted: 'good', Rejected: 'bad' }
export function StatusPill({ status }) {
  return <Pill tone={STATUS_TONE[status] || 'neutral'}>{status}</Pill>
}

const PIPELINE = [
  { zh: '提案', en: 'Propose' },
  { zh: '審議', en: 'Deliberate' },
  { zh: '雙重多數', en: 'Dual-majority' },
  { zh: '時間鎖', en: 'Timelock' },
  { zh: '鏈上生效', en: 'Enact' },
]

export default function Overview() {
  const { state, lang, T } = useChain()
  const seated = ALL_MEMBERS.filter((m) => state.members[m])
  const proposals = Object.values(state.proposals)
  const open = proposals.filter((p) => p.status === 'Open').length
  const reserveTotal = Object.values(state.reservePool).reduce((a, b) => a + b, 0n)

  const stats = [
    { label: T('在席成員', 'Seated members'), value: seated.length, sub: T('一國一席', 'one seat per nation') },
    { label: T('XSU 籃子版本', 'XSU basket version'), value: `v${state.basket.version}`, sub: T(`${state.basket.entries.length} 種 CBDC`, `${state.basket.entries.length} CBDCs`) },
    { label: T('XSU 發行量', 'XSU issued'), value: fmtXsu(state.xsuIssued), sub: T('足額準備', 'fully reserved') },
    { label: T('開放提案', 'Open proposals'), value: open, sub: T(`${proposals.length} 總計`, `${proposals.length} total`) },
    { label: T('準備池總額', 'Reserve pool total'), value: fmtXsu(reserveTotal), sub: T('各 CBDC 合計', 'across CBDCs') },
  ]

  return (
    <>
      <ViewHead
        zh="總覽 · 主權鏈聯邦治理"
        en="Overview · sovereign-chain federation governance"
        lead={T(
          '互通層的中立性,取決於沒有任何單一主權能片面掌控它。本主控台以 TW 席位操作條約理事會:提案、雙重多數表決、時間鎖生效,以及 XSU 籃子與多邊清算。',
          "The interop layer's neutrality rests on no single sovereign controlling it. You operate the treaty council as the TW seat: propose, dual-majority vote, timelock enactment, and XSU basket / multilateral clearing.",
        )}
      />

      <div className="stats">
        {stats.map((s) => (
          <div className="stat" key={s.label}>
            <div className="stat-value">{s.value}</div>
            <div className="stat-label">{s.label}</div>
            <div className="stat-sub muted small">{s.sub}</div>
          </div>
        ))}
      </div>

      <Card title={T('聯邦治理流程', 'Governance pipeline')} sub={T('§11.2 提案 → 審議 → 雙重多數 → 時間鎖 → 生效', '§11.2 propose → deliberate → dual-majority → timelock → enact')}>
        <div className="pipeline">
          {PIPELINE.map((p, i) => (
            <React.Fragment key={p.en}>
              <div className="pipe-step">{lang === 'zh' ? p.zh : p.en}</div>
              {i < PIPELINE.length - 1 && <span className="pipe-arrow">→</span>}
            </React.Fragment>
          ))}
        </div>
      </Card>

      <div className="grid2">
        <Card title={T('理事會成員與籃子權重', 'Council members & basket weights')} sub={T('§11.2 雙重多數的權重維度', '§11.2 the weight axis of dual-majority')}>
          <Table head={[T('成員', 'Member'), T('席位', 'Seat'), T('籃子權重', 'Weight'), T('XSU 餘額', 'XSU balance')]}>
            {ALL_MEMBERS.map((m) => (
              <tr key={m}>
                <td>
                  <div className="cellmain">{memberLabel(m)} <span className="muted small">{memberName(m, lang)}</span></div>
                  <Mono>{MEMBERS[m]?.etoken}</Mono>
                </td>
                <td>{state.members[m] ? <Pill tone="good">{T('在席', 'seated')}</Pill> : <Pill tone="neutral">{T('未在席', 'unseated')}</Pill>}</td>
                <td className="num">{state.basketWeights[m] ? fmtPct(state.basketWeights[m]) : '—'}</td>
                <td className="num">{state.xsuBalances[m] ? fmtXsu(state.xsuBalances[m]) : '—'}</td>
              </tr>
            ))}
          </Table>
        </Card>

        <Card title={T('近期提案', 'Recent proposals')}>
          {proposals.length === 0 ? (
            <Empty>{T('尚無提案', 'No proposals')}</Empty>
          ) : (
            <Table head={['id', T('動作', 'Action'), T('領域', 'Domain'), T('狀態', 'Status')]}>
              {proposals.sort((a, b) => b.id - a.id).slice(0, 7).map((p) => (
                <tr key={p.id}>
                  <td><Mono>{p.id}</Mono></td>
                  <td><Mono title={actionLabel(p.action)}>{actionLabel(p.action)}</Mono></td>
                  <td className="small">{lang === 'zh' ? DOMAINS[actionDomain(p.action)].zh : DOMAINS[actionDomain(p.action)].en}</td>
                  <td><StatusPill status={p.status} /></td>
                </tr>
              ))}
            </Table>
          )}
        </Card>
      </div>

      <div className="privacy">
        <span className="privacy-ic">⚖</span>
        <div>
          <strong>{T('中立性不變式', 'Neutrality invariant')}</strong>
          <p className="muted small">
            {T(
              '鐵橋不屬於任何一國,而由條約理事會共同治理;XSU 籃子與準備池的每一次變動都須通過明確的鏈上門檻(雙重多數),並以時間鎖延遲生效以利各國準備。',
              'The Ferrum Bridge belongs to no single nation; it is co-governed by the treaty council. Every change to the XSU basket or reserve pool must clear an explicit on-chain threshold (dual-majority) and is delayed by a timelock so members can prepare.',
            )}
          </p>
        </div>
      </div>
    </>
  )
}
