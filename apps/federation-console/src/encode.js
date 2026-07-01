// encode.js — SCALE-encode pallet-federation calls, byte-identical to the C# SDK
// (FederationCalls in Calls.cs + the Models.cs FederationAction/XsuBasket encoders).
import { decodeAddress } from '@polkadot/util-crypto'
import { ScaleWriter, bytes32, asU8a } from './scale.js'
import { CALLS, PALLET_INDEX } from './chain.js'

const VOTE = { Aye: 0, Nay: 1, Abstain: 2 }

function encodeBasket(w, basket) {
  w.vec(basket.entries, (ww, e) => {
    ww.asciiFixed(e.cbdc, 3)
    ww.u32(e.weightPpb) // Perbill (parts-per-billion)
  })
  w.u32(basket.version)
}

// FederationAction variant encoding (Models.cs FederationAction).
function encodeAction(w, a) {
  switch (a.type) {
    case 'SetParameter':
      w.u8(0)
      w.bytes(new TextEncoder().encode(a.key)) // BoundedVec<u8> (ASCII)
      w.u128(BigInt(a.value))
      break
    case 'AdmitMember':
      w.u8(1); w.asciiFixed(a.member, 2); break
    case 'RemoveMember':
      w.u8(2); w.asciiFixed(a.member, 2); break
    case 'Reweight':
      w.u8(3); encodeBasket(w, a.basket); break
    case 'SuspendMember':
      w.u8(4); w.asciiFixed(a.member, 2); break
    case 'RuntimeUpgrade':
      w.u8(5); w.fixed(bytes32(a.codeHash)); break
    default:
      throw new Error(`unknown action ${a.type}`)
  }
}

const PARAMS = {
  propose(w, a) {
    encodeAction(w, a.action)
  },
  vote(w, a) {
    w.u64(a.id)
    w.u8(VOTE[a.vote])
  },
  close(w, a) {
    w.u64(a.id)
  },
  setMembership(w, a) {
    w.asciiFixed(a.member, 2)
    w.bool(a.seated)
  },
  setBasket(w, a) {
    encodeBasket(w, a.basket)
  },
  mintXsu(w, a) {
    w.asciiFixed(a.cbdc, 3)
    w.u128(a.amount)
  },
  redeemXsu(w, a) {
    w.asciiFixed(a.cbdc, 3)
    w.u128(a.amount)
  },
  bookClearing(w, a) {
    w.asciiFixed(a.to, 2)
    w.u128(a.amount)
  },
  netAndSettle(w, a) {
    w.u32(a.window)
  },
  publishProofOfReserve() {
    // no params
  },
}

// Adapt UI args to real extrinsic params. On-chain `vote` takes only (id, vote)
// — the council member is the signing origin, so the UI's `member` is dropped.
export function toTxArgs(name, ui) {
  switch (name) {
    case 'vote':
      return { id: ui.id, vote: ui.vote }
    default:
      return ui
  }
}

export function encodeCall(name, args) {
  const meta = CALLS[name]
  const enc = PARAMS[name]
  if (!meta || !enc) throw new Error(`no encoder for call ${name}`)
  const w = new ScaleWriter()
  w.u8(PALLET_INDEX).u8(meta.index)
  enc(w, args)
  return w.toU8a()
}

export { decodeAddress }
