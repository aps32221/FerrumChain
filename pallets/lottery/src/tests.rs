//! `pallet-lottery` 單元測試 / unit tests for the implemented state machine.
//!
//! 涵蓋:設定驗證、開期、登記彩券的把關路徑(含 ZK VK/商家根把關)、承諾—揭示
//! 機制(含最後揭示者偏置防護)、撥款前置條件、稅收累計、暫停/恢復、逾期清掃與
//! 金鑰/商家根設定。完整抽獎(需 ZK 證明 + 視窗化/選號 TODO)另由 `ferrum-zk`
//! 端到端測試覆蓋證明路徑。
#![cfg(test)]

use crate::mock::*;
use crate::{
    ActiveConfig, CurrentDraw, Draws, EligibilityVk, Error, MerchantSetRoot, Pallet,
    PeriodTaxRevenue, Paused, RevealCount,
};
use crate::types::{DrawConfig, DrawState, PrizeTier};
use ferrum_primitives::{FiatAmount, Hash32, ProofBytes, TaxKind};
use frame_support::{assert_noop, assert_ok, traits::Hooks, BoundedVec};
use sp_runtime::DispatchError;

/// 推進區塊並逐塊驅動 `on_initialize`(週期驅動)。
/// Advance blocks, driving `on_initialize` (the cadence hook) each block.
fn run_to_block(n: u64) {
    while System::block_number() < n {
        let next = System::block_number() + 1;
        System::set_block_number(next);
        Lottery::on_initialize(next);
    }
}

const TWD: [u8; 3] = *b"TWD";

fn etwd(minor: u128) -> FiatAmount {
    FiatAmount { currency: TWD, minor_units: minor }
}
fn tier(id: u8, share: u32, winners: u32, cap: u128) -> PrizeTier {
    PrizeTier { tier_id: id, share_ppm: share, winners, unit_cap: etwd(cap) }
}
fn base_cfg() -> DrawConfig {
    DrawConfig {
        period_blocks: 100,
        eligible_kinds: BoundedVec::try_from(vec![TaxKind::ValueAdded]).unwrap(),
        tax_ratio_ppm: 2_000,
        reserve_cap_ppm: 50_000,
        tiers: BoundedVec::try_from(vec![tier(0, 1_000_000, 1, 1_000_000)]).unwrap(),
        allow_foreign: false,
        commit_deadline: 1_000,
        reveal_deadline: 1_000,
        finalize_block: 2_000,
        claim_window: 100,
    }
}
fn root() -> RuntimeOrigin {
    RuntimeOrigin::root()
}
fn open_with(cfg: DrawConfig) {
    assert_ok!(Lottery::set_config(root(), cfg));
    assert_ok!(Lottery::open_draw(root()));
}

#[test]
fn set_config_validates_ratio_and_tiers() {
    new_test_ext().execute_with(|| {
        let mut c = base_cfg();
        c.tax_ratio_ppm = 30_000; // > MaxRatioPpm (20_000)
        assert_noop!(Lottery::set_config(root(), c), Error::<Test>::RatioTooHigh);

        let mut c = base_cfg();
        c.tiers = BoundedVec::try_from(vec![tier(0, 500_000, 1, 1)]).unwrap(); // Σ != 1e6
        assert_noop!(Lottery::set_config(root(), c), Error::<Test>::TiersNotExhaustive);

        assert_ok!(Lottery::set_config(root(), base_cfg()));
        assert!(ActiveConfig::<Test>::get().is_some());
    });
}

#[test]
fn set_config_requires_governance() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Lottery::set_config(RuntimeOrigin::signed(account(1)), base_cfg()),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn open_draw_opens_a_period() {
    new_test_ext().execute_with(|| {
        assert_noop!(Lottery::open_draw(root()), Error::<Test>::ConfigNotSet);
        open_with(base_cfg());
        assert_eq!(CurrentDraw::<Test>::get(), 0);
        assert_eq!(Draws::<Test>::get(0).unwrap().state, DrawState::Open);
    });
}

#[test]
fn register_ticket_gates() {
    new_test_ext().execute_with(|| {
        open_with(base_cfg());
        let who = RuntimeOrigin::signed(account(1));
        let owner = [8u8; 32];
        let proof = ProofBytes::default();

        // not anchored
        assert_noop!(
            Lottery::register_ticket(who.clone(), [1u8; 32], owner, proof.clone(), [10u8; 32]),
            Error::<Test>::InvoiceNotAnchored
        );
        // anchored but ineligible kind
        anchor([2u8; 32], TaxKind::Income, 1);
        assert_noop!(
            Lottery::register_ticket(who.clone(), [2u8; 32], owner, proof.clone(), [11u8; 32]),
            Error::<Test>::IneligibleInvoiceKind
        );
        // eligible VAT invoice, but no merchant-set root yet
        anchor([3u8; 32], TaxKind::ValueAdded, 1);
        assert_noop!(
            Lottery::register_ticket(who.clone(), [3u8; 32], owner, proof.clone(), [12u8; 32]),
            Error::<Test>::MerchantRootUnset
        );
        // root set, but no eligibility VK
        assert_ok!(Lottery::set_merchant_set_root(root(), [7u8; 32]));
        assert_noop!(
            Lottery::register_ticket(who, [3u8; 32], owner, proof, [13u8; 32]),
            Error::<Test>::InvalidVk
        );
    });
}

#[test]
fn register_ticket_rejects_non_canonical_nullifier() {
    new_test_ext().execute_with(|| {
        open_with(base_cfg());
        anchor([3u8; 32], TaxKind::ValueAdded, 1);
        // all-0xFF is above the BLS12-381 scalar modulus → non-canonical, rejected
        // before any merchant-root / VK / proof checks.
        assert_noop!(
            Lottery::register_ticket(
                RuntimeOrigin::signed(account(1)),
                [3u8; 32],
                [8u8; 32],
                ProofBytes::default(),
                [0xFFu8; 32]
            ),
            Error::<Test>::NonCanonicalNullifier
        );
    });
}

#[test]
fn commit_then_reveal_increments_quorum() {
    new_test_ext().execute_with(|| {
        open_with(base_cfg());
        let v = RuntimeOrigin::signed(account(2));
        let seed = [5u8; 32];
        let salt = [6u8; 32];
        let mut buf = seed.to_vec();
        buf.extend_from_slice(&salt);
        let commitment = sp_io::hashing::blake2_256(&buf);

        assert_ok!(Lottery::commit(v.clone(), 0, commitment));
        assert_noop!(Lottery::commit(v.clone(), 0, commitment), Error::<Test>::AlreadyCommitted);
        // wrong reveal
        assert_noop!(Lottery::reveal(v.clone(), 0, [0u8; 32], [0u8; 32]), Error::<Test>::BadReveal);
        // correct reveal
        assert_ok!(Lottery::reveal(v, 0, seed, salt));
        assert_eq!(RevealCount::<Test>::get(0), 1);
    });
}

#[test]
fn reveal_rejected_at_or_after_finalize_block() {
    new_test_ext().execute_with(|| {
        let mut c = base_cfg();
        c.reveal_deadline = 1_000;
        c.finalize_block = 5; // anchor block in the past relative to `now`
        open_with(c);
        System::set_block_number(10);
        assert_noop!(
            Lottery::reveal(RuntimeOrigin::signed(account(2)), 0, [5u8; 32], [6u8; 32]),
            Error::<Test>::RevealAfterFinalizeBlock
        );
    });
}

#[test]
fn fund_period_requires_revenue_snapshot() {
    new_test_ext().execute_with(|| {
        open_with(base_cfg());
        assert_noop!(Lottery::fund_period(root(), 0), Error::<Test>::RevenueNotSnapshotted);
    });
}

#[test]
fn note_settled_revenue_accumulates_into_period() {
    new_test_ext().execute_with(|| {
        open_with(base_cfg());
        Pallet::<Test>::note_settled_revenue(etwd(1_000));
        Pallet::<Test>::note_settled_revenue(etwd(500));
        assert_eq!(PeriodTaxRevenue::<Test>::get(0).unwrap().minor_units, 1_500);
    });
}

#[test]
fn pause_blocks_registration_and_resume_clears() {
    new_test_ext().execute_with(|| {
        open_with(base_cfg());
        anchor([3u8; 32], TaxKind::ValueAdded, 1);
        assert_ok!(Lottery::emergency_pause(root()));
        assert!(Paused::<Test>::get());
        assert_noop!(
            Lottery::register_ticket(
                RuntimeOrigin::signed(account(1)),
                [3u8; 32],
                [8u8; 32],
                ProofBytes::default(),
                [14u8; 32]
            ),
            Error::<Test>::LotteryIsPaused
        );
        // resume is deliberately governance-gated
        assert_noop!(Lottery::resume(RuntimeOrigin::signed(account(1))), DispatchError::BadOrigin);
        assert_ok!(Lottery::resume(root()));
        assert!(!Paused::<Test>::get());
    });
}

#[test]
fn sweep_expired_guards() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Lottery::sweep_expired(RuntimeOrigin::signed(account(1)), 0),
            Error::<Test>::UnknownDraw
        );
        open_with(base_cfg());
        assert_noop!(
            Lottery::sweep_expired(RuntimeOrigin::signed(account(1)), 0),
            Error::<Test>::NotYetExpired
        );
    });
}

#[test]
fn vk_and_merchant_root_setters() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Lottery::set_eligibility_vk(root(), BoundedVec::default()),
            Error::<Test>::InvalidVk
        );
        assert_ok!(Lottery::set_eligibility_vk(root(), BoundedVec::try_from(vec![1u8, 2, 3]).unwrap()));
        assert!(EligibilityVk::<Test>::get().is_some());

        assert_ok!(Lottery::set_merchant_set_root(root(), [7u8; 32]));
        assert_eq!(MerchantSetRoot::<Test>::get(), Some([7u8; 32]));
    });
}

#[test]
fn full_draw_lifecycle_selects_winners() {
    new_test_ext().execute_with(|| {
        let mut c = base_cfg();
        c.period_blocks = 10; // draw 0 opened at block 1 → period_end = 11
        c.commit_deadline = 50;
        c.reveal_deadline = 60;
        c.finalize_block = 70;
        c.claim_window = 1_000;
        c.tiers = BoundedVec::try_from(vec![tier(0, 1_000_000, 3, 1_000_000)]).unwrap(); // 3 winners
        open_with(c);

        // attested reserve + authenticated revenue for this period
        set_reserve(etwd(80_000_000_000));
        Pallet::<Test>::note_settled_revenue(etwd(50_000_000_000));

        // seed 5 entries + their tickets (bypassing the ZK register path here;
        // the eligibility proof path is covered by ferrum-zk's e2e tests)
        let entries: Vec<Hash32> = (0u8..5).map(|i| [i + 100; 32]).collect();
        crate::DrawEntries::<Test>::insert(0, BoundedVec::try_from(entries.clone()).unwrap());
        for h in &entries {
            crate::Tickets::<Test>::insert(
                *h,
                crate::types::LotteryTicket { draw: 0, owner_commitment: [7u8; 32], registered_at: 1 },
            );
        }

        // cadence: at period end on_initialize freezes revenue, Open→Drawing, opens draw 1
        run_to_block(11);
        let rec = Draws::<Test>::get(0).unwrap();
        assert_eq!(rec.state, DrawState::Drawing);
        assert!(rec.revenue_snapshot.is_some());
        assert_eq!(CurrentDraw::<Test>::get(), 1);

        // fund draw 0: pool = min(50e9*0.2%, 80e9*5%) = 100_000_000
        assert_ok!(Lottery::fund_period(root(), 0));
        assert_eq!(crate::PrizePool::<Test>::get(0).unwrap().minor_units, 100_000_000);

        // commit + reveal from two validators (MinReveals = 2 in the mock)
        let mk = |seed: [u8; 32], salt: [u8; 32]| {
            let mut b = seed.to_vec();
            b.extend_from_slice(&salt);
            sp_io::hashing::blake2_256(&b)
        };
        let (s1, sa1) = ([1u8; 32], [2u8; 32]);
        let (s2, sa2) = ([3u8; 32], [4u8; 32]);
        assert_ok!(Lottery::commit(RuntimeOrigin::signed(account(2)), 0, mk(s1, sa1)));
        assert_ok!(Lottery::commit(RuntimeOrigin::signed(account(3)), 0, mk(s2, sa2)));
        assert_ok!(Lottery::reveal(RuntimeOrigin::signed(account(2)), 0, s1, sa1));
        assert_ok!(Lottery::reveal(RuntimeOrigin::signed(account(3)), 0, s2, sa2));
        assert_eq!(RevealCount::<Test>::get(0), 2);

        // seal the entry set (derives root + count from on-chain entries)
        assert_ok!(Lottery::seal_entry_set(root(), 0));
        assert_eq!(crate::EntryCount::<Test>::get(0), 5);

        // cannot finalize before finalize_block
        assert_noop!(
            Lottery::finalize_draw(RuntimeOrigin::signed(account(1)), 0),
            Error::<Test>::TooEarlyToFinalize
        );

        // finalize after finalize_block → deterministic winner selection
        run_to_block(71);
        assert_ok!(Lottery::finalize_draw(RuntimeOrigin::signed(account(1)), 0));
        assert_eq!(Draws::<Test>::get(0).unwrap().state, DrawState::Drawn);

        // exactly 3 distinct winners, all drawn from the entry set
        let winners = crate::Winners::<Test>::get(0, 0);
        assert_eq!(winners.len(), 3);
        for w in winners.iter() {
            assert!(entries.contains(w));
        }
        let distinct: std::collections::BTreeSet<_> = winners.iter().collect();
        assert_eq!(distinct.len(), 3);

        // a real winner's claim reaches the ownership ZK check (no VK set → InvalidVk)
        let w0 = winners[0];
        assert_noop!(
            Lottery::claim_prize(
                RuntimeOrigin::signed(account(1)),
                0,
                0,
                w0,
                account(1),
                ProofBytes::default(),
                [20u8; 32], // canonical nullifier (< field modulus)
                [201u8; 32]
            ),
            Error::<Test>::InvalidVk
        );
    });
}

#[test]
fn non_revealer_is_slashed_and_fallback_folded() {
    new_test_ext().execute_with(|| {
        let mut c = base_cfg();
        c.period_blocks = 10;
        c.commit_deadline = 50;
        c.reveal_deadline = 60;
        c.finalize_block = 70;
        c.claim_window = 1_000;
        c.tiers = BoundedVec::try_from(vec![tier(0, 1_000_000, 1, 1_000_000)]).unwrap();
        open_with(c);
        set_reserve(etwd(80_000_000_000));
        Pallet::<Test>::note_settled_revenue(etwd(50_000_000_000));
        crate::DrawEntries::<Test>::insert(0, BoundedVec::try_from(vec![[100u8; 32]]).unwrap());

        run_to_block(11);
        assert_ok!(Lottery::fund_period(root(), 0));

        let mk = |s: [u8; 32], sa: [u8; 32]| {
            let mut b = s.to_vec();
            b.extend_from_slice(&sa);
            sp_io::hashing::blake2_256(&b)
        };
        // account(2): honest (commit + reveal). account(3): commits, never reveals.
        assert_ok!(Lottery::commit(RuntimeOrigin::signed(account(2)), 0, mk([1u8; 32], [2u8; 32])));
        assert_ok!(Lottery::commit(RuntimeOrigin::signed(account(3)), 0, mk([3u8; 32], [4u8; 32])));
        assert_eq!(Balances::reserved_balance(account(3)), 1_000); // bond held
        assert_ok!(Lottery::reveal(RuntimeOrigin::signed(account(2)), 0, [1u8; 32], [2u8; 32]));
        assert_eq!(Balances::reserved_balance(account(2)), 0); // honest revealer reclaims

        assert_ok!(Lottery::seal_entry_set(root(), 0));
        run_to_block(71);
        let xor_before = crate::RevealedXor::<Test>::get(0);
        assert_ok!(Lottery::finalize_draw(RuntimeOrigin::signed(account(1)), 0));

        // non-revealer bond slashed, and a fallback seed was folded into the entropy
        assert_eq!(crate::TotalSlashed::<Test>::get(), 1_000);
        assert_eq!(Balances::reserved_balance(account(3)), 0);
        assert_eq!(Balances::free_balance(account(3)), 1_000_000 - 1_000);
        assert_ne!(crate::RevealedXor::<Test>::get(0), xor_before);
        // a winner is still selected from the (single-entry) set
        assert_eq!(crate::Winners::<Test>::get(0, 0).len(), 1);
    });
}

#[test]
fn fund_period_clamps_and_debits_reserve() {
    // Exercises the reserve adapter end-to-end via a directly-seeded snapshot.
    new_test_ext().execute_with(|| {
        open_with(base_cfg());
        // seed an attested reserve and a frozen revenue snapshot
        set_reserve(etwd(80_000_000_000));
        Draws::<Test>::mutate(0, |maybe| {
            let r = maybe.as_mut().unwrap();
            r.revenue_snapshot = Some(etwd(50_000_000_000));
            r.state = DrawState::Drawing;
        });
        assert_ok!(Lottery::fund_period(root(), 0));
        // pool = min(50e9 * 2000/1e6, 80e9 * 50000/1e6) = min(100_000_000, 4_000_000_000)
        assert_eq!(crate::PrizePool::<Test>::get(0).unwrap().minor_units, 100_000_000);
        // reserve debited by exactly the pool
        assert_eq!(reserve().unwrap().minor_units, 80_000_000_000 - 100_000_000);
    });
}
