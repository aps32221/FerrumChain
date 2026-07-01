import React, { useState } from 'react'
import { useChain } from '../store.jsx'
import { Card, Field, OpForm, Button, Pill, Table, Mono, Empty, ViewHead, HashInput } from '../ui.jsx'
import { StatusPill } from './Overview.jsx'
import { actionDomain, actionLabel, dualMajority, DOMAINS } from '../chain.js'
import { fmtPct, memberLabel, ALL_MEMBERS, randHash } from '../format.js'

const ACTION_TYPES = ['SetParameter', 'AdmitMember', 'RemoveMember', 'SuspendMember', 'Reweight', 'RuntimeUpgrade']

export default function Proposals() {
  const { state, call, T, lang, live } = useChain()
  const proposals = Object.values(state.proposals).sort((a, b) => b.id - a.id)
  return (
    <>
      <ViewHead
        tag="11.2"
        zh="雙重多數表決"
        en="Dual-majority voting"
        lead={T(
          '一般提案通過,須同時滿足兩個維度:(a) 成員數達門檻(主權平等),且 (b) 贊成方合計 XSU 籃子權重達門檻(經濟份量)。通過後進入時間鎖佇列,到期由 on_initialize 自動生效。',
          'A proposal passes only if it clears both axes: (a) a threshold share of members (sovereign equality), and (b) a threshold share of XSU basket weight among the Aye voters (economic weight). It then queues under a timelock and auto-enacts at the ETA via on_initialize.',
        )}
      />

      <ProposeForm call={call} T={T} state={state} />

      {proposals.length === 0 ? (
        <Card><Empty>{T('尚無提案', 'No proposals')}</Empty></Card>
      ) : (
        proposals.map((p) => <ProposalCard key={p.id} p={p} state={state} call={call} T={T} lang={lang} live={live} />)
      )}

      <Card title={T('治理領域與門檻', 'Governance domains & thresholds')} sub={T('§11.2 表', '§11.2 table')}>
        <Table head={[T('領域', 'Domain'), T('門檻', 'Threshold'), T('生效方式', 'Enactment')]}>
          {Object.entries(DOMAINS).map(([k, d]) => (
            <tr key={k}>
              <td>{lang === 'zh' ? d.zh : d.en}</td>
              <td className="num">{d.thresholdPpb === 0 ? '—' : fmtPct(d.thresholdPpb)}</td>
              <td className="small muted">{lang === 'zh' ? d.note.zh : d.note.en}</td>
            </tr>
          ))}
        </Table>
      </Card>
    </>
  )
}

function ProposeForm({ call, T, state }) {
  const [type, setType] = useState('SetParameter')
  const [key, setKey] = useState('fee.relayer')
  const [value, setValue] = useState('25')
  const [member, setMember] = useState('KR')
  const [codeHash, setCodeHash] = useState(randHash())

  const build = () => {
    switch (type) {
      case 'SetParameter': return { type, key, value: Number(value) }
      case 'AdmitMember':
      case 'RemoveMember':
      case 'SuspendMember': return { type, member }
      case 'Reweight': return { type, basket: { ...state.basket, version: state.basket.version + 1 } }
      case 'RuntimeUpgrade': return { type, codeHash }
      default: return { type }
    }
  }
  const domain = actionDomain({ type })

  return (
    <Card title={T('提出聯邦提案', 'Propose a federation action')} sub={T('§11.4 任一理事會成員', '§11.4 any council member')} accent="var(--foil)">
      <OpForm name="propose" submitLabel={T('提案', 'Propose')} onSubmit={() => call('propose', { action: build() })}>
        <div className="row2">
          <Field label={T('動作', 'Action')}>
            <select className="input" value={type} onChange={(e) => setType(e.target.value)}>
              {ACTION_TYPES.map((t) => <option key={t}>{t}</option>)}
            </select>
          </Field>
          <Field label={T('治理領域', 'Domain')} hint={T(`門檻 ${DOMAINS[domain].thresholdPpb ? fmtPct(DOMAINS[domain].thresholdPpb) : '—'} · 時間鎖 ${DOMAINS[domain].timelock} 塊`, `threshold ${DOMAINS[domain].thresholdPpb ? fmtPct(DOMAINS[domain].thresholdPpb) : '—'} · timelock ${DOMAINS[domain].timelock} blk`)}>
            <input className="input" value={domain} readOnly />
          </Field>
        </div>
        {type === 'SetParameter' && (
          <div className="row2">
            <Field label={T('參數鍵', 'Parameter key')}><input className="input" value={key} onChange={(e) => setKey(e.target.value)} /></Field>
            <Field label={T('值', 'Value')}><input className="input" value={value} onChange={(e) => setValue(e.target.value)} inputMode="numeric" /></Field>
          </div>
        )}
        {(type === 'AdmitMember' || type === 'RemoveMember' || type === 'SuspendMember') && (
          <Field label={T('成員', 'Member')}>
            <select className="input" value={member} onChange={(e) => setMember(e.target.value)}>
              {ALL_MEMBERS.map((m) => <option key={m}>{m}</option>)}
            </select>
          </Field>
        )}
        {type === 'Reweight' && (
          <div className="hintbox small muted">{T('將以目前 XSU 籃子(版本 +1)提出再平衡;於「XSU 籃子與準備」分頁編輯籃子。', 'Proposes a reweight to the current XSU basket (version +1); edit the basket in the “XSU basket & reserve” tab.')}</div>
        )}
        {type === 'RuntimeUpgrade' && (
          <Field label={T('WASM 程式碼雜湊', 'WASM code hash')}>
            <HashInput value={codeHash} onChange={setCodeHash} onRoll={() => setCodeHash(randHash())} />
          </Field>
        )}
      </OpForm>
    </Card>
  )
}

function ProposalCard({ p, state, call, T, lang, live }) {
  const [voter, setVoter] = useState(state.localMember)
  const [choice, setChoice] = useState('Aye')
  const domain = actionDomain(p.action)
  const d = DOMAINS[domain]
  const dm = dualMajority(p.votes, state.basketWeights, d.thresholdPpb)
  const thresholdPct = d.thresholdPpb ? fmtPct(d.thresholdPpb) : '—'

  return (
    <Card
      title={`#${p.id} · ${actionLabel(p.action)}`}
      sub={`${lang === 'zh' ? d.zh : d.en} · ${T('門檻', 'threshold')} ${thresholdPct}`}
      badge={<StatusPill status={p.status} />}
    >
      <div className="grid2">
        <div>
          <div className="field-label">{T('票數', 'Ballots')}</div>
          {Object.keys(p.votes).length === 0 ? (
            <Empty>{T('尚無投票', 'No votes yet')}</Empty>
          ) : (
            <div className="chiplist">
              {Object.entries(p.votes).map(([m, v]) => (
                <span key={m} className="votechip">
                  {memberLabel(m)} <Pill tone={v === 'Aye' ? 'good' : v === 'Nay' ? 'bad' : 'neutral'}>{v}</Pill>
                </span>
              ))}
            </div>
          )}

          <div className="dmbox">
            <DmAxis label={T('成員數維度', 'Member axis')} ok={dm.byCount} detail={`${dm.ayes}/${dm.total} = ${fmtPct(dm.ayesCountPpb)} ≥ ${thresholdPct}`} />
            <DmAxis label={T('籃子權重維度', 'Weight axis')} ok={dm.byWeight} detail={`${fmtPct(dm.ayesWeightPpb)} ≥ ${thresholdPct}`} />
            <div className="dm-result">
              {dm.pass
                ? <Pill tone="good">{T('雙重多數通過', 'Dual-majority passes')}</Pill>
                : <Pill tone="warn">{T('尚未通過', 'Not yet passing')}</Pill>}
            </div>
          </div>
        </div>

        <div>
          {p.status === 'Open' && (
            <div className="voteform">
              <div className="field-label">{T('投票(以理事會席位)', 'Cast a ballot (as a council seat)')}</div>
              <div className="row2">
                <select className="input" value={voter} onChange={(e) => setVoter(e.target.value)} disabled={live} title={live ? T('連線時以本席位投票', 'live: votes as the connected seat') : ''}>
                  {ALL_MEMBERS.filter((m) => state.members[m]).map((m) => <option key={m}>{m}</option>)}
                </select>
                <select className="input" value={choice} onChange={(e) => setChoice(e.target.value)}>
                  <option>Aye</option><option>Nay</option><option>Abstain</option>
                </select>
              </div>
              <div className="opform-foot">
                <Mono className="dim">Federation.vote · 14/1</Mono>
                <div style={{ display: 'flex', gap: 8 }}>
                  <Button kind="ghost" onClick={() => call('vote', { id: p.id, member: voter, vote: choice })}>{T('投票', 'Vote')}</Button>
                  <Button onClick={() => call('close', { id: p.id })}>{T('關閉計票', 'Close')}</Button>
                </div>
              </div>
            </div>
          )}
          {p.status === 'Queued' && (
            <div className="queuebox">
              <Pill tone="info">{T('時間鎖佇列', 'Timelocked')}</Pill>
              <p className="small muted">{T(`ETA #${p.eta} · 到期由 on_initialize 自動生效`, `ETA #${p.eta} · auto-enacts at the ETA via on_initialize`)}</p>
              {!live && <Button onClick={() => call('enactQueued', { id: p.id })}>{T('⏩ 快轉至 ETA 生效', '⏩ Fast-forward to ETA & enact')}</Button>}
            </div>
          )}
          {p.status === 'Enacted' && <Pill tone="good">{T('已生效', 'Enacted')}</Pill>}
        </div>
      </div>
    </Card>
  )
}

function DmAxis({ label, ok, detail }) {
  return (
    <div className="dm-axis">
      <span className={`dm-dot ${ok ? 'ok' : ''}`}>{ok ? '✓' : '✗'}</span>
      <span className="dm-label">{label}</span>
      <span className="dm-detail mono">{detail}</span>
    </div>
  )
}
