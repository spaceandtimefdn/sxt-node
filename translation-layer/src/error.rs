use snafu::Snafu;

/// Represents errors that can occur during blockchain interactions, key management,
/// and transaction processing.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    /// Error when reading an Ethereum key from a file.
    ///
    /// This occurs when the specified file cannot be read due to an I/O issue.
    #[snafu(display("Failed to read Ethereum key from file '{}': {}", path, source))]
    KeyFileRead {
        /// The path of the key file that could not be read.
        path: String,
        /// The underlying I/O error.
        source: std::io::Error,
    },

    /// Error when parsing an Ethereum key from a hexadecimal string.
    ///
    /// This happens if the provided key string is not a valid hex representation.
    #[snafu(display("Failed to parse Ethereum key as hex: {}", source))]
    KeyParse {
        /// The underlying hex parsing error.
        source: hex::FromHexError,
    },

    /// Error when the parsed key length is invalid.
    ///
    /// Ethereum private keys must be exactly 32 bytes long.
    #[snafu(display("Invalid key length: expected 32 bytes, got {}", length))]
    InvalidKeyLength {
        /// The actual length of the provided key.
        length: usize,
    },

    /// Error when failing to create a keypair from a secret key.
    #[snafu(display("Failed to create keypair from secret key"))]
    KeypairCreationError,

    /// Error when fetching the initial nonce for an account.
    #[snafu(display("Error fetching initial nonce: {source}"))]
    FetchInitialNonceError {
        /// The underlying error from the `subxt` library.
        source: subxt::Error,
    },

    /// Error when attempting to connect to a blockchain network.
    #[snafu(display("Error connecting to chain: {source}"))]
    ChainConnectionError {
        /// The underlying error from the `subxt` library.
        source: subxt::Error,
    },

    /// Error when submitting a transaction.
    #[snafu(display("Error submitting tx: {source}"))]
    TransactionError {
        /// The underlying error from the `subxt` library.
        source: subxt::Error,
    },

    /// Error when writing a record batch to IPC format.
    #[snafu(display("Error writing record batch to IPC format: {source}"))]
    ArrowError {
        /// The underlying error from the `arrow` library.
        source: arrow::error::ArrowError,
    },

    /// Error when fetching blockchain events.
    #[snafu(display("Error fetching blockchain events: {}", source))]
    FetchEventsError {
        /// The underlying error from the `subxt` library.
        source: subxt::Error,
    },

    /// The transaction was included in a block but failed execution.
    #[snafu(display("Extrinsic execution failed."))]
    ExtrinsicFailed,

    /// The translation layer has detected that the connection to the substrate node is outdated and there was an error while reconnecting it
    #[snafu(display("Failed to reconnect to the Substrate node: {}", source))]
    ReconnectionError {
        /// Source Error
        source: subxt::Error,
    },
}

/// Type alias for results that return a `Result<T, Error>`, simplifying error handling.
///
/// This is useful for functions that return `Error` as the failure case.
pub type Result<T, E = Error> = std::result::Result<T, E>;
