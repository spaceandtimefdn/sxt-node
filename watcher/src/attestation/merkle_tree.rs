pub use eth_merkle_tree::tree::MerkleTree;
pub use eth_merkle_tree::utils::keccak::keccak256;
use snafu::{ResultExt, Snafu};

/// Errors that may occur while constructing a Merkle tree or hashing data.
#[derive(Debug, Snafu)]
pub enum MerkleTreeError {
    /// Error when performing Keccak hashing on the input data.
    #[snafu(display("Failed to successfully keccak hash the data {:?}: {source}", data))]
    KeccakHashingError {
        /// The data that failed to be hashed.
        data: Vec<String>,
        /// The underlying error from the Keccak hashing function.
        source: eth_merkle_tree::utils::errors::BytesError,
    },

    /// Error when constructing the Merkle tree.
    #[snafu(display("An error occurred while constructing the Merkle tree: {}", source))]
    TreeConstructionError {
        /// The underlying error that occurred during tree construction.
        source: Box<dyn std::error::Error>,
    },

    /// Error indicating that the calculated Merkle root is empty.
    #[snafu(display("An empty state root was calculated"))]
    EmptyStateRoot,
}

/// Computes the Keccak hash for each element in the given data vector.
///
/// # Arguments
/// * `data` - A vector of strings representing the data to be hashed.
///
/// # Returns
/// A `Result` containing a vector of hashed strings, or an error if hashing fails.
pub fn hash_data(data: Vec<String>) -> Result<Vec<String>, MerkleTreeError> {
    let hashed_data = data
        .clone()
        .into_iter()
        .map(|d| keccak256(&d))
        .collect::<Result<Vec<_>, _>>()
        .context(KeccakHashingSnafu { data })?;

    Ok(hashed_data)
}

/// Builds a Merkle tree from the given hashed data.
///
/// # Arguments
/// * `data` - A vector of hashed strings to construct the Merkle tree.
///
/// # Returns
/// A `Result` containing the constructed `MerkleTree`, or an error if tree construction fails.
pub fn build_merkle_tree(data: Vec<String>) -> Result<MerkleTree, MerkleTreeError> {
    let tree = MerkleTree::new(&data).context(TreeConstructionSnafu)?;
    Ok(tree)
}
