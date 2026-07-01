// ============================================================================
// chain.js — a faithful in-browser model of `pallet-interop` (whitepaper §09).
//
// Every operation the console can run maps 1:1 to a real extrinsic of
// `pallet-interop` (module index 15), reproduces that call's validation rules
// and the exact `Error` it would raise, and emits the same `Event`s. No real
// chain connection — this lets an operator rehearse the §3.9 runbook end to end.
//
// Privacy invariant (§09): state holds only commitments, hashes, finality/ZK
// proofs and XSU net amounts — never plaintext PII.
// ============================================================================

import { pseudoHash, MIN_VALIDATOR_BOND, LOCAL_TAG } from './format.js'

export const PALLET_INDEX = 15 // construct_runtime!: Interop = 15

// Call metadata, keyed by handler name. `index` is the pallet call index,
// matching sdk/csharp/Ferrum.Sdk/Calls.cs (InteropCalls).
export const CALLS = {
  registerIssuer: { index: 0, origin: 'Federation', zh: '登記簽發者', en: 'Register issuer' },
  submitInstruction: { index: 1, origin: 'Relayer', zh: '提交清算指令', en: 'Submit instruction' },
  verifyFinality: { index: 2, origin: 'Relayer', zh: '驗證最終性', en: 'Verify finality' },
  netAndSettle: { index: 3, origin: 'Federation', zh: '淨額清算', en: 'Net & settle' },
  registerValidator: { index: 4, origin: 'Signed', zh: '登記驗證者', en: 'Register validator' },
  slashValidator: { index: 5, origin: 'Federation', zh: '罰沒驗證者', en: 'Slash validator' },
  initAuthoritySet: { index: 6, origin: 'Federation', zh: '初始化授權集合', en: 'Init authority set' },
  rotateAuthoritySet: { index: 7, origin: 'Federation', zh: '換屆授權集合', en: 'Rotate authority set' },
  registerIssuerVk: { index: 8, origin: 'Federation', zh: '登記驗證金鑰', en: 'Register VK' },
  verifyForeignProof: { index: 9, origin: 'Relayer', zh: '跨境 ZK 互驗', en: 'Verify foreign proof' },
  registerTreaty: { index: 10, origin: 'Federation', zh: '登記租稅協定', en: 'Register treaty' },
  recognizeForeignInvoice: { index: 11, origin: 'Relayer', zh: '互認電子發票', en: 'Recognize e-invoice' },
  ossRegister: { index: 12, origin: 'Signed', zh: 'OSS 登記', en: 'OSS register' },
  ossReport: { index: 13, origin: 'Signed', zh: 'OSS 申報', en: 'OSS report' },
}

// Composite storage keys.
const k2 = (a, b) => `${a}~${b}`

// A dispatch error mirroring a `pallet-interop` Error variant.
class ChainError extends Error {
  constructor(code, zh, en) {
    super(en)
    this.code = code
    this.zh = zh
    this.en = en
  }
}
const fail = (code, zh, en) => {
  throw new ChainError(code, zh, en)
}
export const isChainError = (e) => e instanceof ChainError

// ----------------------------------------------------------------------------
// Seed state — two sovereign chains (operating as TW, bridging JP), pre-loaded
// so every view has realistic data and the §3.9 flows can run immediately.
// ----------------------------------------------------------------------------
const JP_ISSUER = pseudoHash('jp-issuer-moj')
const TW_SUPPLIER_DID = 'did:fer:tw:z6MkpTWsupplier8x'

export function initialState() {
  return {
    localTag: LOCAL_TAG,
    block: 1_000,
    authoritySets: {
      JP: {
        setId: 7,
        authorities: [
          { id: pseudoHash('jp-auth-0'), weight: 1 },
          { id: pseudoHash('jp-auth-1'), weight: 1 },
          { id: pseudoHash('jp-auth-2'), weight: 1 },
          { id: pseudoHash('jp-auth-3'), weight: 1 },
        ],
      },
    },
    finalizedHeads: {
      JP: { hash: pseudoHash('jp-head-8421330'), number: 8_421_330 },
    },
    trustRegistry: {
      [k2('JP', JP_ISSUER)]: { country: 'JP', issuerKeyHash: JP_ISSUER, scope: 'id', active: true },
    },
    verifyingKeys: {
      [k2('JP', JP_ISSUER)]: pseudoHash('jp-groth16-vk'),
    },
    usedNullifiers: {},
    treaties: {
      [k2('TW', 'JP')]: { a: 'TW', b: 'JP', withholdingCap: 10, method: 'Credit', active: true },
    },
    recognizedInvoices: {},
    ossRegistrations: {
      [TW_SUPPLIER_DID]: { subject: TW_SUPPLIER_DID, home: 'TW', vatIdCommitment: pseudoHash('tw-vat'), active: true },
    },
    localDids: {
      'did:fer:tw:z6MkrTWcitizen42': { docHash: pseudoHash('tw-citizen-42') },
    },
    instructions: {
      0: { id: 0, from: 'JP', to: 'TW', amount: 125_000_000_000n, detailCommitment: pseudoHash('instr-0'), status: 'FinalityVerified' },
      1: { id: 1, from: 'JP', to: 'TW', amount: 48_500_000_000n, detailCommitment: pseudoHash('instr-1'), status: 'Pending' },
    },
    nextInstruction: 2,
    netPositions: {
      [k2('JP', 'TW')]: 312_000_000_000n,
    },
    validators: {
      '5Grwva…Alice': 250_000n * 10n ** 12n,
    },
    totalSlashed: 0n,
  }
}

export const SEED_FIXTURES = { JP_ISSUER, TW_SUPPLIER_DID }

// ----------------------------------------------------------------------------
// Read-only resolver (Rust `Interop::resolve_did`) — not an extrinsic.
// ----------------------------------------------------------------------------
export function resolveDid(state, did) {
  const m = /^did:fer:([a-z]{2}):/.exec(did.trim())
  if (!m) return { kind: 'Invalid' }
  const tag = m[1]
  if (tag === state.localTag) {
    const doc = state.localDids[did.trim()]
    return doc ? { kind: 'Local', docHash: doc.docHash } : { kind: 'LocalUnknown' }
  }
  const country = tag.toUpperCase()
  const recognized = Object.values(state.trustRegistry).some((e) => e.country === country && e.active)
  return { kind: 'Foreign', country, recognized }
}

export function isTrustedIssuer(state, country, issuerKeyHash) {
  const e = state.trustRegistry[k2(country, issuerKeyHash)]
  return !!(e && e.active)
}

// ----------------------------------------------------------------------------
// applyCall — execute an extrinsic against `state`, returning a fresh state,
// the emitted events, and the call descriptor (for the extrinsic preview).
// Throws a ChainError mirroring the pallet's Error on validation failure.
// ----------------------------------------------------------------------------
export function applyCall(state, name, args) {
  const handler = HANDLERS[name]
  if (!handler) throw new Error(`unknown call ${name}`)
  const next = structuredCloneState(state)
  const events = []
  handler(next, args, events)
  next.block = state.block + 1 // simulate inclusion in the next block
  const meta = CALLS[name]
  return {
    state: next,
    events,
    call: { pallet: PALLET_INDEX, module: 'Interop', index: meta.index, name, args },
  }
}

// Deep-ish clone that preserves BigInt values (structuredClone handles BigInt).
function structuredCloneState(s) {
  return structuredClone(s)
}

const HANDLERS = {
  // --- A. Trust-minimized GRANDPA bridge (§3.9 A) ---------------------------
  initAuthoritySet(s, { country, setId, authorities }, ev) {
    s.authoritySets[country] = { setId, authorities }
    ev.push({ name: 'AuthoritySetInitialized', data: { country, setId } })
  },

  rotateAuthoritySet(s, { country, newSetId, targetNumber }, ev) {
    const cur = s.authoritySets[country]
    if (!cur) fail('AuthoritySetNotInitialized', '該國尚未初始化授權集合', 'No authority set for this country')
    // The handoff block's finality is verified under the CURRENT set (simulated ok).
    if (newSetId !== cur.setId + 1)
      fail('NonSequentialSetId', '新 set_id 必須為當前 +1', 'new set_id must be current + 1')
    s.authoritySets[country] = { setId: newSetId, authorities: cur.authorities }
    s.finalizedHeads[country] = { hash: pseudoHash(`${country}-rotate-${targetNumber}`), number: targetNumber }
    ev.push({ name: 'AuthoritySetRotated', data: { country, setId: newSetId } })
  },

  submitInstruction(s, { from, to, amount, detailCommitment }, ev) {
    const id = s.nextInstruction
    s.nextInstruction += 1
    s.instructions[id] = { id, from, to, amount, detailCommitment, status: 'Pending' }
    ev.push({ name: 'InstructionSubmitted', data: { id, from, to, amount } })
  },

  verifyFinality(s, { id, targetNumber }, ev) {
    const instr = s.instructions[id]
    if (!instr) fail('UnknownInstruction', '找不到該清算指令', 'No such instruction')
    if (instr.status !== 'Pending') fail('InvalidStatus', '指令狀態不允許此操作', 'Instruction not in a Pending state')
    const set = s.authoritySets[instr.from]
    if (!set) fail('AuthoritySetNotInitialized', `${instr.from} 尚未初始化授權集合`, `No authority set for ${instr.from}`)
    const head = s.finalizedHeads[instr.from]
    if (head && targetNumber < head.number)
      fail('StaleFinality', '最終化區塊頭未前進(回退/重放)', 'Finalized head did not advance')
    // Simulated: each precommit's ed25519 signature verifies, weight > 2/3.
    s.finalizedHeads[instr.from] = { hash: pseudoHash(`${instr.from}-head-${targetNumber}`), number: targetNumber }
    instr.status = 'FinalityVerified'
    ev.push({ name: 'HeadFinalized', data: { country: instr.from, number: targetNumber } })
    ev.push({ name: 'FinalityVerified', data: { id } })
  },

  netAndSettle(s, { window }, ev) {
    let count = 0
    for (const id of Object.keys(s.instructions)) {
      const instr = s.instructions[id]
      if (instr.status !== 'FinalityVerified') continue
      const key = k2(instr.from, instr.to)
      const updated = (s.netPositions[key] || 0n) + instr.amount
      s.netPositions[key] = updated
      ev.push({ name: 'NetPositionUpdated', data: { from: instr.from, to: instr.to, amount: updated } })
      instr.status = 'Accepted'
      count += 1
    }
    ev.push({ name: 'Netted', data: { window, instructions: count } })
  },

  // --- B. Cross-border identity & ZK verification (§3.9 B) ------------------
  registerIssuer(s, { country, issuerKeyHash, scope, active }, ev) {
    s.trustRegistry[k2(country, issuerKeyHash)] = { country, issuerKeyHash, scope, active }
    ev.push({ name: 'IssuerRegistered', data: { country, issuerKeyHash } })
  },

  registerIssuerVk(s, { country, issuerKeyHash, vk }, ev) {
    if (!isTrustedIssuer(s, country, issuerKeyHash))
      fail('IssuerNotRecognized', '該簽發者未在信任註冊表中受認可', 'Issuer not recognized in the trust registry')
    s.verifyingKeys[k2(country, issuerKeyHash)] = vk
    ev.push({ name: 'IssuerVkRegistered', data: { country, issuerKeyHash } })
  },

  verifyForeignProof(s, { country, issuerKeyHash, nullifier }, ev) {
    if (!isTrustedIssuer(s, country, issuerKeyHash))
      fail('IssuerNotRecognized', '該簽發者未受認可', 'Issuer not recognized')
    if (s.usedNullifiers[nullifier]) fail('ProofReplayed', '該 nullifier 已被使用(重放)', 'Nullifier already spent (replay)')
    if (!s.verifyingKeys[k2(country, issuerKeyHash)])
      fail('VerifyingKeyNotFound', '找不到該簽發者的驗證金鑰', 'No verifying key for this issuer')
    // Simulated: Groth16 verification of the selective-disclosure proof passes.
    s.usedNullifiers[nullifier] = true
    ev.push({ name: 'ForeignProofVerified', data: { country, nullifier } })
  },

  // --- C. Cross-border tax coordination (§3.9 C) ----------------------------
  registerTreaty(s, { a, b, withholdingCap, method, active }, ev) {
    s.treaties[k2(a, b)] = { a, b, withholdingCap, method, active }
    ev.push({ name: 'TreatyRegistered', data: { a, b } })
  },

  recognizeForeignInvoice(s, { country, invoiceHash }, ev) {
    if (!s.finalizedHeads[country])
      fail('NoFinalizedHead', '尚未與該國建立最終性', 'No finality established with this country yet')
    s.recognizedInvoices[k2(country, invoiceHash)] = true
    ev.push({ name: 'ForeignInvoiceRecognized', data: { country, invoiceHash } })
  },

  ossRegister(s, { subject, home, vatIdCommitment, active }, ev) {
    if (s.ossRegistrations[subject]) fail('AlreadyRegisteredOss', '該供應者已登記 OSS', 'Supplier already OSS-registered')
    s.ossRegistrations[subject] = { subject, home, vatIdCommitment, active }
    ev.push({ name: 'OssRegistered', data: { home } })
  },

  ossReport(s, { subject, to, amount, detailCommitment }, ev) {
    const reg = s.ossRegistrations[subject]
    if (!reg) fail('OssNotRegistered', '該供應者未登記 OSS', 'Supplier is not OSS-registered')
    const from = reg.home
    const id = s.nextInstruction
    s.nextInstruction += 1
    s.instructions[id] = { id, from, to, amount, detailCommitment, status: 'Pending' }
    ev.push({ name: 'InstructionSubmitted', data: { id, from, to, amount } })
    ev.push({ name: 'OssReported', data: { from, to, amount } })
  },

  // --- Interop validators (§10 / §11.1) -------------------------------------
  registerValidator(s, { who, bond }, ev) {
    if (s.validators[who]) fail('AlreadyRegistered', '該驗證者已登記', 'Validator already registered')
    if (bond < MIN_VALIDATOR_BOND) fail('InsufficientBond', '質押額低於最低保證金', 'Bond below the minimum required')
    s.validators[who] = bond
    ev.push({ name: 'ValidatorRegistered', data: { who, bond } })
  },

  slashValidator(s, { who, amount }, ev) {
    const bond = s.validators[who]
    if (bond === undefined) fail('UnknownValidator', '找不到該互通驗證者', 'No such interop validator')
    if (bond < amount) fail('SlashExceedsBond', '罰沒金額超過目前保證金', 'Slash exceeds current bond')
    s.validators[who] = bond - amount
    s.totalSlashed = (s.totalSlashed || 0n) + amount
    ev.push({ name: 'ValidatorSlashed', data: { who, amount } })
  },
}
