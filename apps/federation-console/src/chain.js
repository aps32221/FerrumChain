// ============================================================================
// chain.js — a faithful in-browser model of `pallet-federation` (whitepaper §11).
//
// Every operation maps 1:1 to a real extrinsic (module index 14), reproduces its
// validation rules and `Error`s, and emits the same `Event`s. The dual-majority
// rule mirrors `pallets/federation/src/voting.rs::passes_dual_majority`, and
// governance domains/thresholds/timelocks mirror `ferrum_primitives` §11.2.
// ============================================================================

import { pseudoHash, PPB, LOCAL_MEMBER } from './format.js'

export const PALLET_INDEX = 14 // construct_runtime!: Federation = 14
export const MODULE_NAME = 'Federation'

// Call metadata, keyed by handler name. `index` is the pallet call index
// (matches sdk/csharp/Ferrum.Sdk/Calls.cs → FederationCalls).
export const CALLS = {
  propose: { index: 0, origin: 'Council', zh: '提案', en: 'Propose' },
  vote: { index: 1, origin: 'Council', zh: '投票', en: 'Vote' },
  close: { index: 2, origin: 'Signed', zh: '關閉(計票)', en: 'Close (tally)' },
  setMembership: { index: 3, origin: 'Council', zh: '設定席位', en: 'Set membership' },
  setBasket: { index: 4, origin: 'Council', zh: '設定籃子', en: 'Set basket' },
  mintXsu: { index: 5, origin: 'Council', zh: '鑄造 XSU', en: 'Mint XSU' },
  redeemXsu: { index: 6, origin: 'Council', zh: '贖回 XSU', en: 'Redeem XSU' },
  bookClearing: { index: 7, origin: 'Council', zh: '記入清算', en: 'Book clearing' },
  netAndSettle: { index: 8, origin: 'Council', zh: '淨額結算', en: 'Net & settle' },
  publishProofOfReserve: { index: 9, origin: 'Council', zh: '發布儲備證明', en: 'Proof of reserve' },
  // sim-only (on-chain this is `on_initialize` auto-enactment at the timelock ETA)
  enactQueued: { index: -1, origin: 'Sim', zh: '時間鎖生效', en: 'Enact (timelock)' },
}

// Governance domains → threshold (ppb) + timelock (illustrative blocks, per the
// §11.2 table day values) + label. Mirrors `domain_threshold` / FederationAction::domain.
export const DOMAINS = {
  Parameter: { thresholdPpb: Math.round((2 / 3) * PPB), timelock: 7, zh: '參數調整', en: 'Parameter change', note: { zh: '雙重多數 ⅔ · 時間鎖 7 天', en: 'Dual-majority ⅔ · 7-day timelock' } },
  Membership: { thresholdPpb: Math.round((3 / 4) * PPB), timelock: 30, zh: '成員准入/除名', en: 'Member admission / removal', note: { zh: '雙重多數 ¾ · 時間鎖 30 天', en: 'Dual-majority ¾ · 30-day timelock' } },
  Reweighting: { thresholdPpb: Math.round((2 / 3) * PPB), timelock: 7, zh: '籃子再平衡', en: 'Basket reweighting', note: { zh: '雙重多數 ⅔ + 秘書處報告', en: 'Dual-majority ⅔ + secretariat report' } },
  Constitutional: { thresholdPpb: Math.round(0.85 * PPB), timelock: 30, zh: '籃子重定義/條約修訂', en: 'Basket redefinition / treaty amendment', note: { zh: '近乎一致 ≥ 85%', en: 'Near-unanimity ≥ 85%' } },
  Emergency: { thresholdPpb: 0, timelock: 0, zh: '緊急安全升級', en: 'Emergency security upgrade', note: { zh: '技術委員會 + 事後追認', en: 'Tech committee + post-ratify' } },
  Dispute: { thresholdPpb: Math.round((3 / 4) * PPB), timelock: 0, zh: '爭議仲裁/成員停權', en: 'Dispute / member suspension', note: { zh: '雙重多數 ¾(當事國迴避)', en: 'Dual-majority ¾ (party recused)' } },
}

// FederationAction variant → governance domain (mirrors FederationAction::domain).
export function actionDomain(action) {
  switch (action.type) {
    case 'SetParameter': return 'Parameter'
    case 'AdmitMember':
    case 'RemoveMember': return 'Membership'
    case 'Reweight': return 'Reweighting'
    case 'SuspendMember': return 'Dispute'
    case 'RuntimeUpgrade': return 'Constitutional'
    default: return 'Parameter'
  }
}

// A short human description of a FederationAction.
export function actionLabel(action) {
  switch (action.type) {
    case 'SetParameter': return `SetParameter ${action.key} = ${action.value}`
    case 'AdmitMember': return `AdmitMember ${action.member}`
    case 'RemoveMember': return `RemoveMember ${action.member}`
    case 'SuspendMember': return `SuspendMember ${action.member}`
    case 'Reweight': return `Reweight (v${action.basket?.version})`
    case 'RuntimeUpgrade': return `RuntimeUpgrade ${(action.codeHash || '').slice(0, 10)}…`
    default: return action.type
  }
}

// ---- Dual-majority (voting.rs::passes_dual_majority) ----------------------
export function dualMajority(votes, basketWeights, thresholdPpb) {
  const entries = Object.entries(votes)
  const total = entries.length
  const ayes = entries.filter(([, v]) => v === 'Aye').length
  const ayesCountPpb = total === 0 ? 0 : Math.round((ayes / total) * PPB)
  const byCount = ayesCountPpb >= thresholdPpb
  const ayesWeightPpb = entries
    .filter(([, v]) => v === 'Aye')
    .reduce((acc, [m]) => acc + (basketWeights[m] || 0), 0)
  const byWeight = ayesWeightPpb >= thresholdPpb
  return { total, ayes, ayesCountPpb, byCount, ayesWeightPpb, byWeight, pass: byCount && byWeight }
}

// ---- error helper ---------------------------------------------------------
class ChainError extends Error {
  constructor(code, zh, en) { super(en); this.code = code; this.zh = zh; this.en = en }
}
const fail = (code, zh, en) => { throw new ChainError(code, zh, en) }
export const isChainError = (e) => e instanceof ChainError

// ---- seed state -----------------------------------------------------------
export function initialState() {
  return {
    localMember: LOCAL_MEMBER,
    block: 1_000,
    members: { TW: true, JP: true, US: true, DE: true, CN: true },
    // XSU basket: CBDC entries (Perbill ppb), version. §10 illustrative weights.
    basket: {
      version: 3,
      entries: [
        { cbdc: 'USD', weightPpb: 400_000_000 },
        { cbdc: 'EUR', weightPpb: 280_000_000 },
        { cbdc: 'CNY', weightPpb: 120_000_000 },
        { cbdc: 'TWD', weightPpb: 120_000_000 },
        { cbdc: 'JPY', weightPpb: 80_000_000 },
      ],
    },
    // Dual-majority weights per member (Perbill ppb) — sums to 100%.
    basketWeights: { US: 400_000_000, DE: 280_000_000, CN: 120_000_000, TW: 120_000_000, JP: 80_000_000 },
    proposals: {
      0: { id: 0, action: { type: 'SetParameter', key: 'fee.relayer', value: 25 }, votes: { TW: 'Aye', US: 'Aye' }, createdAt: 980, status: 'Open' },
    },
    nextId: 1,
    queued: {}, // eta(block) -> id
    reservePool: { USD: 120_000_000_000n, EUR: 84_000_000_000n, CNY: 36_000_000_000n, TWD: 36_000_000_000n, JPY: 24_000_000_000n },
    xsuIssued: 300_000_000_000n,
    xsuBalances: { TW: 60_000_000_000n, JP: 40_000_000_000n, US: 90_000_000_000n },
    lastProofOfReserve: { block: 990, digest: pseudoHash('por-990') },
  }
}

export const SEED_FIXTURES = {}

// ---- apply a call ---------------------------------------------------------
export function applyCall(state, name, args) {
  const handler = HANDLERS[name]
  if (!handler) throw new Error(`unknown call ${name}`)
  const next = structuredClone(state)
  next.block = state.block + 1 // inclusion block; enactQueued may fast-forward further
  const events = []
  handler(next, args, events)
  const meta = CALLS[name]
  return { state: next, events, call: { pallet: PALLET_INDEX, module: MODULE_NAME, index: meta.index, name, args } }
}

function basketIsBalanced(basket) {
  return basket.entries.reduce((a, e) => a + e.weightPpb, 0) === PPB
}

const HANDLERS = {
  // --- §11.4 governance ----------------------------------------------------
  propose(s, { action }, ev) {
    const id = s.nextId
    s.nextId += 1
    s.proposals[id] = { id, action, votes: {}, createdAt: s.block, status: 'Open' }
    ev.push({ name: 'Proposed', data: { id, by: s.localMember } })
  },

  vote(s, { id, member, vote }, ev) {
    const p = s.proposals[id]
    if (!p) fail('Unknown', '提案不存在', 'Unknown proposal')
    if (p.status !== 'Open') fail('Unknown', '提案非開放表決中', 'Proposal not open')
    const who = member || s.localMember
    if (p.votes[who] !== undefined) fail('AlreadyVoted', '該成員已投票', 'Member already voted')
    p.votes[who] = vote
    ev.push({ name: 'Voted', data: { id, member: who, vote } })
  },

  close(s, { id }, ev) {
    const p = s.proposals[id]
    if (!p) fail('Unknown', '提案不存在', 'Unknown proposal')
    if (p.status !== 'Open') fail('Unknown', '提案非開放表決中', 'Proposal not open')
    const domain = actionDomain(p.action)
    const dm = dualMajority(p.votes, s.basketWeights, DOMAINS[domain].thresholdPpb)
    if (!dm.pass) fail('Rejected', '雙重多數未通過', 'Dual majority not met')
    const eta = s.block + DOMAINS[domain].timelock
    p.status = 'Queued'
    p.eta = eta
    s.queued[eta] = id
    ev.push({ name: 'Queued', data: { id, eta } })
  },

  // sim-only: on-chain this is on_initialize at eta
  enactQueued(s, { id }, ev) {
    const p = s.proposals[id]
    if (!p) fail('Unknown', '提案不存在', 'Unknown proposal')
    if (p.status !== 'Queued') fail('Unknown', '提案未在時間鎖佇列', 'Proposal not queued')
    if (p.eta > s.block) s.block = p.eta // fast-forward to ETA
    delete s.queued[p.eta]
    enactAction(s, p.action, ev)
    p.status = 'Enacted'
    ev.push({ name: 'Enacted', data: { id } })
  },

  setMembership(s, { member, seated }, ev) {
    s.members[member] = seated
    ev.push({ name: 'MembershipChanged', data: { member, seated } })
  },

  setBasket(s, { basket }, ev) {
    if (!basketIsBalanced(basket)) fail('UnbalancedBasket', '籃子權重總和不為 100%', 'Basket weights do not sum to 100%')
    s.basket = basket
    ev.push({ name: 'BasketReweighted', data: { version: basket.version } })
  },

  // --- §11.3 token operations ---------------------------------------------
  mintXsu(s, { cbdc, amount }, ev) {
    if (!s.basket.entries.some((e) => e.cbdc === cbdc)) fail('UnknownCbdc', '該 CBDC 不在目前籃子中', 'CBDC not in the active basket')
    s.reservePool[cbdc] = (s.reservePool[cbdc] || 0n) + amount
    s.xsuIssued += amount // 1:1 fully-collateralized
    const who = s.localMember
    s.xsuBalances[who] = (s.xsuBalances[who] || 0n) + amount
    ev.push({ name: 'XsuMinted', data: { member: who, cbdc, cbdc_amount: amount, xsu_minted: amount } })
  },

  redeemXsu(s, { cbdc, amount }, ev) {
    const who = s.localMember
    if ((s.xsuBalances[who] || 0n) < amount) fail('InsufficientXsu', '成員 XSU 餘額不足', 'Insufficient XSU balance')
    if ((s.reservePool[cbdc] || 0n) < amount) fail('InsufficientReserve', '準備池餘額不足', 'Insufficient reserve-pool balance')
    s.xsuBalances[who] -= amount
    s.reservePool[cbdc] -= amount
    s.xsuIssued -= amount
    ev.push({ name: 'XsuRedeemed', data: { member: who, cbdc, xsu_burned: amount, cbdc_amount: amount } })
  },

  bookClearing(s, { to, amount }, ev) {
    const from = s.localMember
    if ((s.xsuBalances[from] || 0n) < amount) fail('InsufficientXsu', '成員 XSU 餘額不足', 'Insufficient XSU balance')
    s.xsuBalances[from] -= amount
    s.xsuBalances[to] = (s.xsuBalances[to] || 0n) + amount
    ev.push({ name: 'ClearingBooked', data: { from, to, amount } })
  },

  netAndSettle(s, { window }, ev) {
    ev.push({ name: 'NetSettled', data: { window } })
  },

  publishProofOfReserve(s, _args, ev) {
    const seed = Object.entries(s.reservePool).map(([c, b]) => `${c}:${b}`).join('|') + `|${s.xsuIssued}`
    const digest = pseudoHash(seed + '@' + s.block)
    s.lastProofOfReserve = { block: s.block, digest }
    ev.push({ name: 'ProofOfReservePublished', data: { block: s.block, digest } })
  },
}

// Enactment effects per action (mirrors pallet `enact`).
function enactAction(s, action, ev) {
  switch (action.type) {
    case 'AdmitMember':
      s.members[action.member] = true
      ev.push({ name: 'MembershipChanged', data: { member: action.member, seated: true } })
      break
    case 'RemoveMember':
    case 'SuspendMember':
      s.members[action.member] = false
      ev.push({ name: 'MembershipChanged', data: { member: action.member, seated: false } })
      break
    case 'Reweight':
      if (action.basket) s.basket = action.basket
      ev.push({ name: 'BasketReweighted', data: { version: action.basket?.version } })
      break
    case 'RuntimeUpgrade':
      ev.push({ name: 'RuntimeUpgradeTriggered', data: { code_hash: action.codeHash } })
      break
    default:
      break // SetParameter: marked enacted, applied by the executor
  }
}
