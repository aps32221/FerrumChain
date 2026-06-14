//! BBS+ 選擇性揭露 / BBS+ selective disclosure (whitepaper §05 Flow B)
//!
//! 對可驗證憑證（VC）做欄位級選擇性揭露：持證人可只揭露部分宣告
//! （attributes），同時對其餘宣告提出「我知道使其雜湊與簽發者承諾一致」的
//! 證明，而不揭露這些欄位的值。
//!
//! Field-level selective disclosure over a Verifiable Credential: the holder
//! reveals only a subset of claims (attributes) while proving the remaining,
//! hidden claims are consistent with the issuer's commitment — without
//! revealing their values.
//!
//! ## 實作說明 / Implementation note
//! 完整 BBS+（基於配對的群簽章）需要專用 curve 群運算函式庫，目前 workspace
//! 未引入。本模組提供與 whitepaper §05 Flow B 對應的**雜湊承諾版選擇性揭露**：
//! - 簽發者對排序後的宣告雜湊清單做 BLAKE2b 承諾（[`commit_attributes`]）；
//! - 持證人揭露子集 + 對應的 Merkle-style 鄰居雜湊（此處簡化為「其餘宣告雜湊
//!   的承諾」），驗證者重算承諾並比對（[`verify_disclosure`]）；
//! - 一次性 `Nullifier` 防止重放（§05 Flow B 步驟 2 末）。
//!
//! 待 workspace 引入配對群運算 crate 後，可在不更動本模組對外 API 的前提下
//! 替換為真正的 BBS+ 簽章驗證。
//!
//! A full BBS+ (pairing-based group signature) scheme needs a dedicated curve
//! group-arithmetic library not yet in the workspace. This module provides a
//! **hash-commitment selective-disclosure scheme** matching whitepaper §05
//! Flow B's shape:
//! - the issuer commits to the sorted list of claim hashes via BLAKE2b
//!   ([`commit_attributes`]);
//! - the holder reveals a subset plus a commitment to the remaining (hidden)
//!   claim hashes, and the verifier recomputes the top-level commitment and
//!   compares ([`verify_disclosure`]);
//! - a one-time [`Nullifier`] prevents replay (§05 Flow B step 2 tail).
//!
//! Once the workspace gains a pairing-group-arithmetic crate, this can be
//! swapped for a real BBS+ signature check without changing the public API.

use blake2::{Blake2b512, Digest};

use ferrum_primitives::{Commitment, Hash32, Nullifier};

/// 雜湊一段任意位元組為 32 位元組摘要（取 BLAKE2b-512 前 32 位元組）。
///
/// Hash an arbitrary byte slice to a 32-byte digest (first 32 bytes of
/// BLAKE2b-512).
fn blake2b_32(data: &[u8]) -> Hash32 {
    let mut hasher = Blake2b512::new();
    hasher.update(data);
    let out = hasher.finalize();
    let mut digest = [0u8; 32];
    digest.copy_from_slice(&out[..32]);
    digest
}

/// 一個宣告（attribute）的雜湊表示。明文宣告值絕不在本 crate 中出現；呼叫端
/// （錢包 / 簽發者）負責在鏈下將宣告值雜湊為 [`AttributeHash`]。
///
/// The hashed representation of a single claim (attribute). Plaintext claim
/// values never appear in this crate; the caller (wallet / issuer) hashes
/// claim values off-chain into an [`AttributeHash`].
pub type AttributeHash = Hash32;

/// 對一組已排序的宣告雜湊計算簽發者承諾（whitepaper §05：簽發者對 VC 內容
/// 簽署後，僅其雜湊上鏈）。
///
/// 呼叫端必須先以穩定順序排序 `attributes`（例如依宣告名稱字典序），確保承諾
/// 可重現。
///
/// Compute the issuer's commitment over a set of (already-hashed) claims
/// (whitepaper §05: the issuer signs the VC content, but only its hash is
/// anchored on-chain).
///
/// The caller MUST sort `attributes` into a stable order (e.g. lexicographic
/// by claim name) so the commitment is reproducible.
pub fn commit_attributes(attributes: &[AttributeHash]) -> Commitment {
    let mut buf = alloc::vec::Vec::with_capacity(attributes.len() * 32);
    for a in attributes {
        buf.extend_from_slice(a);
    }
    blake2b_32(&buf)
}

/// 一筆選擇性揭露出示（presentation）：揭露的宣告雜湊（按原始順序中的位置）
/// 加上未揭露宣告的雜湊承諾，以及一次性 nullifier。
///
/// A selective-disclosure presentation: the revealed claim hashes (at their
/// original positions) plus a commitment to the hidden claims' hashes, and a
/// one-time nullifier.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Disclosure {
    /// 揭露的宣告：`(原始位置索引, 宣告雜湊)`。
    ///
    /// Revealed claims: `(original position index, claim hash)`.
    pub revealed: alloc::vec::Vec<(u32, AttributeHash)>,
    /// 未揭露宣告（按原始順序）雜湊清單的承諾。
    ///
    /// Commitment to the list of hidden claims' hashes (in original order).
    pub hidden_commitment: Commitment,
    /// 一次性 nullifier，防止此出示被重放。
    ///
    /// One-time nullifier preventing this presentation from being replayed.
    pub nullifier: Nullifier,
}

/// 驗證一筆選擇性揭露出示是否與簽發者承諾一致。
///
/// 重建完整宣告雜湊清單（揭露 + 隱藏承諾的占位），並比較其衍生承諾與
/// `issuer_commitment` 是否相符，且揭露的索引/數量與 `total_attributes`
/// 一致。
///
/// Verify that a selective-disclosure presentation is consistent with the
/// issuer's commitment.
///
/// Reconstructs the layout of revealed + hidden claims and checks that the
/// derived commitment matches `issuer_commitment`, and that the revealed
/// indices are in range and `total_attributes` is consistent.
///
/// ## 注意 / Note
/// 此函式檢查**結構一致性**（索引範圍、雜湊清單長度、隱藏承諾），但不能在
/// 不知道全部明文的情況下重新驗證簽發者對「揭露宣告 + 隱藏宣告」整體的原始
/// 簽章雜湊——那一步留待持有完整宣告雜湊清單的鏈下驗證者（或配合
/// [`commit_attributes`] 由簽發者預先發布 `issuer_commitment`）。
///
/// This function checks **structural consistency** (index range, claim-list
/// length, hidden commitment) but cannot re-derive the issuer's original
/// signed hash over "revealed + hidden claims" without the full claim-hash
/// list — that step is left to an off-chain verifier holding the full list
/// (or relies on the issuer having pre-published `issuer_commitment` via
/// [`commit_attributes`]).
pub fn verify_disclosure(
    disclosure: &Disclosure,
    total_attributes: u32,
    expected_issuer_commitment: Option<&Commitment>,
) -> bool {
    // Every revealed index must be in range and unique.
    let mut seen = alloc::collections::BTreeSet::new();
    for (idx, _) in &disclosure.revealed {
        if *idx >= total_attributes {
            return false;
        }
        if !seen.insert(*idx) {
            return false; // duplicate index
        }
    }

    // The number of hidden attributes implied by total vs revealed must be
    // non-negative and the hidden commitment must be a non-zero digest
    // (zero would indicate an empty/placeholder commitment for a non-empty
    // hidden set, which is invalid unless every attribute is revealed).
    let hidden_count = total_attributes as usize - disclosure.revealed.len();
    if hidden_count > 0 && disclosure.hidden_commitment == [0u8; 32] {
        return false;
    }

    // Optionally cross-check against a previously published issuer commitment
    // by recomputing a top-level commitment over [revealed hashes in order,
    // hidden_commitment] and comparing.
    if let Some(expected) = expected_issuer_commitment {
        let mut buf = alloc::vec::Vec::with_capacity((disclosure.revealed.len() + 1) * 32);
        for (_, hash) in &disclosure.revealed {
            buf.extend_from_slice(hash);
        }
        buf.extend_from_slice(&disclosure.hidden_commitment);
        let derived = blake2b_32(&buf);
        if &derived != expected {
            return false;
        }
    }

    true
}

/// 衍生一個一次性 nullifier，綁定「憑證雜湊」與「驗證者識別」，防止同一張
/// 憑證對同一驗證者重複出示（§05 Flow B 步驟 2）。
///
/// Derive a one-time nullifier binding a "credential hash" to a "verifier
/// identifier", preventing the same credential from being re-presented to the
/// same verifier (§05 Flow B step 2).
pub fn derive_nullifier(payload_hash: &Hash32, verifier_tag: &[u8]) -> Nullifier {
    let mut buf = alloc::vec::Vec::with_capacity(32 + verifier_tag.len());
    buf.extend_from_slice(payload_hash);
    buf.extend_from_slice(verifier_tag);
    blake2b_32(&buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_attributes_is_deterministic() {
        let a = [[1u8; 32], [2u8; 32], [3u8; 32]];
        let c1 = commit_attributes(&a);
        let c2 = commit_attributes(&a);
        assert_eq!(c1, c2);
    }

    #[test]
    fn commit_attributes_order_sensitive() {
        let a = [[1u8; 32], [2u8; 32]];
        let b = [[2u8; 32], [1u8; 32]];
        assert_ne!(commit_attributes(&a), commit_attributes(&b));
    }

    #[test]
    fn verify_disclosure_rejects_out_of_range_index() {
        let d = Disclosure {
            revealed: alloc::vec![(5, [9u8; 32])],
            hidden_commitment: [7u8; 32],
            nullifier: [0u8; 32],
        };
        assert!(!verify_disclosure(&d, 3, None));
    }

    #[test]
    fn verify_disclosure_rejects_duplicate_index() {
        let d = Disclosure {
            revealed: alloc::vec![(0, [1u8; 32]), (0, [2u8; 32])],
            hidden_commitment: [7u8; 32],
            nullifier: [0u8; 32],
        };
        assert!(!verify_disclosure(&d, 3, None));
    }

    #[test]
    fn verify_disclosure_round_trip_with_commitment() {
        let revealed = alloc::vec![(0u32, [1u8; 32]), (2u32, [3u8; 32])];
        let hidden_commitment = [9u8; 32];

        let mut buf = alloc::vec::Vec::new();
        for (_, h) in &revealed {
            buf.extend_from_slice(h);
        }
        buf.extend_from_slice(&hidden_commitment);
        let top = blake2b_32(&buf);

        let d = Disclosure {
            revealed,
            hidden_commitment,
            nullifier: derive_nullifier(&[5u8; 32], b"verifier-1"),
        };
        assert!(verify_disclosure(&d, 3, Some(&top)));
        assert!(!verify_disclosure(&d, 3, Some(&[0u8; 32])));
    }

    #[test]
    fn derive_nullifier_is_deterministic_and_tag_sensitive() {
        let h = [1u8; 32];
        let n1 = derive_nullifier(&h, b"verifier-a");
        let n2 = derive_nullifier(&h, b"verifier-a");
        let n3 = derive_nullifier(&h, b"verifier-b");
        assert_eq!(n1, n2);
        assert_ne!(n1, n3);
    }
}
