//! 單元測試 / Unit tests for `pallet-interop`.
//!
//! 涵蓋:真正的 GRANDPA 最終性驗證(以 ed25519 簽署的 precommit、≥2/3 權重門檻、
//! 授權集合換屆)、跨鏈 DID 解析、跨境零知識互驗的把關邏輯,以及跨境稅務協調
//! (租稅協定、跨境發票互認、OSS VAT)。
#![cfg(test)]

use crate::{
    mock::*,
    pallet::{
        Error, Event, FinalizedHeads, GrandpaAuthoritySets, Instructions, InteropValidators,
        NetPositions, TotalSlashed, UsedNullifiers, MAX_FINALITY_PROOF_LEN,
    },
};
use codec::Encode;
use ferrum_primitives::{
    AgeProofPublicInputs, BoundedVkBytes, ClearingInstruction, CreditMethod, Did, DidResolution,
    GrandpaAuthority, GrandpaAuthoritySet, GrandpaFinalityProof, OssRegistration, ProofBytes,
    SignedPrecommit, TaxTreaty, TrustRegistryEntry, XcmStatus, XsuAmount, MIN_VALIDATOR_BOND,
};
use frame_support::{assert_noop, assert_ok, pallet_prelude::ConstU32, BoundedVec};
use sp_core::{ed25519, Pair, H256};
use sp_runtime::Perbill;

// ----------------------------------------------------------------------------
// 共用建構子 / Shared constructors
// ----------------------------------------------------------------------------

fn instr(from: [u8; 2], to: [u8; 2], amount: u128) -> ClearingInstruction {
    ClearingInstruction {
        from,
        to,
        amount: XsuAmount(amount),
        detail_commitment: [7u8; 32],
        status: XcmStatus::Pending,
    }
}

fn did(tag: &[u8]) -> Did {
    Did {
        chain_tag: BoundedVec::try_from(tag.to_vec()).unwrap(),
        id: BoundedVec::try_from(b"subject-1".to_vec()).unwrap(),
    }
}

fn relayer() -> RuntimeOrigin {
    frame_system::RawOrigin::Signed(1).into()
}

fn root() -> RuntimeOrigin {
    frame_system::RawOrigin::Root.into()
}

// ----------------------------------------------------------------------------
// GRANDPA 測試輔助 / GRANDPA test helpers (real ed25519 signing)
// ----------------------------------------------------------------------------

fn keypair(seed: u8) -> ed25519::Pair {
    ed25519::Pair::from_seed(&[seed; 32])
}

fn pubkey(p: &ed25519::Pair) -> [u8; 32] {
    let mut b = [0u8; 32];
    b.copy_from_slice(p.public().as_ref());
    b
}

/// 以 GRANDPA 慣用的 localized payload 簽署一個 precommit。
/// Sign a precommit over GRANDPA's canonical localized payload.
fn sign_precommit(
    p: &ed25519::Pair,
    target_hash: H256,
    target_number: u32,
    round: u64,
    set_id: u64,
) -> [u8; 64] {
    let message = finality_grandpa::Message::Precommit(finality_grandpa::Precommit {
        target_hash,
        target_number,
    });
    let payload = sp_consensus_grandpa::localized_payload(round, set_id, &message);
    let sig = p.sign(&payload);
    let mut b = [0u8; 64];
    b.copy_from_slice(sig.as_ref());
    b
}

fn signed_precommit(
    p: &ed25519::Pair,
    target_hash: H256,
    target_number: u32,
    round: u64,
    set_id: u64,
) -> SignedPrecommit {
    SignedPrecommit {
        target_hash,
        target_number,
        authority: pubkey(p),
        signature: sign_precommit(p, target_hash, target_number, round, set_id),
    }
}

fn authority_set(pairs: &[ed25519::Pair], set_id: u64) -> GrandpaAuthoritySet {
    let auths: Vec<GrandpaAuthority> =
        pairs.iter().map(|p| GrandpaAuthority { id: pubkey(p), weight: 1 }).collect();
    GrandpaAuthoritySet { authorities: BoundedVec::try_from(auths).unwrap(), set_id }
}

fn encode_proof(
    precommits: Vec<SignedPrecommit>,
    round: u64,
    set_id: u64,
    target_hash: H256,
    target_number: u32,
) -> BoundedVec<u8, ConstU32<MAX_FINALITY_PROOF_LEN>> {
    let proof = GrandpaFinalityProof {
        round,
        set_id,
        target_hash,
        target_number,
        precommits: BoundedVec::try_from(precommits).unwrap(),
    };
    BoundedVec::try_from(proof.encode()).unwrap()
}

// ----------------------------------------------------------------------------
// 信任註冊表 / Trust registry
// ----------------------------------------------------------------------------

#[test]
fn register_issuer_works() {
    new_test_ext().execute_with(|| {
        let entry = TrustRegistryEntry {
            country: *b"TW",
            issuer_key_hash: [1u8; 32],
            scope: BoundedVec::try_from(b"tax".to_vec()).unwrap(),
            active: true,
        };
        assert_ok!(Interop::register_issuer(root(), entry.clone()));
        assert!(Interop::is_trusted_issuer(*b"TW", [1u8; 32]));
        assert!(Interop::country_recognized(*b"TW"));
        System::assert_has_event(
            Event::IssuerRegistered { country: *b"TW", issuer_key_hash: [1u8; 32] }.into(),
        );
    });
}

#[test]
fn register_issuer_requires_federation_origin() {
    new_test_ext().execute_with(|| {
        let entry = TrustRegistryEntry {
            country: *b"TW",
            issuer_key_hash: [1u8; 32],
            scope: BoundedVec::try_from(b"tax".to_vec()).unwrap(),
            active: true,
        };
        assert_noop!(
            Interop::register_issuer(relayer(), entry),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

// ----------------------------------------------------------------------------
// GRANDPA 最終性驗證 / GRANDPA finality verification
// ----------------------------------------------------------------------------

#[test]
fn verify_finality_accepts_supermajority_and_advances_head() {
    new_test_ext().execute_with(|| {
        let (set_id, round) = (5u64, 1u64);
        let pairs = [keypair(1), keypair(2), keypair(3), keypair(4)];
        assert_ok!(Interop::init_authority_set(root(), *b"TW", authority_set(&pairs, set_id)));

        assert_ok!(Interop::submit_instruction(relayer(), instr(*b"TW", *b"JP", 1_000)));

        let h = H256::repeat_byte(0xAB);
        let n = 42u32;
        // 4 名授權者中 3 名投票(> 2/3)。/ 3 of 4 authorities vote (> 2/3).
        let precommits = vec![
            signed_precommit(&pairs[0], h, n, round, set_id),
            signed_precommit(&pairs[1], h, n, round, set_id),
            signed_precommit(&pairs[2], h, n, round, set_id),
        ];
        assert_ok!(Interop::verify_finality(relayer(), 0, encode_proof(precommits, round, set_id, h, n)));

        assert_eq!(Instructions::<Test>::get(0).unwrap().status, XcmStatus::FinalityVerified);
        assert_eq!(FinalizedHeads::<Test>::get(*b"TW"), Some((h, n)));
        System::assert_has_event(Event::FinalityVerified { id: 0 }.into());
    });
}

#[test]
fn verify_finality_rejects_insufficient_weight() {
    new_test_ext().execute_with(|| {
        let (set_id, round) = (5u64, 1u64);
        let pairs = [keypair(1), keypair(2), keypair(3), keypair(4)];
        assert_ok!(Interop::init_authority_set(root(), *b"TW", authority_set(&pairs, set_id)));
        assert_ok!(Interop::submit_instruction(relayer(), instr(*b"TW", *b"JP", 1_000)));

        let (h, n) = (H256::repeat_byte(0xAB), 42u32);
        // 只有 2/4,未達 > 2/3。/ Only 2 of 4 — below > 2/3.
        let precommits = vec![
            signed_precommit(&pairs[0], h, n, round, set_id),
            signed_precommit(&pairs[1], h, n, round, set_id),
        ];
        assert_noop!(
            Interop::verify_finality(relayer(), 0, encode_proof(precommits, round, set_id, h, n)),
            Error::<Test>::BadFinalityProof
        );
    });
}

#[test]
fn verify_finality_rejects_forged_signature() {
    new_test_ext().execute_with(|| {
        let (set_id, round) = (5u64, 1u64);
        let pairs = [keypair(1), keypair(2), keypair(3), keypair(4)];
        assert_ok!(Interop::init_authority_set(root(), *b"TW", authority_set(&pairs, set_id)));
        assert_ok!(Interop::submit_instruction(relayer(), instr(*b"TW", *b"JP", 1_000)));

        let (h, n) = (H256::repeat_byte(0xAB), 42u32);
        let mut forged = signed_precommit(&pairs[2], h, n, round, set_id);
        forged.signature[0] ^= 0xFF; // 竄改簽章 / tamper the signature
        let precommits = vec![
            signed_precommit(&pairs[0], h, n, round, set_id),
            signed_precommit(&pairs[1], h, n, round, set_id),
            forged,
        ];
        assert_noop!(
            Interop::verify_finality(relayer(), 0, encode_proof(precommits, round, set_id, h, n)),
            Error::<Test>::BadFinalityProof
        );
    });
}

#[test]
fn verify_finality_rejects_set_id_mismatch() {
    new_test_ext().execute_with(|| {
        let pairs = [keypair(1), keypair(2), keypair(3), keypair(4)];
        assert_ok!(Interop::init_authority_set(root(), *b"TW", authority_set(&pairs, 5)));
        assert_ok!(Interop::submit_instruction(relayer(), instr(*b"TW", *b"JP", 1_000)));

        let (h, n) = (H256::repeat_byte(0xAB), 42u32);
        // 證明以 set_id=6 簽署,但鏈上集合為 5。/ Proof signed under set 6, chain has set 5.
        let precommits = vec![
            signed_precommit(&pairs[0], h, n, 1, 6),
            signed_precommit(&pairs[1], h, n, 1, 6),
            signed_precommit(&pairs[2], h, n, 1, 6),
        ];
        assert_noop!(
            Interop::verify_finality(relayer(), 0, encode_proof(precommits, 1, 6, h, n)),
            Error::<Test>::SetIdMismatch
        );
    });
}

#[test]
fn verify_finality_rejects_unknown_authority() {
    new_test_ext().execute_with(|| {
        let (set_id, round) = (5u64, 1u64);
        let pairs = [keypair(1), keypair(2), keypair(3), keypair(4)];
        assert_ok!(Interop::init_authority_set(root(), *b"TW", authority_set(&pairs, set_id)));
        assert_ok!(Interop::submit_instruction(relayer(), instr(*b"TW", *b"JP", 1_000)));

        let (h, n) = (H256::repeat_byte(0xAB), 42u32);
        let outsider = keypair(99); // 不在集合內 / not in the set
        let precommits = vec![
            signed_precommit(&pairs[0], h, n, round, set_id),
            signed_precommit(&outsider, h, n, round, set_id),
        ];
        assert_noop!(
            Interop::verify_finality(relayer(), 0, encode_proof(precommits, round, set_id, h, n)),
            Error::<Test>::UnknownAuthority
        );
    });
}

#[test]
fn verify_finality_requires_initialized_set() {
    new_test_ext().execute_with(|| {
        assert_ok!(Interop::submit_instruction(relayer(), instr(*b"TW", *b"JP", 1_000)));
        let (h, n) = (H256::repeat_byte(0xAB), 42u32);
        let pair = keypair(1);
        let precommits = vec![signed_precommit(&pair, h, n, 1, 5)];
        assert_noop!(
            Interop::verify_finality(relayer(), 0, encode_proof(precommits, 1, 5, h, n)),
            Error::<Test>::AuthoritySetNotInitialized
        );
    });
}

#[test]
fn verify_finality_rejects_empty_and_malformed_proof() {
    new_test_ext().execute_with(|| {
        let pairs = [keypair(1), keypair(2), keypair(3), keypair(4)];
        assert_ok!(Interop::init_authority_set(root(), *b"TW", authority_set(&pairs, 5)));
        assert_ok!(Interop::submit_instruction(relayer(), instr(*b"TW", *b"JP", 1_000)));

        let empty: BoundedVec<u8, ConstU32<MAX_FINALITY_PROOF_LEN>> =
            BoundedVec::try_from(Vec::<u8>::new()).unwrap();
        assert_noop!(
            Interop::verify_finality(relayer(), 0, empty),
            Error::<Test>::InvalidFinalityProof
        );

        let garbage: BoundedVec<u8, ConstU32<MAX_FINALITY_PROOF_LEN>> =
            BoundedVec::try_from(vec![0xFFu8; 8]).unwrap();
        assert_noop!(
            Interop::verify_finality(relayer(), 0, garbage),
            Error::<Test>::MalformedFinalityProof
        );
    });
}

#[test]
fn verify_finality_rejects_unknown_instruction() {
    new_test_ext().execute_with(|| {
        let pairs = [keypair(1), keypair(2), keypair(3), keypair(4)];
        assert_ok!(Interop::init_authority_set(root(), *b"TW", authority_set(&pairs, 5)));
        let (h, n) = (H256::repeat_byte(0xAB), 42u32);
        let precommits = vec![
            signed_precommit(&pairs[0], h, n, 1, 5),
            signed_precommit(&pairs[1], h, n, 1, 5),
            signed_precommit(&pairs[2], h, n, 1, 5),
        ];
        assert_noop!(
            Interop::verify_finality(relayer(), 99, encode_proof(precommits, 1, 5, h, n)),
            Error::<Test>::UnknownInstruction
        );
    });
}

#[test]
fn rotate_authority_set_increments_set_id() {
    new_test_ext().execute_with(|| {
        let pairs = [keypair(1), keypair(2), keypair(3), keypair(4)];
        assert_ok!(Interop::init_authority_set(root(), *b"TW", authority_set(&pairs, 5)));

        let (h, n) = (H256::repeat_byte(0xCD), 100u32);
        // 以當前集合(set 5)驗證換屆區塊的最終性。
        // Prove finality of the handoff block under the current set (5).
        let precommits = vec![
            signed_precommit(&pairs[0], h, n, 1, 5),
            signed_precommit(&pairs[1], h, n, 1, 5),
            signed_precommit(&pairs[2], h, n, 1, 5),
        ];
        let new_pairs = [keypair(10), keypair(11), keypair(12)];
        let new_set = authority_set(&new_pairs, 6);
        assert_ok!(Interop::rotate_authority_set(
            root(),
            *b"TW",
            encode_proof(precommits, 1, 5, h, n),
            new_set
        ));

        assert_eq!(GrandpaAuthoritySets::<Test>::get(*b"TW").unwrap().set_id, 6);
        System::assert_has_event(Event::AuthoritySetRotated { country: *b"TW", set_id: 6 }.into());
    });
}

#[test]
fn rotate_authority_set_rejects_non_sequential_set_id() {
    new_test_ext().execute_with(|| {
        let pairs = [keypair(1), keypair(2), keypair(3), keypair(4)];
        assert_ok!(Interop::init_authority_set(root(), *b"TW", authority_set(&pairs, 5)));

        let (h, n) = (H256::repeat_byte(0xCD), 100u32);
        let precommits = vec![
            signed_precommit(&pairs[0], h, n, 1, 5),
            signed_precommit(&pairs[1], h, n, 1, 5),
            signed_precommit(&pairs[2], h, n, 1, 5),
        ];
        let new_pairs = [keypair(10)];
        // set_id 跳號(8 而非 6)。/ Skips to 8 instead of 6.
        let new_set = authority_set(&new_pairs, 8);
        assert_noop!(
            Interop::rotate_authority_set(root(), *b"TW", encode_proof(precommits, 1, 5, h, n), new_set),
            Error::<Test>::NonSequentialSetId
        );
    });
}

// ----------------------------------------------------------------------------
// 多邊淨額清算 / Multilateral netting
// ----------------------------------------------------------------------------

#[test]
fn net_and_settle_aggregates_verified_instructions() {
    new_test_ext().execute_with(|| {
        let (set_id, round) = (5u64, 1u64);
        let pairs = [keypair(1), keypair(2), keypair(3), keypair(4)];
        assert_ok!(Interop::init_authority_set(root(), *b"TW", authority_set(&pairs, set_id)));

        assert_ok!(Interop::submit_instruction(relayer(), instr(*b"TW", *b"JP", 1_000)));
        assert_ok!(Interop::submit_instruction(relayer(), instr(*b"TW", *b"JP", 500)));
        // 第三筆未驗證最終性,不計入。/ Third is unverified, excluded.
        assert_ok!(Interop::submit_instruction(relayer(), instr(*b"TW", *b"JP", 9_999)));

        let (h, n) = (H256::repeat_byte(0xAB), 42u32);
        let pcs = || {
            vec![
                signed_precommit(&pairs[0], h, n, round, set_id),
                signed_precommit(&pairs[1], h, n, round, set_id),
                signed_precommit(&pairs[2], h, n, round, set_id),
            ]
        };
        assert_ok!(Interop::verify_finality(relayer(), 0, encode_proof(pcs(), round, set_id, h, n)));
        assert_ok!(Interop::verify_finality(relayer(), 1, encode_proof(pcs(), round, set_id, h, n)));

        assert_ok!(Interop::net_and_settle(root(), 1));
        assert_eq!(NetPositions::<Test>::get((*b"TW", *b"JP")), XsuAmount(1_500));
        assert_eq!(Instructions::<Test>::get(0).unwrap().status, XcmStatus::Accepted);
        assert_eq!(Instructions::<Test>::get(1).unwrap().status, XcmStatus::Accepted);
        assert_eq!(Instructions::<Test>::get(2).unwrap().status, XcmStatus::Pending);
    });
}

// ----------------------------------------------------------------------------
// 跨鏈 DID 解析 / Cross-chain DID resolution
// ----------------------------------------------------------------------------

#[test]
fn resolve_did_routes_local_and_foreign() {
    new_test_ext().execute_with(|| {
        // 本鏈 DID(tag tw):mock 回傳未錨定。/ Local (tw): mock returns unanchored.
        assert_eq!(Interop::resolve_did(&did(b"tw")), DidResolution::LocalUnknown);

        // 外鏈 DID(tag jp):尚未互認。/ Foreign (jp): not yet recognized.
        assert_eq!(
            Interop::resolve_did(&did(b"jp")),
            DidResolution::Foreign { country: *b"JP", recognized: false }
        );

        // 為 JP 登記受認可簽發者後,解析回報互認。/ After recognizing a JP issuer.
        assert_ok!(Interop::register_issuer(
            root(),
            TrustRegistryEntry {
                country: *b"JP",
                issuer_key_hash: [9u8; 32],
                scope: BoundedVec::try_from(b"id".to_vec()).unwrap(),
                active: true,
            }
        ));
        assert_eq!(
            Interop::resolve_did(&did(b"jp")),
            DidResolution::Foreign { country: *b"JP", recognized: true }
        );
    });
}

// ----------------------------------------------------------------------------
// 跨境零知識互驗 / Cross-border ZK verification (gating)
// ----------------------------------------------------------------------------

fn recognize_jp_issuer() {
    assert_ok!(Interop::register_issuer(
        root(),
        TrustRegistryEntry {
            country: *b"JP",
            issuer_key_hash: [9u8; 32],
            scope: BoundedVec::try_from(b"id".to_vec()).unwrap(),
            active: true,
        }
    ));
}

fn age_inputs(nullifier: [u8; 32]) -> AgeProofPublicInputs {
    AgeProofPublicInputs { issuer_commitment: [1u8; 32], threshold: 18, nullifier }
}

#[test]
fn verify_foreign_proof_requires_recognized_issuer() {
    new_test_ext().execute_with(|| {
        let proof: ProofBytes = BoundedVec::try_from(vec![0u8; 8]).unwrap();
        assert_noop!(
            Interop::verify_foreign_proof(relayer(), *b"JP", [9u8; 32], proof, age_inputs([2u8; 32])),
            Error::<Test>::IssuerNotRecognized
        );
    });
}

#[test]
fn verify_foreign_proof_requires_registered_vk() {
    new_test_ext().execute_with(|| {
        recognize_jp_issuer();
        let proof: ProofBytes = BoundedVec::try_from(vec![0u8; 8]).unwrap();
        assert_noop!(
            Interop::verify_foreign_proof(relayer(), *b"JP", [9u8; 32], proof, age_inputs([2u8; 32])),
            Error::<Test>::VerifyingKeyNotFound
        );
    });
}

#[test]
fn register_issuer_vk_requires_recognition_then_malformed_proof_is_rejected() {
    new_test_ext().execute_with(|| {
        let vk: BoundedVkBytes = BoundedVec::try_from(vec![0xAAu8; 16]).unwrap();
        // 未受認可不得登記 VK。/ Cannot register a VK for an unrecognized issuer.
        assert_noop!(
            Interop::register_issuer_vk(root(), *b"JP", [9u8; 32], vk.clone()),
            Error::<Test>::IssuerNotRecognized
        );

        recognize_jp_issuer();
        assert_ok!(Interop::register_issuer_vk(root(), *b"JP", [9u8; 32], vk));

        // VK 與 proof 皆為垃圾位元組 → 解碼失敗 → MalformedZkProof。
        // Garbage VK + proof bytes → decode fails → MalformedZkProof.
        let proof: ProofBytes = BoundedVec::try_from(vec![0xFFu8; 8]).unwrap();
        assert_noop!(
            Interop::verify_foreign_proof(relayer(), *b"JP", [9u8; 32], proof, age_inputs([2u8; 32])),
            Error::<Test>::MalformedZkProof
        );
    });
}

#[test]
fn verify_foreign_proof_rejects_replayed_nullifier() {
    new_test_ext().execute_with(|| {
        recognize_jp_issuer();
        let vk: BoundedVkBytes = BoundedVec::try_from(vec![0xAAu8; 16]).unwrap();
        assert_ok!(Interop::register_issuer_vk(root(), *b"JP", [9u8; 32], vk));

        // 預先標記 nullifier 已使用。/ Pre-mark the nullifier as spent.
        UsedNullifiers::<Test>::insert([2u8; 32], ());
        let proof: ProofBytes = BoundedVec::try_from(vec![0u8; 8]).unwrap();
        assert_noop!(
            Interop::verify_foreign_proof(relayer(), *b"JP", [9u8; 32], proof, age_inputs([2u8; 32])),
            Error::<Test>::ProofReplayed
        );
    });
}

// ----------------------------------------------------------------------------
// 跨境稅務協調 / Cross-border tax coordination
// ----------------------------------------------------------------------------

#[test]
fn register_treaty_is_bidirectional() {
    new_test_ext().execute_with(|| {
        let treaty = TaxTreaty {
            withholding_cap: Perbill::from_percent(10),
            method: CreditMethod::Credit,
            active: true,
        };
        assert_ok!(Interop::register_treaty(root(), *b"TW", *b"JP", treaty));
        assert!(Interop::treaty_for(*b"TW", *b"JP").is_some());
        // 反方向查詢亦可命中。/ The reverse direction also resolves.
        assert!(Interop::treaty_for(*b"JP", *b"TW").is_some());
        System::assert_has_event(Event::TreatyRegistered { a: *b"TW", b: *b"JP" }.into());
    });
}

#[test]
fn register_treaty_requires_federation_origin() {
    new_test_ext().execute_with(|| {
        let treaty = TaxTreaty {
            withholding_cap: Perbill::from_percent(10),
            method: CreditMethod::Exemption,
            active: true,
        };
        assert_noop!(
            Interop::register_treaty(relayer(), *b"TW", *b"JP", treaty),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn recognize_foreign_invoice_requires_finalized_head() {
    new_test_ext().execute_with(|| {
        // 尚未建立最終性。/ No finality established yet.
        assert_noop!(
            Interop::recognize_foreign_invoice(relayer(), *b"JP", [5u8; 32]),
            Error::<Test>::NoFinalizedHead
        );

        // 與 JP 建立最終性後即可互認其發票。/ After finality with JP, recognize its invoice.
        let (set_id, round) = (5u64, 1u64);
        let pairs = [keypair(1), keypair(2), keypair(3), keypair(4)];
        assert_ok!(Interop::init_authority_set(root(), *b"JP", authority_set(&pairs, set_id)));
        assert_ok!(Interop::submit_instruction(relayer(), instr(*b"JP", *b"TW", 1)));
        let (h, n) = (H256::repeat_byte(0x11), 7u32);
        let precommits = vec![
            signed_precommit(&pairs[0], h, n, round, set_id),
            signed_precommit(&pairs[1], h, n, round, set_id),
            signed_precommit(&pairs[2], h, n, round, set_id),
        ];
        assert_ok!(Interop::verify_finality(relayer(), 0, encode_proof(precommits, round, set_id, h, n)));

        assert_ok!(Interop::recognize_foreign_invoice(relayer(), *b"JP", [5u8; 32]));
        assert!(Interop::is_recognized_invoice(*b"JP", [5u8; 32]));
    });
}

// ----------------------------------------------------------------------------
// OSS 一站式 VAT / One-Stop-Shop VAT
// ----------------------------------------------------------------------------

#[test]
fn oss_register_then_report_creates_destination_instruction() {
    new_test_ext().execute_with(|| {
        let subject = did(b"tw");
        let reg = OssRegistration { home: *b"TW", vat_id_commitment: [3u8; 32], active: true };
        assert_ok!(Interop::oss_register(relayer(), subject.clone(), reg));
        System::assert_has_event(Event::OssRegistered { home: *b"TW" }.into());

        // 重複登記應失敗。/ Duplicate registration fails.
        let reg2 = OssRegistration { home: *b"TW", vat_id_commitment: [3u8; 32], active: true };
        assert_noop!(
            Interop::oss_register(relayer(), subject.clone(), reg2),
            Error::<Test>::AlreadyRegisteredOss
        );

        // 申報 → 依消費地產生 from=TW、to=DE 的清算指令。
        // Report → destination-allocated clearing instruction from TW to DE.
        assert_ok!(Interop::oss_report(relayer(), subject, *b"DE", XsuAmount(250), [8u8; 32]));
        let created = Instructions::<Test>::get(0).unwrap();
        assert_eq!(created.from, *b"TW");
        assert_eq!(created.to, *b"DE");
        assert_eq!(created.amount, XsuAmount(250));
        assert_eq!(created.status, XcmStatus::Pending);
        System::assert_has_event(
            Event::OssReported { from: *b"TW", to: *b"DE", amount: XsuAmount(250) }.into(),
        );
    });
}

#[test]
fn oss_report_requires_registration() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Interop::oss_report(relayer(), did(b"tw"), *b"DE", XsuAmount(10), [0u8; 32]),
            Error::<Test>::OssNotRegistered
        );
    });
}

// ----------------------------------------------------------------------------
// 互通驗證者質押與罰沒 / Interop validator staking & slashing
// ----------------------------------------------------------------------------

#[test]
fn register_validator_requires_minimum_bond() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Interop::register_validator(relayer(), MIN_VALIDATOR_BOND - 1),
            Error::<Test>::InsufficientBond
        );
        assert_ok!(Interop::register_validator(relayer(), MIN_VALIDATOR_BOND));
        assert_eq!(InteropValidators::<Test>::get(1), Some(MIN_VALIDATOR_BOND));
        assert_noop!(
            Interop::register_validator(relayer(), MIN_VALIDATOR_BOND),
            Error::<Test>::AlreadyRegistered
        );
    });
}

#[test]
fn slash_validator_reduces_bond_and_tallies_total() {
    new_test_ext().execute_with(|| {
        assert_ok!(Interop::register_validator(relayer(), MIN_VALIDATOR_BOND));
        assert_ok!(Interop::slash_validator(root(), 1, 1_000));
        assert_eq!(InteropValidators::<Test>::get(1), Some(MIN_VALIDATOR_BOND - 1_000));
        assert_eq!(TotalSlashed::<Test>::get(), 1_000);
        System::assert_has_event(Event::ValidatorSlashed { who: 1, amount: 1_000 }.into());
    });
}

#[test]
fn slash_validator_rejects_amount_exceeding_bond() {
    new_test_ext().execute_with(|| {
        assert_ok!(Interop::register_validator(relayer(), MIN_VALIDATOR_BOND));
        assert_noop!(
            Interop::slash_validator(root(), 1, MIN_VALIDATOR_BOND + 1),
            Error::<Test>::SlashExceedsBond
        );
    });
}

#[test]
fn slash_validator_rejects_unknown_validator() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Interop::slash_validator(root(), 42, 1),
            Error::<Test>::UnknownValidator
        );
    });
}
