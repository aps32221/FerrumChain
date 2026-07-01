//! 電子發票開獎 — 資格零知識證明 / E-invoice lottery eligibility ZK proof (§06)
//!
//! 登記彩券時,持票人須證明「我持有此 `invoice_hash` 的購買秘密、屬已登記商家
//! 集合、且年滿門檻」,並使 nullifier 由 `(owner_secret, invoice_hash, draw_id)`
//! 確定性推導——全程不揭露 DID、發票明細或金額。本模組提供 no_std 友善的
//! Groth16 驗證路徑(`pallet-lottery::register_ticket` 呼叫),電路定義與端到端
//! 證明見測試模組。
//!
//! On ticket registration the holder proves "I possess the purchase secret of
//! this `invoice_hash`, I belong to the registered-merchant set, and I am over
//! the age threshold", with the nullifier deterministically derived from
//! `(owner_secret, invoice_hash, draw_id)` — revealing no DID, line items or
//! amount. This module exposes the no_std-friendly Groth16 verification path
//! (called by `pallet-lottery::register_ticket`); the circuit and an end-to-end
//! proof live in the test module.
//!
//! ## 公開輸入順序 / Public input ordering
//! `[invoice_hash, merchant_set_root, draw_id, threshold, nullifier]`.

use ark_bls12_381::{Bls12_381, Fr};
use ark_ff::PrimeField;
use ark_groth16::{Groth16, PreparedVerifyingKey, Proof};
use ark_std::vec::Vec;

use ferrum_primitives::{Hash32, Nullifier, ProofBytes, VerifyingKeyBytes};

pub use crate::age_proof::VerifyError;
use crate::age_proof::{decode_proof, decode_vk};

/// 驗證資格證明:公開輸入須依
/// `[invoice_hash, merchant_set_root, draw_id, threshold, nullifier]` 排序。
///
/// Verify an eligibility proof; public inputs must be ordered
/// `[invoice_hash, merchant_set_root, draw_id, threshold, nullifier]`.
pub fn verify_eligibility(
    proof: &Proof<Bls12_381>,
    vk: &PreparedVerifyingKey<Bls12_381>,
    public_inputs: &[Fr],
) -> Result<bool, VerifyError> {
    Groth16::<Bls12_381>::verify_proof(vk, proof, public_inputs).map_err(|_| VerifyError::Malformed)
}

/// 將資格證明的公開輸入欄位映射為 BLS12-381 純量場元素,順序為
/// `[invoice_hash, merchant_set_root, draw_id, threshold, nullifier]`。
///
/// Map the eligibility public-input fields into BLS12-381 scalar-field elements
/// in canonical order. 32-byte hashes are read little-endian via
/// `from_le_bytes_mod_order`; `draw_id`/`threshold` convert directly.
pub fn public_inputs_from_fields(
    invoice_hash: &Hash32,
    merchant_set_root: &Hash32,
    draw_id: u64,
    threshold: u32,
    nullifier: &Nullifier,
) -> Vec<Fr> {
    let mut out = Vec::with_capacity(5);
    out.push(Fr::from_le_bytes_mod_order(invoice_hash));
    out.push(Fr::from_le_bytes_mod_order(merchant_set_root));
    out.push(Fr::from(draw_id));
    out.push(Fr::from(threshold as u64));
    out.push(Fr::from_le_bytes_mod_order(nullifier));
    out
}

/// Pallet 友善的一站式進入點:解碼證明與驗證金鑰、映射公開輸入並驗證。
/// 供 `pallet-lottery::register_ticket` 在 no_std runtime 中呼叫。
///
/// Pallet-friendly one-shot entrypoint for `pallet-lottery::register_ticket`.
#[allow(clippy::too_many_arguments)]
pub fn verify(
    proof_bytes: &ProofBytes,
    vk_bytes: &VerifyingKeyBytes,
    invoice_hash: &Hash32,
    merchant_set_root: &Hash32,
    draw_id: u64,
    threshold: u32,
    nullifier: &Nullifier,
) -> Result<bool, VerifyError> {
    let proof = decode_proof(proof_bytes)?;
    let vk = decode_vk(vk_bytes)?;
    let public_inputs =
        public_inputs_from_fields(invoice_hash, merchant_set_root, draw_id, threshold, nullifier);
    verify_eligibility(&proof, &vk, &public_inputs)
}

/// 測試用支援:資格電路(玩具級,綁定全部 5 個公開輸入)。
/// 生產電路應以 Poseidon 雜湊與 Merkle 成員證明取代此處的算術關係。
///
/// Test-support: the eligibility circuit (toy-grade, binding all 5 public
/// inputs). A production circuit replaces these arithmetic relations with
/// Poseidon hashing and a Merkle membership proof.
#[cfg(test)]
pub(crate) mod tests_support {
    use ark_crypto_primitives::sponge::constraints::CryptographicSpongeVar;
    use ark_crypto_primitives::sponge::poseidon::constraints::PoseidonSpongeVar;
    use ark_crypto_primitives::sponge::poseidon::{find_poseidon_ark_and_mds, PoseidonConfig, PoseidonSponge};
    use ark_crypto_primitives::sponge::{Absorb, CryptographicSponge, FieldBasedCryptographicSponge};
    use ark_ff::PrimeField;
    use ark_r1cs_std::fields::fp::FpVar;
    use ark_r1cs_std::prelude::*;
    use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};

    /// Poseidon parameters over `F`: rate 2, capacity 1, 8 full + 57 partial
    /// rounds, α = 5. Constants are generated deterministically from the field.
    pub fn poseidon_config<F: PrimeField>() -> PoseidonConfig<F> {
        let (full_rounds, partial_rounds, alpha, rate, capacity) = (8usize, 57usize, 5u64, 2usize, 1usize);
        let (ark, mds) = find_poseidon_ark_and_mds::<F>(
            F::MODULUS_BIT_SIZE as u64,
            rate,
            full_rounds as u64,
            partial_rounds as u64,
            0,
        );
        PoseidonConfig::new(full_rounds, partial_rounds, alpha, mds, ark, rate, capacity)
    }

    /// Native Poseidon hash of `inputs` (matches the in-circuit gadget).
    pub fn poseidon_hash<F: PrimeField + Absorb>(cfg: &PoseidonConfig<F>, inputs: &[F]) -> F {
        let mut sponge = PoseidonSponge::new(cfg);
        for x in inputs {
            sponge.absorb(x);
        }
        sponge.squeeze_native_field_elements(1)[0]
    }

    pub(crate) fn poseidon_hash_var<F: PrimeField>(
        cfg: &PoseidonConfig<F>,
        cs: ConstraintSystemRef<F>,
        inputs: &[FpVar<F>],
    ) -> Result<FpVar<F>, SynthesisError> {
        let mut sponge = PoseidonSpongeVar::new(cs, cfg);
        sponge.absorb(&inputs.to_vec())?;
        Ok(sponge.squeeze_field_elements(1)?.remove(0))
    }

    /// Production-grade eligibility circuit. Public inputs (in order)
    /// `[invoice_hash, merchant_set_root, draw_id, threshold, nullifier]`. Proves:
    ///   1. **Poseidon-Merkle membership**: `leaf = Poseidon(owner_secret,
    ///      invoice_hash)` is in the tree rooted at `merchant_set_root` (path folded
    ///      with Poseidon two-to-one).
    ///   2. **Nullifier derivation**: `nullifier = Poseidon(owner_secret,
    ///      invoice_hash, draw_id)`.
    ///   3. **Age predicate**: `age = threshold + age_diff` (age ≥ threshold).
    #[derive(Clone)]
    pub struct EligibilityCircuit<F: PrimeField> {
        pub cfg: PoseidonConfig<F>,
        // public
        pub invoice_hash: F,
        pub merchant_set_root: F,
        pub draw_id: F,
        pub threshold: F,
        pub nullifier: F,
        // witnesses
        pub owner_secret: F,
        pub age: F,
        pub age_diff: F,
        /// Merkle authentication path: `(sibling, is_right_child)` leaf→root.
        pub path: alloc::vec::Vec<(F, bool)>,
    }

    impl<F: PrimeField> ConstraintSynthesizer<F> for EligibilityCircuit<F> {
        fn generate_constraints(self, cs: ConstraintSystemRef<F>) -> Result<(), SynthesisError> {
            // public inputs — order must match `public_inputs_from_fields`.
            let invoice = FpVar::new_input(cs.clone(), || Ok(self.invoice_hash))?;
            let root = FpVar::new_input(cs.clone(), || Ok(self.merchant_set_root))?;
            let draw = FpVar::new_input(cs.clone(), || Ok(self.draw_id))?;
            let threshold = FpVar::new_input(cs.clone(), || Ok(self.threshold))?;
            let nullifier = FpVar::new_input(cs.clone(), || Ok(self.nullifier))?;
            // witnesses
            let owner = FpVar::new_witness(cs.clone(), || Ok(self.owner_secret))?;
            let age = FpVar::new_witness(cs.clone(), || Ok(self.age))?;
            let age_diff = FpVar::new_witness(cs.clone(), || Ok(self.age_diff))?;

            // 1. leaf = Poseidon(owner, invoice); fold the path to the root.
            let leaf = poseidon_hash_var(&self.cfg, cs.clone(), &[owner.clone(), invoice.clone()])?;
            let mut cur = leaf;
            for (sib, is_right) in self.path.iter() {
                let sib_var = FpVar::new_witness(cs.clone(), || Ok(*sib))?;
                let b = Boolean::new_witness(cs.clone(), || Ok(*is_right))?;
                let left = FpVar::conditionally_select(&b, &sib_var, &cur)?;
                let right = FpVar::conditionally_select(&b, &cur, &sib_var)?;
                cur = poseidon_hash_var(&self.cfg, cs.clone(), &[left, right])?;
            }
            cur.enforce_equal(&root)?;

            // 2. nullifier = Poseidon(owner, invoice, draw).
            let derived = poseidon_hash_var(&self.cfg, cs.clone(), &[owner, invoice, draw])?;
            derived.enforce_equal(&nullifier)?;

            // 3. age >= threshold.
            age.enforce_equal(&(&threshold + &age_diff))?;
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
    use ark_snark::SNARK;
    use ark_std::rand::{rngs::StdRng, SeedableRng};

    use self::tests_support::{poseidon_config, poseidon_hash, EligibilityCircuit};

    #[test]
    fn public_inputs_order_matches() {
        let pi = public_inputs_from_fields(&[3u8; 32], &[4u8; 32], 9, 18, &[5u8; 32]);
        assert_eq!(pi.len(), 5);
        assert_eq!(pi[2], TestFr::from(9u64));
        assert_eq!(pi[3], TestFr::from(18u64));
    }

    /// 端到端:以**真實 Poseidon-Merkle 成員證明**電路產生 Groth16 證明,序列化後
    /// 再經本 crate 的解碼 + 驗證路徑核驗,並驗證竄改商家集合根會被拒。
    ///
    /// End-to-end: build a **real Poseidon-Merkle membership** proof — leaf =
    /// Poseidon(owner, invoice), folded through a path to the root, with a
    /// Poseidon nullifier and an age predicate — then verify through this crate's
    /// decode + verify path, and confirm a tampered merchant-set root is rejected.
    #[test]
    fn end_to_end_eligibility_proof() {
        let mut rng = StdRng::seed_from_u64(11);
        let cfg = poseidon_config::<TestFr>();
        let f = |n: u64| TestFr::from(n);

        let owner = f(123_456_789);
        let invoice = f(7);
        let draw = f(3);
        let threshold = f(18);

        // leaf = Poseidon(owner, invoice); fold a 3-level path to the root (the
        // native fold mirrors the in-circuit Poseidon two-to-one compression).
        let leaf = poseidon_hash(&cfg, &[owner, invoice]);
        let path = alloc::vec![(f(11), false), (f(22), true), (f(33), false)];
        let mut cur = leaf;
        for (sib, is_right) in &path {
            cur = if *is_right {
                poseidon_hash(&cfg, &[*sib, cur])
            } else {
                poseidon_hash(&cfg, &[cur, *sib])
            };
        }
        let root = cur;
        let nullifier = poseidon_hash(&cfg, &[owner, invoice, draw]);

        let circuit = EligibilityCircuit::<TestFr> {
            cfg: cfg.clone(),
            invoice_hash: invoice,
            merchant_set_root: root,
            draw_id: draw,
            threshold,
            nullifier,
            owner_secret: owner,
            age: f(21),
            age_diff: f(3),
            path,
        };

        let (pk, vk) =
            Groth16::<Bls12_381>::circuit_specific_setup(circuit.clone(), &mut rng).unwrap();
        let pvk = ark_groth16::prepare_verifying_key(&vk);
        let proof = Groth16::<Bls12_381>::prove(&pk, circuit, &mut rng).unwrap();

        // public inputs [invoice, root, draw, threshold, nullifier]
        let public_inputs = [invoice, root, draw, threshold, nullifier];

        let mut proof_buf = alloc::vec::Vec::new();
        proof.serialize_compressed(&mut proof_buf).unwrap();
        let proof_bytes: ProofBytes = ProofBytes::try_from(proof_buf).unwrap();
        let mut vk_buf = alloc::vec::Vec::new();
        vk.serialize_compressed(&mut vk_buf).unwrap();

        let decoded_proof = decode_proof(&proof_bytes).unwrap();
        let decoded_pvk = decode_vk(&vk_buf).unwrap();

        assert!(verify_eligibility(&decoded_proof, &decoded_pvk, &public_inputs).unwrap());
        assert!(verify_eligibility(&proof, &pvk, &public_inputs).unwrap());

        // tampering the merchant-set root (forged membership) must be rejected
        let bad = [invoice, f(999), draw, threshold, nullifier];
        assert!(!verify_eligibility(&proof, &pvk, &bad).unwrap());
    }
}
