//! pallets/federation/src/voting.rs
//!
//! 雙重多數表決規則（whitepaper §11.2）。
//! Dual-majority voting rule (whitepaper §11.2).
//!
//! 一般提案通過,須**同時**滿足兩個維度:(a) 成員數達門檻(主權平等),
//! 且 (b) 贊成方合計 XSU 籃子權重達門檻(經濟份量)。
//!
//! A normal proposal passes only if it clears **both** axes: (a) a threshold
//! share of members (sovereign equality), and (b) a threshold share of XSU
//! basket weight among the Aye voters (economic weight).

use ferrum_primitives::{MemberId, Vote};
use sp_runtime::{Perbill, Saturating};
use sp_std::collections::btree_map::BTreeMap;

// 雙重多數：必須同時通過「成員數」與「籃子權重」兩個維度。
// Dual majority: must pass BOTH the "member count" and "basket weight" axes.
pub fn passes_dual_majority(
    votes: &BTreeMap<MemberId, Vote>,
    basket: &BTreeMap<MemberId, Perbill>, // 各成員 XSU 籃子權重 / each member's XSU basket weight
    threshold: Perbill,                   // 例：from_rational(2u32, 3u32) / e.g. from_rational(2u32, 3u32)
) -> bool {
    let total = votes.len() as u32;
    let ayes = votes.values().filter(|v| **v == Vote::Aye).count() as u32;

    // 維度一：贊成的成員數比例 / Axis 1: share of members voting Aye
    let by_count = Perbill::from_rational(ayes, total.max(1)) >= threshold;

    // 維度二：贊成方的籃子權重總和 / Axis 2: summed basket weight of Aye voters
    let ayes_weight = votes
        .iter()
        .filter(|(_, v)| **v == Vote::Aye)
        .map(|(m, _)| *basket.get(m).unwrap_or(&Perbill::zero()))
        .fold(Perbill::zero(), |a, w| a.saturating_add(w));
    let by_weight = ayes_weight >= threshold;

    by_count && by_weight // 兩維度皆須成立 / both axes must hold
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_std::collections::btree_map::BTreeMap;

    fn member(c: &[u8; 2]) -> MemberId {
        *c
    }

    #[test]
    fn passes_when_both_axes_clear_two_thirds() {
        let mut votes = BTreeMap::new();
        votes.insert(member(b"TW"), Vote::Aye);
        votes.insert(member(b"JP"), Vote::Aye);
        votes.insert(member(b"US"), Vote::Nay);

        let mut basket = BTreeMap::new();
        basket.insert(member(b"TW"), Perbill::from_percent(40));
        basket.insert(member(b"JP"), Perbill::from_percent(30));
        basket.insert(member(b"US"), Perbill::from_percent(30));

        let threshold = Perbill::from_rational(2u32, 3u32);
        // by_count: 2/3 == threshold -> true; by_weight: 70% >= 66.6% -> true
        assert!(passes_dual_majority(&votes, &basket, threshold));
    }

    #[test]
    fn fails_when_weight_axis_falls_short() {
        let mut votes = BTreeMap::new();
        votes.insert(member(b"TW"), Vote::Aye);
        votes.insert(member(b"JP"), Vote::Aye);
        votes.insert(member(b"US"), Vote::Nay);

        let mut basket = BTreeMap::new();
        basket.insert(member(b"TW"), Perbill::from_percent(10));
        basket.insert(member(b"JP"), Perbill::from_percent(10));
        basket.insert(member(b"US"), Perbill::from_percent(80));

        let threshold = Perbill::from_rational(2u32, 3u32);
        // by_count: 2/3 -> true; by_weight: 20% < 66.6% -> false
        assert!(!passes_dual_majority(&votes, &basket, threshold));
    }

    #[test]
    fn fails_when_count_axis_falls_short() {
        let mut votes = BTreeMap::new();
        votes.insert(member(b"TW"), Vote::Aye);
        votes.insert(member(b"JP"), Vote::Nay);
        votes.insert(member(b"US"), Vote::Nay);
        votes.insert(member(b"CN"), Vote::Nay);

        let mut basket = BTreeMap::new();
        basket.insert(member(b"TW"), Perbill::from_percent(90));
        basket.insert(member(b"JP"), Perbill::from_percent(4));
        basket.insert(member(b"US"), Perbill::from_percent(3));
        basket.insert(member(b"CN"), Perbill::from_percent(3));

        let threshold = Perbill::from_rational(2u32, 3u32);
        // by_weight: 90% >= 66.6% -> true; by_count: 1/4 < 66.6% -> false
        assert!(!passes_dual_majority(&votes, &basket, threshold));
    }

    #[test]
    fn empty_votes_do_not_pass() {
        let votes: BTreeMap<MemberId, Vote> = BTreeMap::new();
        let basket: BTreeMap<MemberId, Perbill> = BTreeMap::new();
        let threshold = Perbill::from_rational(2u32, 3u32);
        // total.max(1) avoids div-by-zero; 0/1 = 0% < threshold -> false
        assert!(!passes_dual_majority(&votes, &basket, threshold));
    }
}
