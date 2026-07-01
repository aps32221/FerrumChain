import React, { useState } from 'react'
import { useChain } from '../store.jsx'
import { Card, Field, OpForm, Button, Table, Mono, Pill, ViewHead } from '../ui.jsx'
import { fmtXsu, parseXsu, fmtPct, fmtPpb, pctToPpb, cbdcToMember, memberLabel, PPB } from '../format.js'

export default function Treasury() {
  const { state, call, T } = useChain()
  const cbdcs = state.basket.entries.map((e) => e.cbdc)
  return (
    <>
      <ViewHead
        zh="XSU 籃子與準備池"
        en="XSU basket & reserve pool"
        lead={T(
          'XSU 是中立合成記帳單位,定義為一籃參與國 CBDC 的固定權重組合(數位化 SDR),由 CBDC 準備池 1:1 足額支撐。籃子權重由聯邦治理定期再平衡。',
          'XSU is a neutral synthetic unit of account — a fixed-weight basket of member CBDCs (a digital SDR), backed 1:1 by the CBDC reserve pool. Basket weights are reweighted periodically by federation governance.',
        )}
      />

      <div className="stats">
        <div className="stat"><div className="stat-value">v{state.basket.version}</div><div className="stat-label">{T('籃子版本', 'Basket version')}</div></div>
        <div className="stat"><div className="stat-value">{fmtXsu(state.xsuIssued)}</div><div className="stat-label">{T('XSU 發行量', 'XSU issued')}</div></div>
        <div className="stat"><div className="stat-value">{fmtXsu(Object.values(state.reservePool).reduce((a, b) => a + b, 0n))}</div><div className="stat-label">{T('準備池總額', 'Reserve total')}</div></div>
        <div className="stat"><div className="stat-value">{state.basket.entries.length}</div><div className="stat-label">{T('籃子成份', 'Components')}</div></div>
      </div>

      <BasketEditor state={state} call={call} T={T} />

      <div className="grid2">
        <Card title={T('準備池(各 CBDC)', 'Reserve pool (per CBDC)')} sub={T('§11.3 1:1 足額支撐', '§11.3 fully reserved 1:1')}>
          <Table head={['CBDC', T('成員', 'Member'), T('餘額', 'Balance')]}>
            {Object.entries(state.reservePool).map(([c, bal]) => (
              <tr key={c}>
                <td><Mono>{c}</Mono></td>
                <td>{memberLabel(cbdcToMember(c))}</td>
                <td className="num strong">{fmtXsu(bal)}</td>
              </tr>
            ))}
          </Table>
        </Card>

        <div>
          <MintForm cbdcs={cbdcs} call={call} T={T} />
          <RedeemForm cbdcs={cbdcs} call={call} T={T} />
        </div>
      </div>
    </>
  )
}

function BasketEditor({ state, call, T }) {
  const [rows, setRows] = useState(state.basket.entries.map((e) => ({ cbdc: e.cbdc, pct: fmtPpb(e.weightPpb).toFixed(1) })))
  const sum = rows.reduce((a, r) => a + (parseFloat(r.pct) || 0), 0)
  const balanced = Math.abs(sum - 100) < 0.05
  const setPct = (i, v) => setRows((rs) => rs.map((r, j) => (j === i ? { ...r, pct: v } : r)))

  const submit = () => {
    // convert to ppb and fix rounding drift onto the last entry so the sum is exactly 1e9
    const ppbs = rows.map((r) => pctToPpb(r.pct))
    const drift = PPB - ppbs.reduce((a, b) => a + b, 0)
    ppbs[ppbs.length - 1] += drift
    const entries = rows.map((r, i) => ({ cbdc: r.cbdc, weightPpb: ppbs[i] }))
    call('setBasket', { basket: { entries, version: state.basket.version + 1 } })
  }

  return (
    <Card
      title={T('XSU 籃子權重', 'XSU basket weights')}
      sub={T('§11.3 再平衡 · 權重總和須為 100%', '§11.3 reweighting · weights must sum to 100%')}
      badge={balanced ? <Pill tone="good">{sum.toFixed(1)}%</Pill> : <Pill tone="warn">{sum.toFixed(1)}%</Pill>}
    >
      <Table head={['CBDC', T('成員', 'Member'), T('權重 %', 'Weight %'), T('視覺', 'Bar')]}>
        {rows.map((r, i) => (
          <tr key={r.cbdc}>
            <td><Mono>{r.cbdc}</Mono></td>
            <td>{memberLabel(cbdcToMember(r.cbdc))}</td>
            <td><input className="input weight-in" value={r.pct} onChange={(e) => setPct(i, e.target.value)} inputMode="decimal" /></td>
            <td><span className="wbar"><span className="wbar-fill" style={{ width: `${Math.min(100, parseFloat(r.pct) || 0)}%` }} /></span></td>
          </tr>
        ))}
      </Table>
      <div className="opform-foot">
        <Mono className="dim">Federation.setBasket · 14/4</Mono>
        <Button disabled={!balanced} onClick={submit}>{T('再平衡(版本 +1)', 'Reweight (version +1)')}</Button>
      </div>
    </Card>
  )
}

function MintForm({ cbdcs, call, T }) {
  const [cbdc, setCbdc] = useState(cbdcs[0] || 'USD')
  const [amount, setAmount] = useState('10000')
  return (
    <Card title={T('鑄造 XSU', 'Mint XSU')} sub={T('§11.3 存入 CBDC,1:1 鑄造', '§11.3 deposit CBDC, mint 1:1')}>
      <OpForm name="mintXsu" submitLabel={T('鑄造', 'Mint')} onSubmit={() => call('mintXsu', { cbdc, amount: parseXsu(amount) })}>
        <div className="row2">
          <Field label={T('CBDC', 'CBDC')}>
            <select className="input" value={cbdc} onChange={(e) => setCbdc(e.target.value)}>{cbdcs.map((c) => <option key={c}>{c}</option>)}</select>
          </Field>
          <Field label={T('金額', 'Amount')}>
            <input className="input" value={amount} onChange={(e) => setAmount(e.target.value)} inputMode="decimal" />
          </Field>
        </div>
      </OpForm>
    </Card>
  )
}

function RedeemForm({ cbdcs, call, T }) {
  const [cbdc, setCbdc] = useState(cbdcs[0] || 'USD')
  const [amount, setAmount] = useState('5000')
  return (
    <Card title={T('贖回 XSU', 'Redeem XSU')} sub={T('§11.3 銷毀 XSU,贖回 CBDC', '§11.3 burn XSU, redeem CBDC')}>
      <OpForm name="redeemXsu" submitLabel={T('贖回', 'Redeem')} onSubmit={() => call('redeemXsu', { cbdc, amount: parseXsu(amount) })}>
        <div className="row2">
          <Field label={T('CBDC', 'CBDC')}>
            <select className="input" value={cbdc} onChange={(e) => setCbdc(e.target.value)}>{cbdcs.map((c) => <option key={c}>{c}</option>)}</select>
          </Field>
          <Field label={T('XSU 金額', 'XSU amount')}>
            <input className="input" value={amount} onChange={(e) => setAmount(e.target.value)} inputMode="decimal" />
          </Field>
        </div>
      </OpForm>
    </Card>
  )
}
