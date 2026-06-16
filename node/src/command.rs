//! CLI command dispatch (run node, build-spec, benchmark, key utils…).

use crate::{
    chain_spec,
    cli::{Cli, Subcommand},
    service,
};
use ferrum_runtime::Block;
use sc_cli::SubstrateCli;
use sc_service::PartialComponents;

impl SubstrateCli for Cli {
    fn impl_name() -> String {
        "Ferrum 鐵鏈 Node".into()
    }

    fn impl_version() -> String {
        env!("SUBSTRATE_CLI_IMPL_VERSION").into()
    }

    fn description() -> String {
        "Ferrum sovereign blockchain node — identity + tax + federation (PoSA: Aura + GRANDPA)"
            .into()
    }

    fn author() -> String {
        "Ferrum 鐵鏈 Authors".into()
    }

    fn support_url() -> String {
        "https://github.com/ferrum-chain/ferrum/issues".into()
    }

    fn copyright_start_year() -> i32 {
        2025
    }

    fn load_spec(&self, id: &str) -> Result<Box<dyn sc_service::ChainSpec>, String> {
        Ok(match id {
            "dev" | "" => Box::new(chain_spec::development_config()?),
            "sovereign" | "local" => Box::new(chain_spec::sovereign_config()?),
            path => {
                Box::new(chain_spec::ChainSpec::from_json_file(std::path::PathBuf::from(path))?)
            }
        })
    }
}

/// Parse the CLI and dispatch to the matching command.
pub fn run() -> sc_cli::Result<()> {
    let cli = Cli::from_args();

    match &cli.subcommand {
        Some(Subcommand::Key(cmd)) => cmd.run(&cli),
        Some(Subcommand::BuildSpec(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.sync_run(|config| cmd.run(config.chain_spec, config.network))
        }
        Some(Subcommand::CheckBlock(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|config| {
                let PartialComponents { client, task_manager, import_queue, .. } =
                    service::new_partial(&config)?;
                Ok((cmd.run(client, import_queue), task_manager))
            })
        }
        Some(Subcommand::ExportBlocks(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|config| {
                let PartialComponents { client, task_manager, .. } =
                    service::new_partial(&config)?;
                Ok((cmd.run(client, config.database), task_manager))
            })
        }
        Some(Subcommand::ExportState(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|config| {
                let PartialComponents { client, task_manager, .. } =
                    service::new_partial(&config)?;
                Ok((cmd.run(client, config.chain_spec), task_manager))
            })
        }
        Some(Subcommand::ImportBlocks(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|config| {
                let PartialComponents { client, task_manager, import_queue, .. } =
                    service::new_partial(&config)?;
                Ok((cmd.run(client, import_queue), task_manager))
            })
        }
        Some(Subcommand::PurgeChain(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.sync_run(|config| cmd.run(config.database))
        }
        Some(Subcommand::Revert(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.async_run(|config| {
                let PartialComponents { client, task_manager, backend, .. } =
                    service::new_partial(&config)?;
                Ok((cmd.run(client, backend, None), task_manager))
            })
        }
        Some(Subcommand::ChainInfo(cmd)) => {
            let runner = cli.create_runner(cmd)?;
            runner.sync_run(|config| cmd.run::<Block>(&config))
        }
        #[cfg(feature = "runtime-benchmarks")]
        Some(Subcommand::Benchmark(cmd)) => {
            use crate::service::FullClient;
            use frame_benchmarking_cli::{BenchmarkCmd, SUBSTRATE_REFERENCE_HARDWARE};

            let runner = cli.create_runner(cmd)?;
            match cmd {
                BenchmarkCmd::Pallet(cmd) => runner.sync_run(|config| {
                    cmd.run_with_spec::<sp_runtime::traits::HashingFor<Block>, ()>(Some(
                        config.chain_spec,
                    ))
                }),
                BenchmarkCmd::Block(cmd) => runner.sync_run(|config| {
                    let PartialComponents { client, .. } = service::new_partial(&config)?;
                    cmd.run(client)
                }),
                BenchmarkCmd::Storage(cmd) => runner.sync_run(|config| {
                    let PartialComponents { client, backend, .. } =
                        service::new_partial(&config)?;
                    let db = backend.expose_db();
                    let storage = backend.expose_storage();
                    cmd.run(config, client, db, storage)
                }),
                BenchmarkCmd::Overhead(_) | BenchmarkCmd::Extrinsic(_) => {
                    Err("Overhead/Extrinsic benchmarks not configured".into())
                }
                BenchmarkCmd::Machine(cmd) => {
                    runner.sync_run(|config| cmd.run(&config, SUBSTRATE_REFERENCE_HARDWARE.clone()))
                }
            }
        }
        None => {
            let runner = cli.create_runner(&cli.run)?;
            runner.run_node_until_exit(|config| async move {
                match config.network.network_backend {
                    sc_network::config::NetworkBackendType::Libp2p => service::new_full::<
                        sc_network::NetworkWorker<
                            ferrum_runtime::opaque::Block,
                            <ferrum_runtime::opaque::Block as sp_runtime::traits::Block>::Hash,
                        >,
                    >(config)
                    .map_err(sc_cli::Error::Service),
                    sc_network::config::NetworkBackendType::Litep2p => {
                        service::new_full::<sc_network::Litep2pNetworkBackend>(config)
                            .map_err(sc_cli::Error::Service)
                    }
                }
            })
        }
    }
}
