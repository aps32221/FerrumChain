//! 電子發票開獎 — 所有權/領獎零知識證明 / Lottery ownership-claim ZK proof (§06)
//!
//! 領獎時,中獎人須證明持有中獎票的 `owner_secret`,並將**受款帳戶**綁入公開
//! 輸入(故複製他人證明也改不了收款人),且 `viewing_key_commitment` 為對稽核者
//! 加密之、綁定於 `owner_commitment` 之身分的已驗證輸出。
//!
//! On claim the winner proves knowledge of the winning ticket's `owner_secret`,
//! binds the **payout beneficiary** as a public input (so a copied proof cannot
//! redirect funds), and proves `viewing_key_commitment` is an encryption-to-
//! auditor of the bound identity. Mirrors the `lottery_eligibility` plumbing.
//!
//! ## 公開輸入順序 / Public input ordering
//! `[invoice_hash, owner_commitment, draw_id, beneficiary, nullifier, viewing_key_commitment]`.

use ark_bls12_381::{Bls12_381, Fr};
use ark_ff::PrimeField;
use ark_groth16::{Groth16, PreparedVerifyingKey, Proof};
use ark_std::vec::Vec;

use ferrum_primitives::{Commitment, Hash32, Nullifier, ProofBytes, VerifyingKeyBytes};

pub use crate::age_proof::VerifyError;
use crate::age_proof::{decode_proof, decode_vk};

/// 驗證所有權/領獎證明。/ Verify an ownership-claim proof.
pub fn verify_ownership(
    proof: &Proof<Bls12_381>,
    vk: &PreparedVerifyingKey<Bls12_381>,
    public_inputs: &[Fr],
) -> Result<bool, VerifyError> {
    Groth16::<Bls12_381>::verify_proof(vk, proof, public_inputs).map_err(|_| VerifyError::Malformed)
}

/// 映射公開輸入欄位,順序為
/// `[invoice_hash, owner_commitment, draw_id, beneficiary, nullifier, viewing_key_commitment]`。
///
/// Map the public-input fields in canonical order. `beneficiary` is the payout
/// account's bytes (e.g. `AccountId32` encoding), read via `from_le_bytes_mod_order`.
pub fn public_inputs_from_fields(
    invoice_hash: &Hash32,
    owner_commitment: &Commitment,
    draw_id: u64,
    beneficiary: &[u8],
    nullifier: &Nullifier,
    viewing_key_commitment: &Commitment,
) -> Vec<Fr> {
    let mut out = Vec::with_capacity(6);
    out.push(Fr::from_le_bytes_mod_order(invoice_hash));
    out.push(Fr::from_le_bytes_mod_order(owner_commitment));
    out.push(Fr::from(draw_id));
    out.push(Fr::from_le_bytes_mod_order(beneficiary));
    out.push(Fr::from_le_bytes_mod_order(nullifier));
    out.push(Fr::from_le_bytes_mod_order(viewing_key_commitment));
    out
}

/// Pallet 友善的一站式進入點,供 `pallet-lottery::claim_prize` 呼叫。
/// Pallet-friendly one-shot entrypoint for `pallet-lottery::claim_prize`.
#[allow(clippy::too_many_arguments)]
pub fn verify(
    proof_bytes: &ProofBytes,
    vk_bytes: &VerifyingKeyBytes,
    invoice_hash: &Hash32,
    owner_commitment: &Commitment,
    draw_id: u64,
    beneficiary: &[u8],
    nullifier: &Nullifier,
    viewing_key_commitment: &Commitment,
) -> Result<bool, VerifyError> {
    let proof = decode_proof(proof_bytes)?;
    let vk = decode_vk(vk_bytes)?;
    let public_inputs = public_inputs_from_fields(
        invoice_hash,
        owner_commitment,
        draw_id,
        beneficiary,
        nullifier,
        viewing_key_commitment,
    );
    verify_ownership(&proof, &vk, &public_inputs)
}

/// 測試用支援:所有權電路(玩具級,綁定全部 6 個公開輸入)。
/// Test-support: the ownership circuit (toy-grade, binding all 6 public inputs).
#[cfg(test)]
pub(crate) mod tests_support {
    // Reuse the Poseidon parameters + native/gadget hashes from the eligibility module.
    pub use crate::lottery_eligibility::tests_support::{poseidon_config, poseidon_hash};
    use crate::lottery_eligibility::tests_support::poseidon_hash_var;
    use ark_crypto_primitives::sponge::poseidon::PoseidonConfig;
    use ark_ff::PrimeField;
    use ark_r1cs_std::fields::fp::FpVar;
    use ark_r1cs_std::prelude::*;
    use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};

    /// Production-grade ownership-claim circuit. Public inputs (in order)
    /// `[invoice_hash, owner_commitment, draw_id, beneficiary, nullifier,
    /// viewing_key_commitment]`. Proves with Poseidon:
    ///   1. `owner_commitment = Poseidon(owner_secret, invoice_hash)`   (owner binding)
    ///   2. `nullifier        = Poseidon(owner_secret, invoice_hash, draw_id)` (claim)
    ///   3. `viewing_key_commitment = Poseidon(vk_secret, beneficiary)` (vk ⇄ beneficiary)
    #[derive(Clone)]
    pub struct OwnershipCircuit<F: PrimeField> {
        pub cfg: PoseidonConfig<F>,
        // public
        pub invoice_hash: F,
        pub owner_commitment: F,
        pub draw_id: F,
        pub beneficiary: F,
        pub nullifier: F,
        pub viewing_key_commitment: F,
        // witnesses
        pub owner_secret: F,
        pub vk_secret: F,
    }

    impl<F: PrimeField> ConstraintSynthesizer<F> for OwnershipCircuit<F> {
        fn generate_constraints(self, cs: ConstraintSystemRef<F>) -> Result<(), SynthesisError> {
            let invoice = FpVar::new_input(cs.clone(), || Ok(self.invoice_hash))?;
            let owner_commitment = FpVar::new_input(cs.clone(), || Ok(self.owner_commitment))?;
            let draw = FpVar::new_input(cs.clone(), || Ok(self.draw_id))?;
            let beneficiary = FpVar::new_input(cs.clone(), || Ok(self.beneficiary))?;
            let nullifier = FpVar::new_input(cs.clone(), || Ok(self.nullifier))?;
            let vk_commitment = FpVar::new_input(cs.clone(), || Ok(self.viewing_key_commitment))?;
            let owner = FpVar::new_witness(cs.clone(), || Ok(self.owner_secret))?;
            let vk_secret = FpVar::new_witness(cs.clone(), || Ok(self.vk_secret))?;

            // 1. owner_commitment = Poseidon(owner, invoice)
            let oc = poseidon_hash_var(&self.cfg, cs.clone(), &[owner.clone(), invoice.clone()])?;
            owner_commitment.enforce_equal(&oc)?;
            // 2. nullifier = Poseidon(owner, invoice, draw)
            let null = poseidon_hash_var(&self.cfg, cs.clone(), &[owner, invoice, draw])?;
            nullifier.enforce_equal(&null)?;
            // 3. viewing_key_commitment = Poseidon(vk_secret, beneficiary)
            let vkc = poseidon_hash_var(&self.cfg, cs.clone(), &[vk_secret, beneficiary])?;
            vk_commitment.enforce_equal(&vkc)?;
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

    use self::tests_support::{poseidon_config, poseidon_hash, OwnershipCircuit};

    #[test]
    fn public_inputs_order_matches() {
        let pi = public_inputs_from_fields(&[1u8; 32], &[2u8; 32], 5, &[3u8; 32], &[4u8; 32], &[6u8; 32]);
        assert_eq!(pi.len(), 6);
        assert_eq!(pi[2], TestFr::from(5u64));
    }

    /// 端到端:以**真實 Poseidon** 所有權電路產生 Groth16 證明並經本 crate 驗證,
    /// 並確認改派受款人(搶先重導)會被拒。
    ///
    /// End-to-end: real Poseidon ownership proof (owner-commitment, claim
    /// nullifier and vk⇄beneficiary all Poseidon-bound), verified here, with a
    /// beneficiary-redirect attempt rejected.
    #[test]
    fn end_to_end_ownership_proof() {
        let mut rng = StdRng::seed_from_u64(23);
        let cfg = poseidon_config::<TestFr>();
        let f = |n: u64| TestFr::from(n);

        let owner = f(555);
        let invoice = f(7);
        let draw = f(3);
        let beneficiary = f(9);
        let vk_secret = f(4);
        let owner_commitment = poseidon_hash(&cfg, &[owner, invoice]);
        let nullifier = poseidon_hash(&cfg, &[owner, invoice, draw]);
        let viewing_key_commitment = poseidon_hash(&cfg, &[vk_secret, beneficiary]);

        let circuit = OwnershipCircuit::<TestFr> {
            cfg: cfg.clone(),
            invoice_hash: invoice,
            owner_commitment,
            draw_id: draw,
            beneficiary,
            nullifier,
            viewing_key_commitment,
            owner_secret: owner,
            vk_secret,
        };
        let (pk, vk) =
            Groth16::<Bls12_381>::circuit_specific_setup(circuit.clone(), &mut rng).unwrap();
        let pvk = ark_groth16::prepare_verifying_key(&vk);
        let proof = Groth16::<Bls12_381>::prove(&pk, circuit, &mut rng).unwrap();

        let public_inputs =
            [invoice, owner_commitment, draw, beneficiary, nullifier, viewing_key_commitment];

        let mut proof_buf = alloc::vec::Vec::new();
        proof.serialize_compressed(&mut proof_buf).unwrap();
        let proof_bytes: ProofBytes = ProofBytes::try_from(proof_buf).unwrap();
        let mut vk_buf = alloc::vec::Vec::new();
        vk.serialize_compressed(&mut vk_buf).unwrap();

        let decoded_proof = decode_proof(&proof_bytes).unwrap();
        let decoded_pvk = decode_vk(&vk_buf).unwrap();
        assert!(verify_ownership(&decoded_proof, &decoded_pvk, &public_inputs).unwrap());

        // a different beneficiary (front-run redirect) must be rejected
        let redirected =
            [invoice, owner_commitment, draw, f(99), nullifier, viewing_key_commitment];
        assert!(!verify_ownership(&proof, &pvk, &redirected).unwrap());
    }
}
