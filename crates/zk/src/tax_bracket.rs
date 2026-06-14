//! 稅級門檻零知識證明 / Zero-knowledge tax-bracket threshold proof (whitepaper §06)
//!
//! 與 `age_proof` 同構：證明「所得落於（或不低於）某稅級門檻」而不揭露確切
//! 所得金額。電路與公開輸入結構與年齡證明一致，僅語意不同——因此本模組重用
//! `Groth16::<Bls12_381>::verify_proof` 與相同的解碼工具。
//!
//! Structurally identical to `age_proof`: prove "income falls within (or at or
//! above) a tax bracket threshold" without revealing the exact income amount.
//! The circuit and public-input shape match the age proof — only the
//! semantics differ — so this module reuses the same
//! `Groth16::<Bls12_381>::verify_proof` decode/verify plumbing.
//!
//! ## 公開輸入順序 / Public input ordering
//! `[issuer_commitment, bracket_threshold, nullifier]` — 與
//! `ferrum_primitives::AgeProofPublicInputs` 欄位順序一致（§06 重用 §05 的
//! 證明形狀，由 `pallet-tax::prove_bracket` 呼叫，見 SPEC §4）。
//!
//! `[issuer_commitment, bracket_threshold, nullifier]` — matches the field
//! order of `ferrum_primitives::AgeProofPublicInputs` (§06 reuses the §05
//! proof shape; called from `pallet-tax::prove_bracket`, see SPEC §4).

use ark_bls12_381::{Bls12_381, Fr};
use ark_groth16::{Groth16, PreparedVerifyingKey, Proof};

use ferrum_primitives::AgeProofPublicInputs;

pub use crate::age_proof::VerifyError;
use crate::age_proof::{decode_proof, decode_vk, public_inputs_from};

/// 驗證「所得 ≥ 稅級門檻」（或任意以 `AgeProofPublicInputs` 形狀編碼的稅務
/// 述詞）的零知識證明，毋須揭露確切所得金額。
///
/// Verify a zero-knowledge proof of "income is within/above a tax bracket
/// threshold" (or any tax predicate encoded with the `AgeProofPublicInputs`
/// shape) without revealing the exact income amount.
///
/// 公開輸入依序為 `[issuer_commitment, bracket_threshold, nullifier]`。
///
/// Public inputs are, in order, `[issuer_commitment, bracket_threshold,
/// nullifier]`.
pub fn verify_bracket_threshold(
    proof: &Proof<Bls12_381>,
    vk: &PreparedVerifyingKey<Bls12_381>,
    public_inputs: &[Fr],
) -> Result<bool, VerifyError> {
    Groth16::<Bls12_381>::verify_proof(vk, proof, public_inputs)
        .map_err(|_| VerifyError::Malformed)
}

/// Pallet 友善的一站式進入點：解碼證明、驗證金鑰、映射公開輸入並驗證稅級
/// 門檻證明。供 `pallet-tax::prove_bracket` 在 no_std runtime 中呼叫。
///
/// Pallet-friendly one-shot entrypoint: decode the proof, the verifying key,
/// map the public inputs, and verify a tax-bracket threshold proof. Intended
/// for `pallet-tax::prove_bracket` to call from the no_std runtime.
pub fn verify(
    proof_bytes: &ferrum_primitives::ProofBytes,
    vk_bytes: &ferrum_primitives::VerifyingKeyBytes,
    inputs: &AgeProofPublicInputs,
) -> Result<bool, VerifyError> {
    let proof = decode_proof(proof_bytes)?;
    let vk = decode_vk(vk_bytes)?;
    let public_inputs = public_inputs_from(inputs);
    verify_bracket_threshold(&proof, &vk, &public_inputs)
}

/// 測試用支援模組：提供一個玩具 R1CS 電路 `AgeCircuit`，供 `age_proof` 與
/// `tax_bracket` 的端到端單元測試重用（避免重複定義電路）。
///
/// Test-support module: provides a toy R1CS circuit `AgeCircuit`, reused by
/// both `age_proof` and `tax_bracket` end-to-end unit tests (avoids duplicating
/// the circuit definition).
///
/// 電路證明：`age = threshold + diff`，其中 `diff` 是隱藏的見證
/// （witness），`age`/`threshold` 是公開輸入。這模擬「年齡 ≥ 門檻」或
/// 「所得 ≥ 稅級門檻」的算術關係，不代表完整生產電路。
///
/// The circuit proves: `age = threshold + diff`, where `diff` is a hidden
/// witness and `age`/`threshold` are public inputs. This models the
/// arithmetic relation behind "age ≥ threshold" or "income ≥ bracket
/// threshold"; it is a toy circuit for tests, not the production circuit.
#[cfg(test)]
pub(crate) mod tests_support {
    use ark_ff::Field;
    use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};
    use ark_r1cs_std::fields::fp::FpVar;
    use ark_r1cs_std::prelude::*;

    /// `age = threshold + diff`
    #[derive(Clone)]
    pub struct AgeCircuit<F: Field> {
        pub age: Option<F>,
        pub threshold: Option<F>,
        pub diff: Option<F>,
    }

    impl<F: Field> ConstraintSynthesizer<F> for AgeCircuit<F> {
        fn generate_constraints(self, cs: ConstraintSystemRef<F>) -> Result<(), SynthesisError> {
            // Public inputs.
            let age_var = FpVar::new_input(cs.clone(), || {
                self.age.ok_or(SynthesisError::AssignmentMissing)
            })?;
            let threshold_var = FpVar::new_input(cs.clone(), || {
                self.threshold.ok_or(SynthesisError::AssignmentMissing)
            })?;
            // Hidden witness.
            let diff_var = FpVar::new_witness(cs, || {
                self.diff.ok_or(SynthesisError::AssignmentMissing)
            })?;

            // age == threshold + diff
            let sum = &threshold_var + &diff_var;
            age_var.enforce_equal(&sum)?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_bls12_381::Fr as TestFr;
    use ark_groth16::Groth16;
    use ark_serialize::CanonicalSerialize;
    use ark_std::rand::{rngs::StdRng, SeedableRng};

    use self::tests_support::AgeCircuit;

    #[test]
    fn decode_rejects_garbage() {
        let bytes: ferrum_primitives::ProofBytes =
            ferrum_primitives::ProofBytes::try_from(alloc::vec::Vec::from([0xFFu8; 32])).unwrap();
        assert_eq!(decode_proof(&bytes), Err(VerifyError::Malformed));
    }

    /// 端到端：以「所得 = 稅級門檻 + 差額」電路產生真實 Groth16 證明，並驗證
    /// 稅級門檻證明路徑。
    ///
    /// End-to-end: produce a real Groth16 proof for the "income = bracket
    /// threshold + diff" circuit and verify it through the tax-bracket path.
    #[test]
    fn end_to_end_bracket_proof() {
        let mut rng = StdRng::seed_from_u64(7);
        let circuit = AgeCircuit::<TestFr> {
            age: Some(TestFr::from(55_000u64)),
            threshold: Some(TestFr::from(50_000u64)),
            diff: Some(TestFr::from(5_000u64)),
        };

        let (pk, vk) =
            Groth16::<Bls12_381>::circuit_specific_setup(circuit.clone(), &mut rng).unwrap();
        let pvk = ark_groth16::prepare_verifying_key(&vk);
        let proof = Groth16::<Bls12_381>::prove(&pk, circuit, &mut rng).unwrap();

        let public_inputs = [TestFr::from(55_000u64), TestFr::from(50_000u64)];

        let mut proof_buf = alloc::vec::Vec::new();
        proof.serialize_compressed(&mut proof_buf).unwrap();
        let proof_bytes: ferrum_primitives::ProofBytes =
            ferrum_primitives::ProofBytes::try_from(proof_buf).unwrap();

        let mut vk_buf = alloc::vec::Vec::new();
        vk.serialize_compressed(&mut vk_buf).unwrap();

        let decoded_proof = decode_proof(&proof_bytes).unwrap();
        let decoded_pvk = decode_vk(&vk_buf).unwrap();

        assert!(verify_bracket_threshold(&decoded_proof, &decoded_pvk, &public_inputs).unwrap());
        assert!(verify_bracket_threshold(&proof, &pvk, &public_inputs).unwrap());
    }
}
