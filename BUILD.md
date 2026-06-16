# Ferrum 鐵鏈 — Build & Run / 建置與執行

This document explains how to build and run the **Ferrum sovereign blockchain**
reference implementation (Substrate / FRAME), the pinned toolchain, and a map of
which crate implements which whitepaper section.

> 本文件說明如何建置與執行 **Ferrum 主權區塊鏈**參考實作、釘選的工具鏈,以及
> 「crate ↔ 白皮書章節」對照表。

---

## 1. Pinned toolchain / 釘選工具鏈

| Component | Pin |
|-----------|-----|
| Rust toolchain | **1.95.0** (`rust-toolchain.toml`: `channel = "1.95.0"`) — must be ≥ 1.85 |
| WASM target | `wasm32-unknown-unknown` (auto-installed by the toolchain file) |
| Substrate / FRAME | **`polkadot-sdk` @ git tag `polkadot-stable2412`** (one pinned source, in root `[workspace.dependencies]`) |
| SCALE codec | `parity-scale-codec` 3.7.x, `scale-info` 2.11.x (3.7 is forced by stable2412 `sp-core`→`primitive-types 0.13`; see §2.4) |
| ZK | arkworks `ark-*` 0.4.x (Groth16 / BLS12-381) |

The toolchain is enforced by `rust-toolchain.toml`; do **not** override it. The
WASM target is required because the runtime compiles itself to WASM via
`substrate-wasm-builder` (`runtime/build.rs`).

> **Why ≥ 1.85, not 1.81?** Several transitive deps in the node/networking tree
> (`rpassword`, `security-framework`, `wit-bindgen`, …) have adopted the **2024
> edition**, which only a cargo ≥ 1.85 can parse (`error: feature edition2024 is
> required`). MSRV is a floor, not a ceiling — `polkadot-stable2412` builds
> cleanly on current stable.

### 1.1 System tools / 系統工具

| Tool | Why | Install |
|------|-----|---------|
| **`protoc`** (Protocol Buffers) | `litep2p` / libp2p networking build scripts need it (`Could not find protoc`) | Download a release from <https://github.com/protocolbuffers/protobuf/releases>, unzip, then set `PROTOC` to the `protoc` binary (e.g. `setx PROTOC C:\...\protoc\bin\protoc.exe` on Windows, no admin needed), **or** `apt install protobuf-compiler` / `brew install protobuf`. |
| **LLVM / clang** | binaryen (`wasm-opt-sys`) C++ and RocksDB (`librocksdb-sys`) bindgen, on Windows. Provides `clang-cl`, `libclang.dll` and the clang builtin headers (`stdarg.h`). | Install LLVM (no-admin: silent-install the official `LLVM-*-win64.exe` with `/S /D=<dir>`), then set `CC`/`CXX=clang-cl`, `LIBCLANG_PATH=<dir>\bin`, `BINDGEN_EXTRA_CLANG_ARGS=-I<dir>/lib/clang/<major>/include` (forward slashes!). See §2.3–§2.4. |
| C++ toolchain | builds the vendored binaryen (`wasm-opt-sys`) for the runtime WASM | clang-cl per above, **or** see §2.3 for the MSVC caveat |

---

## 2. Build / 建置

```bash
# Native build of the node binary (debug).
cargo build -p ferrum-node

# Release build (recommended; the runtime WASM is optimized).
cargo build --release -p ferrum-node

# Type-check the whole workspace without producing binaries.
cargo check --workspace
```

The release binary lands at `target/release/ferrum-node`.

> 建議使用 `--release`;runtime 會被編譯為最佳化的 WASM blob 並內嵌進節點。

### 2.1 Vendored dependency pins (Windows + upstream drift)

The root `Cargo.toml` carries a `[patch]` section and a small `vendor/` tree.
These exist **only** to work around upstream/transitive breakage and touch no
Ferrum source:

- **`vendor/core2`** — `cid 0.9` (via `sc-network`) needs `core2 ^0.4`, but the
  author yanked every published `core2`. We vendor the exact `0.4.0` source and
  `[patch.crates-io]` to it.
- **`vendor/fflonk`** — `ring-proof` (a `sp-core` bandersnatch transitive)
  depends on `w3f/fflonk` with no rev; the branch HEAD renamed the package
  `fflonk → w3f-pcs`. We vendor `fflonk` at commit `1e854f35` (the rev
  polkadot-sdk's lockfile used) and patch to it.
- **`vendor/arkworks-substrate`** — its `sp-crypto-ec-utils` dep points at
  polkadot-sdk **master**, whose tree contains `…/test-sim/src/aux/…` — an
  NTFS-illegal path (`aux` is a reserved Windows device name) that libgit2
  cannot check out. The vendored fork rewrites that one dep to the
  `polkadot-stable2412` tag (which has no such path) and is patched in by path.

Commit `Cargo.lock` so these pins (including the yanked `core2 0.4.0` and the
fflonk rev) stay frozen — a fresh `cargo update` of those crates would re-break.

### 2.2 Transitive crate pins (legacy / optional under ≥ 1.85)

> **Historical note.** These pins were added when the toolchain floor was 1.81.
> Now that the workspace builds on **1.95** (§1), they are no longer required to
> dodge `edition2024` — a modern cargo parses those crates fine. They are kept in
> `Cargo.lock` for reproducibility and do no harm; you may `cargo update` them if
> you want newer versions on a ≥ 1.85 toolchain.

The crates below were each pinned to their last 1.81-compatible release:

| Crate | Locked | Reason |
|-------|--------|--------|
| `indexmap` | 2.10.0 | 2.11+ require edition2024 |
| `getrandom` | 0.2/0.3 only | 0.4.x requires edition2024 (via `tempfile`) |
| `tempfile` | 3.14.0 | newer pulls `getrandom 0.4` |
| `base64ct` | 1.6.0 | 1.7+ require edition2024 |
| `blake2b_simd` | 1.0.3 | 1.0.4 requires edition2024 |
| `zeroize_derive` | 1.4.2 | 1.5.0 requires edition2024 |
| `proc-macro-crate` | 3.2.0 | 3.5 pulls `toml_edit 0.25` → `indexmap ^2.13` |
| `backtrace` | 0.3.74 | 0.3.76 requires rustc 1.82 |
| `cxx` / `cxx-build` | 1.0.129 | 1.0.194 requires rustc 1.82 |
| `ed25519-zebra` | 4.0.3 | 4.2.0 requires rustc 1.85 |

This is why **`Cargo.lock` must be committed** (it is now un-ignored in
`.gitignore`). To re-derive these on a clean checkout:
`cargo update -p <crate>@<bad-ver> --precise <good-ver>`.

### 2.3 `wasm-opt` / binaryen native build (Windows MSVC)

`runtime/build.rs` runs `substrate-wasm-builder`, which depends (non-optionally,
pinned by polkadot-sdk) on `wasm-opt-sys` — a vendored **binaryen** C++ tree
compiled via `cc`. On very new MSVC toolsets (e.g. VS 18 / MSVC 14.51) binaryen
0.116's C++ fails to compile. Workarounds:

- Build with the **GNU** toolchain or an LLVM/clang `CXX`, or install an MSVC
  build-tools version ≤ 14.4x (binaryen 0.116 builds cleanly there); **or**
- On Linux/macOS this builds out of the box (CI target is
  `x86_64-unknown-linux-gnu`).

To type-check the runtime/pallets **without** the WASM blob (skips binaryen):
make `runtime/build.rs` a no-op that writes a `WASM_BINARY = None` stub and drop
`substrate-wasm-builder` from the `std` feature — used for compile-verification
only; the committed build.rs keeps the real WASM build for release nodes.

### 2.4 Full node build on Windows — verified recipe / 完整建置流程

**Status: the `ferrum-node` binary builds AND runs on Windows** (Aura authors a
block every 3 s, GRANDPA finalizes; `--version`/`build-spec`/`--dev` all work).
Getting there needs the toolchain plumbing below plus **one external patch**.

**Shortcut:** `build-node.cmd` in the repo root encapsulates the whole
environment + command below — just run `build-node.cmd` (or
`build-node.cmd -p ferrum-runtime` to pass cargo args). Edit the literal tool
paths inside if your install differs. (It is plain ASCII/CRLF on purpose — a
`.cmd` with non-ASCII bytes is misparsed by cmd.exe under a non-UTF-8 code page,
which silently drops the `set` lines.)

#### Environment (set all of these for `cargo build -p ferrum-node`)

```cmd
:: cmd.exe — adjust paths to your LLVM/protoc install
set "PROTOC=C:\Users\<you>\tools\protoc\bin\protoc.exe"           :: §1.1, litep2p
set "CC=C:\Users\<you>\tools\LLVM\bin\clang-cl.exe"               :: binaryen + rocksdb
set "CXX=C:\Users\<you>\tools\LLVM\bin\clang-cl.exe"
set "CXXFLAGS=/EHsc"                                              :: binaryen needs C++ exceptions
set "LIBCLANG_PATH=C:\Users\<you>\tools\LLVM\bin"                 :: rocksdb bindgen
set "BINDGEN_EXTRA_CLANG_ARGS=-IC:/Users/<you>/tools/LLVM/lib/clang/22/include"
::   ^ rocksdb's bindgen can't find clang builtins (stdarg.h). Use FORWARD
::     slashes — bindgen's shell-words parser eats backslashes as escapes.
set "WASM_BUILD_TOOLCHAIN=1.81.0-x86_64-pc-windows-msvc"          :: §2.3 sp-io #[no_mangle]
rustup run 1.95.0 cargo build --release --locked -p ferrum-node
```

`LIBCLANG_PATH`, `PROTOC`, `BINDGEN_EXTRA_CLANG_ARGS` are also persisted to the
User environment, so a fresh terminal picks them up; the build command above
sets them inline regardless. (`-> target\release\ferrum-node.exe`, ~52 MB.)

#### The one external patch — `sc-network` duplicate codec index

`sc-network` 0.34.0 @ `polkadot-stable2412` has a latent **duplicate SCALE index**
in `protocol/message.rs`: `Consensus` is `#[codec(index = 6)]` while the
deprecated, unused `RemoteCallResponse` variant sits at ordinal 6 → also 6.
`parity-scale-codec` **3.6.x tolerated it; 3.7.x's derive rejects it**
(`E0080: Found variants that have duplicate indexes … 6`). Codec **cannot** drop
below 3.7 — stable2412's own `sp-core → primitive-types 0.13.1 → impl-codec ^0.7
→ codec ^3.7.4` (and `primitive-types 0.13.0` is yanked). It is
toolchain-independent (fails on 1.84/1.85/1.95 alike).

**Fix** — give the dead variant a free explicit index. In
`…/.cargo/git/checkouts/polkadot-sdk-*/967989c/substrate/client/network/src/protocol/message.rs`,
above `RemoteCallResponse(RemoteCallResponse)`, add:

```rust
#[codec(index = 14)]          // free index; wire format of this dead variant is irrelevant
RemoteCallResponse(RemoteCallResponse),
```

This keeps a single crate source (no duplicate-crate type errors). ⚠️ It lives
in `~/.cargo` so it is **not reproducible** — a cache re-fetch reverts it, and
CI/teammates won't have it. For a durable fix, maintain a polkadot-sdk **fork**
carrying this one-liner and repoint the whole `polkadot-sdk` source in
`[workspace.dependencies]` at the fork (single source ⇒ no mismatch).

#### Node source fixes (already committed, in-repo)

The node code was reconciled to the real stable2412 API (these are in the repo):
- `node/src/service.rs` — transaction pool via `sc_transaction_pool::Builder`
  (→ `TransactionPoolHandle`, not the removed `FullPool`); `OffchainWorkers::new(…)?`.
- `node/src/command.rs` — `NetworkWorker` parameterized on
  `ferrum_runtime::opaque::Block` (the `OpaqueExtrinsic` block the service layer
  expects), not the full `Block`.

---

## 3. Run / 執行

> **Prerequisite / 先決條件.** All commands below assume the node binary at
> `target/release/ferrum-node` (`…\ferrum-node.exe` on Windows). It **builds and
> runs on Windows** via the §2.4 recipe (and out-of-the-box on Linux/macOS).
> Build it first with `cargo build --release -p ferrum-node`.
>
> On Windows PowerShell, write the binary as `.\target\release\ferrum-node.exe`
> and replace the `\` line-continuations below with backticks (`` ` ``).

The node is a standard Substrate client: **Aura** authoring + **GRANDPA**
finality, libp2p networking, RocksDB storage. `--help` lists every `sc-cli`
flag; the most relevant are tabulated in §3.6.

### 3.0 `--chain` selector / 鏈別選擇

`--chain <id>` (from `node/src/command.rs::load_spec`) accepts:

| `--chain` value | Spec | Genesis |
|-----------------|------|---------|
| `dev` *(or omitted)* | `development` (`ferrum_dev`, `ChainType::Development`) | **1 validator** (Alice); Alice = Sudo; Alice/Bob (+`//stash`) funded 1 M FER each. |
| `sovereign` / `local` | `sovereign` (`ferrum_sovereign`, `ChainType::Local`) | **2 validators** (Alice, Bob) as treaty members; Alice = Sudo; Alice/Bob/Charlie/Dave/Eve (+stashes) funded. |
| *a file path* | loads that raw/plain JSON chain spec | — |

> The Aura/GRANDPA authority set is **fixed at genesis** (this reference build
> wires `pallet-aura`/`pallet-grandpa` directly — there is no `pallet-session`,
> so authorities do not rotate at runtime). Adding a *new* validator therefore
> means producing a new chain spec that embeds its public keys (§3.3), not a
> runtime call.

### 3.1 Development single node / 單節點開發鏈

```bash
# Ephemeral dev chain — Alice authors a block every 3 s and GRANDPA finalizes
# (~6 s); state is in a temp dir wiped on exit.
./target/release/ferrum-node --dev

# Persist state, expose the RPC to the host, and turn up runtime logging.
./target/release/ferrum-node \
  --chain dev --validator --alice \
  --base-path ./.data/alice \
  --rpc-port 9944 --rpc-cors all \
  -l runtime=debug,txpool=info,grandpa=info
```

What you should see in the log (healthy node):

```
🔨 Initializing Genesis block/state ...
🏷  Local node identity is: 12D3KooW…           ← libp2p PeerId (note it for §3.2)
✨ Imported #1 (0x…)                            ← Aura produced a block (every 3 s)
💤 Idle (0 peers), best: #3 (0x…), finalized #1  ← GRANDPA is finalizing
```

`--dev` ⇒ `--chain dev --validator --tmp --alice` with verbose defaults. Stop
with `Ctrl-C`. Wipe a persisted `--base-path` chain with `purge-chain` (§3.5).

### 3.2 Local multi-validator (sovereign) chain / 多驗證者主權鏈

The `sovereign` spec seats Alice and Bob as accredited institutional validators.
Run them as two processes on one machine, on distinct ports, peered together.

**Terminal 1 — Alice (bootnode):**
```bash
./target/release/ferrum-node \
  --chain sovereign --validator --alice \
  --base-path ./.data/alice \
  --port 30333 --rpc-port 9944 \
  --node-key 0000000000000000000000000000000000000000000000000000000000000001
```
The fixed `--node-key` gives Alice a deterministic PeerId
(`12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp`) so Bob can dial her.

**Terminal 2 — Bob (dials Alice):**
```bash
./target/release/ferrum-node \
  --chain sovereign --validator --bob \
  --base-path ./.data/bob \
  --port 30334 --rpc-port 9945 --prometheus-port 9616 \
  --unsafe-force-node-key-generation \
  --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp
```

> `--unsafe-force-node-key-generation` is required because, for a **named chain**
> (not `--dev`), a node will not silently auto-create its libp2p network key —
> it errors `NetworkKeyNotFound`. Alice avoids this by passing a fixed
> `--node-key`; Bob has none, so either pass this flag or pre-generate a stable
> identity once: `ferrum-node key generate-node-key --base-path ./.data/bob --chain sovereign`.

Both should report `2 peers` and a `finalized` height that keeps climbing —
GRANDPA needs both genesis authorities voting to finalize. To select the
alternative networking stack, add `--network-backend litep2p` (default is
`libp2p`; both are wired in `command.rs`).

**A real external validator** (own keys, not the well-known Alice/Bob seeds):
1. Generate an Aura (sr25519) and a GRANDPA (ed25519) key:
   ```bash
   ./target/release/ferrum-node key generate --scheme sr25519   # Aura  + account
   ./target/release/ferrum-node key generate --scheme ed25519   # GRANDPA
   ```
2. Put the **public** keys into a custom genesis (edit
   `node/src/chain_spec.rs` or a plain chain-spec JSON) and `build-spec --raw`.
3. On the validator host, import the **secret** keys into the keystore:
   ```bash
   ./target/release/ferrum-node key insert --base-path ./.data/v1 \
     --chain my-spec.json --scheme sr25519 --key-type aura --suri "<seed>"
   ./target/release/ferrum-node key insert --base-path ./.data/v1 \
     --chain my-spec.json --scheme ed25519 --key-type gran --suri "<seed>"
   ```
4. Run with `--chain my-spec.json --validator` (no `--alice`).

### 3.3 Connect & interact / 連線與互動

- **RPC / WebSocket** — unified JSON-RPC on `ws://127.0.0.1:9944` (and the same
  port over HTTP). Expose to other hosts with `--rpc-external --rpc-cors all`
  (dev only; pair with `--rpc-methods unsafe` for keystore RPCs like
  `author_insertKey`).
- **Polkadot-JS Apps** — open <https://polkadot.js.org/apps> → *Settings* →
  custom endpoint `ws://127.0.0.1:9944`. Extrinsics/state/events for every
  Ferrum pallet appear automatically from runtime metadata.
- **Quick health check:**
  ```bash
  curl -H 'Content-Type: application/json' \
    -d '{"id":1,"jsonrpc":"2.0","method":"system_health","params":[]}' \
    http://127.0.0.1:9944
  ```
- **Prometheus metrics** — `--prometheus-external` (default port `9615`).

### 3.4 Seeding governance state post-launch / 啟動後植入治理狀態

`pallet-identity-fer` (accredited issuers), `pallet-federation` (treaty-council
seats + XSU basket) and `pallet-interop` (trust registry) intentionally expose
**no `GenesisConfig`** (the SPEC keeps their public surface minimal). Seed them
after launch via Sudo/Root and council extrinsics — easiest from Polkadot-JS
Apps → *Developer → Sudo* / *Extrinsics* signed by Alice on `--dev`:

| Call | Origin | Effect (§) |
|------|--------|------------|
| `identity.registerIssuer(who)` | Root (Sudo) | accredits a DID issuer (§05) |
| `federation.setMembership(member, true)` | council member | seats a nation on the treaty council (§11) |
| `federation.setBasket(xsuBasket)` | Root / council | sets the balanced XSU basket (§10) |
| `interop.registerIssuer(entry)` | Root | registers a cross-border issuer in the trust registry (§09) |
| `interop.initAuthoritySet(country, set)` | Root | bootstraps a peer chain's GRANDPA authority set for the light client (§09) |
| `interop.registerIssuerVk(country, issuerKeyHash, vk)` | Root | registers a recognized issuer's Groth16 verifying key (§09) |
| `interop.registerTreaty(a, b, treaty)` | Root | records a bilateral tax treaty (§09) |

(Exact extrinsic names/args follow each pallet's `#[pallet::call]`; confirm
against the metadata shown in Apps. The full §09 cross-border runbook is §3.9;
the local §05 identity-verification flow is §3.8.)

### 3.5 Node-management subcommands / 節點管理子命令

```bash
# Export a chain spec (plain, then a raw/portable one to share with validators).
./target/release/ferrum-node build-spec --chain sovereign            > plain.json
./target/release/ferrum-node build-spec --chain sovereign --raw      > sovereign.json

./target/release/ferrum-node purge-chain --base-path ./.data/alice --chain dev  # wipe DB
./target/release/ferrum-node export-blocks --chain dev --from 1 blocks.bin       # backup
./target/release/ferrum-node import-blocks --chain dev blocks.bin                # restore
./target/release/ferrum-node export-state  --chain dev > state.json              # snapshot state
./target/release/ferrum-node revert --chain dev 5         # roll back 5 finalized blocks
./target/release/ferrum-node chain-info --chain dev       # print best/finalized heads
./target/release/ferrum-node check-block --chain dev 1    # re-execute & validate a block
```

`key` also offers `generate`, `inspect`, `generate-node-key` and `insert`
(see §3.2). `benchmark` is available only when built
`--features runtime-benchmarks` (§4).

### 3.6 Frequently used flags / 常用旗標

| Flag | Purpose |
|------|---------|
| `--dev` | one-shot dev preset: `--chain dev --validator --tmp --alice` |
| `--chain <id\|path>` | select spec: `dev` / `sovereign` / `local` / file (§3.0) |
| `--validator` | author + (try to) finalize blocks; needs keys in the keystore |
| `--alice` … `--eve` | inject a well-known dev seed as keystore keys + `--validator` |
| `--base-path <dir>` | persistent data dir (omit ⇒ OS default; `--tmp` ⇒ throwaway) |
| `--port <n>` | libp2p P2P port (default `30333`) |
| `--rpc-port <n>` | JSON-RPC/WS port (default `9944`) |
| `--rpc-external` / `--rpc-cors all` | expose RPC off-localhost (dev) |
| `--rpc-methods unsafe` | enable keystore/system RPCs (e.g. `author_insertKey`) |
| `--node-key <hex>` | fixed ed25519 P2P identity ⇒ deterministic PeerId (bootnode) |
| `--bootnodes <multiaddr>` | dial a peer: `/ip4/…/tcp/30333/p2p/<PeerId>` |
| `--network-backend libp2p\|litep2p` | choose the networking stack |
| `--pruning <n\|archive>` | state-history retention (`archive` keeps all) |
| `-l <target=level>` | log filter, e.g. `runtime=debug,grandpa=info,sync=trace` |
| `--prometheus-external` | expose metrics (port `9615`) |

### 3.7 First-run troubleshooting / 首次執行排錯

- **No blocks / `best: #0`** — the node isn't authoring. Ensure `--validator`
  **and** that your validator's Aura key is in genesis (use `--alice`/`--bob`
  with `dev`/`sovereign`, or insert keys per §3.2).
- **Imports blocks but `finalized #0`** — GRANDPA can't reach a supermajority;
  on `sovereign` you must run **both** Alice and Bob (§3.2).
- **`0 peers`** — check `--bootnodes` multiaddr/PeerId and that the bootnode's
  `--port` matches; a fixed `--node-key` keeps the PeerId stable across restarts.
- **`Bad input data … DatabaseVersion` / corrupt DB** — you reused a
  `--base-path` across incompatible genesis; `purge-chain` it (§3.5) or use a
  fresh dir.
- **`Wasm runtime not available`** — the runtime WASM blob is missing; rebuild
  with the real (non-stubbed) `runtime/build.rs` and the §2 toolchain.

### 3.8 Identity verification flow (§05) / 身分驗證流程

How a node/verifier establishes "who someone is" **without any PII on-chain**.
Every validator node runs the same runtime logic deterministically, so the
verdict below is identical on every node and enters consensus. Three roles:
an **issuer** (accredited institution), a **holder** (citizen), and a
**verifier** (any relying party / pallet reading chain state). Three pallets
cooperate: `pallet-identity-fer` + `pallet-credential` + `ferrum-zk`.

> 節點如何在**鏈上無個資**的前提下確認「這個人是誰」。每個驗證者節點以相同
> 邏輯確定性執行,故結論一致並進入共識。三種角色:**簽發者**(受認證機構)、
> **持有者**(公民)、**驗證者**(任何依鏈狀態判斷的關係方/pallet)。

**Origins / 來源權限**

| Call | Origin | Who |
|---|---|---|
| `identity.registerIssuer(who)` | `GovernanceOrigin` = `EnsureRoot` | Sudo (accredits an institution) |
| `identity.anchorDid` / `updateRevocation` | `IssuerOrigin` = `EnsureSigned`, gated by the `AccreditedIssuers` roster | accredited issuer |
| `credential.issue` / `revoke` / `setStatus` | `IssuerOrigin` = `EnsureSigned` | accredited issuer |
| `credential.logPresentation` | `ensure_signed` | anyone (the nullifier enforces one-time use) |

#### Setup & issuance (issuer side) / 建立與核發

1. **Accredit an issuer (Root, once).** `identity.registerIssuer(who)` — adds the
   institution to the `AccreditedIssuers` roster (§05).
2. **Anchor the citizen's DID (issuer).** `identity.anchorDid(doc)` with
   `doc = DidDocument { did: { chain_tag: b"tw", id }, controller, doc_hash,
   keys, revocation_commitment, anchored_at }`. **Only `doc_hash` lands on-chain.**
3. **Issue a verifiable credential (issuer).** `credential.issue(anchor)` with
   `anchor = CredentialAnchor { subject: <did>, issuer, kind: Age|Nationality|…,
   payload_hash, status: Active, expires_at }`. No claim values — only the
   issuer-signed `payload_hash`.

#### Presentation & verification (holder + verifier side) / 出示與驗證

4. **Holder builds a ZK selective-disclosure proof off-chain** (e.g. "age ≥ 18")
   bound to `[issuer_commitment, threshold, nullifier]` (the public-input order
   in `crates/zk/src/age_proof.rs`). The birthdate never leaves the wallet.
5. **Log the one-time presentation (replay protection).**
   `credential.logPresentation(nullifier, commitment)` — a second use of the same
   `nullifier` is rejected (`PresentationAlreadyLogged`).
6. **The verifier runs this deterministic checklist** (reads state + verifies the
   proof). Every item is a commitment/hash/proof — never PII:

   | # | Check | Where (read) |
   |---|---|---|
   | 1 | DID is anchored | `identity.Dids[did]` (`resolve`/`exists`) |
   | 2 | credential exists, `status == Active`, not expired | `credential.Credentials[payload_hash]` |
   | 3 | issuer is accredited | `identity.AccreditedIssuers[issuer]` (`is_accredited_issuer`) |
   | 4 | ZK proof is cryptographically valid | `ferrum_zk::verify_age_threshold(proof, vk, [issuer_commitment, threshold, nullifier])` |
   | 5 | nullifier unused (no replay) | `credential.Presentations[nullifier]` |
   | 6 | not revoked | `credential` status + `identity.RevocationAccumulator` |

   > **Where the ZK verify runs (be precise).** `ferrum-zk` is the verification
   > library; on-chain it is invoked where a claim is *consumed* —
   > `pallet-tax::proveBracket` (income-bracket proofs, §06) and
   > `pallet-interop::verifyForeignProof` (cross-border, §3.9). For a purely local
   > "prove age to a dApp", the relying party verifies the Groth16 proof with
   > `ferrum-zk` (off-chain, or via a consuming pallet) and uses
   > `logPresentation` for on-chain replay protection. The identity/credential
   > pallets themselves anchor and gate state; they do not embed a standalone
   > age-proof extrinsic.

#### Revocation / 撤銷

- `credential.revoke(payload_hash)` (issuer) → sets `status = Revoked`; or
  `credential.setStatus(payload_hash, Suspended|Expired|…)`.
- `identity.updateRevocation(commitment)` (issuer) advances the global
  revocation-accumulator commitment (§05). After either, check #2/#6 above fail.

**Cross-border identities** (`did:fer:jp…`) follow the same logic plus the
trust registry and a foreign verifying key — see §3.9.

### 3.9 Chapter 9 — cross-border interop operations / 第 9 章跨境互通操作

This is the end-to-end runbook for **whitepaper §09** as implemented by
`pallet-interop`. It assumes two sovereign Ferrum chains — call them **TW** and
**JP** — each already running (§3.1/§3.2), and that you are operating the **TW**
chain (resolving/clearing against JP).

> 本節是 `pallet-interop` 對應白皮書第 9 章的完整操作手冊。假設兩條主權鏈
> **TW** 與 **JP** 皆已啟動,且你操作的是 **TW** 鏈(對 JP 做解析/清算)。

**Origins in this reference build / 本參考建置的來源權限**

| Role in SPEC | Wired to | Who can call |
|---|---|---|
| `FederationOrigin` (treaty council) | `EnsureRoot` | Sudo (Alice on `--dev`) — all governance calls below |
| `RelayerOrigin` | `EnsureSigned` | **any** signed account — `verifyFinality`, `submitInstruction`, `verifyForeignProof`, `recognizeForeignInvoice` |
| supplier (OSS) | `ensure_signed` | the registering account — `ossRegister`, `ossReport` |

> Read-only helpers — `Interop::resolve_did`, `is_trusted_issuer`,
> `country_recognized`, `treaty_for`, `is_recognized_invoice`, `finalized_head`,
> `net_position` — are **Rust functions on the pallet**, not extrinsics. Other
> pallets call them directly; to read them off-chain, expose a custom runtime API
> / RPC or read the underlying storage maps (`TrustRegistry`, `FinalizedHeads`,
> `TaxTreaties`, …) via `state_getStorage` / Polkadot-JS *Chain state*.

#### A. Trust-minimized GRANDPA bridge / 信任最小化橋接（缺口 1）

The light client's root of trust is **cryptographic GRANDPA finality**, never a
custodian. `verifyFinality` decodes a `ferrum_primitives::GrandpaFinalityProof`,
verifies each precommit's **ed25519** signature, and requires valid weight
**strictly > 2/3** of JP's recognized authority set.

1. **Bootstrap JP's authority set (Root).** Obtain JP's current GRANDPA
   authorities and set id from the JP node (`Grandpa::authorities()` /
   `Grandpa::currentSetId()` in *Chain state*, or its chain spec), then on TW:
   ```
   interop.initAuthoritySet(
     country = 0x4a50,                      // b"JP"
     set     = { authorities: [{ id: <ed25519 pubkey>, weight: 1 }, …], set_id: <n> }
   )
   ```
2. **Submit a clearing instruction (relayer).** Priced in XSU:
   ```
   interop.submitInstruction({ from: b"JP", to: b"TW", amount: <XSU>, detail_commitment: 0x…, status: Pending })
   ```
3. **Build a finality proof and verify it (relayer).** A relayer fetches the JP
   block's GRANDPA justification (`grandpa_proveFinality` RPC on JP), reshapes it
   into `GrandpaFinalityProof { round, set_id, target_hash, target_number,
   precommits: [{ target_hash, target_number, authority, signature }, …] }`,
   SCALE-encodes it, and submits the bytes:
   ```
   interop.verifyFinality(id = 0, finality_proof = 0x<scale-encoded GrandpaFinalityProof>)
   ```
   On success the instruction → `FinalityVerified` and TW's light-client head for
   JP advances (monotonic; a lower target → `StaleFinality`). A forged or
   sub-⅔ proof is rejected (`BadFinalityProof`); a wrong set id →
   `SetIdMismatch`; an unrecognized signer → `UnknownAuthority`.
4. **Rotate JP's authority set across a handoff (Root).** Supply a proof of the
   handoff block under the **current** set; the new set's id must be current + 1:
   ```
   interop.rotateAuthoritySet(country = b"JP", finality_proof = 0x…, new_set = { authorities: […], set_id: <n+1> })
   ```
5. **Net & settle (Root).** Aggregate all `FinalityVerified` instructions in a
   window by `(from, to)` into `NetPositions`:
   ```
   interop.netAndSettle(window = 1)
   ```

#### B. Cross-border identity & ZK verification / 跨國身分互認與 ZK 互驗（缺口 2）

1. **Recognize a JP issuer (Root).** Trust-registry entry of JP's accredited
   issuer key:
   ```
   interop.registerIssuer({ country: b"JP", issuer_key_hash: 0x…, scope: b"id", active: true })
   ```
2. **Register that issuer's Groth16 verifying key (Root).** Required before any
   cross-border proof can verify (issuer must already be recognized):
   ```
   interop.registerIssuerVk(country = b"JP", issuer_key_hash = 0x…, vk = 0x<arkworks VK bytes>)
   ```
3. **Cross-chain DID resolution.** Call `Interop::resolve_did(&did)` (Rust): a
   `did:fer:tw…` resolves to its local document; a `did:fer:jp…` returns
   `Foreign { country: b"JP", recognized: <bool from the trust registry> }`.
4. **Verify a foreign selective-disclosure proof (relayer).** No PII crosses the
   border — only the proof + public inputs. The nullifier prevents replay:
   ```
   interop.verifyForeignProof(country = b"JP", issuer_key_hash = 0x…,
                              proof = 0x<arkworks proof>, inputs = { issuer_commitment, threshold, nullifier })
   ```
   `IssuerNotRecognized` / `VerifyingKeyNotFound` / `MalformedZkProof` /
   `InvalidZkProof` / `ProofReplayed` are the failure modes.

#### C. Cross-border tax coordination / 跨境稅務協調（缺口 3）

1. **Register a bilateral tax treaty (Root)** — double-tax relief; resolvable in
   either direction via `Interop::treaty_for`:
   ```
   interop.registerTreaty(a = b"TW", b = b"JP",
                          treaty = { withholding_cap: <Perbill>, method: Credit|Exemption, active: true })
   ```
2. **Recognize a foreign e-invoice (relayer)** — only allowed once finality with
   the source chain exists (step A.3 ran for JP), else `NoFinalizedHead`:
   ```
   interop.recognizeForeignInvoice(country = b"JP", invoice_hash = 0x…)
   ```
3. **One-Stop-Shop VAT (supplier).** Register once, then report — the report
   allocates revenue to the destination country by producing a clearing
   instruction (`from = home`, `to = destination`) that re-enters the netting
   pipeline (step A.5):
   ```
   interop.ossRegister(subject = <Did>, registration = { home: b"TW", vat_id_commitment: 0x…, active: true })
   interop.ossReport(subject = <Did>, to = b"DE", amount = <XSU>, detail_commitment = 0x…)
   ```

**Privacy invariant (§09).** Everything above carries only commitments, hashes,
finality/ZK proofs and XSU net amounts on-chain — never plaintext PII. Plaintext
stays in the source nation's off-chain encrypted vault.

---

## 4. Tests & benchmarks / 測試與基準

```bash
# Unit tests for every pallet + primitives + zk (uses each crate's mock runtime).
cargo test --workspace

# Build with the benchmarking machinery enabled.
cargo build --release -p ferrum-node --features runtime-benchmarks

# Run a pallet benchmark.
./target/release/ferrum-node benchmark pallet \
    --chain dev --pallet pallet_tax --extrinsic '*' --steps 20 --repeat 5
```

---

## 5. Component map / 元件對照表 (crate ↔ whitepaper section)

| Crate / 路徑 | Whitepaper § | Layer / 層 | Role |
|--------------|--------------|-----------|------|
| `crates/primitives` (`ferrum-primitives`) | §03–§11 | shared | Canonical types: `Did`, `Balance`, `XsuBasket`, `Vote`, `FederationAction`, `FER`, consensus constants. |
| `crates/zk` (`ferrum-zk`) | §04 / §05 | L1 crypto | Groth16/BLS12-381 age & tax-bracket proof verification + BBS+ selective disclosure. |
| `pallets/identity` (`pallet-identity-fer`) | §05 | L2 identity | `did:fer` DID registry; anchors only `doc_hash`; revocation accumulator. |
| `pallets/credential` (`pallet-credential`) | §05 | L2 identity | Verifiable-credential anchors + replay-protected presentation log. |
| `pallets/tax` (`pallet-tax`) | §06 | L3 tax | E-invoice anchoring, withholding, ZK bracket proofs, audit; settles in eTWD. |
| `pallets/treasury` (`pallet-treasury-fer`) | §08 | L3 economy | FER genesis pools, governed issuance, base-fee burn, subsidy fund, eTWD receipts. |
| `pallets/federation` (`pallet-federation`) | §10 / §11 | L4 federation | Treaty council, dual-majority voting, timelock enactment, XSU basket & reserve pool. |
| `pallets/interop` (`pallet-interop`) | §09 / §10 | L4 interop | **Full §09:** real GRANDPA light client (`src/grandpa.rs`: ed25519 precommit verification, >⅔ weight, set rotation), cross-chain DID resolution + ZK cross-verify, tax-treaty registry, cross-border e-invoice recognition, OSS VAT, XSU netting, validator bonds/slashing. Operations runbook in §3.9. |
| `runtime` (`ferrum-runtime`) | §03 / §04 / §07 | runtime | `construct_runtime!`, all `Config` wiring, frame-executive, runtime APIs, WASM build. |
| `runtime/src/consensus.rs` | §07 | consensus | PoSA constants (3s slots, 100 authorities, 250k FER bond, slashing) + Aura/GRANDPA params. |
| `node` (`ferrum-node`) | §04 / §07 | client | libp2p + tokio + RocksDB service; Aura import queue + GRANDPA voter; CLI, chain specs, RPC. |

### Consensus (§07)

PoSA = **Aura** authoring (3-second slots, governed validator set) + **GRANDPA**
BFT finality. Constants live in `runtime/src/consensus.rs` (reproducing the
whitepaper excerpt verbatim) and are sourced from `ferrum-primitives`
(`SLOT_DURATION_MS`, `MAX_AUTHORITIES`, `MIN_VALIDATOR_BOND`).

---

## 6. Cross-module wiring notes / 跨模組接線備註

- **Tax ↔ Treasury.** `pallet-tax` and `pallet-treasury-fer` each export their
  own (distinct) `TreasurySettle` trait. The runtime defines a local
  `TaxTreasuryAdapter` implementing `pallet_tax::TreasurySettle` and delegating
  to the treasury pallet's impl — the orphan-rule-safe way to bridge them.
- **Federation council.** `CouncilMember` is wired to `EnsureCouncilMember`,
  which maps a signed council account to its 2-byte `MemberId` (`CountryId`).
- **Origins.** Governance flows use `EnsureRoot` (Sudo at genesis); issuer and
  relayer flows use `EnsureSigned` and are gated by on-chain rosters.
