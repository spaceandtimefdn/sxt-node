//! substrate transaction submitter
use std::sync::Arc;

use log::{error, info, warn};
use snafu::ResultExt;
use subxt::config::polkadot::PolkadotExtrinsicParamsBuilder as Params;
use subxt::tx::{DefaultPayload, TxProgress, TxStatus};
use subxt::{OnlineClient, PolkadotConfig};
use subxt_signer::sr25519::Keypair;
use sxt_core::sxt_chain_runtime;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{sleep, Duration};

use crate::error::{Error, FetchEventsSnafu, FetchInitialNonceSnafu, Result};

const MAX_RETRIES: usize = 3;

/// A struct responsible for submitting transactions to a Substrate-based blockchain,
/// managing nonces, and handling retries for failed transactions.
#[derive(Clone)]
pub struct TxSubmitter {
    /// A shared client for interacting with the blockchain.
    pub client: Arc<OnlineClient<PolkadotConfig>>,
    /// The cryptographic keypair used to sign transactions.
    signer: Keypair,
    /// A mutex-protected nonce value for tracking transaction sequence numbers.
    nonce: Arc<Mutex<u64>>,
    /// Sender for pushing transaction progress to `TxProgressDb`.
    tx_sender: mpsc::Sender<TxProgress<PolkadotConfig, OnlineClient<PolkadotConfig>>>,
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
        tx_sender: mpsc::Sender<TxProgress<PolkadotConfig, OnlineClient<PolkadotConfig>>>,
    ) -> Result<Self> {
        let nonce = fetch_initial_nonce(&client, &signer).await?;
        Ok(Self {
            client: Arc::new(client),
            signer,
            nonce: Arc::new(Mutex::new(nonce.into())),
            tx_sender,
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

    /// Submits a transaction and returns its hash if successful.
    pub async fn submit_tx_get_hash<T: subxt::ext::scale_encode::EncodeAsFields>(
        &mut self,
        tx: &DefaultPayload<T>,
    ) -> Result<subxt::utils::H256> {
        let mut nonce_guard = self.nonce.lock().await;
        let nonce_value = *nonce_guard;
        let tx_params = Params::new().nonce(nonce_value).build();

        match self
            .client
            .tx()
            .sign_and_submit_then_watch(tx, &self.signer, tx_params)
            .await
        {
            Ok(tx_progress) => {
                *nonce_guard += 1;
                drop(nonce_guard);
                let hash = tx_progress.extrinsic_hash();
                info!("✅ Transaction submitted successfully: {:?}", hash);

                // Send transaction progress to TxProgressDb
                if let Err(err) = self.tx_sender.send(tx_progress).await {
                    error!(
                        "❌ Failed to send transaction progress to TxProgressDb: {}",
                        err
                    );
                }

                Ok(hash)
            }
            Err(err) => {
                error!("❌ Failed to submit transaction: {}", err);
                Err(Error::TransactionError { source: err })
            }
        }
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
