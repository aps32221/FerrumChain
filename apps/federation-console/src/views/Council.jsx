import React, { useState } from 'react'
import { useChain } from '../store.jsx'
import { Card, Field, OpForm, Table, Pill, ViewHead } from '../ui.jsx'
import { fmtPct, memberLabel, memberName, MEMBERS, ALL_MEMBERS } from '../format.js'

export default function Council() {
  const { state, call, T, lang } = useChain()
  return (
    <>
      <ViewHead
        tag="11.1"
        zh="條約理事會"
        en="The Treaty Council"
        lead={T(
          '每個成員國一席,由其央行或法定代表擔任;另設中立技術秘書處負責草擬與監測,但無投票權。互通驗證者由理事會核准,並以本國 FER 質押作跨鏈罰沒保證金。',
          'One seat per member, held by its central bank or statutory representative; a neutral technical secretariat drafts and monitors but holds no vote. Interop validators are council-approved and post national FER as a cross-slashable bond.',
        )}
      />

      <div className="grid3">
        <Card title={T('組成', 'Composition')}>
          <p className="muted small">{T('一國一席,由央行或法定代表擔任;中立技術秘書處草擬與監測但無投票權。', 'One seat per member (central bank / statutory rep); a neutral secretariat drafts & monitors with no vote.')}</p>
        </Card>
        <Card title={T('職權', 'Mandate')}>
          <p className="muted small">{T('成員准入/除名、籃子權重、費率、互通驗證者集合、參數升級、爭議仲裁,皆由理事會表決。', 'Membership, basket weights, fees, the interop validator set, parameter upgrades and dispute arbitration are decided by council vote.')}</p>
        </Card>
        <Card title={T('驗證者', 'Validators')}>
          <p className="muted small">{T('互通驗證者為核准名單,以本國 FER 質押作跨鏈罰沒保證金;偽造中繼、審查或雙簽將遭沒收並複審。', 'Interop validators are an approved list posting national FER as a cross-slashable bond; false relays, censorship or equivocation are slashed and reviewed.')}</p>
        </Card>
      </div>

      <Card title={T('理事會席位', 'Council seats')} sub={T('§11.1 一國一席', '§11.1 one seat per nation')}>
        <Table head={[T('成員', 'Member'), T('國名', 'Nation'), 'CBDC', T('籃子權重', 'Weight'), T('席位', 'Seat')]}>
          {ALL_MEMBERS.map((m) => (
            <tr key={m}>
              <td>{memberLabel(m)}</td>
              <td className="small">{memberName(m, lang)}</td>
              <td><span className="mono">{MEMBERS[m]?.etoken}</span></td>
              <td className="num">{state.basketWeights[m] ? fmtPct(state.basketWeights[m]) : '—'}</td>
              <td>{state.members[m] ? <Pill tone="good">{T('在席', 'seated')}</Pill> : <Pill tone="neutral">{T('未在席', 'unseated')}</Pill>}</td>
            </tr>
          ))}
        </Table>
      </Card>

      <div className="grid2">
        <SetSeat call={call} T={T} seat />
        <SetSeat call={call} T={T} />
      </div>
    </>
  )
}

function SetSeat({ call, T, seat }) {
  const [member, setMember] = useState(seat ? 'KR' : 'KR')
  return (
    <Card
      title={seat ? T('① 准入席位(引導)', '① Seat a member (bootstrap)') : T('② 撤除席位(引導)', '② Unseat a member (bootstrap)')}
      sub={T('理事會 · set_membership', 'Council · set_membership')}
    >
      <OpForm
        name="setMembership"
        submitLabel={seat ? T('准入', 'Seat') : T('撤除', 'Unseat')}
        onSubmit={() => call('setMembership', { member, seated: !!seat })}
      >
        <Field label={T('成員', 'Member')} hint={T('正式變更應走治理提案(AdmitMember/RemoveMember)', 'formal changes go via a governance proposal')}>
          <select className="input" value={member} onChange={(e) => setMember(e.target.value)}>
            {ALL_MEMBERS.map((m) => <option key={m}>{m}</option>)}
          </select>
        </Field>
      </OpForm>
    </Card>
  )
}
