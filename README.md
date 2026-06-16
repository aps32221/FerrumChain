# Ferrum 鐵鏈 — 主權身分與稅務區塊鏈 / Sovereign Identity & Tax Blockchain

> 🇹🇼 繁體中文 ｜ 🇬🇧 English — 本文件全篇雙語對照。
> This document is fully bilingual (Traditional Chinese first, English second).

**Ferrum 鐵鏈**是一套以 Rust 打造的**國家級許可制主權區塊鏈**,在同一條帳本上承載
兩項國家核心職能:**數位身分驗證**與**稅務管理**,並支援**跨境互通**與**聯邦級清算**。
本倉庫同時包含**雙語白皮書**(單一自含網頁)與一套可運作的 **Substrate / FRAME
參考實作**(PoSA:Aura 出塊 + GRANDPA 最終性)。

**Ferrum** is a Rust-built, **national-grade permissioned sovereign blockchain** that
carries two core state functions on one ledger — **digital identity verification**
and **tax administration** — with **cross-border interoperability** and
**federation-level clearing**. This repository contains both the **bilingual
whitepaper** (a single self-contained website) and a working **Substrate / FRAME
reference implementation** (PoSA: Aura authoring + GRANDPA finality).

> 「Ferrum」是拉丁文的「鐵」;鐵的氧化物正是 **rust**——既雙關實作語言,也象徵
> 基礎設施應如鑄鐵般耐久。Rust 的所有權與借用檢查器在編譯期消除整類記憶體安全
> 漏洞,對承載全國身分與財稅資料的帳本是難以替代的安全底線。
>
> "Ferrum" is Latin for *iron*; iron's oxide is literally **rust** — a pun on the
> implementation language and a nod to infrastructure built to last. Rust's
> ownership and borrow checker eliminate whole classes of memory-safety bugs at
> compile time — an irreplaceable safety floor for a ledger holding a nation's
> identity and fiscal data.

---

## 狀態 / Status

白皮書、全部函式庫 crate 與六個 pallet 皆可編譯;**runtime + WASM blob 與
`ferrum-node` 用戶端皆可建置並執行**(每 3 秒出一個區塊、由 GRANDPA 最終化)。
白皮書**第 9 章(跨境互通)已完整實作**於 `pallet-interop`,含真正的 GRANDPA 輕
客戶端最終性驗證(ed25519 簽章、>⅔ 權重門檻、授權集合換屆)、跨鏈 DID 解析、
跨境零知識互驗,以及跨境稅務協調;**29 個 interop 單元測試全數通過**,完整 node
release build 亦成功。建置細節見 [`BUILD.md`](./BUILD.md)。

The whitepaper, all library crates and the six pallets compile; the **runtime +
WASM blob and the `ferrum-node` client build and run** (a block every 3 s,
finalized via GRANDPA). Whitepaper **Chapter 9 (cross-border interop) is fully
implemented** in `pallet-interop` — a real GRANDPA light client (ed25519
signatures, >⅔ weight threshold, authority-set rotation), cross-chain DID
resolution, cross-border ZK verification, and cross-border tax coordination; **all
29 interop unit tests pass** and the full node release build succeeds. See
[`BUILD.md`](./BUILD.md).

---

## 設計原則 / Design principles (§02)

多數公有鏈假設匿名參與、無許可驗證者、以幣價驅動安全性——這與國家基礎設施直接
衝突。Ferrum 從五項原則出發,每項對應具體技術選擇:

Most public chains assume anonymous participation, permissionless validators and
price-driven security — which conflicts with state infrastructure. Ferrum starts
from five principles, each tied to a concrete technical choice:

| # | 原則 / Principle | 說明 / What it means |
|---|---|---|
| 1 | **主權與問責並存** / Sovereignty with accountability | 驗證者為受認證的公私機構,身分公開可究責,仍以密碼學最終性背書。 / Validators are accredited, publicly identified institutions — yet backed by cryptographic finality, not one central point. |
| 2 | **隱私即預設** / Privacy by default | 預設不揭露任何超出必要的資訊;揭露需授權或法定程序並留稽核軌跡。 / Nothing beyond the necessary is disclosed; disclosure needs authorization and leaves an audit trail. |
| 3 | **可稽核但不可全知** / Auditable, not omniscient | 監理在授權範圍內檢視,沒有任何單一角色能讀取全民明文。 / Supervisors audit within scope; no single role can read everyone's plaintext. |
| 4 | **包容性優先** / Inclusion first | 無智慧型手機、無加密資產者仍能透過代理據點完成身分與稅務事務。 / People without a phone or crypto can still transact via assisted points of service. |
| 5 | **可演進** / Evolvable | WASM runtime 無分叉升級,並預留後量子密碼遷移路徑。 / Forkless WASM upgrades, with a post-quantum migration path reserved. |

> **核心不變式 / Core invariant.** L1–L3 永遠只看得到承諾值與雜湊;明文個資存於
> L4 治理下、機構營運的鏈下加密儲存。這讓「鏈的不可竄改性」與「個資的可刪除/
> 可遺忘權」得以共存。 / L1–L3 only ever see commitments and hashes; plaintext PII
> lives in agency-run encrypted off-chain stores governed at L4 — letting chain
> immutability coexist with a right to erasure.

---

## 倉庫結構 / Repository layout

| 路徑 / Path | 內容 / What it is |
|------|------------|
| `index.html` | 雙語(繁中/EN)白皮書——單一自含檔案。 / The bilingual whitepaper — one self-contained file. |
| `Dockerfile`, `nginx.conf` | 以 nginx 在埠 **8088** 提供白皮書。 / Serve the whitepaper via nginx on port **8088**. |
| `crates/primitives` (`ferrum-primitives`) | 共享標準型別(`Did`、`XsuBasket`、`Vote`、`FederationAction`、`FER`、§09 GRANDPA/跨境型別、共識常數)。 / Shared canonical types (incl. §09 GRANDPA/cross-border types). |
| `crates/zk` (`ferrum-zk`) | Groth16 / BLS12-381 年齡與稅級證明 + BBS+ 選擇性揭露。 / Groth16/BLS12-381 age & tax-bracket proofs + BBS+ selective disclosure. |
| `pallets/identity` (`pallet-identity-fer`) | `did:fer` DID 登記——只錨定 `doc_hash`,絕無個資。 / `did:fer` DID registry — anchors only `doc_hash`, never PII. |
| `pallets/credential` | 可驗證憑證錨點 + 防重放的選擇性揭露出示。 / Verifiable-credential anchors + replay-protected presentations. |
| `pallets/tax` | 電子發票錨定、預扣、ZK 稅級證明、稽核;以 CBDC 結算。 / E-invoice anchoring, withholding, ZK bracket proofs, audit; settles in CBDC. |
| `pallets/treasury` (`pallet-treasury-fer`) | FER 創世配置、治理發行、基礎費銷毀、補貼基金。 / FER genesis pools, governed issuance, base-fee burn, subsidy fund. |
| `pallets/federation` | 條約理事會、雙重多數投票、時間鎖執行、XSU 籃子與準備池。 / Treaty council, dual-majority voting, timelock enactment, XSU basket & reserve pool. |
| `pallets/interop` | **完整 §09**:GRANDPA 輕客戶端、跨鏈 DID 解析、跨境 ZK 互驗、租稅協定、跨境發票互認、OSS VAT、XSU 淨額清算、驗證者質押/罰沒。 / **Full §09**: GRANDPA light client, cross-chain DID resolution, cross-border ZK verify, tax-treaty registry, e-invoice recognition, OSS VAT, XSU netting, validator bonds/slashing. |
| `runtime` (`ferrum-runtime`) | `construct_runtime!`、所有 `Config` 接線、runtime API、WASM 建置。 / `construct_runtime!`, all `Config` wiring, runtime APIs, WASM build. |
| `node` (`ferrum-node`) | libp2p + tokio + RocksDB 服務;Aura 匯入佇列 + GRANDPA voter;CLI、chain spec、RPC。 / libp2p + tokio + RocksDB service; Aura import queue + GRANDPA voter; CLI, chain specs, RPC. |
| `SPEC.md` | 每個 crate 的公開介面契約與「crate ↔ 章節」對照。 / Per-crate public-surface contract and the crate ↔ section map. |
| `BUILD.md` | 建置與執行指南、釘選工具鏈、§3.8 身分驗證流程、§3.9 跨境操作手冊。 / Build & run guide, pinned toolchain, §3.8 identity-verification flow, §3.9 cross-border runbook. |

---

## 白皮書 / The whitepaper (`index.html`)

單一自含檔案,無需建置。右上角 **[中｜EN]** 切換鈕可即時切換語言(繁中 ↔ 英文),
連 SVG 圖內文字與程式註解都會切換。字型線上載入,離線時回退系統字型而不破壞版面。
預設語言為繁體中文。

A single self-contained file — no build step. The top-right **[中｜EN]** toggle
switches language instantly (Traditional Chinese ↔ English), down to text inside
SVG diagrams and code comments. Fonts load online and fall back to system fonts
offline without disturbing the layout. Default language: Traditional Chinese.

**檢視方式 / View it:**

```bash
# 方式 A — 直接用瀏覽器開啟 / Option A — just open it in a browser
start index.html            # Windows
open  index.html            # macOS

# 方式 B — 以 nginx 提供於 http://localhost:8088 / Option B — serve via nginx
docker build -t ferrum-whitepaper .
docker run --rm -p 8088:8088 ferrum-whitepaper
```

### 章節目錄(16 章) / Contents (16 chapters)

`01` 摘要 / Executive Summary ·
`02` 背景與設計原則 / Background & Principles ·
`03` 系統架構 / System Architecture ·
`04` 技術堆疊 / Technology Stack ·
`05` 身分驗證層 / Identity Layer ·
`06` 稅務管理層 / Tax Layer ·
`07` 共識與節點 / Consensus & Nodes ·
`08` 代幣金融模型(國內) / Token Model (Domestic) ·
`09` 跨境互通架構 / Cross-border Interop ·
`10` 跨國代幣金融模型 / Cross-national Token Model ·
`11` 聯邦治理與代幣運作 / Federation Governance ·
`12` 詳細驗證流程 / Verification Flows ·
`13` 安全與隱私 / Security & Privacy ·
`14` 國內治理 / Domestic Governance ·
`15` 發展路線圖 / Roadmap ·
`16` 風險與限制 / Risks & Limitations.

---

## 實作架構 / Implementation architecture (Substrate / FRAME)

由密碼學底層到公民/機關介面,共五層(§03):
Five layers, from the cryptographic base to citizen/agency interfaces (§03):

```
L5  應用層 Application    錢包 · 機關入口 · 報稅前端 · 驗證者控制台 · dApp
L4  協定層 Protocol       pallet-identity · -credential · -tax · -treasury · -federation · -interop  (+ ink!)
L3  Runtime/狀態 State    WASM runtime · Merkle trie · 無分叉升級 · 權重計費
L2  共識層 Consensus      PoSA:Aura 出塊 + GRANDPA 最終性 · 質押與罰沒
L1  網路/密碼 Crypto      libp2p · tokio · sr25519/BLAKE2 · arkworks ZK · RocksDB
```

### 身分如何被驗證? / How is identity verified?

節點驗證的不是密碼或明文,而是**密碼學物件**;每個驗證者節點以相同 runtime 邏輯
確定性地得出相同結論。驗證者依序檢查:①DID 已錨定 → ②憑證有效且未過期 → ③簽發者
受認可 → ④零知識證明密碼學成立(例如「年齡 ≥ 18」而不揭露生日)→ ⑤nullifier 未
被使用(防重放)→ ⑥未撤銷。本地用 `identity` + `credential` + `ferrum-zk`,跨境再
加 `interop` 的信任註冊表與外國驗證金鑰。完整呼叫順序見 [`BUILD.md` §3.8](./BUILD.md)。

A node verifies cryptographic objects, never passwords or plaintext; every
validator node reaches the same verdict deterministically. The verifier checks, in
order: ① the DID is anchored → ② the credential is active and unexpired → ③ the
issuer is accredited → ④ the zero-knowledge proof is cryptographically valid (e.g.
"age ≥ 18" without revealing a birthdate) → ⑤ the nullifier is unused (replay
protection) → ⑥ not revoked. Local checks use `identity` + `credential` +
`ferrum-zk`; cross-border adds `interop`'s trust registry and a foreign verifying
key. Full call sequence in [`BUILD.md` §3.8](./BUILD.md).

### 跨境互通如何運作?(第 9 章) / How does cross-border interop work? (Ch. 9)

各國各自部署 Ferrum,組成**主權鏈聯邦**——每國保有自己的鏈、貨幣與治理,經中立互通
層以**信任最小化**方式連結。`pallet-interop` 的三大機制:

Nations each deploy Ferrum and form a **federation of sovereign chains** — each
keeps its own chain, currency and governance, linked **trust-minimally** by a
neutral interop layer. `pallet-interop`'s three mechanisms:

- **信任最小化橋接 / Trust-minimized bridging** — 不靠保管人:每條鏈在對方鏈上運行
  輕客戶端,以**真正的 GRANDPA 最終性證明**(逐一驗 ed25519 precommit 簽章、要求
  投向目標的權重 **>⅔**)為信任根,並支援授權集合換屆。 / No custodian: each chain
  runs a light client of its peers and verifies **real GRANDPA finality proofs**
  (per-precommit ed25519 verification, **>⅔** weight) as its root of trust, with
  authority-set rotation.
- **跨國身分互認 / Cross-border identity recognition** — 跨鏈 DID 解析
  (`did:fer:tw` ↔ `did:fer:jp`)、受認可簽發者的鏈上信任註冊表,以及以註冊驗證
  金鑰進行的**跨境零知識互驗**(個資不跨境)。 / Cross-chain DID resolution, an
  on-chain trust registry of accredited issuers, and **cross-border ZK
  verification** using registered keys — PII never crosses the border.
- **跨境稅務協調 / Cross-border tax coordination** — 租稅協定登記表(雙重課稅減免)、
  跨境電子發票互認,以及 **OSS 一站式 VAT**;跨境流量以中立的 **XSU** 籃子單位計價、
  多邊淨額清算。 / Tax-treaty registry (double-tax relief), cross-border e-invoice
  recognition, and **One-Stop-Shop VAT**; flows are priced in the neutral **XSU**
  basket unit and netted multilaterally.

完整操作手冊(以 TW↔JP 兩鏈為例)見 [`BUILD.md` §3.9](./BUILD.md)。
Full runbook (TW↔JP two-chain example) in [`BUILD.md` §3.9](./BUILD.md).

---

## 建置與執行 / Build & run

完整指南——釘選工具鏈、所需系統工具(`protoc`、LLVM/clang)、Windows 建置流程與
節點操作——請見 [`BUILD.md`](./BUILD.md)。

The full guide — pinned toolchain, required system tools (`protoc`, LLVM/clang),
the Windows build recipe and node operation — is in [`BUILD.md`](./BUILD.md).

```bash
# 建置節點(Linux/macOS 開箱即用)。 / Build the node (Linux/macOS: out of the box).
cargo build --release -p ferrum-node

# Windows:用一鍵腳本設好完整工具鏈環境。 / Windows: one-command toolchain wrapper.
build-node.cmd

# 跑單節點開發鏈(Alice 出塊並最終化)。 / Run a single-node dev chain.
./target/release/ferrum-node --dev

# 跑全部單元測試(各 pallet + primitives + zk)。 / Run all unit tests.
cargo test --workspace
```

健康的節點每 3 秒記錄一個新區塊(`🏆 Imported #N`),其後 `finalized` 高度持續
攀升(GRANDPA)。將 Polkadot-JS Apps 連到 `ws://127.0.0.1:9944` 即可操作每個
pallet 的 extrinsic。多驗證者(主權鏈)設定、金鑰管理、RPC 與節點子命令見
[`BUILD.md` §3](./BUILD.md)。

A healthy node logs a new block every 3 seconds (`🏆 Imported #N`) with the
`finalized` height climbing behind it (GRANDPA). Connect Polkadot-JS Apps to
`ws://127.0.0.1:9944` to drive every pallet's extrinsics. Multi-validator
(sovereign) setup, key management, RPC and node subcommands are in
[`BUILD.md` §3](./BUILD.md).

---

## 免責聲明 / Disclaimer

本文件為供研究與討論之概念性技術設計文件。**並非**投資、法律或稅務建議,亦非任何
政府之官方政策。

A conceptual technical-design document for research and discussion. **Not**
investment, legal or tax advice, and not the official policy of any government.
