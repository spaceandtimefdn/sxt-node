//! Service and ServiceFactory implementation. Specialized wrapper over substrate service.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use futures::FutureExt;
use sc_client_api::{Backend, BlockBackend};
use sc_consensus_grandpa::SharedVoterState;
use sc_service::error::Error as ServiceError;
use sc_service::{Configuration, TaskManager, WarpSyncParams};
use sc_telemetry::{Telemetry, TelemetryWorker};
use sc_transaction_pool_api::OffchainTransactionPoolFactory;
use sxt_runtime::opaque::Block;
use sxt_runtime::{self, RuntimeApi};

/// Full client
pub(crate) type FullClient = sc_service::TFullClient<
    Block,
    RuntimeApi,
    sc_executor::WasmExecutor<(
        sp_io::SubstrateHostFunctions,
        sp_statement_store::runtime_api::HostFunctions,
        native::interface::HostFunctions,
    )>,
>;
type FullBackend = sc_service::TFullBackend<Block>;
type FullSelectChain = sc_consensus::LongestChain<FullBackend, Block>;
type FullGrandpaBlockImport =
    sc_consensus_grandpa::GrandpaBlockImport<FullBackend, Block, FullClient, FullSelectChain>;

/// The transaction pool type definition.
pub type TransactionPool = sc_transaction_pool::FullPool<Block, FullClient>;

/// The minimum period of blocks on which justifications will be
/// imported and generated.
const GRANDPA_JUSTIFICATION_PERIOD: u32 = 512;

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
                sc_rpc_api::DenyUnsafe,
                sc_rpc::SubscriptionTaskExecutor,
            ) -> Result<jsonrpsee::RpcModule<()>, sc_service::Error>,
            (
                sc_consensus_babe::BabeBlockImport<Block, FullClient, FullGrandpaBlockImport>,
                sc_consensus_grandpa::LinkHalf<Block, FullClient, FullSelectChain>,
                sc_consensus_babe::BabeLink<Block>,
            ),
            SharedVoterState,
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

    let executor = sc_service::new_wasm_executor::<(
        sp_io::SubstrateHostFunctions,
        sp_statement_store::runtime_api::HostFunctions,
        native::interface::HostFunctions,
    )>(config);
    let (client, backend, keystore_container, task_manager) =
        sc_service::new_full_parts::<Block, RuntimeApi, _>(
            config.clone(),
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
        &(client.clone() as Arc<_>),
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
            move |deny_unsafe, subscription_executor: node_rpc::SubscriptionTaskExecutor| {
                let deps = node_rpc::FullDeps {
                    client: client.clone(),
                    pool: pool.clone(),
                    select_chain: select_chain.clone(),
                    chain_spec: chain_spec.cloned_box(),
                    deny_unsafe,
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
    pub client: Arc<FullClient>,
    /// The networking service of the node.
    pub network: Arc<dyn sc_network::service::traits::NetworkService>,
    /// The syncing service of the node.
    pub sync: Arc<sc_network_sync::SyncingService<Block>>,
    /// The transaction pool of the node.
    pub transaction_pool: Arc<TransactionPool>,
    /// The rpc handlers of the node.
    pub rpc_handlers: sc_service::RpcHandlers,
}

/// Builds a new service for a full client.
pub fn new_full_base<
    N: sc_network::NetworkBackend<Block, <Block as sp_runtime::traits::Block>::Hash>,
>(
    config: Configuration,
    with_db: bool,
) -> Result<NewFullBase, ServiceError> {
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

    let (block_import, grandpa_link, babe_link) = import_setup;

    let is_dev_mode = config.dev_key_seed.is_some();
    let role = config.role.clone();
    let force_authoring = config.force_authoring.clone();
    let backoff_authoring_blocks: Option<()> = None;
    let name = config.network.node_name.clone();
    let enable_grandpa = !config.disable_grandpa.clone();
    let prometheus_registry = config.prometheus_registry().cloned();

    let mut net_config = sc_network::config::FullNetworkConfiguration::<
        Block,
        <Block as sp_runtime::traits::Block>::Hash,
        N,
    >::new(&config.network);
    let metrics = N::register_notification_metrics(config.prometheus_registry());

    let genesis_hash = &client
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
        grandpa_link.shared_authority_set().clone(),
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
            warp_sync_params: Some(WarpSyncParams::WithProvider(warp_sync)),
            block_relay: None,
            metrics,
        })?;

    let role = config.role.clone();

    if config.offchain_worker.enabled {
        task_manager.spawn_handle().spawn(
            "offchain-workers-runner",
            "offchain-worker",
            sc_offchain::OffchainWorkers::new(sc_offchain::OffchainWorkerOptions {
                runtime_api_provider: client.clone(),
                is_validator: role.is_authority(),
                keystore: Some(keystore_container.keystore()),
                offchain_db: backend.offchain_storage(),
                transaction_pool: Some(OffchainTransactionPoolFactory::new(
                    transaction_pool.clone(),
                )),
                network_provider: Arc::new(network.clone()),
                enable_http_requests: true,
                custom_extensions: |_| vec![],
            })
            .run(client.clone(), task_manager.spawn_handle())
            .boxed(),
        );
    }

    let rpc_handlers = sc_service::spawn_tasks(sc_service::SpawnTasksParams {
        config: config,
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

    // Only run the flightsql client if the --with-db flag is enabled on the node binary
    // This saves a significant amount of storage on nodes that only need rpc functionality and
    // validators that only want to participate in consensus
    if with_db {
        sxt_core::sql::spawn_flightsql_tasks::<FullClient, Block, FullBackend>(
            "flightsql-task",
            &task_manager.spawn_essential_handle(),
            client.clone(),
        );
    }

    if role.clone().is_authority() {
        // Build and start the block proposer task
        let mut proposer_factory = sc_basic_authorship::ProposerFactory::new(
            task_manager.spawn_handle(),
            client.clone(),
            transaction_pool.clone(),
            prometheus_registry.as_ref(),
            telemetry.as_ref().map(|x| x.handle()),
        );

        // Set the proposer to a maximum of 15 Mebibytes
        proposer_factory.set_default_block_size_limit(sxt_runtime::MAX_BLOCK_SIZE as usize);

        let client_clone = client.clone();
        let slot_duration = babe_link.config().slot_duration();

        let select_chain = select_chain.clone();
        let keystore = keystore_container.keystore();
        let babe_config = sc_consensus_babe::BabeParams {
            keystore,
            client: client.clone(),
            select_chain,
            env: proposer_factory,
            block_import,
            sync_oracle: sync_service.clone(),
            justification_sync_link: sync_service.clone(),
            create_inherent_data_providers: move |parent, ()| {
                let client_clone = client_clone.clone();
                async move {
                    let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
                    let slot = sp_consensus_babe::inherents::InherentDataProvider::from_timestamp_and_slot_duration(*timestamp, slot_duration,);
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
            block_proposal_slot_portion: sc_consensus_babe::SlotProportion::new(0.5),
            max_block_proposal_slot_portion: None,
            telemetry: telemetry.as_ref().map(|x| x.handle()),
        };

        // Start the babe task with the configuration, tying everything together
        let babe = sc_consensus_babe::start_babe(babe_config)?;
        task_manager.spawn_essential_handle().spawn_blocking(
            "babe-proposer",
            Some("block-authoring"),
            babe,
        );
    }

    if enable_grandpa {
        // if the node isn't actively participating in consensus then it doesn't
        // need a keystore, regardless of which protocol we use below.
        let keystore = if role.is_authority() {
            Some(keystore_container.keystore())
        } else {
            None
        };

        let grandpa_config = sc_consensus_grandpa::Config {
            gossip_duration: Duration::from_millis(333),
            justification_generation_period: GRANDPA_JUSTIFICATION_PERIOD,
            name: Some(name),
            observer_enabled: false,
            keystore,
            local_role: role.clone(),
            telemetry: telemetry.as_ref().map(|x| x.handle()),
            protocol_name: grandpa_protocol_name,
        };

        // start the full GRANDPA voter
        // NOTE: non-authorities could run the GRANDPA observer protocol, but at
        // this point the full voter should provide better guarantees of block
        // and vote data availability than the observer. The observer has not
        // been tested extensively yet and having most nodes in a network run it
        // could lead to finality stalls.
        let grandpa_config = sc_consensus_grandpa::GrandpaParams {
            config: grandpa_config,
            link: grandpa_link,
            network: network.clone(),
            sync: Arc::new(sync_service.clone()),
            notification_service: grandpa_notification_service,
            voting_rule: sc_consensus_grandpa::VotingRulesBuilder::default().build(),
            prometheus_registry: prometheus_registry.clone(),
            shared_voter_state: SharedVoterState::empty(),
            telemetry: telemetry.as_ref().map(|x| x.handle()),
            offchain_tx_pool_factory: OffchainTransactionPoolFactory::new(transaction_pool.clone()),
        };

        // the GRANDPA voter task is considered infallible, i.e.
        // if it fails we take down the service with it.
        task_manager.spawn_essential_handle().spawn_blocking(
            "grandpa-voter",
            None,
            sc_consensus_grandpa::run_grandpa_voter(grandpa_config)?,
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
pub fn new_full(config: Configuration, with_db: bool) -> Result<TaskManager, ServiceError> {
    let database_path = config.database.path().map(Path::to_path_buf);

    let task_manager = match config.network.network_backend {
        sc_network::config::NetworkBackendType::Libp2p => {
            let task_manager = new_full_base::<sc_network::NetworkWorker<_, _>>(config, with_db)
                .map(|NewFullBase { task_manager, .. }| task_manager)?;
            task_manager
        }
        sc_network::config::NetworkBackendType::Litep2p => {
            let task_manager = new_full_base::<sc_network::Litep2pNetworkBackend>(config, with_db)
                .map(|NewFullBase { task_manager, .. }| task_manager)?;
            task_manager
        }
    };

    Ok(task_manager)
}
