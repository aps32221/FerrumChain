# Ferrum 鐵鏈 — Module Author Contract (SPEC.md)

**Status: BINDING.** This document is the integration contract between independent
module authors and the workspace. Code that does not honor it will not link.

The authoritative design is the whitepaper at `index.html`. Section numbers
below (§NN) refer to it. Honor the exact file paths and API shapes the
whitepaper shows:

- `pallets/identity/src/lib.rs`
- `crates/zk/src/age_proof.rs`
- `runtime/src/consensus.rs`
- `pallets/federation/src/voting.rs`
- `pallets/federation/src/lib.rs`

---

## 0. Global rules (apply to EVERY crate)

1. **Pinned Substrate source — copy VERBATIM.** Every FRAME/Substrate
   dependency comes ONLY from the root `[workspace.dependencies]` table, pinned to:

   > **`polkadot-sdk` @ git tag `polkadot-stable2412`**
   > (`https://github.com/paritytech/polkadot-sdk.git`, `tag = "polkadot-stable2412"`)

   In your crate's `Cargo.toml` reference deps as, e.g.:
   ```toml
   frame-support = { workspace = true }
   frame-system  = { workspace = true }
   codec         = { workspace = true }
   scale-info    = { workspace = true }
   ferrum-primitives = { workspace = true }
   ```
   **Do NOT** introduce a different git rev/tag or a crates.io version of any
   FRAME crate. One source, pinned in one place.

2. **`no_std` mandatory.** First line of every `lib.rs`:
   ```rust
   #![cfg_attr(not(feature = "std"), no_std)]
   ```
   Provide a `std` feature that turns on `std` for every dependency, and add
   `default = ["std"]`. Also provide `runtime-benchmarks` and `try-runtime`
   feature passthroughs.

3. **Every pallet MUST provide:**
   - a `#[frame_support::pallet]` module with a `Config` trait
     (`Config: frame_system::Config`), `#[pallet::storage]` items,
     `#[pallet::call]` extrinsics, `#[pallet::event]`, `#[pallet::error]`;
   - `src/mock.rs` (a `construct_runtime!` test runtime) — feature/`cfg(test)` gated;
   - `src/tests.rs` (unit tests against the mock);
   - `src/weights.rs` (a `WeightInfo` trait + a `SubstrateWeight<T>` impl, even
     if values are placeholder) and a `#[cfg(feature = "runtime-benchmarks")]`
     `src/benchmarking.rs` stub using `frame_benchmarking::v2`.

4. **Privacy invariant (§03/§05/§06/§09).** NEVER store plaintext PII on-chain.
   Persist only `ferrum_primitives::{Hash32, Commitment, Nullifier}` and the
   anchor structs already defined in primitives. Plaintext lives in agency-run
   off-chain encrypted vaults.

5. **Consume shared types from `ferrum-primitives`.** Do not redefine `Did`,
   `Balance`, `AccountId`, `XsuBasket`, `Vote`, `FederationAction`, etc. Re-export
   from primitives if a downstream crate needs them.

6. **Public surface for the runtime.** Each pallet's `Config` associated types
   and its `Pallet`/`Call`/`Event`/`Error` are what the runtime wires. Keep
   `Config` bounds minimal and document every associated type.

---

## 1. `ferrum-primitives` — DONE (do not modify without lead sign-off)

- **Crate / path:** `ferrum-primitives` / `crates/primitives`
- Already implemented. It exports the canonical types listed throughout this
  SPEC. If you believe a shared type is missing, request it from the lead — do
  not add module-local duplicates.

Key exports you will consume:
`AccountId, Balance, Nonce, BlockNumber, Moment, Hashing, Hash, Hash32,`
`Commitment, Nullifier, Signature, Did, DidDocument, DidKeyRef, KeyKind,`
`CredentialAnchor, CredentialKind, CredentialStatus, FiatAmount, FiatCurrency,`
`TaxBracket, TaxKind, InvoiceAnchor, TaxObligation, CountryId, FederationId,`
`MemberId, ProposalId, Vote, GovernanceDomain, FederationAction, XsuBasket,`
`BasketEntry, XsuAmount, CbdcCode, TrustRegistryEntry, XcmStatus,`
`ClearingInstruction, AgeProofPublicInputs, ProofBytes, VerifyingKeyBytes,`
`FER, FER_DECIMALS, MIN_VALIDATOR_BOND, SLOT_DURATION_MS, MAX_AUTHORITIES,`
`domain_threshold, domain()`.

---

## 2. `ferrum-zk` — Zero-knowledge crate (§05)

- **Crate / path:** `ferrum-zk` / `crates/zk` (file `crates/zk/src/age_proof.rs`)
- **Purpose:** selective-disclosure and tax-bracket ZK proofs using **arkworks
  Groth16/PLONK over BLS12-381** plus BBS+ (§04/§05). Pure verification logic;
  `no_std`-compatible (gate prover/keygen behind `std`).
- **Dependencies:** `ark-bls12-381, ark-groth16, ark-ff, ark-ec, ark-serialize,
  ark-std` (all `{ workspace = true }`), `ferrum-primitives`.
- **Required public API — honor the whitepaper excerpt EXACTLY** (`crates/zk/src/age_proof.rs`):
  ```rust
  pub fn verify_age_threshold(
      proof: &Proof<Bls12_381>,
      vk: &PreparedVerifyingKey<Bls12_381>,
      public_inputs: &[Fr],            // [issuer_commitment, threshold, nullifier]
  ) -> Result<bool, VerifyError> {
      Groth16::<Bls12_381>::verify_proof(vk, proof, public_inputs)
          .map_err(|_| VerifyError::Malformed)
  }
  ```
  Also expose:
  - `pub enum VerifyError { Malformed, InvalidProof }`
  - `pub fn public_inputs_from(p: &AgeProofPublicInputs) -> Vec<Fr>` — maps the
    primitives struct into field elements in the documented order.
  - `pub fn decode_proof(bytes: &ProofBytes) -> Result<Proof<Bls12_381>, VerifyError>`
  - `pub fn decode_vk(bytes: &VerifyingKeyBytes) -> Result<PreparedVerifyingKey<Bls12_381>, VerifyError>`
- **Consumes from primitives:** `AgeProofPublicInputs, Commitment, Nullifier,
  ProofBytes, VerifyingKeyBytes`.
- **Consumed by:** `pallet-identity-fer`, `pallet-tax`, `pallet-interop`.
- **No FRAME dependency** — this is a plain library crate, not a pallet (no
  `Config`, `mock.rs`, `benchmarking.rs` required; DO provide unit tests).

---

## 3. `pallet-identity-fer` — Identity layer (§05)

- **Crate / path:** `pallet-identity-fer` / `pallets/identity` (`pallets/identity/src/lib.rs`)
- **Purpose:** anchor `did:fer` DIDs and revocation commitments; **only the
  `doc_hash` is on-chain, never PII** (§05, §04 excerpt).
- **Required storage:**
  - `Dids: StorageMap<_, Blake2_128Concat, Did, DidDocument>`
  - `DidByController: StorageMap<_, Blake2_128Concat, T::AccountId, Did>`
  - `RevocationAccumulator: StorageValue<_, Commitment, ValueQuery>`
  - `AccreditedIssuers: StorageMap<_, Blake2_128Concat, T::AccountId, bool>`
- **Required extrinsics (Call signatures):**
  ```rust
  pub fn anchor_did(origin, doc: DidDocument) -> DispatchResult;          // issuer-only
  pub fn rotate_keys(origin, did: Did, keys: BoundedVec<DidKeyRef, ConstU32<MAX_DID_KEYS>>) -> DispatchResult;
  pub fn update_revocation(origin, commitment: Commitment) -> DispatchResult; // issuer-only
  pub fn register_issuer(origin, who: T::AccountId) -> DispatchResult;    // governance-only
  ```
- **Config trait (public surface for the runtime):**
  ```rust
  pub trait Config: frame_system::Config {
      type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
      type IssuerOrigin: EnsureOrigin<Self::RuntimeOrigin>;     // accredited issuers
      type GovernanceOrigin: EnsureOrigin<Self::RuntimeOrigin>; // chain governance
      type LocalChainTag: Get<BoundedVec<u8, ConstU32<MAX_TAG_LEN>>>;
      type WeightInfo: WeightInfo;
  }
  ```
- **Consumes from primitives:** `Did, DidDocument, DidKeyRef, Commitment, KeyKind, MAX_DID_KEYS, MAX_TAG_LEN`.
- **Cross-module deps:** `ferrum-zk` (verify presentation proofs). `pallet-credential`,
  `pallet-tax`, `pallet-interop` read `Dids`/`RevocationAccumulator` — expose a
  `pub trait DidRegistry` (or `impl` getters) so they don't depend on storage layout.

---

## 4. `pallet-tax` — Tax administration layer (§06)

- **Crate / path:** `pallet-tax` / `pallets/tax`
- **Purpose:** e-invoice anchoring, programmable withholding, ZK bracket proofs,
  authorized audit; **obligations always fiat-denominated**, settled in eTWD (§06).
- **Required storage:**
  - `Invoices: StorageMap<_, Blake2_128Concat, Hash32, InvoiceAnchor>`
  - `Obligations: StorageMap<_, Blake2_128Concat, (Did, u64), TaxObligation>`
  - `Brackets: StorageValue<_, BoundedVec<TaxBracket, ConstU32<32>>, ValueQuery>`
  - `AuditLog: StorageMap<_, Blake2_128Concat, Hash32, /* access commitment */ Commitment>`
- **Required extrinsics:**
  ```rust
  pub fn anchor_invoice(origin, anchor: InvoiceAnchor) -> DispatchResult;
  pub fn withhold(origin, subject: Did, kind: TaxKind, amount: FiatAmount) -> DispatchResult;
  pub fn file_obligation(origin, obligation: TaxObligation) -> DispatchResult;
  pub fn prove_bracket(origin, proof: ProofBytes, inputs: AgeProofPublicInputs) -> DispatchResult; // reuse ZK shape
  pub fn settle(origin, subject: Did, slot: u64) -> DispatchResult;        // pays in eTWD
  pub fn authorize_audit(origin, invoice: Hash32, viewing_key_commitment: Commitment) -> DispatchResult;
  ```
- **Config trait:**
  ```rust
  pub trait Config: frame_system::Config {
      type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
      type Treasury: ferrum_primitives::TreasurySettle<Self::AccountId>; // see pallet-treasury-fer
      type AuditorOrigin: EnsureOrigin<Self::RuntimeOrigin>;
      type WeightInfo: WeightInfo;
  }
  ```
- **Consumes from primitives:** `Did, InvoiceAnchor, TaxObligation, TaxBracket, TaxKind, FiatAmount, Hash32, Commitment, ProofBytes, AgeProofPublicInputs`.
- **Cross-module deps:** `pallet-identity-fer` (resolve `Did`), `pallet-treasury-fer`
  (settlement), `ferrum-zk` (bracket proof verification).

---

## 5. `pallet-credential` — Verifiable Credentials (§05)

- **Crate / path:** `pallet-credential` / `pallets/credential`
- **Purpose:** anchor issuer-signed VC hashes and lifecycle status; support
  selective-disclosure presentation logging (§05 Flow B). **No claim values on-chain.**
- **Required storage:**
  - `Credentials: StorageMap<_, Blake2_128Concat, Hash32 /* payload_hash */, CredentialAnchor>`
  - `BySubject: StorageMap<_, Blake2_128Concat, Did, BoundedVec<Hash32, ConstU32<64>>>`
  - `Presentations: StorageMap<_, Blake2_128Concat, Nullifier, Commitment>` // one-time, replay-protected
- **Required extrinsics:**
  ```rust
  pub fn issue(origin, anchor: CredentialAnchor) -> DispatchResult;       // issuer-only
  pub fn revoke(origin, payload_hash: Hash32) -> DispatchResult;          // issuer-only
  pub fn set_status(origin, payload_hash: Hash32, status: CredentialStatus) -> DispatchResult;
  pub fn log_presentation(origin, nullifier: Nullifier, commitment: Commitment) -> DispatchResult;
  ```
- **Config trait:**
  ```rust
  pub trait Config: frame_system::Config {
      type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
      type IssuerOrigin: EnsureOrigin<Self::RuntimeOrigin>;
      type WeightInfo: WeightInfo;
  }
  ```
- **Consumes from primitives:** `CredentialAnchor, CredentialKind, CredentialStatus, Did, Hash32, Nullifier, Commitment`.
- **Cross-module deps:** `pallet-identity-fer` (subject DID must exist),
  `ferrum-zk` (presentation proof verification optional).

---

## 6. `pallet-treasury-fer` — Treasury / dual-asset (§08)

- **Crate / path:** `pallet-treasury-fer` / `pallets/treasury`
- **Purpose:** manage **FER** genesis allocation pools, governed low-inflation
  issuance (~3%/yr), EIP-1559-style base-fee burn, subsidy fund that makes
  citizen identity checks & basic filing free; receive **eTWD** tax settlements (§08).
- **Required storage:**
  - `Pools: StorageMap<_, Blake2_128Concat, /* pool id */ u8, Balance>` // staking/treasury/subsidy/dev/ecosystem
  - `EtwdReceipts: StorageMap<_, Blake2_128Concat, Hash32, FiatAmount>`  // tax receipt commitments
  - `TotalBurned: StorageValue<_, Balance, ValueQuery>`
- **Required extrinsics:**
  ```rust
  pub fn mint(origin, pool: u8, amount: Balance) -> DispatchResult;       // governance-only
  pub fn burn(origin, amount: Balance) -> DispatchResult;                 // base-fee burn
  pub fn subsidize(origin, who: T::AccountId, amount: Balance) -> DispatchResult;
  pub fn record_settlement(origin, receipt: Hash32, amount: FiatAmount) -> DispatchResult; // from pallet-tax
  ```
- **Public trait the runtime + `pallet-tax` wire (export from this crate, NOT primitives unless lead adds it):**
  ```rust
  pub trait TreasurySettle<AccountId> {
      fn settle_fiat(payer: &AccountId, receipt: Hash32, amount: FiatAmount) -> DispatchResult;
  }
  ```
  > NOTE: `pallet-tax::Config::Treasury` is bound to this trait. If you prefer it
  > to live in primitives, request the lead to add `ferrum_primitives::TreasurySettle`;
  > until then export it from `pallet-treasury-fer` and have `pallet-tax` depend on it.
- **Config trait:**
  ```rust
  pub trait Config: frame_system::Config {
      type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
      type Currency: frame_support::traits::Currency<Self::AccountId, Balance = Balance>;
      type GovernanceOrigin: EnsureOrigin<Self::RuntimeOrigin>;
      type WeightInfo: WeightInfo;
  }
  ```
- **Consumes from primitives:** `Balance, FiatAmount, Hash32, FER`.
- **Cross-module deps:** consumed by `pallet-tax` (settlement target).

---

## 7. `pallet-federation` — Federation governance (§11)

- **Crate / path:** `pallet-federation` / `pallets/federation`
  (files `pallets/federation/src/lib.rs` and `pallets/federation/src/voting.rs`)
- **Purpose:** treaty council, **dual-majority** voting, timelock queue, XSU
  basket + reserve pool governance, forkless-upgrade enactment (§11).
- **Required storage:**
  - `Members: StorageMap<_, Blake2_128Concat, MemberId, bool>`
  - `BasketWeights: StorageValue<_, BoundedVec<(MemberId, Perbill), ConstU32<MAX_FEDERATION_MEMBERS>>, ValueQuery>` // map form per voting.rs `BTreeMap<MemberId, Perbill>`
  - `Proposals: StorageMap<_, Blake2_128Concat, ProposalId, Proposal<T>>`
  - `NextId: StorageValue<_, ProposalId, ValueQuery>`
  - `Queued: StorageMap<_, Blake2_128Concat, BlockNumber /* eta */, ProposalId>`
- **Required free function — honor `pallets/federation/src/voting.rs` EXACTLY:**
  ```rust
  pub fn passes_dual_majority(
      votes: &BTreeMap<MemberId, Vote>,
      basket: &BTreeMap<MemberId, Perbill>,   // each member's XSU basket weight
      threshold: Perbill,                      // e.g. from_rational(2u32, 3u32)
  ) -> bool {
      let total = votes.len() as u32;
      let ayes = votes.values().filter(|v| **v == Vote::Aye).count() as u32;
      let by_count = Perbill::from_rational(ayes, total.max(1)) >= threshold;
      let ayes_weight = votes.iter()
          .filter(|(_, v)| **v == Vote::Aye)
          .map(|(m, _)| *basket.get(m).unwrap_or(&Perbill::zero()))
          .fold(Perbill::zero(), |a, w| a.saturating_add(w));
      let by_weight = ayes_weight >= threshold;
      by_count && by_weight
  }
  ```
- **Required extrinsics — honor `pallets/federation/src/lib.rs` EXACTLY:**
  ```rust
  pub fn propose(origin: OriginFor<T>, action: FederationAction) -> DispatchResult; // CouncilMember origin
  pub fn vote(origin: OriginFor<T>, id: ProposalId, vote: Vote) -> DispatchResult;
  pub fn close(origin: OriginFor<T>, id: ProposalId) -> DispatchResult;             // runs passes_dual_majority, queues under timelock
  ```
  `on_initialize` must auto-`enact` queued proposals whose `eta` has arrived.
- **Config trait:**
  ```rust
  pub trait Config: frame_system::Config {
      type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
      type CouncilMember: EnsureOrigin<Self::RuntimeOrigin, Success = MemberId>;
      type TimelockFor: Get<BlockNumber>;  // or a fn(&FederationAction)->BlockNumber per domain (§11.2 table)
      type WeightInfo: WeightInfo;
  }
  ```
  Use `ferrum_primitives::domain_threshold(action.domain())` for `Proposal::threshold()`
  and the §11.2 timelock table for `TimelockFor`.
- **Consumes from primitives:** `MemberId, ProposalId, Vote, FederationAction,
  GovernanceDomain, XsuBasket, BasketEntry, domain_threshold, MAX_FEDERATION_MEMBERS`.
- **Cross-module deps:** `pallet-interop` reads `BasketWeights` & `Members` for clearing.

---

## 8. `pallet-interop` — Cross-border bridge & clearing (§09–§10)

- **Crate / path:** `pallet-interop` / `pallets/interop`
- **Purpose:** XCM-style cross-consensus messaging verified via GRANDPA finality
  proofs (no trusted custodian), cross-chain DID/issuer trust registry, and XSU
  multilateral netting / CBDC settlement (§09 Flow E, §10).
- **Required storage:**
  - `TrustRegistry: StorageMap<_, Blake2_128Concat, (CountryId, Hash32), TrustRegistryEntry>`
  - `Instructions: StorageMap<_, Blake2_128Concat, u64, ClearingInstruction>`
  - `NetPositions: StorageMap<_, Blake2_128Concat, (CountryId, CountryId), XsuAmount>`
  - `NextInstruction: StorageValue<_, u64, ValueQuery>`
- **Required extrinsics:**
  ```rust
  pub fn register_issuer(origin, entry: TrustRegistryEntry) -> DispatchResult; // federation-governed
  pub fn submit_instruction(origin, instr: ClearingInstruction) -> DispatchResult;
  pub fn verify_finality(origin, id: u64, finality_proof: BoundedVec<u8, ConstU32<4096>>) -> DispatchResult;
  pub fn net_and_settle(origin, window: u32) -> DispatchResult;   // multilateral netting
  ```
- **Config trait:**
  ```rust
  pub trait Config: frame_system::Config {
      type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
      type FederationOrigin: EnsureOrigin<Self::RuntimeOrigin>; // treaty council
      type RelayerOrigin: EnsureOrigin<Self::RuntimeOrigin>;
      type WeightInfo: WeightInfo;
  }
  ```
- **Consumes from primitives:** `CountryId, TrustRegistryEntry, ClearingInstruction,
  XcmStatus, XsuAmount, Commitment, Hash32`.
- **Cross-module deps:** `pallet-federation` (basket weights, members),
  `ferrum-zk` (verify cross-border selective-disclosure proofs),
  `pallet-identity-fer`/`pallet-credential` (cross-chain DID & issuer recognition).

---

## 9. `runtime` and `node` (owned by lead; listed for context)

- **`runtime` / `runtime`** (`runtime/src/consensus.rs`): composes all pallets via
  `construct_runtime!`, uses **sc-consensus-aura** + **sc-finality-grandpa** (§07),
  pallet-contracts for the ink! sandbox (§03/§04). Consensus constants per the
  whitepaper excerpt — `SlotDuration = 3_000`, `MaxAuthorities = 100`,
  `MinValidatorBond = 250_000 * FER`, `EquivocationSlash = 100%`, `OfflineSlash = 1%`.
  Use the primitives constants `SLOT_DURATION_MS`, `MAX_AUTHORITIES`, `MIN_VALIDATOR_BOND`.
- **`node` / `node`:** libp2p networking + **tokio** async runtime + RocksDB/paritydb
  storage (§04). Builds the service, Aura import queue, GRANDPA voter.

Module authors must keep their pallet's public `Config`/`Call`/`Event`/`Error`
stable so the runtime can wire them without churn.

---

## 10. Substrate version string — copy into your `Cargo.toml`

> **polkadot-sdk @ git tag `polkadot-stable2412`** — referenced ONLY via
> `{ workspace = true }`. Rust toolchain: **1.81.0** (see `rust-toolchain.toml`),
> WASM target `wasm32-unknown-unknown`.

Any deviation breaks the single-`Cargo.lock` guarantee and will be rejected.
