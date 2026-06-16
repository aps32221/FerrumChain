//! # Ferrum 鐵鏈 — node entrypoint (主權節點 / sovereign node)
//!
//! libp2p networking + tokio async runtime + RocksDB/paritydb storage (§04),
//! assembling an Aura import queue and a GRANDPA voter (§07).
#![warn(missing_docs)]

mod chain_spec;
mod cli;
mod command;
mod rpc;
mod service;

fn main() -> sc_cli::Result<()> {
    command::run()
}
