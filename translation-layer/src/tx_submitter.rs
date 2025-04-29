//! substrate transaction submitter
use std::sync::Arc;

use log::{error, info, warn};
use snafu::ResultExt;
use subxt::config::polkadot::PolkadotExtrinsicParamsBuilder as Params;
use subxt::tx::{DefaultPayload, TxProgress, TxStatus};
use subxt::utils::H256;
use subxt::{OnlineClient, PolkadotConfig};
use subxt_signer::sr25519::Keypair;
use sxt_core::sxt_chain_runtime;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{sleep, Duration};

use crate::error::{Error, FetchEventsSnafu, FetchInitialNonceSnafu, Result};

const MAX_RETRIES: usize = 3;

pub type DefaultHeader =
    subxt::config::substrate::SubstrateHeader<u32, subxt::config::substrate::BlakeTwo256>;

/// A struct responsible for submitting transactions to a Substrate-based blockchain,
/// managing nonces, and handling retries for failed transactions.
#[derive(Clone, Debug)]
pub struct TxSubmitter {
    /// A shared client for interacting with the blockchain.
    pub client: Arc<Mutex<OnlineClient<PolkadotConfig>>>,
    /// The cryptographic keypair used to sign transactions.
    signer: Keypair,
    /// A mutex-protected nonce value for tracking transaction sequence numbers.
    nonce: Arc<Mutex<u64>>,
    /// Sender for pushing transaction progress to `TxProgressDb`.
    tx_sender: mpsc::Sender<(
        TxProgress<PolkadotConfig, OnlineClient<PolkadotConfig>>,
        H256,
        Option<u64>,
    )>,
    /// RPC url
    rpc_url: String,
}

impl TxSubmitter {
    /// Creates a new `TxSubmitter`, initializing it with the current account nonce.
    ///
    /// # Arguments
    ///
    /// * `client` - An instance of `OnlineClient` for blockchain interaction.
    /// * `signer` - The keypair used to sign transactions.
    ///
    /// # Returns
    ///
    /// Returns a `TxSubmitter` instance or an error if the nonce fetch fails.
    pub async fn new(
        client: OnlineClient<PolkadotConfig>,
        signer: Keypair,
        tx_sender: mpsc::Sender<(
            TxProgress<PolkadotConfig, OnlineClient<PolkadotConfig>>,
            H256,
            Option<u64>,
        )>,
        rpc_url: String,
    ) -> Result<Self> {
        let nonce = fetch_initial_nonce(&client, &signer).await?;
        Ok(Self {
            client: Arc::new(Mutex::new(client)),
            signer,
            nonce: Arc::new(Mutex::new(nonce.into())),
            tx_sender,
            rpc_url,
        })
    }

    /// Submits a transaction with automatic retry logic and nonce management.
    ///
    /// # Arguments
    ///
    /// * `tx` - The transaction payload to be submitted.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the transaction is successfully submitted and processed.
    pub async fn submit<T: subxt::ext::scale_encode::EncodeAsFields>(
        &mut self,
        tx: &DefaultPayload<T>,
    ) -> Result<()> {
        for attempt in 0..=MAX_RETRIES {
            let mut nonce_guard = self.nonce.lock().await;
            let nonce_value = *nonce_guard;
            let tx_params = Params::new().nonce(nonce_value).build();

            match self
                .client
                .lock()
                .await
                .tx()
                .sign_and_submit_then_watch(tx, &self.signer, tx_params)
                .await
            {
                Ok(progress) => {
                    *nonce_guard += 1;
                    drop(nonce_guard);
                    info!(
                        "✅ Successfully submitted transaction on attempt {}",
                        attempt + 1
                    );

                    // Watch the transaction progress
                    return self.watch_tx_progress(progress).await;
                }
                Err(err) if attempt < MAX_RETRIES => {
                    warn!("⚠️ Attempt {} failed: {}. Retrying...", attempt + 1, err);
                    sleep(Duration::from_millis(500 * (attempt as u64 + 1))).await;
                }
                Err(err) => {
                    error!(
                        "❌ Failed to submit transaction after {} attempts: {}",
                        MAX_RETRIES + 1,
                        err
                    );
                    return Err(Error::TransactionError { source: err });
                }
            }
        }
        Err(Error::TransactionError {
            source: subxt::Error::Other("Unexpected transaction failure".into()),
        })
    }

    /// Watches the progress of a submitted transaction and logs relevant status updates.
    async fn watch_tx_progress(
        &self,
        mut progress: TxProgress<PolkadotConfig, OnlineClient<PolkadotConfig>>,
    ) -> Result<()> {
        while let Some(event) = progress.next().await {
            match event {
                Ok(TxStatus::Validated) => info!("📄 Transaction has been validated."),
                Ok(TxStatus::Broadcasted { num_peers }) => {
                    info!("📡 Transaction broadcasted to {} peers.", num_peers);
                }
                Ok(TxStatus::InBestBlock(details)) => {
                    info!(
                        "📦 Transaction is in the best block {:?}",
                        details.block_hash()
                    );
                }
                Ok(TxStatus::InFinalizedBlock(details)) => {
                    info!(
                        "✅ Transaction finalized in block {:?}",
                        details.block_hash()
                    );
                    let _ = self.check_extrinsic_success(details).await;
                    return Ok(());
                }
                Ok(TxStatus::NoLongerInBestBlock) => {
                    warn!("⚠️ Transaction is no longer in the best block. It might have been replaced or forked.");
                }
                Ok(TxStatus::Error { message }) => {
                    error!("❌ Error while watching transaction progress: {message}")
                }
                Ok(TxStatus::Dropped { message }) => {
                    error!("❌ Error transaction dropped: {message}")
                }
                Ok(TxStatus::Invalid { message }) => {
                    error!("❌ Error transaction invalid: {message}")
                }
                Err(err) => {
                    error!("❌ Error while watching transaction progress: {}", err);
                    return Ok(());
                }
            }
        }

        Ok(())
    }

    /// Checks if the transaction (extrinsic) execution was successful.
    async fn check_extrinsic_success(
        &self,
        details: subxt::tx::TxInBlock<PolkadotConfig, OnlineClient<PolkadotConfig>>,
    ) -> Result<()> {
        let events = details.fetch_events().await.context(FetchEventsSnafu)?;

        for event in events.iter() {
            match event {
                Ok(event)
                    if event.pallet_name() == "System"
                        && event.variant_name() == "ExtrinsicSuccess" =>
                {
                    info!("✅ Extrinsic execution was successful.");
                    return Ok(());
                }
                Ok(event)
                    if event.pallet_name() == "System"
                        && event.variant_name() == "ExtrinsicFailed" =>
                {
                    error!("❌ Extrinsic execution failed: {:?}", event);
                    return Err(Error::ExtrinsicFailed);
                }
                _ => {}
            }
        }

        error!("⚠️ No explicit success event found, assuming failure.");
        Err(Error::ExtrinsicFailed)
    }

    /// Submit a transaction and return its hash.
    /// No mortality limit (immortal transaction).
    pub async fn submit_tx_get_hash<T: subxt::ext::scale_encode::EncodeAsFields>(
        &mut self,
        tx: &DefaultPayload<T>,
    ) -> Result<subxt::utils::H256> {
        self.submit_tx_get_hash_inner(tx, None).await
    }

    /// Submit a transaction with optional mortality.
    /// `Some(mortal_block_lifespan)` -> mortal tx, `None` -> immortal.
    pub async fn submit_tx_get_hash_with_mortality<T: subxt::ext::scale_encode::EncodeAsFields>(
        &mut self,
        tx: &DefaultPayload<T>,
        from_block: &subxt::config::substrate::SubstrateHeader<
            u32,
            subxt::config::substrate::BlakeTwo256,
        >,
        for_n_blocks: u64,
    ) -> Result<subxt::utils::H256> {
        self.submit_tx_get_hash_inner(tx, Some((from_block.clone(), for_n_blocks)))
            .await
    }

    /// Shared inner logic that accepts an Option<u64> for mortality.
    async fn submit_tx_get_hash_inner<T: subxt::ext::scale_encode::EncodeAsFields>(
        &mut self,
        tx: &DefaultPayload<T>,
        mortality: Option<(DefaultHeader, u64)>,
    ) -> Result<subxt::utils::H256> {
        for attempt in 0..=MAX_RETRIES {
            let mut nonce_guard = self.nonce.lock().await;
            let nonce_value = *nonce_guard;

            let mut params = Params::new().nonce(nonce_value);

            let mut block_number = None;
            if let Some((ref header, lifespan)) = mortality {
                params = params.mortal(header, lifespan);
                block_number = Some(header.number.into());
            }

            let tx_params = params.build();

            let client = self.client.lock().await;
            let tx_result = client
                .tx()
                .sign_and_submit_then_watch(tx, &self.signer, tx_params)
                .await;
            drop(client);

            match tx_result {
                Ok(tx_progress) => {
                    *nonce_guard += 1;
                    drop(nonce_guard);

                    let hash = tx_progress.extrinsic_hash();
                    info!("✅ Transaction submitted successfully: {:?}", hash);

                    if let Err(err) = self.tx_sender.send((tx_progress, hash, block_number)).await {
                        error!("❌ Failed to send transaction progress: {}", err);
                    }

                    return Ok(hash);
                }

                Err(err) if attempt < MAX_RETRIES => {
                    let err_str = err.to_string();

                    if is_stale_nonce_error(&err_str) {
                        warn!(
                            "🔁 Nonce likely stale (attempt {}): {}",
                            attempt + 1,
                            err_str
                        );

                        match fetch_initial_nonce(&*self.client.lock().await, &self.signer).await {
                            Ok(latest_nonce) => {
                                *nonce_guard = latest_nonce as u64;
                                info!("🔄 Refreshed nonce: {}", latest_nonce);
                            }
                            Err(fetch_err) => {
                                error!("❌ Failed to refresh nonce: {}", fetch_err);
                                return Err(fetch_err);
                            }
                        }
                    } else if is_background_disconnect(&err_str) {
                        warn!(
                            "🔌 Connection dropped (attempt {}): {}. Reconnecting...",
                            attempt + 1,
                            err_str
                        );
                        drop(nonce_guard);
                        self.reconnect_and_refresh_nonce().await?;
                    } else {
                        warn!(
                            "⚠️ Transient failure (attempt {}): {}. Retrying...",
                            attempt + 1,
                            err_str
                        );
                        drop(nonce_guard);
                        sleep(Duration::from_millis(500 * (attempt as u64 + 1))).await;
                    }
                }

                Err(err) => {
                    error!("❌ Giving up after {} attempts: {}", MAX_RETRIES + 1, err);
                    return Err(Error::TransactionError { source: err });
                }
            }
        }

        Err(Error::TransactionError {
            source: subxt::Error::Other("Exceeded retry limit".into()),
        })
    }

    /// Attempts to reconnect the `OnlineClient` using the stored RPC URL.
    ///
    /// This is used when the background task is closed, and a fresh client is needed.
    pub async fn try_reconnect(&mut self) -> Result<()> {
        info!("🔌 Attempting to reconnect to RPC at {}", self.rpc_url);

        match OnlineClient::<PolkadotConfig>::from_url(&self.rpc_url).await {
            Ok(new_client) => {
                let mut client_guard = self.client.lock().await;
                *client_guard = new_client;
                info!("✅ Successfully reconnected to chain.");
                Ok(())
            }
            Err(err) => {
                error!("❌ Failed to reconnect to RPC: {}", err);
                Err(Error::TransactionError { source: err })
            }
        }
    }

    /// Reset the connection and refresh the nonce
    async fn reconnect_and_refresh_nonce(&mut self) -> Result<()> {
        // reconnect to the chain (requires &mut self)
        self.try_reconnect().await?;

        // explicitly scope and drop the client guard before mutably borrowing self
        let refreshed_nonce = {
            let guard = self.client.lock().await;
            let nonce = fetch_initial_nonce(&guard, &self.signer).await?;
            drop(guard); // ✅ force drop
            nonce
        };

        // Now it's safe to mutate self
        *self.nonce.lock().await = refreshed_nonce as u64;
        info!("🔄 Refreshed nonce post-reconnect: {}", refreshed_nonce);

        Ok(())
    }
}

/// Fetches the initial nonce for an account.
///
/// # Arguments
///
/// * `api` - A reference to the `OnlineClient` for querying blockchain storage.
/// * `keypair` - The keypair whose account nonce is being fetched.
///
/// # Returns
///
/// Returns the nonce value as a `u32` or an error if the fetch operation fails.
async fn fetch_initial_nonce(api: &OnlineClient<PolkadotConfig>, keypair: &Keypair) -> Result<u32> {
    let nonce_query = sxt_chain_runtime::api::storage()
        .system()
        .account(keypair.public_key().to_account_id());

    let nonce = api
        .storage()
        .at_latest()
        .await
        .context(FetchInitialNonceSnafu)?
        .fetch(&nonce_query)
        .await
        .context(FetchInitialNonceSnafu)?;

    if let Some(nonce) = nonce {
        return Ok(nonce.nonce);
    }

    Ok(0)
}

fn is_stale_nonce_error(err: &str) -> bool {
    err.contains("Priority is too low")
        || err.contains("Transaction is outdated")
        || err.contains("Stale")
}

fn is_background_disconnect(err: &str) -> bool {
    err.contains("background task closed")
        || err.contains("connection closed")
        || err.contains("restart required")
}
