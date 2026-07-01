import React, { useState } from 'react'
import { useChain } from '../store.jsx'
import { Card, Field, OpForm, Table, Mono, Pill, Empty, ViewHead, HashInput } from '../ui.jsx'
import { shortHash, countryLabel, fmtXsu, parseXsu, randHash, ALL_COUNTRIES as ALL, FOREIGN_COUNTRIES } from '../format.js'

export default function Tax() {
  const { state, call, T } = useChain()
  return (
    <>
      <ViewHead
        tag="C"
        zh="跨境稅務協調"
        en="Cross-border tax coordination"
        lead={T(
          '鏈上租稅協定登記表自動套用稅率與扣抵;跨境發票可跨鏈互認;OSS 一站式 VAT 依消費地原則分配稅收。',
          'An on-chain tax-treaty registry auto-applies rates and credits; e-invoices recognize across chains; One-Stop-Shop VAT allocates revenue by the destination principle.',
        )}
      />

      <div className="grid2">
        <RegisterTreaty call={call} T={T} />
        <RecognizeInvoice state={state} call={call} T={T} />
        <OssRegister call={call} T={T} />
        <OssReport state={state} call={call} T={T} />
      </div>

      <Card title={T('租稅協定登記表', 'Tax-treaty registry')} sub={T('§09 雙重課稅減免', '§09 double-tax relief')}>
        {Object.values(state.treaties).length === 0 ? (
          <Empty>{T('尚無協定', 'No treaties')}</Empty>
        ) : (
          <Table head={[T('甲方', 'Party A'), T('乙方', 'Party B'), T('扣繳上限', 'Withholding cap'), T('減免方式', 'Relief method'), T('狀態', 'Status')]}>
            {Object.values(state.treaties).map((t) => (
              <tr key={t.a + t.b}>
                <td>{countryLabel(t.a)}</td>
                <td>{countryLabel(t.b)}</td>
                <td className="num">{t.withholdingCap}%</td>
                <td><Pill tone="neutral">{t.method}</Pill></td>
                <td>{t.active ? <Pill tone="good">active</Pill> : <Pill tone="bad">inactive</Pill>}</td>
              </tr>
            ))}
          </Table>
        )}
      </Card>

      <div className="grid2">
        <Card title={T('互認的外國電子發票', 'Recognized foreign e-invoices')}>
          {Object.keys(state.recognizedInvoices).length === 0 ? (
            <Empty>{T('尚無互認發票', 'No recognized invoices')}</Empty>
          ) : (
            <Table head={[T('來源國', 'Source'), T('發票雜湊', 'Invoice hash')]}>
              {Object.keys(state.recognizedInvoices).map((k) => {
                const [country, hash] = k.split('~')
                return (
                  <tr key={k}>
                    <td>{countryLabel(country)}</td>
                    <td><Mono title={hash}>{shortHash(hash)}</Mono></td>
                  </tr>
                )
              })}
            </Table>
          )}
        </Card>

        <Card title={T('OSS VAT 登記', 'OSS VAT registrations')}>
          {Object.values(state.ossRegistrations).length === 0 ? (
            <Empty>{T('尚無登記', 'No registrations')}</Empty>
          ) : (
            <Table head={[T('供應者 DID', 'Supplier DID'), T('登記國', 'Home'), 'VAT', T('狀態', 'Status')]}>
              {Object.values(state.ossRegistrations).map((r) => (
                <tr key={r.subject}>
                  <td><Mono title={r.subject}>{shortHash(r.subject, 14, 4)}</Mono></td>
                  <td>{countryLabel(r.home)}</td>
                  <td><Mono title={r.vatIdCommitment}>{shortHash(r.vatIdCommitment, 8, 4)}</Mono></td>
                  <td>{r.active ? <Pill tone="good">active</Pill> : <Pill tone="bad">inactive</Pill>}</td>
                </tr>
              ))}
            </Table>
          )}
        </Card>
      </div>
    </>
  )
}

function RegisterTreaty({ call, T }) {
  const [a, setA] = useState('TW')
  const [b, setB] = useState('DE')
  const [cap, setCap] = useState('10')
  const [method, setMethod] = useState('Credit')
  return (
    <Card title={T('① 登記租稅協定', '① Register tax treaty')} sub={T('理事會 · 雙重課稅減免', 'Council · double-tax relief')}>
      <OpForm
        name="registerTreaty"
        submitLabel={T('登記', 'Register')}
        onSubmit={() => call('registerTreaty', { a, b, withholdingCap: Number(cap), method, active: true })}
      >
        <div className="row2">
          <Field label={T('甲方', 'Party A')}>
            <select className="input" value={a} onChange={(e) => setA(e.target.value)}>{ALL.map((c) => <option key={c}>{c}</option>)}</select>
          </Field>
          <Field label={T('乙方', 'Party B')}>
            <select className="input" value={b} onChange={(e) => setB(e.target.value)}>{ALL.map((c) => <option key={c}>{c}</option>)}</select>
          </Field>
        </div>
        <div className="row2">
          <Field label={T('扣繳上限 %', 'Withholding cap %')}>
            <input className="input" value={cap} onChange={(e) => setCap(e.target.value)} inputMode="numeric" />
          </Field>
          <Field label={T('減免方式', 'Relief method')}>
            <select className="input" value={method} onChange={(e) => setMethod(e.target.value)}>
              <option>Credit</option>
              <option>Exemption</option>
            </select>
          </Field>
        </div>
      </OpForm>
    </Card>
  )
}

function RecognizeInvoice({ state, call, T }) {
  const bridged = Object.keys(state.finalizedHeads)
  const [country, setCountry] = useState(bridged[0] || 'JP')
  const [hash, setHash] = useState(randHash())
  const hasHead = !!state.finalizedHeads[country]
  return (
    <Card title={T('② 互認電子發票', '② Recognize e-invoice')} sub={T('中繼者 · 需已建立最終性', 'Relayer · requires finality')}>
      <OpForm
        name="recognizeForeignInvoice"
        submitLabel={T('互認', 'Recognize')}
        onSubmit={() => call('recognizeForeignInvoice', { country, invoiceHash: hash })}
      >
        <Field label={T('來源國', 'Source country')} hint={hasHead ? T('已有最終化區塊頭 ✓', 'finalized head present ✓') : T('尚無最終化區塊頭 → NoFinalizedHead', 'no finalized head → NoFinalizedHead')}>
          <select className="input" value={country} onChange={(e) => setCountry(e.target.value)}>
            {FOREIGN_COUNTRIES.map((c) => <option key={c}>{c}</option>)}
          </select>
        </Field>
        <Field label={T('發票雜湊', 'Invoice hash')}>
          <HashInput value={hash} onChange={setHash} onRoll={() => setHash(randHash())} />
        </Field>
      </OpForm>
    </Card>
  )
}

function OssRegister({ call, T }) {
  const [subject, setSubject] = useState('did:fer:tw:z6MkqTWdigitalco')
  const [home, setHome] = useState('TW')
  const [vat, setVat] = useState(randHash())
  return (
    <Card title={T('③ OSS 登記', '③ OSS VAT register')} sub={T('供應者 · 單一入口', 'Supplier · single entry')}>
      <OpForm
        name="ossRegister"
        submitLabel={T('登記', 'Register')}
        onSubmit={() => call('ossRegister', { subject, home, vatIdCommitment: vat, active: true })}
      >
        <Field label={T('供應者 DID', 'Supplier DID')}>
          <input className="input mono" value={subject} onChange={(e) => setSubject(e.target.value)} spellCheck={false} />
        </Field>
        <div className="row2">
          <Field label={T('登記國', 'Home')}>
            <select className="input" value={home} onChange={(e) => setHome(e.target.value)}>{ALL.map((c) => <option key={c}>{c}</option>)}</select>
          </Field>
          <Field label={T('VAT 承諾 (無個資)', 'VAT commitment (no PII)')}>
            <HashInput value={vat} onChange={setVat} onRoll={() => setVat(randHash())} />
          </Field>
        </div>
      </OpForm>
    </Card>
  )
}

function OssReport({ state, call, T }) {
  const regs = Object.values(state.ossRegistrations)
  const [subject, setSubject] = useState(regs[0]?.subject || '')
  const [to, setTo] = useState('DE')
  const [amount, setAmount] = useState('21000')
  const [commit, setCommit] = useState(randHash())
  return (
    <Card title={T('④ OSS 跨境申報', '④ OSS cross-border report')} sub={T('供應者 · 依消費地分配', 'Supplier · destination principle')}>
      <OpForm
        name="ossReport"
        submitLabel={T('申報', 'Report')}
        onSubmit={() => call('ossReport', { subject, to, amount: parseXsu(amount), detailCommitment: commit })}
      >
        <Field label={T('供應者 DID', 'Supplier DID')}>
          <select className="input" value={subject} onChange={(e) => setSubject(e.target.value)}>
            {regs.length === 0 && <option value="">{T('需先登記', 'register first')}</option>}
            {regs.map((r) => (
              <option key={r.subject} value={r.subject}>{shortHash(r.subject, 16, 4)} ({r.home})</option>
            ))}
          </select>
        </Field>
        <div className="row2">
          <Field label={T('消費地 (目的國)', 'Destination')}>
            <select className="input" value={to} onChange={(e) => setTo(e.target.value)}>{ALL.map((c) => <option key={c}>{c}</option>)}</select>
          </Field>
          <Field label={T('稅額 (XSU)', 'Tax (XSU)')}>
            <input className="input" value={amount} onChange={(e) => setAmount(e.target.value)} inputMode="decimal" />
          </Field>
        </div>
        <Field label={T('明細承諾', 'Detail commitment')} hint={T('產生 from=登記國 的清算指令,進入淨額清算', 'creates a from=home clearing instruction into netting')}>
          <HashInput value={commit} onChange={setCommit} onRoll={() => setCommit(randHash())} />
        </Field>
      </OpForm>
    </Card>
  )
}
