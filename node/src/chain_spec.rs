//! # Ferrum 鐵鏈 — chain specifications (development + sovereign)
//!
//! 建立開發鏈與主權鏈規格:植入受認證驗證者(Aura + GRANDPA 金鑰)、初始
//! FER 餘額、Sudo 治理金鑰。條約理事會席位、XSU 籃子與受認證簽發者則於節點
//! 啟動後由治理(Sudo/Root)以 extrinsic 植入(見 BUILD.md),因相關 pallet
//! 不提供 `GenesisConfig`。
//!
//! Builds the development and sovereign chain specs: seeds accredited
//! validators (Aura + GRANDPA keys), initial FER balances and a Sudo governance
//! key. Treaty-council seats, the XSU basket and accredited issuers are seeded
//! post-launch via governance (Sudo/Root) extrinsics (see BUILD.md), since those
//! pallets expose no `GenesisConfig`.

use ferrum_primitives::{AccountId, Balance, Signature, FER};
use ferrum_runtime::{
    BalancesConfig, RuntimeGenesisConfig, SudoConfig, WASM_BINARY,
};
use sc_service::ChainType;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_core::{sr25519, Pair, Public};
use sp_runtime::traits::{IdentifyAccount, Verify};

/// Specialized `ChainSpec` for the Ferrum runtime.
pub type ChainSpec = sc_service::GenericChainSpec;

/// One million FER endowment per genesis account.
const ENDOWMENT: Balance = 1_000_000 * FER;

/// Generate a crypto pair from a seed phrase.
fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
    TPublic::Pair::from_string(&format!("//{}", seed), None)
        .expect("static values are valid; qed")
        .public()
}

type AccountPublic = <Signature as Verify>::Signer;

/// Derive an `AccountId` from a seed.
fn account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
    AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
    AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Derive an `(AuraId, GrandpaId)` authority key pair from a seed.
fn authority_keys_from_seed(seed: &str) -> (AuraId, GrandpaId) {
    (get_from_seed::<AuraId>(seed), get_from_seed::<GrandpaId>(seed))
}

/// Development chain spec — single accredited validator (Alice), Alice is Sudo.
pub fn development_config() -> Result<ChainSpec, String> {
    let wasm_binary = WASM_BINARY.ok_or_else(|| "Development WASM not available".to_string())?;

    Ok(ChainSpec::builder(wasm_binary, None)
        .with_name("Ferrum Development")
        .with_id("ferrum_dev")
        .with_chain_type(ChainType::Development)
        .with_genesis_config_patch(genesis(
            // One accredited validator authoring slots (§07).
            vec![authority_keys_from_seed("Alice")],
            // Sudo / governance key (bootstraps issuers + council, §05/§11).
            account_id_from_seed::<sr25519::Public>("Alice"),
            // Pre-funded accounts.
            vec![
                account_id_from_seed::<sr25519::Public>("Alice"),
                account_id_from_seed::<sr25519::Public>("Bob"),
                account_id_from_seed::<sr25519::Public>("Alice//stash"),
                account_id_from_seed::<sr25519::Public>("Bob//stash"),
            ],
        ))
        .build())
}

/// Sovereign (local-testnet style) chain spec — multiple accredited validators
/// representing treaty members; Alice acts as the bootstrap governance key.
pub fn sovereign_config() -> Result<ChainSpec, String> {
    let wasm_binary = WASM_BINARY.ok_or_else(|| "Sovereign WASM not available".to_string())?;

    Ok(ChainSpec::builder(wasm_binary, None)
        .with_name("Ferrum Sovereign")
        .with_id("ferrum_sovereign")
        .with_chain_type(ChainType::Local)
        .with_genesis_config_patch(genesis(
            // Accredited institutional validators (central bank, tax authority…).
            vec![
                authority_keys_from_seed("Alice"),
                authority_keys_from_seed("Bob"),
            ],
            account_id_from_seed::<sr25519::Public>("Alice"),
            vec![
                account_id_from_seed::<sr25519::Public>("Alice"),
                account_id_from_seed::<sr25519::Public>("Bob"),
                account_id_from_seed::<sr25519::Public>("Charlie"),
                account_id_from_seed::<sr25519::Public>("Dave"),
                account_id_from_seed::<sr25519::Public>("Eve"),
                account_id_from_seed::<sr25519::Public>("Alice//stash"),
                account_id_from_seed::<sr25519::Public>("Bob//stash"),
            ],
        ))
        .build())
}

/// Compose the genesis JSON patch wiring Aura/GRANDPA authorities, balances and
/// the Sudo key.
fn genesis(
    initial_authorities: Vec<(AuraId, GrandpaId)>,
    sudo_key: AccountId,
    endowed_accounts: Vec<AccountId>,
) -> serde_json::Value {
    let _ = (BalancesConfig::default(), SudoConfig { key: None });

    serde_json::json!({
        "balances": {
            "balances": endowed_accounts
                .iter()
                .cloned()
                .map(|k| (k, ENDOWMENT))
                .collect::<Vec<_>>(),
        },
        "aura": {
            "authorities": initial_authorities
                .iter()
                .map(|x| x.0.clone())
                .collect::<Vec<_>>(),
        },
        "grandpa": {
            "authorities": initial_authorities
                .iter()
                .map(|x| (x.1.clone(), 1u64))
                .collect::<Vec<_>>(),
        },
        "sudo": { "key": Some(sudo_key) },
        // pallet-lottery params (§06 e-invoice lottery). Field names are camelCase
        // per the FRAME genesis-config serde convention. VAT-only eligibility, a 0.2%
        // tax-proportional pool capped at 5% of the attested eTWD reserve, and a
        // three-tier split. Merchant root + circuit VKs are set post-genesis by
        // governance, so the first draw is not auto-opened.
        "lottery": {
            "periodBlocks": 100_000u32,
            "eligibleKinds": [3u8],                       // 3 = TaxKind::ValueAdded
            "taxRatioPpm": 2_000u32,                      // 0.2%
            "reserveCapPpm": 50_000u32,                   // 5%
            "tiers": [
                [0u8, 500_000u32, 1u32, 30_000_000u128],
                [1u8, 300_000u32, 100u32, 1_000_000u128],
                [2u8, 200_000u32, 10_000u32, 200_000u128]
            ],
            "allowForeign": false,
            "commitDeadline": 90_000u32,
            "revealDeadline": 95_000u32,
            "finalizeBlock": 96_000u32,
            "claimWindow": 1_296_000u32,
            "merchantSetRoot": serde_json::Value::Null,
            "eligibilityVk": Vec::<u8>::new(),
            "ownershipVk": Vec::<u8>::new(),
            "openFirstDraw": false,
        },
    })
}

#[allow(dead_code)]
fn _assert_genesis_type(_: &RuntimeGenesisConfig) {}
