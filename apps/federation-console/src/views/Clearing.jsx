import React, { useState } from 'react'
import { useChain } from '../store.jsx'
import { Card, Field, OpForm, Button, Table, Mono, Empty, ViewHead } from '../ui.jsx'
import { fmtXsu, parseXsu, shortHash, memberLabel, ALL_MEMBERS, FOREIGN_MEMBERS } from '../format.js'

export default function Clearing() {
  const { state, call, T } = useChain()
  return (
    <>
      <ViewHead
        zh="多邊清算與儲備證明"
        en="Multilateral clearing & proof of reserve"
        lead={T(
          '跨境流量先以 XSU 軋差,只就淨部位以各國 CBDC 結算,降低流動性需求與外匯摩擦。準備池每日發布鏈上儲備證明,公開可稽核。',
          'Cross-border flows net in XSU first; only net positions settle in CBDCs, cutting liquidity needs and FX friction. The reserve pool publishes a daily on-chain proof-of-reserves, publicly auditable.',
        )}
      />

      <Card title={T('成員 XSU 餘額', 'Member XSU balances')} sub={T('§11.3 多邊清算經此軋差', '§11.3 multilateral clearing nets here')}>
        <Table head={[T('成員', 'Member'), T('XSU 餘額', 'XSU balance')]}>
          {ALL_MEMBERS.map((m) => (
            <tr key={m}>
              <td>{memberLabel(m)} {m === state.localMember && <span className="muted small">({T('本席', 'you')})</span>}</td>
              <td className="num strong">{fmtXsu(state.xsuBalances[m] || 0n)}</td>
            </tr>
          ))}
        </Table>
      </Card>

      <div className="grid2">
        <BookClearing call={call} T={T} />
        <NetSettle call={call} T={T} />
      </div>

      <ProofOfReserve state={state} call={call} T={T} />
    </>
  )
}

function BookClearing({ call, T }) {
  const [to, setTo] = useState(FOREIGN_MEMBERS[0])
  const [amount, setAmount] = useState('12000')
  return (
    <Card title={T('記入清算', 'Book clearing')} sub={T('§11.3 以 XSU 計價的雙邊軋差', '§11.3 bilateral netting in XSU')}>
      <OpForm name="bookClearing" submitLabel={T('記入', 'Book')} onSubmit={() => call('bookClearing', { to, amount: parseXsu(amount) })}>
        <div className="row2">
          <Field label={T('對手方', 'Counterparty')}>
            <select className="input" value={to} onChange={(e) => setTo(e.target.value)}>{FOREIGN_MEMBERS.map((m) => <option key={m}>{m}</option>)}</select>
          </Field>
          <Field label={T('XSU 金額', 'XSU amount')}>
            <input className="input" value={amount} onChange={(e) => setAmount(e.target.value)} inputMode="decimal" />
          </Field>
        </div>
      </OpForm>
    </Card>
  )
}

function NetSettle({ call, T }) {
  const [window, setWindow] = useState(1)
  return (
    <Card title={T('淨額結算', 'Net & settle')} sub={T('§11.3 窗口收盤僅結算淨部位', '§11.3 only net positions settle at close')}>
      <OpForm name="netAndSettle" submitLabel={T('結算', 'Settle')} onSubmit={() => call('netAndSettle', { window: Number(window) })}>
        <Field label={T('清算窗口', 'Clearing window')}>
          <input className="input" value={window} onChange={(e) => setWindow(e.target.value)} inputMode="numeric" />
        </Field>
      </OpForm>
    </Card>
  )
}

function ProofOfReserve({ state, call, T }) {
  const por = state.lastProofOfReserve
  return (
    <Card title={T('鏈上儲備證明', 'On-chain proof of reserve')} sub={T('§11.3 公開可稽核', '§11.3 publicly auditable')}>
      {por ? (
        <div className="porline">
          <Pill>#{por.block}</Pill> <Mono title={por.digest}>{shortHash(por.digest)}</Mono>
        </div>
      ) : (
        <Empty>{T('尚未發布', 'Not yet published')}</Empty>
      )}
      <div className="opform-foot">
        <Mono className="dim">Federation.publishProofOfReserve · 14/9</Mono>
        <Button onClick={() => call('publishProofOfReserve', {})}>{T('發布儲備證明', 'Publish proof of reserve')}</Button>
      </div>
    </Card>
  )
}

// local Pill (avoid extra import churn)
function Pill({ children }) {
  return <span className="pill pill-info">{children}</span>
}
