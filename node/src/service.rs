//! Service and ServiceFactory implementation. Specialized wrapper over substrate service.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use codec::Encode;
use futures::prelude::*;
use polkadot_sdk::sc_client_api::{Backend, BlockBackend};
use polkadot_sdk::sc_consensus_babe::{self, SlotProportion};
use polkadot_sdk::sc_network::event::Event;
use polkadot_sdk::sc_network::{NetworkBackend, NetworkEventStream};
use polkadot_sdk::sc_network_sync::strategy::warp::WarpSyncConfig;
use polkadot_sdk::sc_service::config::Configuration;
use polkadot_sdk::sc_service::error::Error as ServiceError;
use polkadot_sdk::sc_service::TaskManager;
use polkadot_sdk::sc_telemetry::{Telemetry, TelemetryWorker};
use polkadot_sdk::sc_transaction_pool_api::OffchainTransactionPoolFactory;
use polkadot_sdk::sp_core::offchain::{OffchainStorage, STORAGE_PREFIX};
use polkadot_sdk::sp_runtime::traits::Block as BlockT;
use polkadot_sdk::{
    sc_authority_discovery,
    sc_basic_authorship,
    sc_consensus,
    sc_consensus_grandpa,
    sc_consensus_slots,
    sc_executor,
    sc_network,
    sc_network_statement,
    sc_network_sync,
    sc_offchain,
    sc_rpc,
    sc_service,
    sc_statement_store,
    sc_storage_monitor,
    sc_telemetry,
    sc_transaction_pool,
    sp_consensus_babe,
    sp_io,
    sp_statement_store,
    sp_timestamp,
    sp_transaction_storage_proof,
};
use proof_of_sql_static_setups::io::initialize_from_config;
use sxt_runtime::opaque::Block;
use sxt_runtime::{self, RuntimeApi};

use crate::cli::Cli;

#[cfg(not(feature = "runtime-benchmarks"))]
pub type HostFunctions = (
    sp_io::SubstrateHostFunctions,
    sp_statement_store::runtime_api::HostFunctions,
    native::interface::HostFunctions,
    native::client::client::HostFunctions,
    native::config::config::HostFunctions,
);

#[cfg(feature = "runtime-benchmarks")]
pub type HostFunctions = (
    sp_io::SubstrateHostFunctions,
    sp_statement_store::runtime_api::HostFunctions,
    native::interface::HostFunctions,
    native::client::client::HostFunctions,
    native::config::config::HostFunctions,
    polkadot_sdk::frame_benchmarking::benchmarking::HostFunctions,
);

pub type RuntimeExecutor = sc_executor::WasmExecutor<HostFunctions>;

/// Full client
pub(crate) type FullClient = sc_service::TFullClient<Block, RuntimeApi, RuntimeExecutor>;
type FullBackend = sc_service::TFullBackend<Block>;
type FullSelectChain = sc_consensus::LongestChain<FullBackend, Block>;
type FullGrandpaBlockImport =
    sc_consensus_grandpa::GrandpaBlockImport<FullBackend, Block, FullClient, FullSelectChain>;

/// The transaction pool type definition.
pub type TransactionPool = sc_transaction_pool::FullPool<Block, FullClient>;

/// The minimum period of blocks on which justifications will be
/// imported and generated.
const GRANDPA_JUSTIFICATION_PERIOD: u32 = 512;

/// Validate the consumer CLI group into a concrete
/// `ProverDbConsumerConfig` (URL non-optional) and seed it under
/// `PROVER_DB_CONFIG_KEY` in offchain persistent local storage.
///
/// `--prover-db-url` is the single switch that enables the consumer:
/// present ⇒ write the config; absent ⇒ the OCW stays dormant and we
/// don't write anything. `--prover-db-include` is allowed (with its
/// `*.*` default) regardless of whether the URL is set; without a URL
/// it just has nothing to filter.
#[expect(
    clippy::result_large_err,
    reason = "ServiceError is from substrate and cannot be modified"
)]
fn configure_prover_db_consumer(
    backend: &FullBackend,
    cli: &crate::cli::ProverDbConsumerCli,
) -> Result<(), ServiceError> {
    let Some(url) = cli.prover_db_url.as_ref() else {
        return Ok(());
    };

    let cfg = sxt_core::prover_db_indexer::ProverDbConsumerConfig {
        url: url.as_str().to_string(),
        include: cli.prover_db_include.clone(),
    };

    let Some(mut storage) = backend.offchain_storage() else {
        return Err(ServiceError::Other(
            "backend did not expose an offchain storage handle; \
             cannot apply --prover-db-url / --prover-db-include"
                .into(),
        ));
    };
    storage.set(
        STORAGE_PREFIX,
        sxt_core::prover_db_indexer::PROVER_DB_CONFIG_KEY,
        &cfg.encode(),
    );
    Ok(())
}

#[allow(clippy::type_complexity)]
#[expect(
    clippy::result_large_err,
    reason = "ServiceError is from substrate and cannot be modified"
)]
pub fn new_partial(
    config: &Configuration,
) -> Result<
    sc_service::PartialComponents<
        FullClient,
        FullBackend,
        FullSelectChain,
        sc_consensus::DefaultImportQueue<Block>,
        sc_transaction_pool::FullPool<Block, FullClient>,
        (
            impl Fn(
                sc_rpc::SubscriptionTaskExecutor,
            ) -> Result<jsonrpsee::RpcModule<()>, sc_service::Error>,
            (
                sc_consensus_babe::BabeBlockImport<Block, FullClient, FullGrandpaBlockImport>,
                sc_consensus_grandpa::LinkHalf<Block, FullClient, FullSelectChain>,
                sc_consensus_babe::BabeLink<Block>,
            ),
            sc_consensus_grandpa::SharedVoterState,
            Option<Telemetry>,
            Arc<sc_statement_store::Store>,
        ),
    >,
    ServiceError,
> {
    let telemetry = config
        .telemetry_endpoints
        .clone()
        .filter(|x| !x.is_empty())
        .map(|endpoints| -> Result<_, sc_telemetry::Error> {
            let worker = TelemetryWorker::new(16)?;
            let telemetry = worker.handle().new_telemetry(endpoints);
            Ok((worker, telemetry))
        })
        .transpose()?;

    let executor = sc_service::new_wasm_executor::<HostFunctions>(&config.executor);
    let (client, backend, keystore_container, task_manager) =
        sc_service::new_full_parts::<Block, RuntimeApi, _>(
            config,
            telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
            executor,
        )?;
    let client = Arc::new(client);

    let telemetry = telemetry.map(|(worker, telemetry)| {
        task_manager
            .spawn_handle()
            .spawn("telemetry", None, worker.run());
        telemetry
    });

    let select_chain = sc_consensus::LongestChain::new(backend.clone());

    let transaction_pool = sc_transaction_pool::BasicPool::new_full(
        config.transaction_pool.clone(),
        config.role.clone().is_authority().into(),
        config.prometheus_registry(),
        task_manager.spawn_essential_handle(),
        client.clone(),
    );

    let (grandpa_block_import, grandpa_link) = sc_consensus_grandpa::block_import(
        client.clone(),
        GRANDPA_JUSTIFICATION_PERIOD,
        &client,
        select_chain.clone(),
        telemetry.as_ref().map(|x| x.handle()),
    )?;

    let (block_import, babe_link) = sc_consensus_babe::block_import(
        sc_consensus_babe::configuration(&*client)?,
        grandpa_block_import.clone(),
        client.clone(),
    )?;

    let slot_duration = babe_link.config().slot_duration();
    let (import_queue, babe_worker_handle) = sc_consensus_babe::import_queue(
        sc_consensus_babe::ImportQueueParams {
            link: babe_link.clone(),
            block_import: block_import.clone(),
            justification_import: Some(Box::new(grandpa_block_import.clone())),
            client: client.clone(),
            select_chain: select_chain.clone(),
            create_inherent_data_providers: move |_, ()| async move {
                let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

                let slot =
                    sp_consensus_babe::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
                        *timestamp,
                        slot_duration,
                    );

                Ok((slot, timestamp))
            },
            spawner: &task_manager.spawn_essential_handle(),
            registry: config.prometheus_registry(),
            telemetry: telemetry.as_ref().map(|x| x.handle()),
            offchain_tx_pool_factory: OffchainTransactionPoolFactory::new(transaction_pool.clone()),
        },
    )?;

    // Returned as 'other'
    let import_setup = (block_import, grandpa_link, babe_link);

    let statement_store = sc_statement_store::Store::new_shared(
        &config.data_path,
        Default::default(),
        client.clone(),
        keystore_container.local_keystore(),
        config.prometheus_registry(),
        &task_manager.spawn_handle(),
    )
    .map_err(|e| ServiceError::Other(format!("Statement store error: {:?}", e)))?;

    let (rpc_extensions_builder, rpc_setup) = {
        let (_, grandpa_link, _) = &import_setup;

        let justification_stream = grandpa_link.justification_stream();
        let shared_authority_set = grandpa_link.shared_authority_set().clone();
        let shared_voter_state = sc_consensus_grandpa::SharedVoterState::empty();
        let shared_voter_state2 = shared_voter_state.clone();

        let finality_proof_provider = sc_consensus_grandpa::FinalityProofProvider::new_for_service(
            backend.clone(),
            Some(shared_authority_set.clone()),
        );

        let client = client.clone();
        let pool = transaction_pool.clone();
        let select_chain = select_chain.clone();
        let keystore = keystore_container.keystore();
        let chain_spec = config.chain_spec.cloned_box();

        let rpc_backend = backend.clone();
        let rpc_statement_store = statement_store.clone();
        let rpc_extensions_builder =
            move |subscription_executor: node_rpc::SubscriptionTaskExecutor| {
                let deps = node_rpc::FullDeps {
                    client: client.clone(),
                    pool: pool.clone(),
                    select_chain: select_chain.clone(),
                    chain_spec: chain_spec.cloned_box(),
                    babe: node_rpc::BabeDeps {
                        keystore: keystore.clone(),
                        babe_worker_handle: babe_worker_handle.clone(),
                    },
                    grandpa: node_rpc::GrandpaDeps {
                        shared_voter_state: shared_voter_state.clone(),
                        shared_authority_set: shared_authority_set.clone(),
                        justification_stream: justification_stream.clone(),
                        subscription_executor: subscription_executor.clone(),
                        finality_provider: finality_proof_provider.clone(),
                    },
                    statement_store: rpc_statement_store.clone(),
                    backend: rpc_backend.clone(),
                };

                node_rpc::create_full(deps).map_err(Into::into)
            };

        (rpc_extensions_builder, shared_voter_state2)
    };

    Ok(sc_service::PartialComponents {
        client,
        backend,
        task_manager,
        import_queue,
        keystore_container,
        select_chain,
        transaction_pool,
        other: (
            rpc_extensions_builder,
            import_setup,
            rpc_setup,
            telemetry,
            statement_store,
        ),
    })
}

/// Result of [`new_full_base`].
pub struct NewFullBase {
    /// The task manager of the node.
    pub task_manager: TaskManager,
    /// The client instance of the node.
    #[expect(dead_code)]
    pub client: Arc<FullClient>,
    /// The networking service of the node.
    #[expect(dead_code)]
    pub network: Arc<dyn sc_network::service::traits::NetworkService>,
    /// The syncing service of the node.
    #[expect(dead_code)]
    pub sync: Arc<sc_network_sync::SyncingService<Block>>,
    /// The transaction pool of the node.
    #[expect(dead_code)]
    pub transaction_pool: Arc<TransactionPool>,
    /// The rpc handlers of the node.
    #[expect(dead_code)]
    pub rpc_handlers: sc_service::RpcHandlers,
}

/// Creates a full service from the configuration.
#[expect(
    clippy::result_large_err,
    reason = "ServiceError is from substrate and cannot be modified"
)]
pub fn new_full_base<N: NetworkBackend<Block, <Block as BlockT>::Hash>>(
    config: Configuration,
    cli: &Cli,
) -> Result<NewFullBase, ServiceError> {
    let role = config.role;
    let force_authoring = config.force_authoring;
    let backoff_authoring_blocks =
        Some(sc_consensus_slots::BackoffAuthoringOnFinalizedHeadLagging {
            // Never wait more than 2 slots before authoring blocks, regardless of delay in
            // finality.
            max_interval: 2u32,
            // Start to consider backing off block authorship once we have 50 or more unfinalized
            // blocks at the head of the chain.
            unfinalized_slack: 50u32,
            // A reasonable default for the authoring bias, or reciprocal interval scaling, is 2.
            // Effectively meaning that consider the unfinalized head suffix length to grow half as
            // fast as in actuality.
            authoring_bias: 2u32,
        });
    let name = config.network.node_name.clone();
    let enable_grandpa = !config.disable_grandpa;
    let prometheus_registry = config.prometheus_registry().cloned();
    let enable_offchain_worker = config.offchain_worker.enabled;

    let sc_service::PartialComponents {
        client,
        backend,
        mut task_manager,
        import_queue,
        keystore_container,
        select_chain,
        transaction_pool,
        other: (rpc_builder, import_setup, rpc_setup, mut telemetry, statement_store),
    } = new_partial(&config)?;

    // The prover-db consumer (URL + include set) only takes effect when
    // offchain indexing is enabled, since the OCW only runs in that
    // case. Fail loud if the operator set --prover-db-url without
    // enabling indexing, rather than silently ignoring it.
    if cli.prover_db.prover_db_url.is_some() && !config.offchain_worker.indexing_enabled {
        return Err(ServiceError::Other(
            "--prover-db-url was set but --enable-offchain-indexing is not \
             true; the prover-db-indexer offchain worker would be inactive. \
             Restart with --enable-offchain-indexing=true, or omit \
             --prover-db-url."
                .into(),
        ));
    }
    configure_prover_db_consumer(backend.as_ref(), &cli.prover_db)?;

    let metrics = N::register_notification_metrics(
        config.prometheus_config.as_ref().map(|cfg| &cfg.registry),
    );
    let shared_voter_state = rpc_setup;
    let auth_disc_publish_non_global_ips = config.network.allow_non_globals_in_dht;
    let auth_disc_public_addresses = config.network.public_addresses.clone();

    let mut net_config = sc_network::config::FullNetworkConfiguration::<_, _, N>::new(
        &config.network,
        config
            .prometheus_config
            .as_ref()
            .map(|cfg| cfg.registry.clone()),
    );

    let genesis_hash = client
        .block_hash(0)
        .ok()
        .flatten()
        .expect("Genesis block exists; qed");
    let peer_store_handle = net_config.peer_store_handle();

    let grandpa_protocol_name =
        sc_consensus_grandpa::protocol_standard_name(&genesis_hash, &config.chain_spec);
    let (grandpa_protocol_config, grandpa_notification_service) =
        sc_consensus_grandpa::grandpa_peers_set_config::<_, N>(
            grandpa_protocol_name.clone(),
            metrics.clone(),
            Arc::clone(&peer_store_handle),
        );
    net_config.add_notification_protocol(grandpa_protocol_config);

    let (statement_handler_proto, statement_config) =
        sc_network_statement::StatementHandlerPrototype::new::<_, _, N>(
            genesis_hash,
            config.chain_spec.fork_id(),
            metrics.clone(),
            Arc::clone(&peer_store_handle),
        );
    net_config.add_notification_protocol(statement_config);

    let warp_sync = Arc::new(sc_consensus_grandpa::warp_proof::NetworkProvider::new(
        backend.clone(),
        import_setup.1.shared_authority_set().clone(),
        Vec::default(),
    ));

    let (network, system_rpc_tx, tx_handler_controller, network_starter, sync_service) =
        sc_service::build_network(sc_service::BuildNetworkParams {
            config: &config,
            net_config,
            client: client.clone(),
            transaction_pool: transaction_pool.clone(),
            spawn_handle: task_manager.spawn_handle(),
            import_queue,
            block_announce_validator_builder: None,
            warp_sync_config: Some(WarpSyncConfig::WithProvider(warp_sync)),
            block_relay: None,
            metrics,
        })?;

    let rpc_handlers = sc_service::spawn_tasks(sc_service::SpawnTasksParams {
        config,
        backend: backend.clone(),
        client: client.clone(),
        keystore: keystore_container.keystore(),
        network: network.clone(),
        rpc_builder: Box::new(rpc_builder),
        transaction_pool: transaction_pool.clone(),
        task_manager: &mut task_manager,
        system_rpc_tx,
        tx_handler_controller,
        sync_service: sync_service.clone(),
        telemetry: telemetry.as_mut(),
    })?;

    let (block_import, grandpa_link, babe_link) = import_setup;

    if let sc_service::config::Role::Authority = &role {
        let proposer = sc_basic_authorship::ProposerFactory::new(
            task_manager.spawn_handle(),
            client.clone(),
            transaction_pool.clone(),
            prometheus_registry.as_ref(),
            telemetry.as_ref().map(|x| x.handle()),
        );

        let client_clone = client.clone();
        let slot_duration = babe_link.config().slot_duration();
        let babe_config = sc_consensus_babe::BabeParams {
            keystore: keystore_container.keystore(),
            client: client.clone(),
            select_chain,
            env: proposer,
            block_import,
            sync_oracle: sync_service.clone(),
            justification_sync_link: sync_service.clone(),
            create_inherent_data_providers: move |parent, ()| {
                let client_clone = client_clone.clone();
                async move {
                    let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

                    let slot =
						sp_consensus_babe::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
							*timestamp,
							slot_duration,
						);

                    let storage_proof =
                        sp_transaction_storage_proof::registration::new_data_provider(
                            &*client_clone,
                            &parent,
                        )?;

                    Ok((slot, timestamp, storage_proof))
                }
            },
            force_authoring,
            backoff_authoring_blocks,
            babe_link,
            block_proposal_slot_portion: SlotProportion::new(0.5),
            max_block_proposal_slot_portion: None,
            telemetry: telemetry.as_ref().map(|x| x.handle()),
        };

        let babe = sc_consensus_babe::start_babe(babe_config)?;
        task_manager.spawn_essential_handle().spawn_blocking(
            "babe-proposer",
            Some("block-authoring"),
            babe,
        );
    }

    // Spawn authority discovery module.
    if role.is_authority() {
        let authority_discovery_role =
            sc_authority_discovery::Role::PublishAndDiscover(keystore_container.keystore());
        let dht_event_stream =
            network
                .event_stream("authority-discovery")
                .filter_map(|e| async move {
                    match e {
                        Event::Dht(e) => Some(e),
                        _ => None,
                    }
                });
        let (authority_discovery_worker, _service) =
            sc_authority_discovery::new_worker_and_service_with_config(
                sc_authority_discovery::WorkerConfig {
                    publish_non_global_ips: auth_disc_publish_non_global_ips,
                    public_addresses: auth_disc_public_addresses,
                    ..Default::default()
                },
                client.clone(),
                Arc::new(network.clone()),
                Box::pin(dht_event_stream),
                authority_discovery_role,
                prometheus_registry.clone(),
            );

        task_manager.spawn_handle().spawn(
            "authority-discovery-worker",
            Some("networking"),
            authority_discovery_worker.run(),
        );
    }

    // if the node isn't actively participating in consensus then it doesn't
    // need a keystore, regardless of which protocol we use below.
    let keystore = if role.is_authority() {
        Some(keystore_container.keystore())
    } else {
        None
    };

    let grandpa_config = sc_consensus_grandpa::Config {
        // FIXME #1578 make this available through chainspec
        gossip_duration: std::time::Duration::from_millis(333),
        justification_generation_period: GRANDPA_JUSTIFICATION_PERIOD,
        name: Some(name),
        observer_enabled: false,
        keystore,
        local_role: role,
        telemetry: telemetry.as_ref().map(|x| x.handle()),
        protocol_name: grandpa_protocol_name,
    };

    if enable_grandpa {
        // start the full GRANDPA voter
        // NOTE: non-authorities could run the GRANDPA observer protocol, but at
        // this point the full voter should provide better guarantees of block
        // and vote data availability than the observer. The observer has not
        // been tested extensively yet and having most nodes in a network run it
        // could lead to finality stalls.
        let grandpa_params = sc_consensus_grandpa::GrandpaParams {
            config: grandpa_config,
            link: grandpa_link,
            network: network.clone(),
            sync: Arc::new(sync_service.clone()),
            notification_service: grandpa_notification_service,
            telemetry: telemetry.as_ref().map(|x| x.handle()),
            voting_rule: sc_consensus_grandpa::VotingRulesBuilder::default().build(),
            prometheus_registry: prometheus_registry.clone(),
            shared_voter_state,
            offchain_tx_pool_factory: OffchainTransactionPoolFactory::new(transaction_pool.clone()),
        };

        // the GRANDPA voter task is considered infallible, i.e.
        // if it fails we take down the service with it.
        task_manager.spawn_essential_handle().spawn_blocking(
            "grandpa-voter",
            None,
            sc_consensus_grandpa::run_grandpa_voter(grandpa_params)?,
        );
    }

    // Spawn statement protocol worker
    let statement_protocol_executor = {
        let spawn_handle = task_manager.spawn_handle();
        Box::new(move |fut| {
            spawn_handle.spawn("network-statement-validator", Some("networking"), fut);
        })
    };
    let statement_handler = statement_handler_proto.build(
        network.clone(),
        sync_service.clone(),
        statement_store.clone(),
        prometheus_registry.as_ref(),
        statement_protocol_executor,
    )?;
    task_manager.spawn_handle().spawn(
        "network-statement-handler",
        Some("networking"),
        statement_handler.run(),
    );

    if enable_offchain_worker {
        let client_provider = Arc::new(crate::client_provider::FullClientHandle(client.clone()));
        let config_store = Arc::new(HashMap::from_iter(cli.ocw_config.iter().cloned()));
        task_manager.spawn_handle().spawn(
            "offchain-workers-runner",
            "offchain-work",
            sc_offchain::OffchainWorkers::new(sc_offchain::OffchainWorkerOptions {
                runtime_api_provider: client.clone(),
                keystore: Some(keystore_container.keystore()),
                offchain_db: backend.offchain_storage(),
                transaction_pool: Some(OffchainTransactionPoolFactory::new(
                    transaction_pool.clone(),
                )),
                network_provider: Arc::new(network.clone()),
                is_validator: role.is_authority(),
                enable_http_requests: true,
                custom_extensions: move |_| {
                    vec![
                        Box::new(statement_store.clone().as_statement_store_ext()) as Box<_>,
                        Box::new(native::client::ClientExt(client_provider.clone())) as Box<_>,
                        Box::new(native::config::ConfigExt(config_store.clone())) as Box<_>,
                    ]
                },
            })
            .run(client.clone(), task_manager.spawn_handle())
            .boxed(),
        );
    }

    network_starter.start_network();
    Ok(NewFullBase {
        task_manager,
        client,
        network,
        sync: sync_service,
        transaction_pool,
        rpc_handlers,
    })
}

/// Builds a new service for a full client.
#[expect(
    clippy::result_large_err,
    reason = "ServiceError is from substrate and cannot be modified"
)]
pub fn new_full(config: Configuration, cli: Cli) -> Result<TaskManager, ServiceError> {
    let database_path = config.database.path().map(Path::to_path_buf);

    futures::executor::block_on(initialize_from_config(&cli.proof_of_sql_public_setup_args))
        .map_err(|e| ServiceError::Other(e.to_string()))?;

    let task_manager = match config.network.network_backend {
        sc_network::config::NetworkBackendType::Libp2p => {
            new_full_base::<sc_network::NetworkWorker<_, _>>(config, &cli)
                .map(|NewFullBase { task_manager, .. }| task_manager)?
        }
        sc_network::config::NetworkBackendType::Litep2p => {
            new_full_base::<sc_network::Litep2pNetworkBackend>(config, &cli)
                .map(|NewFullBase { task_manager, .. }| task_manager)?
        }
    };

    if let Some(database_path) = database_path {
        sc_storage_monitor::StorageMonitorService::try_spawn(
            cli.storage_monitor,
            database_path,
            &task_manager.spawn_essential_handle(),
        )
        .map_err(|e| ServiceError::Application(e.into()))?;
    }

    Ok(task_manager)
}
