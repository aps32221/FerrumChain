import React, { createContext, useContext, useState, useCallback, useMemo } from 'react'
import { applyCall, initialState, isChainError, CALLS, PALLET_INDEX } from './chain.js'
import { toTxArgs } from './encode.js'
import { connectChain, makeSigner, addressOf, submitCall } from './rpc.js'

const Ctx = createContext(null)

export function StoreProvider({ children }) {
  const [state, setState] = useState(initialState)
  const [lang, setLang] = useState('zh')
  const [log, setLog] = useState([]) // newest first
  const [toast, setToast] = useState(null)

  // Live-connection state. When `conn` + `pair` are set, calls submit real
  // extrinsics via author_submitExtrinsic; otherwise they run the offline model.
  const [conn, setConn] = useState(null) // { rpc, genesis, specVersion, ... }
  const [pair, setPair] = useState(null)
  const [account, setAccount] = useState(null) // ss58 address
  const [connecting, setConnecting] = useState(false)
  const live = !!(conn && pair)

  const T = useCallback((zh, en) => (lang === 'zh' ? zh : en), [lang])

  const pushEvents = useCallback((entries) => {
    setLog((l) => [...entries.reverse(), ...l].slice(0, 200))
  }, [])

  // Connect to a node and load a signer (seed/URI, e.g. //Alice on --dev).
  const connect = useCallback(async (endpoint, accountUri) => {
    setConnecting(true)
    try {
      const c = await connectChain(endpoint)
      const p = await makeSigner(accountUri)
      setConn(c)
      setPair(p)
      setAccount(addressOf(p))
      setToast({
        ok: true,
        zh: `✓ 已連線 ${c.chainName} · spec ${c.specVersion}`,
        en: `✓ Connected to ${c.chainName} · spec ${c.specVersion}`,
      })
      return true
    } catch (e) {
      setToast({ ok: false, code: 'ConnectFailed', zh: `✗ 連線失敗 — ${e.message}`, en: `✗ Connect failed — ${e.message}` })
      return false
    } finally {
      setConnecting(false)
    }
  }, [])

  const disconnect = useCallback(() => {
    try {
      conn?.rpc?.close()
    } catch {}
    setConn(null)
    setPair(null)
    setAccount(null)
    setToast({ ok: true, zh: '已離線 — 切回模擬模式', en: 'Disconnected — back to simulation' })
  }, [conn])

  // Dispatch a call. Live: SCALE-encode + sign + author_submitExtrinsic, then
  // optimistically mirror the change locally. Offline: run the faithful model.
  const call = useCallback(
    async (name, uiArgs) => {
      const meta = CALLS[name]
      if (conn && pair) {
        try {
          const txArgs = toTxArgs(name, uiArgs, state)
          const res = await submitCall(conn, pair, name, txArgs)
          setToast({
            ok: true,
            zh: `✓ ${meta.zh} · 已提交`,
            en: `✓ ${meta.en} · submitted`,
            call: { name, pallet: PALLET_INDEX, index: meta.index },
            hash: res.hash,
          })
          pushEvents([{ block: 'live', event: 'Submitted', data: { hash: res.hash }, call: { name, pallet: PALLET_INDEX, index: meta.index } }])
          // best-effort optimistic local mirror so tables keep evolving
          try {
            setState(applyCall(state, name, uiArgs).state)
          } catch {}
          return true
        } catch (e) {
          setToast({ ok: false, code: 'SubmitFailed', zh: `✗ 提交失敗 — ${e.message}`, en: `✗ Submit failed — ${e.message}` })
          return false
        }
      }
      // offline simulation
      try {
        const res = applyCall(state, name, uiArgs)
        setState(res.state)
        pushEvents(res.events.map((e) => ({ block: res.state.block, event: e.name, data: e.data, call: res.call })))
        setToast({
          ok: true,
          zh: `✓ ${meta.zh} · 已上鏈 #${res.state.block}`,
          en: `✓ ${meta.en} · included in #${res.state.block}`,
          call: res.call,
        })
        return true
      } catch (e) {
        if (isChainError(e)) setToast({ ok: false, code: e.code, zh: `✗ ${e.code} — ${e.zh}`, en: `✗ ${e.code} — ${e.en}` })
        else setToast({ ok: false, code: 'Error', zh: `✗ ${e.message}`, en: `✗ ${e.message}` })
        return false
      }
    },
    [state, conn, pair, pushEvents],
  )

  const dismissToast = useCallback(() => setToast(null), [])
  const reset = useCallback(() => { setState(initialState()); setLog([]); setToast(null) }, [])

  const value = useMemo(
    () => ({
      state, lang, setLang, T, call, log, toast, dismissToast, reset,
      live, conn, account, connecting, connect, disconnect,
    }),
    [state, lang, T, call, log, toast, dismissToast, reset, live, conn, account, connecting, connect, disconnect],
  )
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>
}

export function useChain() {
  const v = useContext(Ctx)
  if (!v) throw new Error('useChain must be used within StoreProvider')
  return v
}
