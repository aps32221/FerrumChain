// encode.js — SCALE-encode pallet-interop calls, byte-identical to the C# SDK
// (InteropCalls in Calls.cs + the Models.cs encoders). Each encoder is keyed by
// the same handler name used in chain.js CALLS, and prefixes module/call index.
import { decodeAddress } from '@polkadot/util-crypto'
import { ScaleWriter, bytes32, perbillFromPercent, asU8a } from './scale.js'
import { CALLS, PALLET_INDEX } from './chain.js'

const XCM_STATUS = { Pending: 0, FinalityVerified: 1, Accepted: 2, Rejected: 3 }
const CREDIT_METHOD = { Credit: 0, Exemption: 1 }

// Encode a `did:fer:<tag>:<id>` string the way pallet `Did` expects:
// asciiVec(chain_tag) ++ bytes(id).
function encodeDid(w, did) {
  const m = /^did:fer:([a-z]{2}):(.+)$/.exec(String(did).trim())
  if (!m) throw new Error(`malformed DID: ${did}`)
  w.asciiVec(m[1])
  w.bytes(new TextEncoder().encode(m[2]))
}

// Decode an account reference (SS58 address or 0x-hex 32 bytes) to AccountId bytes.
function accountBytes(who) {
  const s = String(who).trim()
  if (s.startsWith('0x')) return bytes32(s)
  try {
    return decodeAddress(s)
  } catch {
    throw new Error(`not a valid address: ${who}`)
  }
}

// Per-call parameter encoders (everything after module/call index).
const PARAMS = {
  registerIssuer(w, a) {
    w.asciiFixed(a.country, 2)
    w.fixed(bytes32(a.issuerKeyHash))
    w.asciiVec(a.scope)
    w.bool(a.active)
  },
  submitInstruction(w, a) {
    w.asciiFixed(a.from, 2)
    w.asciiFixed(a.to, 2)
    w.u128(a.amount)
    w.fixed(bytes32(a.detailCommitment))
    w.u8(XCM_STATUS[a.status ?? 'Pending'])
  },
  verifyFinality(w, a) {
    w.u64(a.id)
    w.bytes(asU8a(a.finalityProofHex)) // BoundedVec<u8>
  },
  netAndSettle(w, a) {
    w.u32(a.window)
  },
  registerValidator(w, a) {
    w.u128(a.bond) // signer is the validator; only the bond is a param
  },
  slashValidator(w, a) {
    w.fixed(accountBytes(a.who))
    w.u128(a.amount)
  },
  initAuthoritySet(w, a) {
    w.asciiFixed(a.country, 2)
    encodeAuthoritySet(w, a.set)
  },
  rotateAuthoritySet(w, a) {
    w.asciiFixed(a.country, 2)
    w.bytes(asU8a(a.finalityProofHex))
    encodeAuthoritySet(w, a.newSet)
  },
  registerIssuerVk(w, a) {
    w.asciiFixed(a.country, 2)
    w.fixed(bytes32(a.issuerKeyHash))
    w.bytes(asU8a(a.vk))
  },
  verifyForeignProof(w, a) {
    w.asciiFixed(a.country, 2)
    w.fixed(bytes32(a.issuerKeyHash))
    w.bytes(asU8a(a.proofHex))
    // AgeProofPublicInputs: issuer_commitment ++ threshold(u32) ++ nullifier
    w.fixed(bytes32(a.issuerCommitment))
    w.u32(a.threshold)
    w.fixed(bytes32(a.nullifier))
  },
  registerTreaty(w, a) {
    w.asciiFixed(a.a, 2)
    w.asciiFixed(a.b, 2)
    // TaxTreaty: withholding_cap(Perbill) ++ method ++ active
    w.u32(perbillFromPercent(a.withholdingCap))
    w.u8(CREDIT_METHOD[a.method])
    w.bool(a.active)
  },
  recognizeForeignInvoice(w, a) {
    w.asciiFixed(a.country, 2)
    w.fixed(bytes32(a.invoiceHash))
  },
  ossRegister(w, a) {
    encodeDid(w, a.subject)
    // OssRegistration: home ++ vat_id_commitment ++ active
    w.asciiFixed(a.home, 2)
    w.fixed(bytes32(a.vatIdCommitment))
    w.bool(a.active)
  },
  ossReport(w, a) {
    encodeDid(w, a.subject)
    w.asciiFixed(a.to, 2)
    w.u128(a.amount)
    w.fixed(bytes32(a.detailCommitment))
  },
}

function encodeAuthoritySet(w, set) {
  w.vec(set.authorities, (ww, auth) => {
    ww.fixed(bytes32(auth.id))
    ww.u64(auth.weight)
  })
  w.u64(set.setId)
}

// Adapt the UI's friendly args into the real extrinsic params. The UI carries
// simplified fields (e.g. a target block number) for the offline simulation;
// live submission needs the actual runtime shape (proof blobs, full authority
// sets, public inputs). Operator-supplied blobs flow through *Hex fields.
export function toTxArgs(name, ui, state) {
  switch (name) {
    case 'submitInstruction':
      return { from: ui.from, to: ui.to, amount: ui.amount, detailCommitment: ui.detailCommitment, status: 'Pending' }
    case 'verifyFinality':
      return { id: ui.id, finalityProofHex: ui.finalityProofHex || '0x' }
    case 'registerValidator':
      return { bond: ui.bond } // the signing account is the validator
    case 'rotateAuthoritySet':
      return {
        country: ui.country,
        finalityProofHex: ui.finalityProofHex || '0x',
        newSet: { authorities: state.authoritySets[ui.country]?.authorities ?? [], setId: ui.newSetId },
      }
    case 'verifyForeignProof':
      return {
        country: ui.country,
        issuerKeyHash: ui.issuerKeyHash,
        proofHex: ui.proofHex || '0x',
        issuerCommitment: ui.issuerCommitmentHex || ui.issuerKeyHash,
        threshold: ui.threshold ?? 18,
        nullifier: ui.nullifier,
      }
    default:
      return ui // already in runtime shape
  }
}

// Encode a full call: module index ++ call index ++ params.
export function encodeCall(name, args) {
  const meta = CALLS[name]
  const enc = PARAMS[name]
  if (!meta || !enc) throw new Error(`no encoder for call ${name}`)
  const w = new ScaleWriter()
  w.u8(PALLET_INDEX).u8(meta.index)
  enc(w, args)
  return w.toU8a()
}
