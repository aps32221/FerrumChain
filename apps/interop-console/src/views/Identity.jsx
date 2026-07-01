import React, { useState } from 'react'
import { useChain } from '../store.jsx'
import { resolveDid } from '../chain.js'
import { Card, Field, OpForm, Table, Mono, Pill, Empty, ViewHead, HashInput } from '../ui.jsx'
import { shortHash, countryLabel, randHash, pseudoHash, FOREIGN_COUNTRIES } from '../format.js'

export default function Identity() {
  const { state, call, T } = useChain()
  return (
    <>
      <ViewHead
        tag="B"
        zh="跨國身分互認與 ZK 互驗"
        en="Cross-border identity & ZK verification"
        lead={T(
          '聯邦維護鏈上信任註冊表,記錄各國受認證簽發者公鑰;選擇性揭露證明可在他國鏈上以註冊金鑰驗證——個資不跨境。',
          'The federation keeps an on-chain trust registry of accredited issuer keys; selective-disclosure proofs verify on a foreign chain using registered keys — PII never crosses the border.',
        )}
      />

      <div className="grid2">
        <RegisterIssuer call={call} T={T} />
        <RegisterVk state={state} call={call} T={T} />
        <ResolveDid state={state} T={T} />
        <VerifyForeignProof state={state} call={call} T={T} />
      </div>

      <Card title={T('信任註冊表', 'Trust registry')} sub={T('§09 跨國身分互認', '§09 mutual issuer recognition')}>
        {Object.values(state.trustRegistry).length === 0 ? (
          <Empty>{T('尚無受認可簽發者', 'No recognized issuers')}</Empty>
        ) : (
          <Table head={[T('國家', 'Country'), T('簽發者公鑰雜湊', 'Issuer key hash'), T('範圍', 'Scope'), 'VK', T('狀態', 'Status')]}>
            {Object.values(state.trustRegistry).map((e) => {
              const hasVk = !!state.verifyingKeys[`${e.country}~${e.issuerKeyHash}`]
              return (
                <tr key={e.country + e.issuerKeyHash}>
                  <td>{countryLabel(e.country)}</td>
                  <td><Mono title={e.issuerKeyHash}>{shortHash(e.issuerKeyHash)}</Mono></td>
                  <td><Mono>{e.scope}</Mono></td>
                  <td>{hasVk ? <Pill tone="good">{T('已註冊', 'registered')}</Pill> : <Pill tone="warn">{T('缺', 'missing')}</Pill>}</td>
                  <td>{e.active ? <Pill tone="good">active</Pill> : <Pill tone="bad">inactive</Pill>}</td>
                </tr>
              )
            })}
          </Table>
        )}
      </Card>

      <Card title={T('已用 nullifier', 'Spent nullifiers')} sub={T('§09 跨境證明防重放', '§09 cross-border replay protection')}>
        {Object.keys(state.usedNullifiers).length === 0 ? (
          <Empty>{T('尚無已驗證的跨境證明', 'No cross-border proofs verified yet')}</Empty>
        ) : (
          <div className="chiplist">
            {Object.keys(state.usedNullifiers).map((n) => (
              <Mono key={n} title={n}>{shortHash(n, 8, 6)}</Mono>
            ))}
          </div>
        )}
      </Card>
    </>
  )
}

function RegisterIssuer({ call, T }) {
  const [country, setCountry] = useState('JP')
  const [key, setKey] = useState(pseudoHash('jp-issuer-nta'))
  const [scope, setScope] = useState('id')
  return (
    <Card title={T('① 登記簽發者', '① Register issuer')} sub={T('理事會 · 信任註冊表', 'Council · trust registry')}>
      <OpForm
        name="registerIssuer"
        submitLabel={T('登記', 'Register')}
        onSubmit={() => call('registerIssuer', { country, issuerKeyHash: key, scope, active: true })}
      >
        <div className="row2">
          <Field label={T('國家', 'Country')}>
            <select className="input" value={country} onChange={(e) => setCountry(e.target.value)}>
              {FOREIGN_COUNTRIES.map((c) => <option key={c}>{c}</option>)}
            </select>
          </Field>
          <Field label={T('範圍', 'Scope')}>
            <input className="input" value={scope} onChange={(e) => setScope(e.target.value)} />
          </Field>
        </div>
        <Field label={T('簽發者公鑰雜湊', 'Issuer key hash')}>
          <HashInput value={key} onChange={setKey} onRoll={() => setKey(randHash())} />
        </Field>
      </OpForm>
    </Card>
  )
}

function RegisterVk({ state, call, T }) {
  const issuers = Object.values(state.trustRegistry)
  const [sel, setSel] = useState(issuers[0] ? `${issuers[0].country}~${issuers[0].issuerKeyHash}` : '')
  const [vk, setVk] = useState(randHash(48))
  const [country, key] = sel.split('~')
  return (
    <Card title={T('② 登記驗證金鑰', '② Register verifying key')} sub={T('理事會 · Groth16 VK', 'Council · Groth16 VK')}>
      <OpForm
        name="registerIssuerVk"
        submitLabel={T('登記 VK', 'Register VK')}
        onSubmit={() => call('registerIssuerVk', { country, issuerKeyHash: key, vk })}
      >
        <Field label={T('受認可簽發者', 'Recognized issuer')}>
          <select className="input" value={sel} onChange={(e) => setSel(e.target.value)}>
            {issuers.map((e) => (
              <option key={e.country + e.issuerKeyHash} value={`${e.country}~${e.issuerKeyHash}`}>
                {e.country} · {shortHash(e.issuerKeyHash, 8, 4)}
              </option>
            ))}
          </select>
        </Field>
        <Field label={T('Groth16 驗證金鑰 (arkworks bytes)', 'Groth16 VK (arkworks bytes)')}>
          <HashInput value={vk} onChange={setVk} onRoll={() => setVk(randHash(48))} />
        </Field>
      </OpForm>
    </Card>
  )
}

function ResolveDid({ state, T }) {
  const [did, setDid] = useState('did:fer:jp:z6MkfJPcitizen91')
  const res = resolveDid(state, did)
  return (
    <Card title={T('③ 跨鏈 DID 解析', '③ Cross-chain DID resolution')} sub={T('唯讀 · 通用解析器', 'Read-only · universal resolver')}>
      <Field label={T('DID', 'DID')} hint={T('識別碼天生帶有來源鏈標記', 'identifiers carry a source-chain tag')}>
        <input className="input mono" value={did} onChange={(e) => setDid(e.target.value)} spellCheck={false} />
      </Field>
      <div className="resolvebox">
        <DidResult res={res} T={T} />
      </div>
      <div className="hintbox small muted">
        {T('試試:', 'Try: ')}
        {['did:fer:tw:z6MkrTWcitizen42', 'did:fer:tw:unknown00', 'did:fer:de:z6MkfDEcitizen'].map((d) => (
          <button key={d} type="button" className="linklike" onClick={() => setDid(d)}>{d}</button>
        ))}
      </div>
    </Card>
  )
}

function DidResult({ res, T }) {
  if (res.kind === 'Invalid') return <Pill tone="bad">{T('格式錯誤', 'Malformed DID')}</Pill>
  if (res.kind === 'Local')
    return (
      <span className="resline">
        <Pill tone="good">Local</Pill> {T('本鏈已錨定', 'anchored locally')} · <Mono title={res.docHash}>{shortHash(res.docHash)}</Mono>
      </span>
    )
  if (res.kind === 'LocalUnknown')
    return (
      <span className="resline">
        <Pill tone="warn">LocalUnknown</Pill> {T('本鏈標記但未錨定', 'local-tagged but not anchored')}
      </span>
    )
  return (
    <span className="resline">
      <Pill tone="info">Foreign</Pill> {countryLabel(res.country)} ·{' '}
      {res.recognized ? <Pill tone="good">{T('已互認', 'recognized')}</Pill> : <Pill tone="bad">{T('未互認', 'not recognized')}</Pill>}
    </span>
  )
}

function VerifyForeignProof({ state, call, T }) {
  const { live } = useChain()
  const issuers = Object.values(state.trustRegistry)
  const [sel, setSel] = useState(issuers[0] ? `${issuers[0].country}~${issuers[0].issuerKeyHash}` : '')
  const [nullifier, setNullifier] = useState(randHash())
  const [proof, setProof] = useState('0x')
  const [issuerCommitment, setIssuerCommitment] = useState(randHash())
  const [threshold, setThreshold] = useState(18)
  const [country, key] = sel.split('~')
  return (
    <Card title={T('④ 跨境 ZK 互驗', '④ Verify foreign ZK proof')} sub={T('中繼者 · 個資不跨境', 'Relayer · no PII crosses')}>
      <OpForm
        name="verifyForeignProof"
        submitLabel={T('驗證', 'Verify')}
        onSubmit={() =>
          call('verifyForeignProof', {
            country, issuerKeyHash: key, nullifier,
            proofHex: proof, issuerCommitmentHex: issuerCommitment, threshold: Number(threshold),
          })
        }
      >
        <Field label={T('受認可簽發者', 'Recognized issuer')}>
          <select className="input" value={sel} onChange={(e) => setSel(e.target.value)}>
            {issuers.map((e) => (
              <option key={e.country + e.issuerKeyHash} value={`${e.country}~${e.issuerKeyHash}`}>
                {e.country} · {shortHash(e.issuerKeyHash, 8, 4)}
              </option>
            ))}
          </select>
        </Field>
        {live && (
          <>
            <Field label={T('Groth16 證明 (arkworks hex)', 'Groth16 proof (arkworks hex)')}>
              <input className="input mono" value={proof} onChange={(e) => setProof(e.target.value)} spellCheck={false} />
            </Field>
            <div className="row2">
              <Field label={T('簽發者承諾', 'Issuer commitment')}>
                <HashInput value={issuerCommitment} onChange={setIssuerCommitment} onRoll={() => setIssuerCommitment(randHash())} />
              </Field>
              <Field label={T('門檻 (述詞)', 'Threshold (predicate)')}>
                <input className="input" value={threshold} onChange={(e) => setThreshold(e.target.value)} inputMode="numeric" />
              </Field>
            </div>
          </>
        )}
        <Field label={T('一次性 nullifier', 'One-time nullifier')} hint={T('重複提交將觸發 ProofReplayed', 'resubmitting triggers ProofReplayed')}>
          <HashInput value={nullifier} onChange={setNullifier} onRoll={() => setNullifier(randHash())} />
        </Field>
      </OpForm>
    </Card>
  )
}
