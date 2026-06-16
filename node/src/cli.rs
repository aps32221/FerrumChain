//! CLI definition for the Ferrum node (clap-derived, via `sc-cli`).

use sc_cli::RunCmd;

/// Ferrum 鐵鏈 sovereign node CLI.
#[derive(Debug, clap::Parser)]
pub struct Cli {
    /// Possible subcommands.
    #[command(subcommand)]
    pub subcommand: Option<Subcommand>,

    /// The standard Substrate `run` command (networking, RPC, keystore…).
    #[clap(flatten)]
    pub run: RunCmd,
}

/// Subcommands available on the Ferrum node.
#[derive(Debug, clap::Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum Subcommand {
    /// Key management CLI utilities.
    #[command(subcommand)]
    Key(sc_cli::KeySubcommand),

    /// Build a chain specification.
    BuildSpec(sc_cli::BuildSpecCmd),

    /// Validate blocks.
    CheckBlock(sc_cli::CheckBlockCmd),

    /// Export blocks.
    ExportBlocks(sc_cli::ExportBlocksCmd),

    /// Export the state of a given block into a chain spec.
    ExportState(sc_cli::ExportStateCmd),

    /// Import blocks.
    ImportBlocks(sc_cli::ImportBlocksCmd),

    /// Remove the whole chain.
    PurgeChain(sc_cli::PurgeChainCmd),

    /// Revert the chain to a previous state.
    Revert(sc_cli::RevertCmd),

    /// Sub-commands concerned with benchmarking.
    #[cfg(feature = "runtime-benchmarks")]
    #[command(subcommand)]
    Benchmark(frame_benchmarking_cli::BenchmarkCmd),

    /// Print the chain info.
    ChainInfo(sc_cli::ChainInfoCmd),
}
