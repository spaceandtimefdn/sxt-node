use std::collections::HashMap;
use std::sync::Arc;

use linked_list::LinkedList;
use log::error;
use subxt::{OnlineClient, PolkadotConfig};
use tokio::sync::{mpsc, Mutex};

use crate::tx_submitter::TxUpdate;

/// Alias for transaction status type in a Substrate-based blockchain.
pub type TxStatus = subxt::tx::TxStatus<PolkadotConfig, OnlineClient<PolkadotConfig>>;

/// Alias for tracking transaction progress asynchronously.
pub type TxProgress = subxt::tx::TxProgress<PolkadotConfig, OnlineClient<PolkadotConfig>>;

/// A hashmap storing transaction history, where:
/// - The key is the transaction hash (as a `String`).
/// - The value is a linked list of `Arc<TxStatus>` representing transaction statuses over time.
pub type TxDb = HashMap<String, LinkedList<Arc<TxStatus>>>;

/// A thread-safe database for tracking transaction progress in real-time.
///
/// This structure listens for transaction updates via a Tokio MPSC channel and stores
/// the history of transaction statuses for each extrinsic hash.
pub struct TxProgressDb {
    /// Shared map containing transaction progress history.
    tx_map: Arc<Mutex<TxDb>>,
    /// Asynchronous receiver for transaction status updates.
    rx: Mutex<mpsc::Receiver<TxUpdate>>,
}

impl TxProgressDb {
    /// Creates a new instance of `TxProgressDb` with the given receiver.
    ///
    /// # Arguments
    /// * `rx` - The receiver that listens for transaction progress updates.
    ///
    /// # Returns
    /// A new instance of `TxProgressDb`.
    pub fn new(rx: mpsc::Receiver<TxUpdate>) -> Self {
        Self {
            tx_map: Arc::new(Mutex::new(HashMap::new())),
            rx: Mutex::new(rx),
        }
    }

    /// Starts the transaction progress listener loop.
    ///
    /// This function continuously listens for transaction updates and stores
    /// the progress history in a shared `HashMap`. Each transaction's updates are processed
    /// in a separate async task to avoid blocking the main event loop.
    pub async fn run(self: Arc<Self>) {
        let mut rx = self.rx.lock().await; // Lock receiver

        while let Some((progress, _, _)) = rx.recv().await {
            let tx_hash = format!("{:#x}", progress.extrinsic_hash());
            let tx_map = Arc::clone(&self.tx_map);

            tokio::spawn(async move {
                let mut progress = progress;
                let mut history = LinkedList::new();

                while let Some(event) = progress.next().await {
                    match event {
                        Ok(status) => {
                            let arc_status = Arc::new(status);
                            history.push_back(Arc::clone(&arc_status));

                            // ✅ Store the intermediate status update in `tx_map`
                            let mut map = tx_map.lock().await;
                            map.entry(tx_hash.clone())
                                .or_insert_with(LinkedList::new)
                                .push_back(arc_status);
                        }
                        Err(err) => {
                            error!("Error watching transaction {}: {}", tx_hash, err);
                            return;
                        }
                    }
                }
            });
        }
    }

    /// Retrieves the history of a transaction based on its hash.
    ///
    /// # Arguments
    /// * `tx_hash` - The transaction hash as a string.
    ///
    /// # Returns
    /// * `Some(Vec<Arc<TxStatus>>)` - A vector containing the transaction status history.
    /// * `None` - If the transaction hash is not found.
    pub async fn get_history(&self, tx_hash: &str) -> Option<Vec<Arc<TxStatus>>> {
        self.tx_map
            .lock()
            .await
            .get(tx_hash)
            .map(|list| list.iter().cloned().collect())
    }
}
