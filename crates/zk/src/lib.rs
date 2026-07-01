//! # Ferrum 鐵鏈 — `ferrum-zk`：零知識選擇性揭露
//! # Ferrum 鐵鏈 — `ferrum-zk`: zero-knowledge selective disclosure
//!
//! 本 crate 提供 whitepaper §04/§05 所述的零知識證明驗證邏輯：
//! - **年齡門檻證明**（`age_proof`）— 證明「年齡 ≥ 門檻」而不揭露生日。
//! - **稅級門檻證明**（`tax_bracket`）— 證明「所得落於某稅級」而不揭露確切金額（§06）。
//! - **BBS+ 選擇性揭露**（`bbs`）— 對可驗證憑證做欄位級選擇性揭露（§05 Flow B）。
//!
//! This crate provides the zero-knowledge proof *verification* logic described
//! in whitepaper §04/§05:
//! - **Age threshold proofs** (`age_proof`) — prove "age ≥ threshold" without
//!   revealing the birthdate.
//! - **Tax-bracket threshold proofs** (`tax_bracket`) — prove "income falls in
//!   bracket X" without revealing the exact amount (§06).
//! - **BBS+ selective disclosure** (`bbs`) — field-level selective disclosure
//!   over verifiable credentials (§05 Flow B).
//!
//! ## 隱私不變式 / Privacy invariant
//! 本 crate 從不持久化任何明文個資；只處理 `ferrum_primitives::{Commitment,
//! Nullifier, Hash32}` 與 arkworks 的證明/驗證金鑰位元組。鏈上 pallet 只應儲存
//! 承諾與 nullifier，絕不儲存明文宣告值。
//!
//! This crate never persists plaintext PII; it only operates on
//! `ferrum_primitives::{Commitment, Nullifier, Hash32}` and arkworks
//! proof/verifying-key byte blobs. On-chain pallets must only store
//! commitments and nullifiers, never plaintext claim values.
//!
//! ## no_std
//! 這是一個普通函式庫 crate（無 FRAME `Config`/pallet），但提供
//! `#![cfg_attr(not(feature = "std"), no_std)]` 與一個對 pallet 友善的
//! `verify` 進入點，供 `pallet-identity-fer` / `pallet-tax` / `pallet-interop`
//! 在 runtime 中（WASM, no_std）呼叫。
//!
//! This is a plain library crate (no FRAME `Config`/pallet) but it is
//! `#![cfg_attr(not(feature = "std"), no_std)]` and exposes a pallet-friendly
//! `verify` entrypoint for `pallet-identity-fer` / `pallet-tax` /
//! `pallet-interop` to call from the runtime (WASM, no_std).
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod age_proof;
pub mod tax_bracket;
pub mod bbs;
pub mod lottery_eligibility;
pub mod lottery_ownership;

// Re-export the canonical age-proof API at the crate root for ergonomic
// `ferrum_zk::verify_age_threshold(...)` call sites, matching the whitepaper
// excerpt's module path expectations from consuming pallets.
// 在 crate 根重新匯出年齡證明 API，方便下游 pallet 以
// `ferrum_zk::verify_age_threshold(...)` 呼叫。
pub use age_proof::{
    decode_proof, decode_vk, public_inputs_from, verify_age_threshold, VerifyError,
};

pub use ark_bls12_381::{Bls12_381, Fr};
pub use ark_groth16::{PreparedVerifyingKey, Proof};

use ark_ff::{BigInteger, PrimeField};

/// 判斷一個 32 位元組值是否為 **canonical** 的 BLS12-381 純量(值 < 場模數),
/// 使鏈上位元組表示與電路所綁定的場元素一一對應 —— 杜絕以非正規重編碼進行
/// 重放(同一場元素的兩種位元組表示)。
///
/// Whether a 32-byte value is a **canonical** BLS12-381 scalar (value < the
/// field modulus). This makes the on-chain byte representation map bijectively
/// to the field element the circuit binds, preventing replay via a non-canonical
/// re-encoding (two byte strings reducing to the same field element). The pallet
/// keys its nullifier anti-replay maps on this canonical representation.
pub fn is_canonical_scalar(bytes: &ferrum_primitives::Nullifier) -> bool {
    let fe = Fr::from_le_bytes_mod_order(&bytes[..]);
    // Re-encode the reduced field element; equal iff the input was already < r.
    let canonical = fe.into_bigint().to_bytes_le();
    canonical.as_slice() == &bytes[..]
}

#[cfg(test)]
mod canonical_tests {
    use super::is_canonical_scalar;

    #[test]
    fn detects_canonical_and_non_canonical_scalars() {
        // zero and a small value are < r → canonical
        assert!(is_canonical_scalar(&[0u8; 32]));
        let mut small = [0u8; 32];
        small[0] = 1;
        assert!(is_canonical_scalar(&small));
        // all-ones (a 256-bit value, above the 255-bit BLS12-381 scalar modulus)
        // reduces, so it is NOT canonical
        assert!(!is_canonical_scalar(&[0xFFu8; 32]));
        // small repeated bytes are far below the modulus → canonical
        assert!(is_canonical_scalar(&[12u8; 32]), "12*32 should be canonical");
        assert!(is_canonical_scalar(&[13u8; 32]), "13*32 should be canonical");
    }
}
