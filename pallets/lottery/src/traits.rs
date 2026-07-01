//! Loose-coupling traits — the lottery composes existing pallets through these
//! rather than depending on them directly, so it stays independently testable
//! and the integration points are explicit (see `docs/einvoice-lottery-design.md`).

use ferrum_primitives::{BlockNumber, FiatAmount, Hash32, TaxKind};
use frame_support::pallet_prelude::DispatchResult;

/// Read-only access to `pallet-tax` invoice anchors. The anchoring **block
/// height** (not a caller-supplied Moment) is exposed so the lottery windows
/// entries deterministically and immune to validator timestamp influence.
pub trait InvoiceRegistry {
    /// The invoice's tax category (eligibility is restricted to e.g. `ValueAdded`).
    fn invoice_kind(invoice_hash: &Hash32) -> Option<TaxKind>;
    /// The block height at which the invoice was anchored.
    fn anchored_block(invoice_hash: &Hash32) -> Option<BlockNumber>;
    /// Whether the invoice is anchored at all (and, post-hardening, merchant-signed).
    fn is_anchored(invoice_hash: &Hash32) -> bool;
}

/// Records a PII-free eTWD prize receipt via `pallet-treasury-fer`. Restricted to
/// this pallet's internal origin so external callers cannot squat a receipt key.
/// The value itself moves off-chain on the CBDC rail; this records only a
/// commitment + amount.
pub trait TreasuryPayout<AccountId> {
    fn credit_fiat(beneficiary: &AccountId, receipt_key: Hash32, amount: FiatAmount) -> DispatchResult;
}

/// The central bank's on-chain-attested eTWD reserve. The prize pool is clamped
/// to and atomically debited from this same attested quantity, so every prize is
/// backed 1:1 and funding fails closed when the reserve is insufficient.
pub trait AttestedReserve {
    fn attested_balance() -> FiatAmount;
    /// Debit `amount`; `Err` if the attested balance is insufficient.
    fn try_debit(amount: FiatAmount) -> DispatchResult;
    /// Return recycled / unclaimed funds to the reserve accounting.
    fn credit(amount: FiatAmount);
}
