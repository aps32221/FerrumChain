import React, { useState } from 'react'
import { useChain } from '../store.jsx'
import { Card, Field, OpForm, Table, Mono, Empty, ViewHead, Pill } from '../ui.jsx'
import { fmtFer, parseFer, MIN_VALIDATOR_BOND } from '../format.js'

export default function Validators() {
  const { state, call, T } = useChain()
  return (
    <>
      <ViewHead
        tag="§11.1"
        zh="互通驗證者"
        en="Interop validators"
        lead={T(
          '互通驗證者由理事會核准,並以本國 FER 質押作跨鏈罰沒保證金;偽造中繼、審查或雙簽將遭沒收並複審。',
          'Interop validators are council-approved and post national FER as a cross-slashable bond; false relays, censorship or equivocation are slashed and reviewed.',
        )}
      />

      <div className="grid2">
        <RegisterValidator call={call} T={T} />
        <SlashValidator state={state} call={call} T={T} />
      </div>

      <Card
        title={T('互通驗證者集合', 'Interop validator set')}
        sub={T(`最低保證金 ${fmtFer(MIN_VALIDATOR_BOND)} FER · 累計罰沒 ${fmtFer(state.totalSlashed)} FER`, `Min bond ${fmtFer(MIN_VALIDATOR_BOND)} FER · total slashed ${fmtFer(state.totalSlashed)} FER`)}
      >
        {Object.keys(state.validators).length === 0 ? (
          <Empty>{T('尚無驗證者', 'No validators')}</Empty>
        ) : (
          <Table head={[T('帳戶', 'Account'), T('質押 (FER)', 'Bond (FER)'), T('狀態', 'Status')]}>
            {Object.entries(state.validators).map(([who, bond]) => (
              <tr key={who}>
                <td><Mono>{who}</Mono></td>
                <td className="num strong">{fmtFer(bond)}</td>
                <td>{bond >= MIN_VALIDATOR_BOND ? <Pill tone="good">{T('合格', 'bonded')}</Pill> : <Pill tone="warn">{T('低於門檻', 'under-bonded')}</Pill>}</td>
              </tr>
            ))}
          </Table>
        )}
      </Card>
    </>
  )
}

function RegisterValidator({ call, T }) {
  const [who, setWho] = useState('5FHneW…Bob')
  const [bond, setBond] = useState('250000')
  return (
    <Card title={T('① 登記驗證者', '① Register validator')} sub={T('簽署 · 質押本國 FER', 'Signed · post national FER')}>
      <OpForm name="registerValidator" submitLabel={T('登記並質押', 'Register & bond')} onSubmit={() => call('registerValidator', { who, bond: parseFer(bond) })}>
        <Field label={T('帳戶', 'Account')}>
          <input className="input mono" value={who} onChange={(e) => setWho(e.target.value)} />
        </Field>
        <Field label={T('質押額 (FER)', 'Bond (FER)')} hint={T('須 ≥ 250,000 FER,否則 InsufficientBond', 'must be ≥ 250,000 FER, else InsufficientBond')}>
          <input className="input" value={bond} onChange={(e) => setBond(e.target.value)} inputMode="decimal" />
        </Field>
      </OpForm>
    </Card>
  )
}

function SlashValidator({ state, call, T }) {
  const vals = Object.keys(state.validators)
  const [who, setWho] = useState(vals[0] || '')
  const [amount, setAmount] = useState('50000')
  return (
    <Card title={T('② 罰沒驗證者', '② Slash validator')} sub={T('理事會裁決 · 跨鏈不當行為', 'Council ruling · cross-chain misbehavior')} accent="var(--danger)">
      <OpForm name="slashValidator" submitLabel={T('罰沒', 'Slash')} onSubmit={() => call('slashValidator', { who, amount: parseFer(amount) })}>
        <Field label={T('驗證者', 'Validator')}>
          <select className="input" value={who} onChange={(e) => setWho(e.target.value)}>
            {vals.length === 0 && <option value="">{T('無驗證者', 'none')}</option>}
            {vals.map((v) => <option key={v} value={v}>{v}</option>)}
          </select>
        </Field>
        <Field label={T('罰沒額 (FER)', 'Slash amount (FER)')} hint={T('不可超過現有保證金,否則 SlashExceedsBond', 'cannot exceed bond, else SlashExceedsBond')}>
          <input className="input" value={amount} onChange={(e) => setAmount(e.target.value)} inputMode="decimal" />
        </Field>
      </OpForm>
    </Card>
  )
}
