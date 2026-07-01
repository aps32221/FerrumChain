# Ferrum Design of Record — 電子發票開獎 / E-Invoice Lottery

**Status:** Design of record · **Scope:** new `pallet-lottery` + whitepaper §06 subsection · **Domestic-only (L4)**
**Reuses:** `pallet-tax` (invoice anchors), `pallet-treasury-fer` (eTWD settlement receipts), `ferrum-zk` (Groth16 decode/verify plumbing), Aura+GRANDPA consensus
**Honors:** no plaintext PII on-chain · non-speculative / fiat-denominated / fully-reserved eTWD · national sovereignty / no global token · reuse over reinvention
**New cryptographic work explicitly required (NOT reuse):** an **eligibility circuit** and an **ownership/claim circuit** in `ferrum-zk`, each with its own governance-set verifying key. `verify_age_threshold` is an age-threshold circuit only and does **not** prove invoice ownership, set membership, nullifier derivation, or recipient binding. This design specifies the new circuits and reuses `ferrum-zk` only at the `decode_proof` / `decode_vk` / `verify_proof` level.

---

## Part 1 — Whitepaper Chapter Draft (bilingual)

**Placement decision.** The lottery is a *sub-section under §06 Tax Administration Layer* (`id="tax"`), **not** a new top-level chapter — this preserves the 01–16 two-digit scheme and reflects the true dependency graph: the feature extends §06's existing "電子發票錨定 / E-invoice anchoring" primitive and settles via §08 (`id="token"`, 代幣金融模型) eTWD. It cross-references §08 (prize rail), §09 (`id="interop"`, optional cross-border recognition), §13 (`id="security"`, viewing-key audit), and §14 (`id="governance"`, parameter governance). No `tocList` change is required (sub-sections under §06 need no entry). Render as an `<h3>` block placed after §06's "核心機制 / Core mechanisms" list. The §03 architecture figure (Fig 3.1, L4 box) is updated to list `pallet-identity / pallet-tax / pallet-credential / pallet-treasury-fer / pallet-lottery`.

Below is the draft content in the index.html paired-sibling style (`class="zh"` / `class="en"`, never inline-mixed).

```html
<h3 class="zh" id="tax-lottery">6.x 電子發票開獎 — 以隱私為前提的全民稅務誘因</h3>
<h3 class="en">6.x E-Invoice Lottery — A Privacy-Preserving Civic Tax Incentive</h3>
```

### 動機與淵源 / Motivation & lineage

> **zh** — 臺灣自 1951 年起以「統一發票給獎」鼓勵消費者索取發票、商家誠實開立，將每一張發票號碼變成一張彩券，藉中獎誘因把逃漏稅的監督權交給數百萬消費者。Ferrum 將此制度原生上鏈：每一筆已依 §6 錨定且**由商家本人簽署、且對應到一筆真實已結算營業稅**的電子發票承諾（`invoice_hash`）即為一張彩券，毋須另立帳本、毋須揭露個資、毋須持有任何投機代幣。開獎獎金以央行發行、十足準備的 eTWD（§8）給付；獎金的真正價值移轉走央行數位貨幣（CBDC）軌道，鏈上僅留存去識別化的收據承諾與金額。獎池規模為當期**經認證之已結算稅收**的治理比率（「稅務等比率」），並以央行鏈上認證之 eTWD 準備餘額封頂。其政策目的不變：以正當誘因擴大稅基、提升開立合規率，而非博弈。

> **en** — Since 1951 Taiwan's Uniform-Invoice Lottery (統一發票) has turned every receipt number into a lottery ticket, enlisting millions of consumers to demand receipts and police merchant under-reporting through a prize incentive. Ferrum makes this mechanism native to the chain: every e-invoice that is (i) anchored under §6, (ii) **signed by the issuing merchant**, and (iii) **backed by a real, settled VAT amount** — its `invoice_hash` commitment — *is* a ticket. No parallel ledger, no PII disclosure, no need to hold any speculative token. Prizes are denominated in central-bank-issued, fully-reserved **eTWD** (§8); the *value* moves on the CBDC rail, and the chain keeps only a PII-free receipt commitment and amount. The prize pool is sized as a governed ratio of the period's **authenticated** settled tax revenue (「稅務等比率」), and is capped by the central bank's on-chain-attested eTWD reserve balance. The policy goal is unchanged: broaden the tax base and raise issuance-compliance through a legitimate incentive — not gambling.

### 錨定發票如何成為隱私彩券 / How anchored invoices become privacy tickets

> **zh** — 彩券的身分即 `invoice_hash`（`Hash32`）。「一張發票＝一張彩券」由三道門共同保證：（1）`pallet-tax` 的 `anchor_invoice` 強制 `ensure!(who == anchor.issuer)`，**只有持商家金鑰者**可錨定，且對同一雜湊去重（`InvoiceAlreadyAnchored`）；（2）`pallet-lottery` 以 `invoice_hash` 為主鍵，登記不可重複；（3）登記時須在 ZK 中證明持有人**握有該特定發票的購買秘密**，故無法登記他人發票。錨定區塊高度（非可被驗證者操弄的牆鐘時間戳）決定該票落入哪一期。登記彩券（`register_ticket`）時，鏈上僅新增承諾與一個**電路推導**的 nullifier：持票人以 `ferrum-zk` Groth16（**新的資格電路**）證明「我持有此 `invoice_hash` 的購買秘密、屬已登記消費者、且年滿十八」，而**不揭露** DID、發票明細或金額。買方身分只存在於 ZK 見證中，永不上鏈。每位消費者每張發票僅一筆有效登記，因 nullifier 由 `(消費者秘密, invoice_hash, draw_id, 域)` 確定性推導並於電路內驗證，無法以新 nullifier 重複登記同一身分／發票。

> **en** — A ticket's identity *is* its `invoice_hash` (`Hash32`). "One invoice = one ticket" is guaranteed by three gates: (1) `pallet-tax::anchor_invoice` enforces `ensure!(who == anchor.issuer)`, so **only the merchant key-holder** can anchor, and de-duplicates each hash (`InvoiceAlreadyAnchored`); (2) `pallet-lottery` keys tickets on `invoice_hash`, so re-registration is rejected; (3) registration requires a ZK proof that the holder **possesses the purchase secret of that specific invoice**, so no one can register an invoice they did not buy. The **block height** at which the invoice was anchored (not a validator-influenceable wall-clock timestamp) decides which period the ticket falls into. Registering a ticket (`register_ticket`) adds only commitments and a **circuit-derived** nullifier on-chain: the holder uses the `ferrum-zk` Groth16 path with a **new eligibility circuit** to prove "I possess the purchase secret of this `invoice_hash`, I am a registered consumer, and age ≥ 18" **without revealing** the DID, line items, or amount. The buyer's identity exists only inside the ZK witness — never on-chain. Each consumer gets one valid registration per invoice because the nullifier is **deterministically derived** from `(owner_secret, invoice_hash, draw_id, domain)` and that derivation is checked inside the circuit — a fresh opaque nullifier cannot re-register the same identity/invoice.

### 可驗證的公開開獎 / The verifiable public draw

> **zh** — Ferrum 的共識為 Aura + GRANDPA，Aura 區塊不含每槽 VRF 秘密，區塊雜湊可被出塊者操弄，故不能直接作為公開抽獎熵源。開獎採**承諾—揭示**機制並錨定於 **GRANDPA 已最終化之未來區塊雜湊**：N 位具保證金的議會／驗證者先提交 `H(seed‖salt)`，於揭示期（嚴格早於 `finalize_block`）公開 `seed`，最終隨機數 `R = blake2_256(⊕seedᵢ ‖ block_hash(B_fin) ‖ draw_id)`。為杜絕「最後揭示者偏置」與「出塊者碾磨」，本設計強制：（a）`reveal_deadline < finalize_block`，且揭示在 `finalize_block` 當下或之後一律拒收，使 `B_fin` 之雜湊在所有揭示落定時尚未存在、不可被任何揭示者預知；（b）`finalize_draw` 必須在 `finalize_block` 之後的 `BlockHashCount`（約 256 區塊）內呼叫，否則 `block_hash(B_fin)` 將回傳預設零值——此情形觸發**自動重種或取消**，絕不退化為僅 `RevealedXor`；（c）缺席揭示者**不以「省略」處理，而以一個固定且公開的後備種子值代入**，使單一驗證者扣留份額無法在兩個結果間翻轉抽獎，僅在達 `MinReveals` 法定數時方為有效抽獎，且承諾保證金 `commit_deposit` 受治理綁定為**不小於單期一個串謀集合所能改向之獎金上限**，使「承諾後扣留」嚴格虧損。任何人皆可由公開的 `pallet-tax` 錨定集（依錨定區塊高度視窗化）、揭示事件與最終化雜湊**獨立重算每位中獎者**並以 Merkle 證明核對。

> **en** — Ferrum's consensus is Aura + GRANDPA. Aura blocks carry no per-slot VRF secret and the block hash is grindable by its author, so it cannot serve as draw entropy. The draw uses **commit–reveal anchored to a GRANDPA-finalized future block hash**: N bonded council/validator participants post `H(seed‖salt)`, reveal `seed` during a window that ends **strictly before** `finalize_block`, and the final randomness is `R = blake2_256(⊕seedᵢ ‖ block_hash(B_fin) ‖ draw_id)`. To remove last-revealer bias and author grinding, the design enforces: (a) `reveal_deadline < finalize_block`, and reveals at or after `finalize_block` are rejected, so `B_fin`'s hash does not yet exist when the last reveal lands and is unknowable to any revealer; (b) `finalize_draw` must be called within `BlockHashCount` (~256 blocks) of `finalize_block`, otherwise `block_hash(B_fin)` would return the default zero hash — that condition triggers **automatic re-seed or cancellation**, never a silent collapse to `RevealedXor` alone; (c) a **missing reveal is replaced by a fixed, published fallback seed value, not treated as omission**, so a single withheld share cannot flip the draw between two outcomes; the draw is valid only at `MinReveals` quorum, and `commit_deposit` is governance-bound to be **no less than the maximum prize a colluding set could redirect in one period**, making commit-then-withhold strictly loss-making. Anyone can **independently recompute every winner** from the public `pallet-tax` anchor set (windowed by anchoring **block height**), the reveal events, and the finalized block hash, and check each against its Merkle proof.

### 稅務等比率 CBDC 獎池 / Tax-proportional CBDC prize pool

```html
<div class="grid c3">
  <div class="card">
    <div class="ic zh">獎池 · eTWD</div><div class="ic en">PrizePool · eTWD</div>
    <h4 class="zh">十足準備、不增發、鏈下移轉</h4><h4 class="en">Fully reserved, never minted, settled off-chain</h4>
    <p class="zh">獎池為純帳務計數器（FiatAmount），對應央行於 CBDC 軌道上既有之 eTWD 準備。鏈上不鑄造、不持有可花用之 eTWD 餘額；給付為一筆去識別化收據承諾，真正價值移轉走央行軌道。FER 僅為驗證者保證金，公民永不持有。</p>
    <p class="en">The pool is a pure accounting counter (FiatAmount) against eTWD the central bank already holds on the CBDC rail. The chain mints and holds no spendable eTWD balance; payout is a PII-free receipt commitment, and the value moves on the CBDC rail. FER is only a validator bond; citizens never hold it.</p>
  </div>
  <div class="card">
    <div class="ic zh">稅務等比率</div><div class="ic en">Tax-proportional</div>
    <h4 class="zh">資金比率 r</h4><h4 class="en">Funding ratio r</h4>
    <p class="zh">獎池 = r × 當期經認證之已結算稅收（僅統一發票 ValueAdded），r 為治理參數（ppm），並以央行鏈上認證之 eTWD 準備餘額封頂。稅收基底於期末快照後凍結，funding 僅讀取快照。</p>
    <p class="en">Pool = r × period authenticated settled tax revenue (ValueAdded only); r is a governed parameter (ppm), clamped by the CB's on-chain-attested eTWD reserve balance. The revenue base is snapshotted and frozen at period end; funding reads only the snapshot.</p>
  </div>
  <div class="card">
    <div class="ic zh">分級比例 w[]</div><div class="ic en">Tier split w[]</div>
    <h4 class="zh">等比率分配</h4><h4 class="en">Equal-ratio distribution</h4>
    <p class="zh">獎池依固定比例向量 w[]（Σ=1）分為各獎級，單票設上限，所有不可分配之餘額（含溢出與整除餘塵）一律回流準備，不折入受限獎級。</p>
    <p class="en">The pool is split across tiers by a fixed proportion vector w[] (Σ=1), with a per-ticket cap; all unspendable residue (overflow and floor-division dust alike) recycles to reserve and is never folded into a capped tier.</p>
  </div>
</div>
```

### 領獎與 eTWD 給付 / Claim & eTWD payout

> **zh** — 領獎（`claim_prize`）以 `ensure_signed` 提交，可由任一中繼者或臨櫃輔助點代送、**免手續費**，使無錢包者亦能領獎（原則四）。中獎人以 ZK **所有權電路**證明自己持有中獎票：電路之**公開輸入**包含 `invoice_hash`、鏈上 `Tickets[invoice_hash].owner_commitment`、`draw_id` 域、**領獎受款帳戶**，以及一個確定性推導之領獎 nullifier；鏈上將 `owner_commitment` 餵入驗證器作為公開輸入，故複製他人證明無法改派受款人，且任何旁觀者讀得中獎雜湊亦無法冒領。選擇性揭露之 `viewing_key_commitment` 為電路之**已驗證公開輸出**（證明其為對稽核者加密之、綁定於 `owner_commitment` 之身分），而非可任填之參數，使稽核軌跡不可偽。給付透過 `pallet-treasury-fer` 之 `credit_fiat` 記錄一筆收據，收據鍵為 `H(invoice_hash ‖ claim_nullifier)`（非原始 `invoice_hash`），故無法將公開之中獎雜湊與付款金額交叉連結，亦無法被外部呼叫者預占；該方法僅供本 pallet 經授權之內部 origin 呼叫。逾期未領之獎金由 `sweep_expired` 於 `ClaimWindow` 嚴格屆滿後冪等地回流主權準備帳務。

> **en** — Claiming (`claim_prize`) is `ensure_signed`, relayable by any assisted point-of-service, and **fee-free**, so winners without a wallet can still claim (Principle 4). The winner proves ownership with the ZK **ownership circuit** whose **public inputs** include `invoice_hash`, the on-chain `Tickets[invoice_hash].owner_commitment`, the `draw_id` domain, **the payout beneficiary account**, and a deterministically derived claim nullifier. The pallet feeds `owner_commitment` into the verifier as a public input, so a copied proof cannot redirect the recipient, and an observer who reads a winning hash cannot impersonate the owner. The selective-disclosure `viewing_key_commitment` is a **verified public output of the proof** (it is proven to be an encryption-to-auditor of the identity bound to `owner_commitment`), not a free argument — so the audit trail is unforgeable. Payout records a receipt via `pallet-treasury-fer::credit_fiat` keyed by `H(invoice_hash ‖ claim_nullifier)` (**not** the raw `invoice_hash`), so no one can join the public winning hash to a payout amount, and no external caller can squat the receipt key; that method is callable only by this pallet's authorized internal origin. Unclaimed prizes recycle to the sovereign-reserve accounting via `sweep_expired`, idempotently, strictly after `ClaimWindow` expires.

### 隱私邊界（誠實揭露）/ Privacy boundary (honest disclosure)

> **zh** — 須坦承一項殘餘洩漏：`pallet-tax::Invoices` 為公開集合，其 `issuer`（商家帳戶）與錨定資訊可被讀取。將彩券資格綁定於此公開錨定集，意味每張中獎收據可被觀察者對應到「哪一商家、何時開立」，長期相關可能洩漏商家層級之消費型態，即使消費者 DID 隱藏。為降低此面，本設計建議 `InvoiceAnchor` 以 `issuer_commitment` 取代明文 `issuer` 儲存（治理維護之商家集合根 `MerchantSetRoot` 下之成員），或將彩券資格與公開錨定集解耦。在採行前，須明確標示「無從推得購買明細」之說法僅及於消費者端，不及於商家側中繼資料。

> **en** — One residual leak must be acknowledged: `pallet-tax::Invoices` is a public set whose `issuer` (merchant account) and anchoring metadata are readable. Binding lottery eligibility to this public anchor set means each winning receipt can be correlated by an observer to "which merchant issued it, and when," potentially leaking merchant-level purchase patterns over time even though the consumer DID is hidden. To reduce this surface, the design recommends storing `issuer_commitment` (membership under a governance-maintained `MerchantSetRoot`) in place of the plaintext `issuer`, or decoupling lottery eligibility from the public anchor set. Until adopted, the "no purchase detail derivable" claim is scoped to the consumer side only — not to merchant-side metadata.

### 不變量 / Invariants (warn callout)

```html
<div class="note warn zh"><div class="lab">不變量</div>
無個資上鏈 · 獎金僅以 eTWD 計價且價值移轉走 CBDC 軌道（絕不以 FER 計價、鏈上不持可花用 eTWD 餘額）· 公民毋須持有 FER · 抽獎熵錨定於已最終化區塊且揭示嚴格早於該區塊 · 票之資格須商家簽署且對應真實已結算稅額 · 國內主權功能，無全球彩券、無跨境獎金價值轉移。</div>
<div class="note warn en"><div class="lab">INVARIANTS</div>
No PII on-chain · prizes are eTWD-denominated and the value moves on the CBDC rail (never priced in FER; the chain holds no spendable eTWD balance) · citizens never hold FER · draw entropy anchored to a finalized block, with reveals strictly before it · a ticket is eligible only if merchant-signed and backed by a real settled tax amount · a domestic sovereign function — no global lottery, no cross-border value transfer of the prize.</div>
```

**Governed-parameter table (for the chapter):**

| 參數 / Parameter | 型別 / Type | 範例 / Example | 說明 / Meaning |
|---|---|---|---|
| `tax_ratio` (r) | `u32` ppm | `2_000` (0.2%) | 稅收→獎池比率 / share of tax revenue → pool |
| `tier_split` (w[]) | `BoundedVec<u32,16>` ppm | `[500000,300000,200000]` | 各獎級比例 Σ=1,000,000 / tier proportions |
| `reserve_cap` | `u32` ppm | `50_000` (5%) | 單期獎池佔央行認證 eTWD 準備上限 / max pool vs attested eTWD reserve |
| `cadence` | `BlockNumber` | ~2 月 / ~2 months | 開獎週期（臺灣雙月）/ draw period |
| `eligible_kinds` | `BoundedVec<TaxKind,8>` | `[ValueAdded]` | 合格發票種類 / eligible invoice kinds |
| `claim_window` | `BlockNumber` | ~90 天 / ~90 days | 領獎期限 / claim deadline |
| `commit_deposit` | `Balance` (FER) | governed, ≥ max redirectable prize | 抽獎承諾保證金（僅驗證者）/ commit bond (validators only) |
| `min_reveals` | `u32` | governed (high) | 有效抽獎法定揭示數 / quorum for a valid draw |

---

## Part 2 — Pallet Spec (`pallet-lottery`)

**SPEC.md — `pallet-lottery`** · L4 domestic governance · whitepaper §06 subsection

### Overview

`pallet-lottery` turns merchant-authenticated, payment-backed, already-anchored `pallet-tax` e-invoices into privacy-preserving lottery tickets, runs a manipulation-resistant commit–reveal draw, sizes a fiat prize pool as a governed ratio of authenticated period tax revenue, and records eTWD prize receipts through `pallet-treasury-fer` (with the value moving off-chain on the CBDC rail). It invents the draw/randomness, the prize-pool accounting, **and two new ZK circuits with their own verifying keys** (eligibility, ownership). Everything else is composed from existing primitives via traits. No new money type, no new identity type, no plaintext PII.

**Dependency on `pallet-tax` (hard, not optional):** `anchor_invoice` MUST enforce `ensure!(who == anchor.issuer)` and MUST record the **anchoring block height** (see Integration). Without authenticated, payment-backed anchoring, ticket supply is forgeable and the lottery is unsound. This is a required precondition, elevated from "optional hardening."

### Key types (add to `crates/primitives`, §6 tax layer; new ZK inputs added to `ferrum-zk`)

```rust
pub type DrawId  = u64;
pub type PeriodId = u64;   // == DrawId; one draw per period

/// A registered entry. Ticket identity is the `invoice_hash` map key — this
/// struct holds only binding commitments. NO PII, no buyer DID on-chain.
#[derive(Clone, Eq, PartialEq, Encode, Decode, MaxEncodedLen, TypeInfo, RuntimeDebug)]
pub struct LotteryTicket {
    pub draw: DrawId,
    /// owner_commitment = BLAKE2b(owner_did ‖ invoice_hash ‖ owner_secret)
    pub owner_commitment: Commitment,
    pub registered_at: BlockNumber,
}

/// Single lifecycle enum (no dual DrawPhase/DrawState). Any "ticketing view"
/// is derived by `fn phase(&DrawState) -> {Open|Drawing|Settled}`.
pub enum DrawState { Pending, Open, Drawing, Drawn, Settled, Cancelled }

pub struct PrizeTier { pub tier_id: u8, pub share_ppm: u32, pub winners: u32, pub unit_cap: FiatAmount }

pub struct DrawConfig {
    pub period_blocks: BlockNumber,
    pub eligible_kinds: BoundedVec<TaxKind, ConstU32<8>>,
    pub tax_ratio_ppm: u32,
    pub tiers: BoundedVec<PrizeTier, ConstU32<16>>,
    pub allow_foreign: bool,
    pub commit_deadline: BlockNumber,
    pub reveal_deadline: BlockNumber,   // MUST be < finalize_block (enforced)
    pub finalize_block: BlockNumber,    // B_fin > reveal_deadline; the entropy anchor
    pub deposit: Balance,               // FER commit bond (validators only), >= max redirectable prize
}

pub struct DrawRecord {
    pub config: DrawConfig,             // SNAPSHOTTED per draw — immutable once opened
    pub period_start_block: BlockNumber,// windowing is by BLOCK HEIGHT, not Moment
    pub period_end_block: BlockNumber,
    pub revenue_snapshot: FiatAmount,   // PeriodTaxRevenue frozen at period_end transition
    pub pool: FiatAmount,               // eTWD accounting, snapshotted at funding
    pub state: DrawState,
}

/// === NEW ferrum-zk circuit I/O — these are NOT verify_age_threshold ===

/// Eligibility circuit public inputs (register_ticket). Proves: prover holds the
/// purchase secret committed in the invoice; issuer_commitment ∈ MerchantSetRoot;
/// age ≥ threshold; and nullifier == H(owner_secret ‖ invoice_hash ‖ draw_id ‖ "entry").
pub struct EligibilityPublicInputs {
    pub invoice_hash: Hash32,           // BOUND: the specific invoice being registered
    pub merchant_set_root: Hash32,      // BOUND: registered-merchant accumulator root
    pub draw_id: DrawId,                // domain
    pub threshold: u32,                 // e.g. age >= 18
    pub nullifier: CanonicalNullifier,  // circuit OUTPUT, canonical field element
}

/// Ownership/claim circuit public inputs (claim_prize). Proves: prover knows the
/// owner_secret behind owner_commitment for invoice_hash; nullifier ==
/// H(owner_secret ‖ invoice_hash ‖ draw_id ‖ "claim"); viewing_key_commitment is a
/// correct encryption-to-auditor of the bound identity; and beneficiary is committed.
pub struct ClaimPublicInputs {
    pub invoice_hash: Hash32,           // BOUND
    pub owner_commitment: Commitment,   // BOUND: fed from on-chain Tickets[invoice_hash]
    pub draw_id: DrawId,                // domain
    pub beneficiary: AccountId32,       // BOUND: the payout recipient
    pub nullifier: CanonicalNullifier,  // circuit OUTPUT, canonical field element
    pub viewing_key_commitment: Commitment, // verified OUTPUT (encryption-to-auditor)
}
```

**Canonical nullifiers.** `CanonicalNullifier` is a 32-byte value the circuit constrains to be a canonical BLS12-381 scalar (high bits constrained so `from_le_bytes_mod_order` is the identity, no reduction). On-chain anti-replay maps key on this canonical representation — the exact value the proof binds — eliminating the storage-vs-circuit representation mismatch that arises from reducing arbitrary 32-byte values mod the field. Entry vs claim nullifiers use distinct domain tags (`"entry"` / `"claim"`).

Reused verbatim: `Hash32`, `Commitment` (=`Hash32`), `Did`, `FiatAmount { currency: [u8;3], minor_units: u128 }`, `ProofBytes` (`BoundedVec<u8,2048>`), `VerifyingKeyBytes`, `TaxKind`, `InvoiceAnchor`. **New** helpers on `FiatAmount` (currency-asserting `checked_add` / `checked_sub` on `minor_units`) are introduced here — they do not exist in `primitives` today.

### Config trait

```rust
#[pallet::config]
pub trait Config: frame_system::Config + pallet_tax::Config {
    type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

    /// Read-only access to pallet-tax::Invoices, incl. the anchoring block height.
    type Tax: InvoiceRegistry;     // invoice(&Hash32)->Option<InvoiceAnchor>; anchored_block(&Hash32)->Option<BlockNumber>; is_anchored(&Hash32)->bool
    /// Authenticated, value-backed revenue feed (NEW callback from pallet-tax settlement).
    type RevenueFeed: AuthenticatedRevenue;   // see Integration; not a permissionless extrinsic
    /// eTWD prize receipt recorder (NEW, restricted-origin method on pallet-treasury-fer).
    type PrizeTreasury: TreasuryPayout<Self::AccountId>;
    /// On-chain-attested eTWD reserve balance the CB updates; pool is clamped & debited against it.
    type EtwdReserve: AttestedReserve;        // attested_balance()->FiatAmount; try_debit(FiatAmount)->DispatchResult

    type GovernanceOrigin: EnsureOrigin<Self::RuntimeOrigin>;   // L4; same as tax/treasury (§14)
    type EmergencyOrigin:  EnsureOrigin<Self::RuntimeOrigin>;   // break-glass (CB ⊕ auditor)
    type RegistrarOrigin:  EnsureOrigin<Self::RuntimeOrigin>;   // MerchantSetRoot admin

    type PrizeCurrency: Get<FiatCurrency>;     // b"TWD" — every payout asserted equal-currency
    type LocalCountry:  Get<CountryId>;        // b"tw"
    type MaxRatioPpm:   Get<u32>;              // constitutional ceiling, e.g. 20_000 (2%)
    type MinPeriod:     Get<BlockNumberFor<Self>>;
    type ClaimWindow:   Get<BlockNumberFor<Self>>;
    type MinReveals:    Get<u32>;              // quorum for a valid draw (set high)
    type WeightInfo: WeightInfo;
}
```

**No `type Randomness`.** The commit–reveal-over-finalized-hash beacon is the *sole* entropy source; the runtime wires no `frame_support::traits::Randomness` provider (Aura has no VRF, and `RandomnessCollectiveFlip` is author-grindable — exactly what this design rejects). The associated type is deliberately omitted to prevent a contributor from wiring an insecure beacon.

### Storage

| Item | Type | Purpose |
|---|---|---|
| `ActiveConfig` | `StorageValue<DrawConfig>` | Governed template for the next draw. |
| `Draws` | `StorageMap<Blake2_128Concat, DrawId, DrawRecord>` | Per-draw immutable snapshot + state. |
| `NextDrawId` | `StorageValue<DrawId, ValueQuery>` | Monotonic id. |
| `CurrentDraw` | `StorageValue<DrawId, ValueQuery>` | The `Open` draw entries accrue to. |
| `Paused` | `StorageValue<bool, ValueQuery>` | Global kill-switch. |
| `MerchantSetRoot` | `StorageValue<Hash32>` | Accumulator root of registered merchants; circuit proves `issuer_commitment` membership. |
| `Tickets` | `StorageMap<Blake2_128Concat, Hash32, LotteryTicket>` | `invoice_hash → ticket`. Key reuse enforces **one ticket per invoice**. |
| `DrawTicketCount` | `StorageMap<Blake2_128Concat, DrawId, u32>` | Entries per draw. |
| `EntryNullifiers` | `StorageMap<Blake2_128Concat, CanonicalNullifier, ()>` | Anti-replay on registration (canonical key). |
| `ClaimNullifiers` | `StorageMap<Blake2_128Concat, CanonicalNullifier, ()>` | A winning ticket claimable at most once (canonical key). |
| `EntryRoot` | `StorageMap<Blake2_128Concat, DrawId, Hash32>` | Merkle root over sorted eligible `invoice_hash` set. |
| `EntryCount` | `StorageMap<Blake2_128Concat, DrawId, u64>` | Size of entry set. |
| `Commits` | `StorageDoubleMap<DrawId, AccountId, Commit>` | Per-participant commit + reveal flag. |
| `RevealedXor` | `StorageMap<Blake2_128Concat, DrawId, Hash32>` | Running XOR of revealed seeds. |
| `RevealCount` | `StorageMap<Blake2_128Concat, DrawId, u32>` | Valid reveals (quorum). |
| `Entropy` | `StorageMap<Blake2_128Concat, DrawId, Hash32>` | Final `R` (published). |
| `Winners` | `StorageDoubleMap<DrawId, u8 /*tier*/, BoundedVec<Hash32, ConstU32<1024>>>` | Winning `invoice_hash`es per tier. |
| `PeriodTaxRevenue` | `StorageMap<Blake2_128Concat, PeriodId, FiatAmount, ValueQuery>` | Running per-period authenticated revenue tally (mutating; frozen into `DrawRecord.revenue_snapshot` at period end). |
| `PrizePool` | `StorageMap<Blake2_128Concat, DrawId, FiatAmount, ValueQuery>` | Funded eTWD accounting pool per draw (fiat counter; distinct from FER `Pools`). |
| `ClaimAudit` | `StorageMap<Blake2_128Concat, Hash32, Commitment>` | `claim_receipt_key → verified viewing-key commitment` (immutable audit). |
| `EligibilityVk` | `StorageValue<VerifyingKeyBytes>` | Governance-set Groth16 VK for the **eligibility** circuit. |
| `OwnershipVk` | `StorageValue<VerifyingKeyBytes>` | Governance-set Groth16 VK for the **ownership/claim** circuit. |

**VK safety.** `register_ticket`/`claim_prize` verify **only** against `EligibilityVk`/`OwnershipVk` from storage, which MUST be governance-set and MUST NOT be empty/default. A `decode_vk(&Vec::new())` stub (as in `pallet-tax::prove_bracket`) is explicitly forbidden; a unit test asserts an empty VK is rejected (`InvalidVk`).

### Extrinsics (call index, origin, effect)

| # | Extrinsic | Origin | Effect |
|---|---|---|---|
| 0 | `register_ticket(invoice_hash, owner_commitment, proof, inputs)` | `ensure_signed`, **fee-free** | Anchored, merchant-signed invoice → ticket. See flow. |
| 1 | `claim_prize(draw_id, tier, invoice_hash, proof, inputs)` | `ensure_signed`, **fee-free** (relayable) | Verify winner + ZK ownership (beneficiary-bound) + nullifier; record eTWD receipt; write verified audit. |
| 2 | `set_eligibility_vk(vk)` / `set_ownership_vk(vk)` | `GovernanceOrigin` | Store/rotate circuit VKs; reject empty. |
| 3 | `set_tax_ratio(ppm)` | `GovernanceOrigin` | `ensure!(ppm <= MaxRatioPpm)`. Effective next draw (snapshot). |
| 4 | `set_tiers(tiers)` | `GovernanceOrigin` | `ensure!(Σ share_ppm == 1_000_000)`; each `winners>0`. |
| 5 | `set_cadence(period_blocks)` | `GovernanceOrigin` | `ensure!(period_blocks >= MinPeriod)`. |
| 6 | `set_eligible_kinds(kinds)` / `set_allow_foreign(bool)` | `GovernanceOrigin` | Scope entry pool / cross-border toggle. |
| 7 | `set_merchant_set_root(root)` | `RegistrarOrigin` | Update registered-merchant accumulator root. |
| 8 | `open_draw()` | `GovernanceOrigin` | Bootstrap / re-open; snapshot `ActiveConfig` into a fresh `DrawRecord`. |
| 9 | `commit(draw_id, commitment)` | validator/council signed | Reserve `deposit` FER; store `Commit`. Reject after `commit_deadline`. |
| 10 | `reveal(draw_id, seed, salt)` | committer signed | Require `H(seed‖salt)==commitment`, `block <= reveal_deadline` **and `block < finalize_block`**; `RevealedXor ^= seed`; unreserve. |
| 11 | `seal_entry_set(draw_id, entry_root, entry_count)` | `GovernanceOrigin` | After `period_end_block` and after revenue snapshot exists, anchor Merkle root + count of eligible invoices (recomputable by anyone). |
| 12 | `fund_period(draw_id)` | `GovernanceOrigin` | Read **only** `DrawRecord.revenue_snapshot`; compute pool §3; clamp to `EtwdReserve::attested_balance × rc`; `EtwdReserve::try_debit(pool)` atomically; write `PrizePool`. Fails closed if reserve insufficient. |
| 13 | `finalize_draw(draw_id)` | permissionless signed | `block > finalize_block` **and `block <= finalize_block + BlockHashCount`** (else `EntropyExpired` → re-seed/cancel path); `RevealCount >= MinReveals`; `EntryRoot` set; slash non-revealers; `R = blake2_256(RevealedXor_with_fallbacks ‖ block_hash(finalize_block) ‖ draw_id)`; select winners (bounded work); `state=Drawn`. |
| 14 | `auto_refund(draw_id)` | permissionless, post-deadline | If quorum missed by `finalize_block + grace`, return funded pool to `EtwdReserve` and mark `Cancelled` — **trustless** path, no governance needed. |
| 15 | `sweep_expired(draw_id)` | permissionless, post-window | `ensure!(block > draw_block + ClaimWindow)` strictly; idempotent (→ `Settled`); move exactly residual `PrizePool[draw]` back to `EtwdReserve` atomically. |
| 16 | `emergency_pause()` | `EmergencyOrigin` | `Paused=true`; stops auto-draw and payout. Idempotent. |
| 17 | `resume()` | `GovernanceOrigin` | Clear `Paused` after review (deliberately higher origin than pause). |
| 18 | `cancel_draw(id)` | `GovernanceOrigin` | Mark `Cancelled`; return snapshotted pool to `EtwdReserve`. Cannot cancel `Settled`. |

**Cadence + snapshot driver** — `on_initialize(now)`: if `!Paused` and `now >= CurrentDraw.period_end_block` and state `Open`: **freeze** `DrawRecord.revenue_snapshot = PeriodTaxRevenue[draw]`, set `Drawing`, emit `DrawClosed`, and `open_next_draw` (snapshots `ActiveConfig`). Funding is forbidden before the snapshot exists. Cadence is emergent from `period_blocks`; survives restarts; no cron.

### `register_ticket` flow

1. `ensure_signed`; `ensure!(!Paused)`.
2. `anchor = T::Tax::invoice(&invoice_hash).ok_or(InvoiceNotAnchored)?` — must already exist in `pallet-tax::Invoices` (which, post-hardening, means **merchant-signed**).
3. `ensure!(eligible_kinds.contains(anchor.kind), IneligibleInvoiceKind)` (e.g. `ValueAdded`).
4. `b = T::Tax::anchored_block(&invoice_hash).ok_or(InvoiceNotAnchored)?`; `draw = derive_draw_by_block(b)`; `ensure!(phase(draw)==Open, DrawClosed)`. **Windowing is by anchoring block height, never `anchored_at` Moment.**
5. `ensure!(!Tickets::contains_key(invoice_hash), TicketAlreadyRegistered)`.
6. `ensure!(inputs.nullifier is canonical && !EntryNullifiers::contains_key(inputs.nullifier), NullifierUsed)`.
7. `ensure!(inputs.invoice_hash == invoice_hash && inputs.draw_id == draw && inputs.merchant_set_root == MerchantSetRoot, ProofInputMismatch)`. Verify the **eligibility** proof vs `EligibilityVk` (non-empty) via `ferrum_zk::{decode_proof, decode_vk, verify_proof}` over `EligibilityPublicInputs`. The circuit proves possession of the invoice purchase secret, `issuer_commitment ∈ MerchantSetRoot`, age ≥ threshold, and `nullifier = H(owner_secret ‖ invoice_hash ‖ draw_id ‖ "entry")`. `ensure!(ok, InvalidProof)`.
8. Insert `Tickets`, `EntryNullifiers`; `DrawTicketCount[draw] += 1`.
9. Emit `TicketRegistered { invoice_hash, draw }`.

### `claim_prize` flow

1. `ensure_signed` (relayer ok); `ensure!(!Paused)`.
2. `ensure!(block <= draw_block + ClaimWindow, ClaimWindowClosed)`.
3. `ensure!(invoice_hash ∈ Winners[draw_id][tier], NotAWinner)`.
4. `ensure!(state == Drawn || state == Settled, WrongPhase)`.
5. `ensure!(inputs.nullifier is canonical && !ClaimNullifiers::contains_key(inputs.nullifier), AlreadyClaimed)`.
6. `ticket = Tickets::get(invoice_hash).ok_or(NotAWinner)?`. Bind public inputs: `ensure!(inputs.invoice_hash == invoice_hash && inputs.owner_commitment == ticket.owner_commitment && inputs.draw_id == draw_id && inputs.beneficiary == <intended recipient>, ProofInputMismatch)`. **The beneficiary account is a circuit public input**, so a copied proof can only ever pay the legitimately intended recipient.
7. Verify the **ownership** proof vs `OwnershipVk` (non-empty); the circuit proves knowledge of `owner_secret` behind `owner_commitment`, `nullifier = H(owner_secret ‖ invoice_hash ‖ draw_id ‖ "claim")`, and that `viewing_key_commitment` is a correct encryption-to-auditor of the bound identity. `ensure!(ok, InvalidProof)`.
8. `amount = per_winner(tier)`; `ensure!(amount.currency == PrizeCurrency, CurrencyMismatch)`; `PrizePool[draw] = PrizePool[draw].checked_sub(amount).ok_or(InsufficientPool)?`.
9. `receipt_key = blake2_256(invoice_hash ‖ inputs.nullifier)`; `T::PrizeTreasury::credit_fiat(beneficiary, receipt_key, amount)?` — records a PII-free receipt in `EtwdReceipts` under a **fresh per-claim key** (not the raw `invoice_hash`, so it cannot be squatted or cross-linked). The eTWD value moves off-chain on the CBDC rail.
10. Insert `ClaimNullifiers`; `ClaimAudit[receipt_key] = inputs.viewing_key_commitment` (verified, not caller-asserted).
11. Emit `PrizeClaimed { receipt_key, draw, tier, amount }` (note: keyed on `receipt_key`, **not** `invoice_hash`, to avoid linking payout to the winning invoice).

### End-to-end draw flow

```
open_draw ─► [Open] register_ticket × M (fee-free; merchant-signed, ZK invoice-possession)
                 │  validators commit() before commit_deadline (FER bonded, deposit >= max redirectable)
on_initialize ─► period_end_block ─► FREEZE revenue_snapshot ─► [Drawing]
                 │  reveal() before reveal_deadline AND before finalize_block ─► RevealedXor accrues
seal_entry_set ─► EntryRoot/EntryCount over sorted eligible invoice_hash set (windowed by anchoring block)
fund_period   ─► pool = clamp(r × revenue_snapshot, attested_eTWD_reserve × rc); EtwdReserve.try_debit (atomic)
finalize_draw ─► finalize_block < block <= finalize_block+BlockHashCount; slash non-revealers;
                 R = blake2_256(RevealedXor_with_fixed_fallbacks ‖ block_hash(B_fin) ‖ draw_id)
                 winners = expand(R) over entry set (bounded probes) ─► [Drawn]
                 (quorum missed / entropy expired ─► auto_refund ─► [Cancelled], trustless)
claim_prize × ─► ZK ownership (beneficiary-bound) + canonical nullifier ─► eTWD receipt (fresh key) ─► [Settled]
sweep_expired ─► residual eTWD accounting ─► sovereign reserve (idempotent, strictly post-window)
```

### Winner selection (bounded, conservation-safe)

For tier `t`, slot `j`: `idx = blake2_256(R ‖ "ferrum-draw" ‖ draw_id ‖ tier_id ‖ j)` as u256; `pos = idx mod EntryCount`; winner is the leaf at `pos` in the **sorted** entry list. Sampling without replacement, but with a **bounded probe count** `MAX_PROBES` per slot to cap weight; if probes exhaust, the slot's amount is recycled to reserve. If `winners_requested > EntryCount`, winners are **capped to `EntryCount`** and the unallocated tier amount is recycled (`PoolRecycled`). `finalize_draw` weight is benchmarked against the worst-case rejection-sampling bound. Selection is fully deterministic from `R` and the public sorted entry list → recomputable by anyone with a Merkle proof per leaf. (Because output is deterministic from `R`, the randomness fixes above — no last-revealer bias, no author grinding — are load-bearing for selection integrity.)

### Events / Errors

**Events:** `DrawOpened`, `TicketRegistered{invoice_hash,draw}`, `Committed`, `Revealed`, `RevenueSnapshotted{draw,revenue}`, `EntrySetSealed`, `PrizePoolFunded{draw,revenue,pool,tiers}`, `PrizePoolCapped`, `Sealed{draw,entropy}`, `DrawCompleted`, `PrizeClaimed{receipt_key,draw,tier,amount}`, `PoolRecycled`, `AutoRefunded{draw}`, `ExpiredSwept`, `LotteryPaused`, `LotteryResumed`, `DrawCancelled`, `EligibilityVkSet`, `OwnershipVkSet`, `MerchantSetRootSet`. (All keyed on hashes/ids/amounts — **no PII**; payout events use `receipt_key`, never the winning `invoice_hash`.)

**Errors:** `InvoiceNotAnchored`, `IneligibleInvoiceKind`, `DrawClosed`, `TicketAlreadyRegistered`, `NullifierUsed`, `NonCanonicalNullifier`, `InvalidProof`, `MalformedProof`, `InvalidVk`, `ProofInputMismatch`, `NotAWinner`, `AlreadyClaimed`, `ClaimWindowClosed`, `InsufficientPool`, `CurrencyMismatch`, `RatioTooHigh`, `TiersNotExhaustive`, `CadenceTooShort`, `CommitClosed`, `AlreadyCommitted`, `BadReveal`, `RevealAfterFinalizeBlock`, `InsufficientReveals`, `EntropyExpired`, `RevenueNotSnapshotted`, `EntrySetNotSealed`, `TooEarlyToFinalize`, `WrongPhase`, `ForeignEntriesDisabled`, `LotteryPaused`, `NotYetExpired`, `InsufficientReserve`.

### Integration — exactly how it composes

**With `pallet-tax` (required modifications, acknowledged as real code changes):**
- **REQUIRED:** `anchor_invoice` MUST add `ensure!(who == anchor.issuer)` so only the merchant key-holder can anchor — this is load-bearing for ticket integrity, not optional. Because `InvoiceAnchor.issuer` is `ferrum_primitives::AccountId` (not `T::AccountId`), the check converts `who` into the primitives `AccountId` representation (or stores the issuer as `T::AccountId`); the type reconciliation is part of this change.
- **REQUIRED:** `anchor_invoice` MUST capture and store `System::block_number()` at anchor time (a new field/side-map), because windowing and draw derivation use the anchoring **block height**, never the caller-supplied `anchored_at` Moment (which is `u64` ms and validator-influenceable, and cannot be compared to `BlockNumber` bounds). The read trait exposes `anchored_block(&Hash32) -> Option<BlockNumber>`.
- Add a read trait `InvoiceRegistry` over `Invoices` for `register_ticket` step 2/4 and entry-set sealing. Entry pool = `Invoices` keys with `anchored_block ∈ [period_start_block, period_end_block)` and `kind ∈ eligible_kinds`. `invoice_hash` is reused as ticket ID.
- **Authenticated revenue (real change to the settlement path, not "just reads it"):** `PeriodTaxRevenue` is incremented **only** by `T::RevenueFeed`, a callback the settlement path invokes for **authenticated, value-backed, VAT (`ValueAdded`)** settlements, deduplicated by receipt and cross-checked against actual eTWD movement. This requires a new hook/trait callback added to the settlement code: since `FiatAmount` carries currency but not `TaxKind`, the integration point is `pallet-tax::settle`, where the obligation's invoice kind is resolvable, **not** the permissionless `pallet-treasury-fer::record_settlement` (which moves no value and can be called by anyone with an arbitrary amount). A permissionless receipt-writing extrinsic must never feed `PeriodTaxRevenue`. The tally is frozen into `revenue_snapshot` at the period-end transition, and the same receipt cannot be counted across periods.

**With `pallet-treasury-fer`:**
- **No on-chain eTWD balance ledger exists.** `Pools` holds **FER** `Balance` (pool id 1 = sovereign reserve is a FER allocation), and `EtwdReceipts` is a `Hash32 → FiatAmount` receipt log that moves **no value**. Therefore the prize pool is a **pure accounting counter**: `PrizePool`/`PeriodTaxRevenue`/`revenue_snapshot` are `FiatAmount` tallies, and the actual eTWD disbursement is an **off-chain CBDC instruction**. No claim that `Pools`/pool 1 is debited for prizes — debiting FER would burn a validator bond, contradicting "citizens never hold FER." All earlier "debit `POOL_SOVEREIGN_RESERVE`" language is replaced by debiting the **attested eTWD reserve accounting**.
- **Attested eTWD reserve (`type EtwdReserve`):** a `FiatAmount` StorageValue the central bank attests/updates. `fund_period` clamps the pool to `attested_balance × rc / PPM` and calls `try_debit(pool)` **atomically** with funding; if the attested reserve is insufficient, funding **fails closed**. This makes the "every eTWD prize backed 1:1" guarantee enforced in the same accounting unit, not asserted against an unrelated FER balance.
- **Payout receipt (`type PrizeTreasury`):** add one restricted-origin credit method `trait TreasuryPayout<AccountId> { fn credit_fiat(beneficiary: &AccountId, receipt_key: Hash32, amount: FiatAmount) -> DispatchResult; }`, mirroring `do_record_settlement`'s `ReceiptAlreadyRecorded` dedup but **callable only by this pallet's internal origin** (so external callers cannot pre-squat a receipt key to DoS a winner). It records a PII-free receipt under `receipt_key = H(invoice_hash ‖ claim_nullifier)`; this distinct key also avoids any spurious `ReceiptAlreadyRecorded` collision with the invoice's own settlement receipt under `pallet-tax::settle`. The receipt is purely a commitment + amount; **the winner receives value off-chain on the CBDC rail** — the chain pays no one directly.

**Cross-border (optional, §09 `id="interop"`):** only if `allow_foreign=true`; a foreign invoice is an entry only once recognized via existing `pallet_interop::recognize_foreign_invoice` (requires a GRANDPA-finalized head). Only `invoice_hash` + `CountryId` cross — never PII. A foreign winner is still paid in the drawing nation's CBDC; no XSU prize. Federation governs interop only, never a nation's lottery parameters.

---

## Part 3 — Economic Model (稅務等比率)

All arithmetic is integer on `u128` minor units, floor division, `PPM = 1_000_000`. Currency of every `FiatAmount` is asserted `== b"TWD"` (eTWD) before any add/sub (the currency-asserting helpers are **new**, not in `primitives` today). **Money-in = money-out is a pure accounting identity over fiat counters**; the actual eTWD value lives on the CBDC rail off-chain, clamped to and debited from the central bank's on-chain-attested eTWD reserve.

### Notation

| Symbol | Meaning | Source |
|---|---|---|
| `Trev` | period **authenticated** settled VAT revenue (minor units, eTWD) | `DrawRecord.revenue_snapshot` (frozen at period end) |
| `r` | funding ratio (ppm) | `tax_ratio_ppm` |
| `w_k` | tier `k` share (ppm), `Σ w_k = PPM` | `tiers[k].share_ppm` |
| `n_k` | winners in tier `k` | `tiers[k].winners` |
| `cap_k` | per-winner cap (minor units) | `tiers[k].unit_cap` |
| `Rsv` | **CB-attested on-chain eTWD reserve balance** | `EtwdReserve::attested_balance()` |
| `rc` | reserve draw cap (ppm) | `reserve_cap` |

### Formulas

**(a) Pool sizing — the headline 等比率 (fully-reserved by atomic clamp+debit):**
```
Pool_raw   = floor(Trev × r / PPM)
ReserveCap = floor(Rsv × rc / PPM)          // Rsv is attested eTWD, same unit as the prize
Pool       = min(Pool_raw, ReserveCap)
EtwdReserve.try_debit(Pool)                 // atomic with funding; FAILS CLOSED if insufficient
```
If `Pool_raw > ReserveCap`, clamp and emit `PrizePoolCapped`. Because the clamp and debit are against the **same attested eTWD quantity** and occur atomically with funding (and the revenue base is a frozen snapshot), solvency is **enforced**, not asserted.

**(b) Tier allocation — fixed-proportion split (conservation-exact, dust to reserve):**
```
Tier_k    = floor(Pool × w_k / PPM)   for each k
remainder = Pool − Σ_k Tier_k          // floor dust
```
The floor-division `remainder` is **recycled to reserve** (`PoolRecycled`), never folded into any tier (folding into a capped tier would be a silent no-op; folding into an uncapped tier would be order-dependent). This makes conservation accounting unambiguous and order-independent. Invariant checked: `Σ_k Tier_k + remainder = Pool ≤ ReserveCap`.

**(c) Per-winner payout (flat mode is the default and only currently-safe mode):**
```
payout_k   = min( floor(Tier_k / n_k), cap_k )
overflow_k = Tier_k − payout_k × n_k       // recycled to reserve (PoolRecycled)
```
**Unspendable residue always recycles to the sovereign reserve** — never to a capped tier and never carried implicitly.

*Contribution-weighted mode (strong 等比率, odds ∝ tax paid) is **disabled until** `c_i` is bound to an authenticated settlement commitment.* `c_i` would have to equal the eTWD receipt amount `pallet-treasury-fer` actually recorded for that invoice, supplied as a committed public input the circuit checks against on-chain state; a self-asserted ZK `c_i` lets an attacker claim an arbitrarily large share. Until that binding exists, only the flat mode is permitted.

### Worked example — one draw period

Inputs: `Trev = 50,000,000,000` minor units (NT$500,000,000.00) · `r = 2,000 ppm` (0.2%) · `w = [500000, 300000, 200000]` · `Rsv = 80,000,000,000` (NT$800,000,000.00, attested eTWD) · `rc = 50,000 ppm` (5%) · tier winner counts `n = [1, 100, 10000]` · `cap = [30,000,000, 1,000,000, 200,000]`.

```
Pool_raw   = 50,000,000,000 × 2000 / 1,000,000 = 100,000,000        (NT$1,000,000.00)
ReserveCap = 80,000,000,000 × 50000 / 1,000,000 = 4,000,000,000     (attested eTWD × 5%)
Pool       = min(100,000,000, 4,000,000,000)   = 100,000,000        (cap NOT hit)
EtwdReserve.try_debit(100,000,000)             = Ok                 (atomic with funding)

Tier_0 (50%) = 50,000,000
Tier_1 (30%) = 30,000,000
Tier_2 (20%) = 20,000,000
Σ = 100,000,000 = Pool  ✓  (remainder = 0 ⇒ no dust to recycle)

Per-winner:
  Tier_0: floor(50,000,000 / 1)     = 50,000,000 → min(·,30,000,000) = 30,000,000
          overflow_0 = 50,000,000 − 30,000,000 = 20,000,000  → PoolRecycled to reserve
  Tier_1: floor(30,000,000 / 100)   = 300,000    → min(·,1,000,000)  = 300,000   (×100 = 30,000,000, overflow 0)
  Tier_2: floor(20,000,000 / 10000) = 2,000      → min(·,200,000)    = 2,000     (×10000 = 20,000,000, overflow 0)

Paid out (if all claimed): 30,000,000 + 30,000,000 + 20,000,000 = 80,000,000
Recycled to sovereign reserve:                       20,000,000 (Tier_0 overflow)
Σ accounted = 100,000,000 = Pool  ✓  (closed loop; eTWD never minted on-chain, value moves on CBDC rail)
```

Stress check — reserve clamp engaged: if instead `r = 100,000 ppm` (10%, still ≤ a `MaxRatioPpm` of e.g. 200,000) then `Pool_raw = 5,000,000,000 > ReserveCap = 4,000,000,000` → `Pool = 4,000,000,000`, `EtwdReserve.try_debit(4,000,000,000)` succeeds against the attested balance, `PrizePoolCapped` emitted, every prize still backed 1:1 by attested eTWD. Any unclaimed amount after the ~90-day `ClaimWindow` is returned to the attested eTWD reserve accounting by `sweep_expired` (strictly post-window, idempotent), keeping money-in (authenticated settled tax eTWD) = money-out (paid winners + recycled). The prize pool is a strictly closed-loop accounting reallocation of fully-reserved eTWD whose value moves on the CBDC rail.

---

### Constraint compliance (whole design)

| Constraint | How honored |
|---|---|
| No plaintext PII on-chain | Storage/events carry only `Hash32`/`Commitment`/canonical `Nullifier`/`Did`/`FiatAmount`; tickets are `invoice_hash`; eligibility & ownership proven in **two new ZK circuits** (invoice-possession, owner-binding, beneficiary-binding, nullifier-derivation); audit via a **verified** viewing-key commitment; payout events keyed on `receipt_key`, not the winning invoice. Residual merchant-side metadata leakage from the public anchor set is disclosed and mitigated via `MerchantSetRoot`/`issuer_commitment`. |
| Non-speculative / fiat / fully-reserved | Pool, funding cut, tiers, payouts all `FiatAmount` (eTWD) accounting counters; pool is **clamped to and atomically debited from the CB-attested on-chain eTWD reserve**, fails closed if insufficient; the chain mints/holds no spendable eTWD and pays no one directly — value moves on the CBDC rail; FER is only a validator commit bond; citizens never hold FER; participation/claim fee-free. |
| National sovereignty / no global token | One `pallet-lottery` + one CBDC per nation; `GovernanceOrigin` is local L4 (§14); federation governs interop (§09) only; foreign winners paid in the drawing nation's CBDC; no XSU/cross-border value transfer of the prize. |
| Manipulation resistance | Commit–reveal XOR (unbiasable if ≥1 honest) anchored to a GRANDPA-finalized block hash, with `reveal_deadline < finalize_block` (reveals rejected at/after `finalize_block`), finalize bounded within `BlockHashCount` (else re-seed/cancel), missing reveals replaced by a **fixed published fallback** (not omission), `commit_deposit ≥ max redirectable prize`, high `MinReveals`, a **trustless `auto_refund`** on quorum-miss, non-revealer slashing on a timeout path, and bounded winner-selection probes. No `frame` `Randomness` provider is wired. |
| Reuse over reinvention | Entries = `pallet-tax::Invoices` anchors (with required `who==issuer` + anchoring-block hardening); revenue from an authenticated settlement callback; payout = restricted-origin `credit_fiat` recording PII-free `EtwdReceipts`; ZK = `ferrum-zk` `decode_proof`/`decode_vk`/`verify_proof` plumbing with **two purpose-built circuits/VKs** (eligibility, ownership) — `verify_age_threshold` is age-only and is **not** reused for ownership semantics. Only the draw beacon, prize-pool accounting, and the two circuits are new. |

**Files grounding this design of record:** `G:\project\ferrum\ferrum\crates\primitives\src\lib.rs` (`Hash32`/`Commitment`/`Did`/`FiatAmount{currency:[u8;3],minor_units:u128}`/`AgeProofPublicInputs`/`TaxKind`/`InvoiceAnchor{issuer:AccountId, anchored_at:Moment}`), `G:\project\ferrum\ferrum\crates\zk\src\age_proof.rs` (`verify_age_threshold`/`public_inputs_from`/`from_le_bytes_mod_order` — motivates canonical nullifiers and the new circuits), `G:\project\ferrum\ferrum\pallets\tax\src\lib.rs` (`Invoices`/`anchor_invoice` `ensure_signed` with discarded `_who`/`AuditLog`/`prove_bracket` empty-VK stub/`GovernanceOrigin`), `G:\project\ferrum\ferrum\pallets\treasury\src\lib.rs` (`TreasurySettle::settle_fiat`/`EtwdReceipts`/`record_settlement` permissionless/`ReceiptAlreadyRecorded`/`POOL_SOVEREIGN_RESERVE=1`/`Pools:StorageMap<u8,Balance>` = FER), `G:\project\ferrum\ferrum\runtime\src\consensus.rs` (Aura+GRANDPA, no native VRF — motivates the finalized-hash entropy anchor), `G:\project\ferrum\ferrum\index.html` (TOC anchors §06 `id="tax"`, §08 `id="token"`, §09 `id="interop"`, §13 `id="security"`, §14 `id="governance"`; bilingual paired-sibling section style).