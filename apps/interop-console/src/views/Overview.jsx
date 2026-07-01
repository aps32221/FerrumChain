import React from 'react'
import { useChain } from '../store.jsx'
import { Card, Table, Mono, Pill, Empty, ViewHead, StatusPill } from '../ui.jsx'
import { fmtXsu, fmtFer, countryLabel, countryName, shortHash, COUNTRIES } from '../format.js'

export default function Overview() {
  const { state, lang, T } = useChain()
  const instr = Object.values(state.instructions)
  const pending = instr.filter((i) => i.status === 'Pending').length
  const verified = instr.filter((i) => i.status === 'FinalityVerified').length
  const settled = instr.filter((i) => i.status === 'Accepted').length

  const stats = [
    { label: T('橋接鏈', 'Bridged chains'), value: Object.keys(state.finalizedHeads).length, sub: T('輕客戶端最終性', 'light-client finality') },
    { label: T('受認可簽發者', 'Recognized issuers'), value: Object.values(state.trustRegistry).filter((e) => e.active).length, sub: T('信任註冊表', 'trust registry') },
    { label: T('租稅協定', 'Tax treaties'), value: Object.values(state.treaties).filter((t) => t.active).length, sub: T('雙重課稅減免', 'double-tax relief') },
    { label: T('待清算指令', 'Pending instructions'), value: pending, sub: T(`${verified} 已驗證 · ${settled} 已清算`, `${verified} verified · ${settled} settled`) },
    { label: T('互通驗證者', 'Interop validators'), value: Object.keys(state.validators).length, sub: T(`累計罰沒 ${fmtFer(state.totalSlashed)} FER`, `${fmtFer(state.totalSlashed)} FER slashed`) },
  ]

  return (
    <>
      <ViewHead
        zh="總覽 · 主權鏈聯邦"
        en="Overview · sovereign-chain federation"
        lead={T(
          '你正以 TW 鏈操作,透過中立互通層對其他主權鏈做身分解析、ZK 互驗、稅務協調與 XSU 多邊淨額清算。',
          'You are operating the TW chain, resolving identity, ZK-verifying, coordinating tax and clearing XSU net positions against other sovereign chains via the neutral interop layer.',
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

      <div className="grid2">
        <Card title={T('聯邦成員與淨部位', 'Federation members & net positions')} sub={T('§10 以 XSU 計價清算', '§10 cleared in XSU')}>
          <Table head={[T('鏈 / CBDC', 'Chain / CBDC'), T('最終化區塊頭', 'Finalized head'), T('對 TW 淨額 (XSU)', 'Net vs TW (XSU)')]}>
            {Object.keys(COUNTRIES).filter((c) => c !== 'TW').map((c) => {
              const head = state.finalizedHeads[c]
              const inbound = state.netPositions[`${c}~TW`] || 0n
              const outbound = state.netPositions[`TW~${c}`] || 0n
              const net = inbound - outbound
              return (
                <tr key={c}>
                  <td>
                    <div className="cellmain">{countryLabel(c)} <span className="muted small">{countryName(c, lang)}</span></div>
                    <Mono>{COUNTRIES[c].cbdc}</Mono>
                  </td>
                  <td>{head ? <span>#{head.number.toLocaleString()}</span> : <Pill tone="warn">{T('未橋接', 'unbridged')}</Pill>}</td>
                  <td className="num strong">{net === 0n ? '—' : fmtXsu(net)}</td>
                </tr>
              )
            })}
          </Table>
        </Card>

        <Card title={T('近期清算指令', 'Recent clearing instructions')}>
          {instr.length === 0 ? (
            <Empty>{T('尚無指令', 'No instructions')}</Empty>
          ) : (
            <Table head={['id', T('路徑', 'Route'), T('XSU', 'XSU'), T('狀態', 'Status')]}>
              {instr.sort((a, b) => b.id - a.id).slice(0, 7).map((i) => (
                <tr key={i.id}>
                  <td><Mono>{i.id}</Mono></td>
                  <td>{countryLabel(i.from)} → {countryLabel(i.to)}</td>
                  <td className="num">{fmtXsu(i.amount)}</td>
                  <td><StatusPill status={i.status} /></td>
                </tr>
              ))}
            </Table>
          )}
        </Card>
      </div>

      <div className="privacy">
        <span className="privacy-ic">🛡</span>
        <div>
          <strong>{T('隱私不變式(跨境)', 'Privacy invariant (cross-border)')}</strong>
          <p className="muted small">
            {T(
              '跨境傳遞的只有零知識證明、最終性證明與承諾值;明文個資永遠停留在來源國的鏈下加密庫,僅在授權或條約程序下逐筆解密並留稽核軌跡。',
              'Only ZK proofs, finality proofs and commitments cross borders; plaintext PII stays in the source nation\'s off-chain vault, decrypted item-by-item only under authorization, always leaving an audit trail.',
            )}
          </p>
        </div>
      </div>
    </>
  )
}
