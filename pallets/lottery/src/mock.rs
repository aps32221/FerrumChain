//! 測試用模擬執行環境 / Mock runtime for `pallet-lottery` unit tests.
//!
//! `pallet-tax`(發票錨定)、`pallet-treasury-fer`(eTWD 收據)與央行認證準備
//! 以執行緒區域(thread-local)mock 介面注入,讓測試可登記發票、設定準備餘額
//! 並觀察得獎收據——毋須拉入整條 runtime。
#![cfg(test)]

use crate as pallet_lottery;
use crate::traits::{AttestedReserve, InvoiceRegistry, TreasuryPayout};
use ferrum_primitives::{AccountId, BlockNumber, FiatAmount, Hash32, TaxKind};
use frame_support::{derive_impl, parameter_types};
use frame_system::EnsureRoot;
use sp_runtime::{traits::IdentityLookup, BuildStorage, DispatchError, DispatchResult};
use std::cell::RefCell;
use std::collections::BTreeMap;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
    pub enum Test {
        System: frame_system,
        Balances: pallet_balances,
        Lottery: pallet_lottery,
    }
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountId = AccountId;
    type Lookup = IdentityLookup<Self::AccountId>;
    type AccountData = pallet_balances::AccountData<u128>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
    type Balance = u128;
    type AccountStore = System;
    type ExistentialDeposit = frame_support::traits::ConstU128<1>;
}

// ---- thread-local mock backing stores -------------------------------------
thread_local! {
    /// invoice_hash -> (kind, anchored block)
    static INVOICES: RefCell<BTreeMap<Hash32, (TaxKind, BlockNumber)>> = RefCell::new(BTreeMap::new());
    /// the attested eTWD reserve
    static RESERVE: RefCell<Option<FiatAmount>> = RefCell::new(None);
    /// recorded prize receipts (beneficiary, receipt_key, amount)
    static RECEIPTS: RefCell<Vec<(AccountId, Hash32, FiatAmount)>> = RefCell::new(Vec::new());
}

/// 測試輔助:登記一筆已錨定發票。/ Test helper: register an anchored invoice.
pub fn anchor(hash: Hash32, kind: TaxKind, block: BlockNumber) {
    INVOICES.with(|m| m.borrow_mut().insert(hash, (kind, block)));
}
/// 測試輔助:設定認證 eTWD 準備餘額。/ Set the attested eTWD reserve.
pub fn set_reserve(amount: FiatAmount) {
    RESERVE.with(|r| *r.borrow_mut() = Some(amount));
}
/// 測試輔助:讀取目前準備餘額。/ Read the current reserve.
pub fn reserve() -> Option<FiatAmount> {
    RESERVE.with(|r| *r.borrow())
}
/// 測試輔助:讀取已記錄之得獎收據。/ Read recorded prize receipts.
#[allow(dead_code)]
pub fn receipts() -> Vec<(AccountId, Hash32, FiatAmount)> {
    RECEIPTS.with(|r| r.borrow().clone())
}
fn reset_mocks() {
    INVOICES.with(|m| m.borrow_mut().clear());
    RESERVE.with(|r| *r.borrow_mut() = None);
    RECEIPTS.with(|r| r.borrow_mut().clear());
}

/// 發票錨定唯讀介面 mock。/ Mock invoice-anchor read API.
pub struct MockTax;
impl InvoiceRegistry for MockTax {
    fn invoice_kind(invoice_hash: &Hash32) -> Option<TaxKind> {
        INVOICES.with(|m| m.borrow().get(invoice_hash).map(|(k, _)| *k))
    }
    fn anchored_block(invoice_hash: &Hash32) -> Option<BlockNumber> {
        INVOICES.with(|m| m.borrow().get(invoice_hash).map(|(_, b)| *b))
    }
    fn is_anchored(invoice_hash: &Hash32) -> bool {
        INVOICES.with(|m| m.borrow().contains_key(invoice_hash))
    }
}

/// eTWD 得獎收據記錄器 mock。/ Mock eTWD prize-receipt recorder.
pub struct MockTreasury;
impl TreasuryPayout<AccountId> for MockTreasury {
    fn credit_fiat(beneficiary: &AccountId, receipt_key: Hash32, amount: FiatAmount) -> DispatchResult {
        RECEIPTS.with(|r| r.borrow_mut().push((beneficiary.clone(), receipt_key, amount)));
        Ok(())
    }
}

/// 央行認證 eTWD 準備餘額 mock。/ Mock attested eTWD reserve.
pub struct MockReserve;
impl AttestedReserve for MockReserve {
    fn attested_balance() -> FiatAmount {
        reserve().unwrap_or(FiatAmount { currency: *b"TWD", minor_units: 0 })
    }
    fn try_debit(amount: FiatAmount) -> DispatchResult {
        RESERVE.with(|r| {
            let mut g = r.borrow_mut();
            match &mut *g {
                Some(res) if res.currency == amount.currency && res.minor_units >= amount.minor_units => {
                    res.minor_units -= amount.minor_units;
                    Ok(())
                }
                _ => Err(DispatchError::Other("insufficient attested eTWD reserve")),
            }
        })
    }
    fn credit(amount: FiatAmount) {
        RESERVE.with(|r| {
            let mut g = r.borrow_mut();
            match &mut *g {
                Some(res) => res.minor_units = res.minor_units.saturating_add(amount.minor_units),
                None => *g = Some(amount),
            }
        });
    }
}

parameter_types! {
    pub const LotteryPrizeCurrency: ferrum_primitives::FiatCurrency = *b"TWD";
    pub const LotteryMaxRatioPpm: u32 = 20_000; // 2%
    pub const LotteryMinReveals: u32 = 1;
    pub const LotteryCommitDeposit: u128 = 1_000;
    pub const LotteryAgeThreshold: u32 = 18;
}

impl pallet_lottery::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type Tax = MockTax;
    type AgeThreshold = LotteryAgeThreshold;
    type PrizeTreasury = MockTreasury;
    type EtwdReserve = MockReserve;
    type GovernanceOrigin = EnsureRoot<AccountId>;
    type EmergencyOrigin = EnsureRoot<AccountId>;
    type RegistrarOrigin = EnsureRoot<AccountId>;
    type PrizeCurrency = LotteryPrizeCurrency;
    type MaxRatioPpm = LotteryMaxRatioPpm;
    type MinReveals = LotteryMinReveals;
    type CommitDeposit = LotteryCommitDeposit;
    type Currency = Balances;
    type WeightInfo = ();
}

/// 全 `b` 32 位元組帳戶。/ An all-`b` 32-byte account.
pub fn account(b: u8) -> AccountId {
    AccountId::from([b; 32])
}

/// 建立測試用初始狀態(並重置 mock 後端)。
/// Build genesis storage for the mock runtime (and reset mock backends).
pub fn new_test_ext() -> sp_io::TestExternalities {
    reset_mocks();
    let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
    // Endow the validator/committer accounts so they can bond the commit deposit.
    pallet_balances::GenesisConfig::<Test> {
        balances: (1u8..=5).map(|b| (account(b), 1_000_000u128)).collect(),
    }
    .assimilate_storage(&mut t)
    .unwrap();
    let mut ext = sp_io::TestExternalities::new(t);
    ext.execute_with(|| System::set_block_number(1));
    ext
}
