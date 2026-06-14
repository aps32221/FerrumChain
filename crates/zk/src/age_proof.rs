//! 零知識年齡證明 / Zero-knowledge age proof (whitepaper §05 excerpt)
//!
//! 驗證者只收到證明與公開輸入，即可確認「持證人已成年」，過程中不接觸生日、
//! 姓名或證件號。
//!
//! A verifier receives only a proof and public inputs, confirming "the holder
//! is of age" without ever touching a birthdate, name or ID number.
//!
//! ## 公開輸入順序 / Public input ordering
//! `[issuer_commitment, threshold, nullifier]` — 與
//! `ferrum_primitives::AgeProofPublicInputs` 的欄位順序一致（見
//! [`public_inputs_from`]）。
//!
//! `[issuer_commitment, threshold, nullifier]` — matches the field order of
//! `ferrum_primitives::AgeProofPublicInputs` (see [`public_inputs_from`]).

use ark_bls12_381::{Bls12_381, Fr};
use ark_ff::PrimeField;
use ark_groth16::{Groth16, PreparedVerifyingKey, Proof};
use ark_serialize::CanonicalDeserialize;
use ark_std::vec::Vec;

use ferrum_primitives::{AgeProofPublicInputs, ProofBytes, VerifyingKeyBytes};

/// 驗證過程中可能發生的錯誤。
///
/// Errors that can occur while decoding or verifying a proof.
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum VerifyError {
    /// 證明或驗證金鑰的位元組編碼不正確（無法解碼）。
    ///
    /// The proof or verifying-key bytes are malformed (failed to decode), or
    /// the underlying pairing check itself errored.
    Malformed,
    /// 證明可被解碼，但驗證結果為「無效」（pairing 檢查未通過）。
    ///
    /// The proof decoded successfully but the pairing check returned
    /// `false` — the proof is cryptographically invalid.
    InvalidProof,
}

/// 驗證「年齡 ≥ 18」（或任意門檻）的零知識證明，毋須揭露出生日期。
///
/// Verify a zero-knowledge proof of "age ≥ 18" (or any threshold) without
/// revealing the birth date.
///
/// 公開輸入依序為 `[issuer_commitment, threshold, nullifier]`，見
/// [`public_inputs_from`]。
///
/// Public inputs are, in order, `[issuer_commitment, threshold, nullifier]` —
/// see [`public_inputs_from`].
///
/// 此函式逐字對應 whitepaper §05 / `crates/zk/src/age_proof.rs` 節錄。
///
/// This function mirrors the whitepaper §05 / `crates/zk/src/age_proof.rs`
/// excerpt verbatim.
pub fn verify_age_threshold(
    proof: &Proof<Bls12_381>,
    vk: &PreparedVerifyingKey<Bls12_381>,
    public_inputs: &[Fr], // [issuer_commitment, threshold, nullifier]
) -> Result<bool, VerifyError> {
    Groth16::<Bls12_381>::verify_proof(vk, proof, public_inputs)
        .map_err(|_| VerifyError::Malformed)
}

/// 將 `ferrum_primitives::AgeProofPublicInputs` 映射為 BLS12-381 純量場元素，
/// 順序為 `[issuer_commitment, threshold, nullifier]`。
///
/// Maps `ferrum_primitives::AgeProofPublicInputs` into BLS12-381 scalar-field
/// elements in the order `[issuer_commitment, threshold, nullifier]`.
///
/// `Commitment`/`Nullifier` 是 32 位元組雜湊；以小端序解讀為
/// `Fr::from_le_bytes_mod_order`（落在純量場模數內，不視為密碼學承諾本身，
/// 僅作為電路的公開輸入綁定）。`threshold` 是一個小整數，直接轉換。
///
/// `Commitment`/`Nullifier` are 32-byte hashes; they are interpreted as
/// little-endian bytes via `Fr::from_le_bytes_mod_order` (reduced modulo the
/// scalar field — this binds the public input to the circuit, it does not
/// claim the hash *is* the field element in a stronger cryptographic sense).
/// `threshold` is a small integer and is converted directly.
pub fn public_inputs_from(p: &AgeProofPublicInputs) -> Vec<Fr> {
    let issuer_commitment = Fr::from_le_bytes_mod_order(&p.issuer_commitment);
    let threshold = Fr::from(p.threshold as u64);
    let nullifier = Fr::from_le_bytes_mod_order(&p.nullifier);
    let mut out = Vec::with_capacity(3);
    out.push(issuer_commitment);
    out.push(threshold);
    out.push(nullifier);
    out
}

/// 將鏈上 `ProofBytes`（arkworks `CanonicalSerialize` 壓縮編碼）解碼為
/// `Proof<Bls12_381>`。
///
/// Decode on-chain `ProofBytes` (arkworks `CanonicalSerialize`-compressed
/// encoding) into a `Proof<Bls12_381>`.
pub fn decode_proof(bytes: &ProofBytes) -> Result<Proof<Bls12_381>, VerifyError> {
    Proof::<Bls12_381>::deserialize_compressed(bytes.as_slice())
        .map_err(|_| VerifyError::Malformed)
}

/// 將鏈下分發的 `VerifyingKeyBytes`（arkworks `VerifyingKey<Bls12_381>` 壓縮
/// 編碼）解碼並準備為 `PreparedVerifyingKey<Bls12_381>`。
///
/// Decode off-chain-distributed `VerifyingKeyBytes` (arkworks
/// `VerifyingKey<Bls12_381>`-compressed encoding) and prepare it as a
/// `PreparedVerifyingKey<Bls12_381>`.
pub fn decode_vk(bytes: &VerifyingKeyBytes) -> Result<PreparedVerifyingKey<Bls12_381>, VerifyError> {
    let vk = ark_groth16::VerifyingKey::<Bls12_381>::deserialize_compressed(bytes.as_slice())
        .map_err(|_| VerifyError::Malformed)?;
    Ok(ark_groth16::prepare_verifying_key(&vk))
}

/// Pallet 友善的一站式進入點：解碼證明、驗證金鑰、映射公開輸入並驗證。
/// 供 `pallet-identity-fer` / `pallet-tax` / `pallet-interop` 在 no_std
/// runtime 中呼叫。
///
/// Pallet-friendly one-shot entrypoint: decode the proof, the verifying key,
/// map the public inputs and verify. Intended for `pallet-identity-fer` /
/// `pallet-tax` / `pallet-interop` to call from the no_std runtime.
///
/// 回傳 `Ok(true)` 表示證明有效；`Ok(false)` 表示證明可解碼但驗證未通過；
/// `Err` 表示輸入位元組無法解碼。
///
/// Returns `Ok(true)` if the proof is valid; `Ok(false)` if the proof decoded
/// but verification failed; `Err` if the input bytes could not be decoded.
pub fn verify(
    proof_bytes: &ProofBytes,
    vk_bytes: &VerifyingKeyBytes,
    inputs: &AgeProofPublicInputs,
) -> Result<bool, VerifyError> {
    let proof = decode_proof(proof_bytes)?;
    let vk = decode_vk(vk_bytes)?;
    let public_inputs = public_inputs_from(inputs);
    verify_age_threshold(&proof, &vk, &public_inputs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_bls12_381::Fr as TestFr;
    use ark_ff::Field;
    use ark_groth16::Groth16;
    use ark_relations_test::AgeCircuit;
    use ark_snark::SNARK;
    use ark_std::rand::{rngs::StdRng, SeedableRng};

    // The relations test helper lives in `tax_bracket::test_circuit` to avoid
    // pulling `ark-relations`/`ark-r1cs-std` into non-test builds; we reuse it
    // here via a thin shim module declared below.
    mod ark_relations_test {
        pub use crate::tax_bracket::tests_support::AgeCircuit;
    }

    #[test]
    fn public_inputs_from_orders_fields_correctly() {
        let inputs = AgeProofPublicInputs {
            issuer_commitment: [1u8; 32],
            threshold: 18,
            nullifier: [2u8; 32],
        };
        let fe = public_inputs_from(&inputs);
        assert_eq!(fe.len(), 3);
        assert_eq!(fe[0], TestFr::from_le_bytes_mod_order(&[1u8; 32]));
        assert_eq!(fe[1], TestFr::from(18u64));
        assert_eq!(fe[2], TestFr::from_le_bytes_mod_order(&[2u8; 32]));
    }

    #[test]
    fn decode_proof_rejects_garbage() {
        let bytes: ProofBytes = ProofBytes::try_from(alloc::vec![0xFFu8; 32]).unwrap();
        assert_eq!(decode_proof(&bytes), Err(VerifyError::Malformed));
    }

    #[test]
    fn decode_vk_rejects_garbage() {
        let bytes: VerifyingKeyBytes = alloc::vec![0xAAu8; 16];
        assert_eq!(decode_vk(&bytes), Err(VerifyError::Malformed));
    }

    /// 端到端：產生一個玩具電路的真實 Groth16 證明，序列化後再用本 crate
    /// 的解碼 + 驗證路徑檢查。
    ///
    /// End-to-end: produce a real Groth16 proof for a toy circuit, serialize
    /// it, then check it through this crate's decode + verify path.
    #[test]
    fn end_to_end_groth16_roundtrip() {
        use ark_serialize::CanonicalSerialize;

        let mut rng = StdRng::seed_from_u64(42);
        let circuit = AgeCircuit::<TestFr> {
            age: Some(TestFr::from(21u64)),
            threshold: Some(TestFr::from(18u64)),
            diff: Some(TestFr::from(3u64)),
        };

        let (pk, vk) =
            Groth16::<Bls12_381>::circuit_specific_setup(circuit.clone(), &mut rng).unwrap();
        let pvk = ark_groth16::prepare_verifying_key(&vk);

        let proof = Groth16::<Bls12_381>::prove(&pk, circuit, &mut rng).unwrap();

        // public inputs for this toy circuit: [age, threshold]
        let public_inputs = [TestFr::from(21u64), TestFr::from(18u64)];

        // round-trip through ProofBytes / VerifyingKeyBytes
        let mut proof_buf = alloc::vec::Vec::new();
        proof.serialize_compressed(&mut proof_buf).unwrap();
        let proof_bytes: ProofBytes = ProofBytes::try_from(proof_buf).unwrap();

        let mut vk_buf = alloc::vec::Vec::new();
        vk.serialize_compressed(&mut vk_buf).unwrap();

        let decoded_proof = decode_proof(&proof_bytes).unwrap();
        let decoded_pvk = decode_vk(&vk_buf).unwrap();

        assert!(verify_age_threshold(&decoded_proof, &decoded_pvk, &public_inputs).unwrap());
        // sanity: the freshly-built pvk verifies too
        assert!(verify_age_threshold(&proof, &pvk, &public_inputs).unwrap());
    }
}
