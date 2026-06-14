export const meta = {
  name: 'ferrum-build',
  description: 'Build the Ferrum sovereign blockchain (Substrate/Rust) from the index.html whitepaper: Opus architects+integrates, Sonnet writes each pallet/crate',
  phases: [
    { title: 'Architect', detail: 'Opus: workspace scaffold, shared primitives crate, per-module contract (SPEC.md)', model: 'opus' },
    { title: 'Build Modules', detail: 'Sonnet: one agent per pallet/crate writes its code in isolation' },
    { title: 'Integrate', detail: 'Opus: wire runtime + node + consensus, reconcile types, BUILD.md', model: 'opus' },
    { title: 'Verify', detail: 'Opus: cross-module consistency review' },
  ],
}

const ROOT = 'G:/project/ferrum/ferrum'

// ---- Phase 1: Architect (Opus) -----------------------------------------
phase('Architect')
const ARCH_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  properties: {
    substrate_source: { type: 'string', description: 'Exact dependency source for FRAME/Substrate, e.g. polkadot-sdk umbrella crate version or git tag, that ALL modules must use verbatim' },
    primitives_crate: { type: 'string', description: 'Name of the shared primitives crate and the key public types it exports (Did, AccountId aliases, XSU types, etc.)' },
    workspace_members: { type: 'array', items: { type: 'string' }, description: 'Every Cargo workspace member path' },
    spec_path: { type: 'string', description: 'Absolute path to the SPEC.md contract file written for module agents' },
    notes: { type: 'string', description: 'Anything module agents must know to produce code that links together' },
  },
  required: ['substrate_source', 'primitives_crate', 'workspace_members', 'spec_path'],
}

const arch = await agent(
`You are the lead architect for "Ferrum 鐵鏈", a permissioned sovereign blockchain implemented in Rust on the Substrate / Polkadot-SDK (FRAME) framework. The authoritative specification is the whitepaper at ${ROOT}/index.html (a single self-contained HTML file; read it in chunks — sections are id="summary","principles","arch","stack","identity","tax","consensus","token","interop","xtoken","fedgov","flows","security","governance","roadmap","risks"). Code excerpts in the whitepaper reference real file paths: pallets/identity/src/lib.rs, crates/zk/src/age_proof.rs, runtime/src/consensus.rs, pallets/federation/src/voting.rs, pallets/federation/src/lib.rs — honor those paths and the APIs shown.

Your job is the SCAFFOLDING + CONTRACT so that independent module authors produce code that links into one Cargo workspace. Do ALL of the following by writing real files under ${ROOT}:

1. Read enough of index.html (Sec 03 architecture, Sec 04 tech stack table) to fix the exact tech choices: Substrate/Polkadot SDK, sc-consensus-aura, sc-finality-grandpa, libp2p, tokio, sr25519/BLAKE2, arkworks (Groth16/PLONK)+BBS+, ink!/pallet-contracts, RocksDB.

2. Choose ONE concrete dependency source for FRAME/Substrate that every crate will use VERBATIM (pin it — a specific polkadot-sdk release tag or version). Write the root workspace ${ROOT}/Cargo.toml with [workspace] members for: crates/primitives, crates/zk, pallets/identity, pallets/tax, pallets/credential, pallets/treasury, pallets/federation, pallets/interop, runtime, node. Use [workspace.dependencies] to pin shared deps in ONE place so modules just reference them with workspace=true. Add rust-toolchain.toml and a .gitignore (target/).

3. Write the shared crates/primitives crate IN FULL (Cargo.toml + src/lib.rs, no_std-compatible): the common types every pallet needs — Did, DidDocument, account/balance aliases, Moment, tax types, the XSU basket types and the FederationId/CountryId, error-free shared structs and constants. This crate is the linchpin; get it complete and self-consistent.

4. Write ${ROOT}/SPEC.md — the binding contract for module authors. For EACH module (identity, tax, credential, treasury, federation, interop, zk) specify: crate name, exact path, purpose (cite whitepaper section), required storage items, the extrinsics/Call signatures, the public types it must expose for the runtime to wire it, which primitives types it consumes, and any cross-module dependency. State the Substrate version string they MUST copy. Demand every pallet be no_std (#![cfg_attr(not(feature=\"std\"), no_std)]), provide a Config trait, mock.rs + tests.rs, and a benchmarking/weights stub.

Return the structured summary. Do not write the pallets themselves — only scaffolding, primitives, and SPEC.md.`,
  { phase: 'Architect', model: 'opus', label: 'architect', schema: ARCH_SCHEMA }
)

log(`Architect done. Substrate: ${arch?.substrate_source ?? 'n/a'} — SPEC at ${arch?.spec_path ?? ROOT + '/SPEC.md'}`)

// ---- Phase 2: Build each module (Sonnet, parallel) ---------------------
phase('Build Modules')
const MODULES = [
  { name: 'identity',   path: 'pallets/identity',   sec: 'Sec 05 身分驗證層 (id="identity")',
    desc: 'pallet-identity: DID registry anchoring only doc_hash (H256) — NEVER PII. Accredited issuers (Issuers map). register_did / update / revoke. Mirror the lib.rs excerpt in the whitepaper exactly, then complete it.' },
  { name: 'zk',         path: 'crates/zk',           sec: 'Sec 05 + Sec 04 (arkworks)',
    desc: 'crates/zk: zero-knowledge selective-disclosure. age_proof.rs must verify "age >= 18" without revealing birthdate (Groth16 via arkworks), plus tax-bracket threshold proofs and BBS+ selective disclosure. Mirror the age_proof.rs excerpt. This is a normal library crate (std ok), but expose a no_std-friendly verify entrypoint for pallets.' },
  { name: 'tax',        path: 'pallets/tax',         sec: 'Sec 06 稅務管理層 (id="tax")',
    desc: 'pallet-tax: fiat-denominated obligations, filing, assessment, CBDC settlement references, audit trail via events. Fee-free basic filing. Bracket proofs verified via crates/zk verify entrypoint. No plaintext income on chain — only commitments.' },
  { name: 'credential', path: 'pallets/credential',  sec: 'Sec 05 / Sec 13',
    desc: 'pallet-credential: verifiable credentials issuance/presentation/revocation anchored as commitments, viewing-key authorized disclosure, revocation registry.' },
  { name: 'treasury',   path: 'pallets/treasury',    sec: 'Sec 08 代幣金融模型(國內) (id="token")',
    desc: 'pallet-treasury: the domestic FER token financial model — issuance/supply policy, staking rewards & slashing accounting, fee/weight treasury, allocation per the whitepaper Sec 08 allocation chart. Read Sec 08 carefully for parameters.' },
  { name: 'federation', path: 'pallets/federation',  sec: 'Sec 11 聯邦治理與代幣運作 (id="fedgov")',
    desc: 'pallet-federation: treaty council (one-country-one-seat + neutral non-voting secretariat), DUAL-MAJORITY voting (must pass BOTH member-count AND XSU-basket weight) — implement passes_dual_majority() and the voting.rs logic exactly as shown; propose -> vote -> dual majority -> timelock -> enact (WASM upgrade) lifecycle from lib.rs excerpt; XSU basket & reserve pool (full collateral, mint/redeem, multilateral clearing, daily proof-of-reserve). Mirror BOTH whitepaper excerpts (voting.rs and lib.rs) precisely, then complete.' },
  { name: 'interop',    path: 'pallets/interop',     sec: 'Sec 09 跨境互通 + Sec 10 跨國代幣 (id="interop","xtoken")',
    desc: 'pallet-interop: cross-border interoperability & messaging — cross-chain settlement messages, interop validators with FER staked and cross-chain slashing, the XSU cross-national flows. Read Sec 09, 10, and flow E in Sec 12.' },
]

const MOD_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  properties: {
    crate_name: { type: 'string', description: 'The exact [package] name in Cargo.toml' },
    path: { type: 'string' },
    files: { type: 'array', items: { type: 'string' }, description: 'All files written (absolute or repo-relative)' },
    public_api: { type: 'string', description: 'What the runtime must wire: Config trait associated types/bounds, the Call extrinsics, key storage items, and exported public types' },
    consumes: { type: 'string', description: 'Which primitives/other-crate types it depends on' },
    notes: { type: 'string' },
  },
  required: ['crate_name', 'path', 'public_api'],
}

const built = await parallel(MODULES.map(m => () =>
  agent(
`You implement ONE module of the Ferrum sovereign blockchain (Substrate/FRAME, Rust). Workspace root: ${ROOT}.

FIRST read these, in order:
- ${ROOT}/SPEC.md  (the binding contract — your crate name, path, required storage/calls, the exact Substrate version string, and the public API the runtime expects)
- ${ROOT}/crates/primitives/src/lib.rs  (shared types you MUST reuse, do not redefine)
- ${ROOT}/Cargo.toml  (use [workspace.dependencies] with { workspace = true } — never invent new versions)
- The whitepaper ${ROOT}/index.html, specifically ${m.sec}. If the whitepaper shows a code excerpt for this module's files, reproduce it verbatim and build the rest around it.

YOUR MODULE — ${m.name} at ${m.path}:
${m.desc}

Write COMPLETE, idiomatic, compile-intended Rust under ${ROOT}/${m.path}: Cargo.toml (deps via workspace=true), src/lib.rs with #![cfg_attr(not(feature = "std"), no_std)] for pallets, a Config trait, storage, events, errors, extrinsics with #[pallet::weight], plus src/mock.rs, src/tests.rs, and a weights/benchmarking stub. Match the SPEC's public API exactly so the runtime can wire you. Comments bilingual (繁中 + English) like the whitepaper. Do NOT touch other modules, the runtime, or the node. Do NOT store any PII/plaintext on chain — only commitments/hashes.

Return the structured summary of what you exposed.`,
    { phase: 'Build Modules', model: 'sonnet', label: `build:${m.name}`, schema: MOD_SCHEMA }
  )
))

const ok = built.filter(Boolean)
log(`Modules built: ${ok.map(b => b.crate_name).join(', ')} (${ok.length}/${MODULES.length})`)

// ---- Phase 3: Integrate (Opus) -----------------------------------------
phase('Integrate')
const apiDigest = ok.map(b => `### ${b.crate_name} @ ${b.path}\nPUBLIC API: ${b.public_api}\nCONSUMES: ${b.consumes ?? ''}\nNOTES: ${b.notes ?? ''}`).join('\n\n')

const integ = await agent(
`You are the integrator for the Ferrum sovereign blockchain. The shared scaffolding (root Cargo.toml, crates/primitives, SPEC.md) and all protocol modules are already written under ${ROOT}. Your job is to wire them into a buildable Substrate node + runtime.

Each module reported its public API:

${apiDigest}

Read ${ROOT}/SPEC.md, ${ROOT}/crates/primitives/src/lib.rs, and EACH module's src/lib.rs (pallets/identity, pallets/tax, pallets/credential, pallets/treasury, pallets/federation, pallets/interop, crates/zk) to learn their real Config traits and types — trust the source over the digest. Also re-read index.html Sec 03 (architecture), Sec 04 (stack), Sec 07 (consensus id="consensus"), and the consensus.rs / federation excerpts.

Then WRITE under ${ROOT}:
1. runtime/  — Cargo.toml + src/lib.rs with construct_runtime! including system + all pallets, a Config impl for every pallet (wiring associated types to primitives and to each other, e.g. tax -> zk verifier, federation -> XSU/interop), frame-executive, transaction payment / fee-free classes per whitepaper, RuntimeApi, VERSION, and WASM build (build.rs + substrate-wasm-builder). Create runtime/src/consensus.rs reproducing the whitepaper PoSA excerpt (Aura authoring + GRANDPA finality, governed validator set) and wire Aura + GRANDPA + staking/slashing config.
2. node/ — Cargo.toml + src/{main.rs, service.rs, chain_spec.rs, cli.rs, command.rs, rpc.rs}: full service assembling sc-consensus-aura + sc-finality-grandpa over libp2p/tokio/RocksDB, a development + sovereign chain spec seeding accredited validators and genesis (issuers, treaty council seats, XSU basket), CLI/commands, and RPC.
3. Reconcile any type/name mismatches between modules and runtime by EDITING the offending crate minimally; note each fix.
4. ${ROOT}/BUILD.md — how to build/run (cargo build --release, run dev node), the pinned toolchain, and a component map (which crate = which whitepaper section/layer).
5. Update README.md? Leave the existing one; add a short "## 實作 / Implementation" section pointing to BUILD.md.

Use workspace=true deps. Return a concise prose report: what you wired, every reconciliation edit you made (file + why), and any remaining gaps a compile would surface.`,
  { phase: 'Integrate', model: 'opus', label: 'integrate' }
)

// ---- Phase 4: Verify (Opus) --------------------------------------------
phase('Verify')
const VERIFY_SCHEMA = {
  type: 'object',
  additionalProperties: false,
  properties: {
    consistent: { type: 'boolean', description: 'true if no blocking cross-module inconsistency found' },
    workspace_members_present: { type: 'boolean' },
    issues: { type: 'array', items: { type: 'object', additionalProperties: false, properties: {
      severity: { type: 'string', enum: ['blocker', 'major', 'minor'] },
      file: { type: 'string' },
      problem: { type: 'string' },
      fix: { type: 'string' },
    }, required: ['severity', 'problem'] } },
    summary: { type: 'string' },
  },
  required: ['consistent', 'issues', 'summary'],
}

const verdict = await agent(
`Static consistency review of the assembled Ferrum workspace at ${ROOT} (do NOT attempt a full Substrate compile — it downloads polkadot-sdk and takes far too long; instead reason from the source). Check:
- Root Cargo.toml [workspace] members match the directories that actually exist; every member has a Cargo.toml.
- Every type a pallet imports from crates/primitives actually exists there with that name/signature.
- runtime construct_runtime! references real pallet crate names; each pallet's Config impl in the runtime supplies every associated type the pallet's Config trait requires.
- Cross-module wiring is coherent: tax<->zk verifier, federation dual-majority + XSU, interop staking/slashing, consensus Aura+GRANDPA.
- Dependency versions are pinned consistently via workspace.dependencies (no conflicting versions of the same FRAME crate).
- The whitepaper-referenced files exist with the shown APIs: pallets/identity/src/lib.rs, crates/zk/src/age_proof.rs, runtime/src/consensus.rs, pallets/federation/src/{voting.rs,lib.rs}.
You may run lightweight checks via shell (e.g. cargo metadata --no-deps, cargo fmt --check, or rg) but skip anything that compiles deps. List concrete issues with file + fix. Return structured verdict.`,
  { phase: 'Verify', model: 'opus', label: 'verify', schema: VERIFY_SCHEMA }
)

return {
  substrate: arch?.substrate_source,
  modules_built: ok.map(b => b.crate_name),
  integration_report: integ,
  verify: verdict,
}
