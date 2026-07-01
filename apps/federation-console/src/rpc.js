// rpc.js — live submission path. A tiny JSON-RPC client over a native WebSocket
// plus a hand-assembled v4 signed extrinsic, mirroring the C# SDK's
// FerrumClient.SignAndSendAsync byte-for-byte (immortal era, zero tip, the
// runtime's 8-field SignedExtra, NO CheckMetadataHash). Submitted via the raw
// `author_submitExtrinsic` JSON-RPC method using the call indices in CALLS.
import { u8aConcat, u8aToHex } from '@polkadot/util'
import { blake2AsU8a, cryptoWaitReady, decodeAddress, encodeAddress } from '@polkadot/util-crypto'
import { Keyring } from '@polkadot/keyring'
import { ScaleWriter } from './scale.js'
import { encodeCall } from './encode.js'

// ---- JSON-RPC over WebSocket --------------------------------------------
class WsRpc {
  constructor(url) {
    this.url = url
    this.id = 0
    this.pending = new Map()
    this.ws = null
  }
  connect() {
    return new Promise((resolve, reject) => {
      let ws
      try {
        ws = new WebSocket(this.url)
      } catch (e) {
        return reject(e)
      }
      this.ws = ws
      ws.onopen = () => resolve(this)
      ws.onerror = () => reject(new Error(`cannot connect to ${this.url}`))
      ws.onclose = () => {
        for (const { reject } of this.pending.values()) reject(new Error('connection closed'))
        this.pending.clear()
      }
      ws.onmessage = (ev) => {
        let msg
        try {
          msg = JSON.parse(ev.data)
        } catch {
          return
        }
        const p = this.pending.get(msg.id)
        if (!p) return
        this.pending.delete(msg.id)
        if (msg.error) p.reject(new Error(msg.error.message || 'RPC error'))
        else p.resolve(msg.result)
      }
    })
  }
  call(method, params = []) {
    const id = ++this.id
    return new Promise((resolve, reject) => {
      if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return reject(new Error('not connected'))
      this.pending.set(id, { resolve, reject })
      this.ws.send(JSON.stringify({ id, jsonrpc: '2.0', method, params }))
    })
  }
  close() {
    try {
      this.ws && this.ws.close()
    } catch {}
  }
}

// Connect and read the chain constants needed to sign (genesis + runtime version).
export async function connectChain(endpoint) {
  await cryptoWaitReady()
  const rpc = await new WsRpc(endpoint).connect()
  const [genesis, rv, chainName] = await Promise.all([
    rpc.call('chain_getBlockHash', [0]),
    rpc.call('state_getRuntimeVersion', []),
    rpc.call('system_chain', []).catch(() => 'Ferrum'),
  ])
  return {
    rpc,
    endpoint,
    genesis,
    specVersion: rv.specVersion,
    transactionVersion: rv.transactionVersion,
    chainName,
    specName: rv.specName,
  }
}

// Build a sr25519 signer from a seed/URI (e.g. "//Alice" on a --dev chain, or a
// 0x-hex 32-byte secret seed printed by `ferrum-node key generate`).
export async function makeSigner(uri, ss58Format = 42) {
  await cryptoWaitReady()
  const keyring = new Keyring({ type: 'sr25519', ss58Format })
  const pair = keyring.addFromUri(uri.trim())
  return pair
}

export function addressOf(pair, ss58Format = 42) {
  return encodeAddress(pair.publicKey, ss58Format)
}

// Sign a call and submit it via `author_submitExtrinsic`. Returns the extrinsic hash.
export async function submitCall(conn, pair, name, txArgs) {
  const callBytes = encodeCall(name, txArgs)
  const address = encodeAddress(pair.publicKey)
  const nonce = Number(await conn.rpc.call('system_accountNextIndex', [address]))

  // extra (signed): immortal era ++ compact(nonce) ++ compact(tip = 0)
  const extra = new ScaleWriter().u8(0).compact(nonce).compact(0).toU8a()
  // additional signed: specVersion ++ txVersion ++ genesis ++ era checkpoint (== genesis)
  const additional = new ScaleWriter()
    .u32(conn.specVersion)
    .u32(conn.transactionVersion)
    .fixed(conn.genesis)
    .fixed(conn.genesis)
    .toU8a()

  let payload = u8aConcat(callBytes, extra, additional)
  if (payload.length > 256) payload = blake2AsU8a(payload, 256) // sign the hash for long payloads
  const signature = pair.sign(payload) // sr25519, 64 bytes

  // v4 signed extrinsic: 0x84 ++ MultiAddress::Id(pubkey) ++ MultiSignature::Sr25519(sig) ++ extra ++ call
  const body = u8aConcat(
    Uint8Array.of(0x84),
    Uint8Array.of(0x00),
    pair.publicKey,
    Uint8Array.of(0x01),
    signature,
    extra,
    callBytes,
  )
  const framed = new ScaleWriter().bytes(body).toU8a() // compact length prefix ++ body
  const hash = await conn.rpc.call('author_submitExtrinsic', [u8aToHex(framed)])
  return { hash, nonce, callHex: u8aToHex(callBytes) }
}

export { decodeAddress }
