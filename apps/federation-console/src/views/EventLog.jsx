import React from 'react'
import { useChain } from '../store.jsx'
import { Card, Mono, Empty, ViewHead, Pill } from '../ui.jsx'
import { MODULE_NAME } from '../chain.js'
import { fmtXsu, shortHash, memberLabel, fmtPct } from '../format.js'

function renderData(data) {
  return Object.entries(data).map(([k, v]) => {
    let display = v
    if (k === 'cbdc_amount' || k === 'xsu_minted' || k === 'xsu_burned') display = `${fmtXsu(v)} XSU`
    else if (k === 'amount') display = `${fmtXsu(typeof v === 'object' ? v.value ?? v : v)} XSU`
    else if (typeof v === 'string' && v.startsWith('0x')) display = shortHash(v, 8, 6)
    else if (k === 'member' || k === 'by' || k === 'from' || k === 'to') display = memberLabel(v)
    else if (k === 'code_hash' || k === 'digest') display = shortHash(v, 8, 6)
    return (
      <span className="kv" key={k}>
        <span className="kv-k">{k}</span>
        <span className="kv-v"><Mono>{String(display)}</Mono></span>
      </span>
    )
  })
}

export default function EventLog() {
  const { log, T } = useChain()
  return (
    <>
      <ViewHead
        zh="事件記錄"
        en="Event log"
        lead={T(
          '每次操作對應一筆 pallet-federation 外部呼叫,以下為其發出的鏈上事件(最新在上)。',
          'Each operation maps to a pallet-federation extrinsic; below are the on-chain events it emitted (newest first).',
        )}
      />
      <Card>
        {log.length === 0 ? (
          <Empty>{T('尚無事件 — 從各操作面板送出一筆呼叫。', 'No events yet — submit a call from any panel.')}</Empty>
        ) : (
          <ul className="eventlog">
            {log.map((e, i) => (
              <li key={i}>
                <span className="ev-block">#{e.block}</span>
                <span className="ev-name"><Pill tone="info">{e.event}</Pill></span>
                <span className="ev-call"><Mono>{MODULE_NAME}.{e.call.name}</Mono><span className="dim mono"> · {e.call.pallet}/{e.call.index}</span></span>
                <span className="ev-data">{renderData(e.data)}</span>
              </li>
            ))}
          </ul>
        )}
      </Card>
    </>
  )
}
