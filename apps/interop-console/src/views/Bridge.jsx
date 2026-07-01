import React, { useState } from 'react'
import { useChain } from '../store.jsx'
import { Card, Field, OpForm, Table, StatusPill, Mono, Empty, ViewHead, HashInput } from '../ui.jsx'
import { fmtXsu, parseXsu, shortHash, countryLabel, randHash, FOREIGN_COUNTRIES, LOCAL_COUNTRY } from '../format.js'

export default function Bridge() {
  const { state, call, T } = useChain()
  return (
    <>
      <ViewHead
        tag="A"
        zh="信任最小化 GRANDPA 橋接"
        en="Trust-minimized GRANDPA bridge"
        lead={T(
          '輕客戶端的信任根是密碼學 GRANDPA 最終性,而非受託保管人。驗證 ≥ ⅔ 權重的 ed25519 預提交簽章後,才接受跨鏈訊息。',
          "The light client's root of trust is cryptographic GRANDPA finality, never a custodian. Cross-chain messages are accepted only after ed25519 precommits clearing > ⅔ weight verify.",
        )}
      />

      <div className="grid2">
        <SubmitInstruction state={state} call={call} T={T} />
        <VerifyFinality state={state} call={call} T={T} />
        <RotateSet state={state} call={call} T={T} />
        <NetSettle state={state} call={call} T={T} />
      </div>

      <Card title={T('清算指令', 'Clearing instructions')} sub={T('§10 以 XSU 計價', '§10, priced in XSU')}>
        {Object.values(state.instructions).length === 0 ? (
          <Empty>{T('尚無指令', 'No instructions')}</Empty>
        ) : (
          <Table head={['id', T('來源', 'From'), T('目的', 'To'), T('金額 (XSU)', 'Amount (XSU)'), T('明細承諾', 'Detail commitment'), T('狀態', 'Status')]}>
            {Object.values(state.instructions)
              .sort((a, b) => a.id - b.id)
              .map((i) => (
                <tr key={i.id}>
                  <td><Mono>{i.id}</Mono></td>
                  <td>{countryLabel(i.from)}</td>
                  <td>{countryLabel(i.to)}</td>
                  <td className="num">{fmtXsu(i.amount)}</td>
                  <td><Mono title={i.detailCommitment}>{shortHash(i.detailCommitment)}</Mono></td>
                  <td><StatusPill status={i.status} /></td>
                </tr>
              ))}
          </Table>
        )}
      </Card>

      <div className="grid2">
        <Card title={T('輕客戶端最終化區塊頭', 'Light-client finalized heads')}>
          {Object.keys(state.finalizedHeads).length === 0 ? (
            <Empty>{T('尚無最終化區塊頭', 'No finalized heads')}</Empty>
          ) : (
            <Table head={[T('鏈', 'Chain'), T('區塊號', 'Number'), T('區塊雜湊', 'Hash')]}>
              {Object.entries(state.finalizedHeads).map(([c, h]) => (
                <tr key={c}>
                  <td>{countryLabel(c)}</td>
                  <td className="num">#{h.number.toLocaleString()}</td>
                  <td><Mono title={h.hash}>{shortHash(h.hash)}</Mono></td>
                </tr>
              ))}
            </Table>
          )}
        </Card>

        <Card title={T('授權集合', 'GRANDPA authority sets')}>
          {Object.keys(state.authoritySets).length === 0 ? (
            <Empty>{T('尚無授權集合', 'No authority sets')}</Empty>
          ) : (
            <Table head={[T('鏈', 'Chain'), 'set_id', T('授權者數', 'Authorities')]}>
              {Object.entries(state.authoritySets).map(([c, s]) => (
                <tr key={c}>
                  <td>{countryLabel(c)}</td>
                  <td className="num">{s.setId}</td>
                  <td className="num">{s.authorities.length}</td>
                </tr>
              ))}
            </Table>
          )}
        </Card>
      </div>

      <Card title={T('雙邊淨部位', 'Bilateral net positions')} sub={T('§10 / §11.3 多邊淨額清算', '§10 / §11.3 multilateral netting')}>
        {Object.keys(state.netPositions).length === 0 ? (
          <Empty>{T('尚無淨部位', 'No net positions')}</Empty>
        ) : (
          <Table head={[T('來源', 'From'), T('目的', 'To'), T('淨額 (XSU)', 'Net (XSU)')]}>
            {Object.entries(state.netPositions).map(([k, v]) => {
              const [from, to] = k.split('~')
              return (
                <tr key={k}>
                  <td>{countryLabel(from)}</td>
                  <td>{countryLabel(to)}</td>
                  <td className="num strong">{fmtXsu(v)}</td>
                </tr>
              )
            })}
          </Table>
        )}
      </Card>
    </>
  )
}

function SubmitInstruction({ call, T }) {
  const [from, setFrom] = useState('JP')
  const [to, setTo] = useState('TW')
  const [amount, setAmount] = useState('48500')
  const [commit, setCommit] = useState(randHash())
  return (
    <Card title={T('① 提交清算指令', '① Submit clearing instruction')} sub={T('中繼者 · XSU 計價', 'Relayer · in XSU')}>
      <OpForm
        name="submitInstruction"
        submitLabel={T('提交', 'Submit')}
        onSubmit={() => call('submitInstruction', { from, to, amount: parseXsu(amount), detailCommitment: commit })}
      >
        <div className="row2">
          <Field label={T('來源國', 'From')}>
            <select className="input" value={from} onChange={(e) => setFrom(e.target.value)}>
              {[...FOREIGN_COUNTRIES, LOCAL_COUNTRY].map((c) => <option key={c}>{c}</option>)}
            </select>
          </Field>
          <Field label={T('目的國', 'To')}>
            <select className="input" value={to} onChange={(e) => setTo(e.target.value)}>
              {[LOCAL_COUNTRY, ...FOREIGN_COUNTRIES].map((c) => <option key={c}>{c}</option>)}
            </select>
          </Field>
        </div>
        <Field label={T('金額 (XSU)', 'Amount (XSU)')}>
          <input className="input" value={amount} onChange={(e) => setAmount(e.target.value)} inputMode="decimal" />
        </Field>
        <Field label={T('明細承諾 (無個資)', 'Detail commitment (no PII)')}>
          <HashInput value={commit} onChange={setCommit} onRoll={() => setCommit(randHash())} />
        </Field>
      </OpForm>
    </Card>
  )
}

function VerifyFinality({ state, call, T }) {
  const { live } = useChain()
  const pending = Object.values(state.instructions).filter((i) => i.status === 'Pending')
  const [id, setId] = useState(pending[0]?.id ?? 0)
  const [num, setNum] = useState(8_421_400)
  const [proof, setProof] = useState('0x')
  return (
    <Card title={T('② 驗證最終性', '② Verify finality')} sub={T('中繼者 · GRANDPA 證明', 'Relayer · GRANDPA proof')}>
      <OpForm
        name="verifyFinality"
        submitLabel={T('驗證', 'Verify')}
        onSubmit={() => call('verifyFinality', { id: Number(id), targetNumber: Number(num), finalityProofHex: proof })}
      >
        <Field label={T('待驗指令 id', 'Pending instruction id')}>
          <select className="input" value={id} onChange={(e) => setId(e.target.value)}>
            {pending.length === 0 && <option value="">{T('無待驗指令', 'none pending')}</option>}
            {pending.map((i) => (
              <option key={i.id} value={i.id}>
                #{i.id} · {i.from}→{i.to} · {fmtXsu(i.amount)} XSU
              </option>
            ))}
          </select>
        </Field>
        {live ? (
          <Field label={T('GRANDPA 最終性證明 (SCALE hex)', 'GRANDPA finality proof (SCALE hex)')} hint={T('grandpa_proveFinality 重整後的 GrandpaFinalityProof', 'reshaped GrandpaFinalityProof bytes')}>
            <input className="input mono" value={proof} onChange={(e) => setProof(e.target.value)} spellCheck={false} />
          </Field>
        ) : (
          <Field label={T('來源鏈最終化區塊號', 'Source finalized block #')} hint={T('須單調遞增,否則 StaleFinality', 'must be monotonic, else StaleFinality')}>
            <input className="input" value={num} onChange={(e) => setNum(e.target.value)} inputMode="numeric" />
          </Field>
        )}
      </OpForm>
    </Card>
  )
}

function RotateSet({ state, call, T }) {
  const { live } = useChain()
  const chains = Object.keys(state.authoritySets)
  const [country, setCountry] = useState(chains[0] || 'JP')
  const cur = state.authoritySets[country]
  const [num, setNum] = useState(8_500_000)
  const [proof, setProof] = useState('0x')
  return (
    <Card title={T('③ 換屆授權集合', '③ Rotate authority set')} sub={T('理事會 · 換屆交接', 'Council · set handoff')}>
      <OpForm
        name="rotateAuthoritySet"
        submitLabel={T('換屆', 'Rotate')}
        onSubmit={() =>
          call('rotateAuthoritySet', { country, newSetId: (cur?.setId ?? 0) + 1, targetNumber: Number(num), finalityProofHex: proof })
        }
      >
        <Field label={T('鏈', 'Chain')}>
          <select className="input" value={country} onChange={(e) => setCountry(e.target.value)}>
            {chains.map((c) => <option key={c}>{c}</option>)}
          </select>
        </Field>
        <Field label={T('新 set_id', 'New set_id')} hint={T('須為當前 +1,沿用現有授權者', 'current + 1; reuses current authorities')}>
          <input className="input" value={(cur?.setId ?? 0) + 1} readOnly />
        </Field>
        {live ? (
          <Field label={T('交接區塊最終性證明 (SCALE hex)', 'Handoff finality proof (SCALE hex)')} hint={T('以當前集合驗證', 'verified under the current set')}>
            <input className="input mono" value={proof} onChange={(e) => setProof(e.target.value)} spellCheck={false} />
          </Field>
        ) : (
          <Field label={T('交接區塊號', 'Handoff block #')}>
            <input className="input" value={num} onChange={(e) => setNum(e.target.value)} inputMode="numeric" />
          </Field>
        )}
      </OpForm>
    </Card>
  )
}

function NetSettle({ state, call, T }) {
  const [window, setWindow] = useState(1)
  const ready = Object.values(state.instructions).filter((i) => i.status === 'FinalityVerified').length
  return (
    <Card title={T('④ 淨額清算', '④ Net & settle')} sub={T('理事會 · 多邊軋差', 'Council · multilateral netting')}>
      <OpForm name="netAndSettle" submitLabel={T('清算', 'Settle')} onSubmit={() => call('netAndSettle', { window: Number(window) })}>
        <Field label={T('清算窗口', 'Clearing window')}>
          <input className="input" value={window} onChange={(e) => setWindow(e.target.value)} inputMode="numeric" />
        </Field>
        <div className="hintbox small muted">
          {T(
            `本窗口將軋差 ${ready} 筆「已驗證最終性」指令進入淨部位。`,
            `This window will net ${ready} FinalityVerified instruction(s) into positions.`,
          )}
        </div>
      </OpForm>
    </Card>
  )
}
