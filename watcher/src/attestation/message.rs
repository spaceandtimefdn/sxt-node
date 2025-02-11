/// Creates an attestation message by concatenating the state root and block number.
///
/// # Arguments
/// * `state_root` - A reference to the state root, typically a cryptographic hash.
/// * `block_number` - The block number associated with this attestation.
///
/// # Returns
/// A `Vec<u8>` containing the serialized attestation message.
///
pub fn create_attestation_message(state_root: impl AsRef<[u8]>, block_number: u32) -> Vec<u8> {
    let mut msg = Vec::with_capacity(state_root.as_ref().len() + std::mem::size_of::<u32>());
    msg.extend_from_slice(state_root.as_ref());
    msg.extend_from_slice(&block_number.to_be_bytes());
    msg
}
