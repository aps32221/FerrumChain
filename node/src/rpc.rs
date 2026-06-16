//! Full-node RPC extensions (System + TransactionPayment).

use std::sync::Arc;

use ferrum_primitives::{AccountId, Nonce};
use ferrum_runtime::opaque::Block;
use jsonrpsee::RpcModule;
use sc_transaction_pool_api::TransactionPool;
use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};

/// Full client dependencies handed to the RPC builder.
pub struct FullDeps<C, P> {
    /// The client instance (runtime API access).
    pub client: Arc<C>,
    /// Transaction pool (author + pending-extrinsic queries).
    pub pool: Arc<P>,
}

/// Instantiate all full-node RPC extensions.
pub fn create_full<C, P>(
    deps: FullDeps<C, P>,
) -> Result<RpcModule<()>, Box<dyn std::error::Error + Send + Sync>>
where
    C: ProvideRuntimeApi<Block>,
    C: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError> + 'static,
    C: Send + Sync + 'static,
    C::Api: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Nonce>,
    C::Api: BlockBuilder<Block>,
    P: TransactionPool + 'static,
{
    use substrate_frame_rpc_system::{System, SystemApiServer};

    let mut module = RpcModule::new(());
    let FullDeps { client, pool } = deps;

    module.merge(System::new(client.clone(), pool).into_rpc())?;

    Ok(module)
}
