//! GRANDPA 輕客戶端最終性驗證 / GRANDPA light-client finality verification (§09)
//!
//! 「鐵橋」的信任根來自**密碼學最終性**而非中介機構:每條鏈在對方鏈上運行一個
//! 輕客戶端,直接驗證對方的 GRANDPA 最終性證明後才接受跨鏈訊息。本模組實作該
//! 驗證的核心:對一個 [`GrandpaFinalityProof`],逐一驗證其 precommit 的 ed25519
//! 簽章(以 GRANDPA 慣用的 localized payload 為簽署訊息),並要求投向 commit
//! 目標的有效權重**嚴格超過總權重的 2/3**(GRANDPA 超多數最終性條件)。
//!
//! The "Ferrum Bridge"'s root of trust is **cryptographic finality**, not an
//! intermediary: each chain runs an on-chain light client of its peers and
//! verifies their GRANDPA finality proofs before accepting messages. This
//! module implements the core of that check: for a [`GrandpaFinalityProof`] it
//! verifies each precommit's ed25519 signature (over GRANDPA's canonical
//! *localized payload*) and requires that the valid weight voting for the
//! commit target is **strictly greater than 2/3 of total weight** — GRANDPA's
//! supermajority finality condition.
//!
//! ## 保守性 / Conservatism
//! 我們只計入 `target == commit target` 的 precommit;對「投給後代區塊」的票
//! (GRANDPA 容許,需祖系證明)則保守地忽略。這只會**低估**權重,絕不會把未
//! 最終化的區塊誤判為已最終化——對主權系統而言是正確的安全方向。
//!
//! We only count precommits whose target equals the commit target; votes for
//! descendant blocks (which GRANDPA permits, via an ancestry proof) are
//! conservatively ignored. This can only ever *under*-count weight, never
//! accept a non-finalized block — the safe direction for sovereign systems.

use codec::Decode;
use ferrum_primitives::{GrandpaAuthoritySet, GrandpaFinalityProof};
use sp_consensus_grandpa::{check_message_signature, AuthorityId, AuthoritySignature};
use sp_std::vec::Vec;

/// GRANDPA 最終性驗證可能的失敗原因。
/// Reasons a GRANDPA finality proof can fail verification.
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum GrandpaVerifyError {
    /// 證明的 set_id 與鏈上記錄的授權集合不符。
    /// The proof's set_id does not match the on-chain authority set.
    SetIdMismatch,
    /// 某 precommit 的簽署者不在受認可的授權集合內。
    /// A precommit was signed by a key outside the recognized authority set.
    UnknownAuthority,
    /// 同一授權者重複投票。/ The same authority voted more than once.
    DuplicateAuthority,
    /// 某 precommit 的 ed25519 簽章無效。/ A precommit's ed25519 signature is invalid.
    BadSignature,
    /// 投向 commit 目標的有效權重未達超多數(>2/3)。
    /// Valid weight for the commit target did not reach supermajority (>2/3).
    NotEnoughWeight,
    /// 授權者公鑰或簽章位元組無法解碼。
    /// An authority key or signature blob could not be decoded.
    MalformedKey,
}

/// 對受認可的授權集合 `set` 驗證最終性證明 `proof`。
///
/// Verify the finality proof `proof` against the recognized authority set
/// `set`. Returns `Ok(())` iff authorities carrying **strictly more than 2/3**
/// of total weight validly precommitted to `(proof.target_hash,
/// proof.target_number)` under `proof.set_id` and `proof.round`.
pub fn verify_finality_proof(
    set: &GrandpaAuthoritySet,
    proof: &GrandpaFinalityProof,
) -> Result<(), GrandpaVerifyError> {
    // 簽章把 set_id 綁進 payload;集合不符即無從驗證。
    // The set_id is bound into every signature; a mismatch is unverifiable.
    if proof.set_id != set.set_id {
        return Err(GrandpaVerifyError::SetIdMismatch);
    }

    let total = set.total_weight();
    let mut seen: Vec<[u8; 32]> = Vec::new();
    let mut acc: u64 = 0u64;

    for sp in proof.precommits.iter() {
        // 只計入投向 commit 目標的票(見模組層級說明的保守性)。
        // Only count votes for the commit target (see module-level note).
        if sp.target_hash != proof.target_hash || sp.target_number != proof.target_number {
            continue;
        }

        // 簽署者必須屬於受認可的授權集合。
        // The signer must belong to the recognized authority set.
        let weight = match set.authorities.iter().find(|a| a.id == sp.authority) {
            Some(a) => a.weight,
            None => return Err(GrandpaVerifyError::UnknownAuthority),
        };

        // 同一授權者不得重複計票。/ An authority must not be double-counted.
        if seen.iter().any(|s| s == &sp.authority) {
            return Err(GrandpaVerifyError::DuplicateAuthority);
        }

        // 還原 GRANDPA 訊息並驗證 ed25519 簽章(localized payload =
        // (Message::Precommit, round, set_id).encode())。
        // Reconstruct the GRANDPA message and verify the ed25519 signature.
        let id = AuthorityId::decode(&mut &sp.authority[..])
            .map_err(|_| GrandpaVerifyError::MalformedKey)?;
        let sig = AuthoritySignature::decode(&mut &sp.signature[..])
            .map_err(|_| GrandpaVerifyError::MalformedKey)?;
        let message = finality_grandpa::Message::Precommit(finality_grandpa::Precommit {
            target_hash: sp.target_hash,
            target_number: sp.target_number,
        });

        if !check_message_signature(&message, &id, &sig, proof.round, proof.set_id) {
            return Err(GrandpaVerifyError::BadSignature);
        }

        seen.push(sp.authority);
        acc = acc.saturating_add(weight);
    }

    // GRANDPA 超多數:有效權重需**嚴格大於**總權重的 2/3。
    // GRANDPA supermajority: valid weight must be strictly greater than 2/3.
    if acc.saturating_mul(3) > total.saturating_mul(2) {
        Ok(())
    } else {
        Err(GrandpaVerifyError::NotEnoughWeight)
    }
}
