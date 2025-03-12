CREATE SCHEMA IF NOT EXISTS ETHEREUM_BEACON;
CREATE TABLE IF NOT EXISTS ETHEREUM_BEACON.BLOCKS (
    block_number bigint not null,
    slot_number bigint not null,
    time_stamp timestamp not null,
    epoch_number bigint not null,
    proposer_index bigint not null,
    parent_root bytea not null,
    state_root bytea not null,
    randao_reveal bytea not null,
    graffiti bytea not null,
    eth1_block_hash bytea not null,
    eth1_deposit_count bigint not null,
    eth1_deposit_root bytea not null,
    signature bytea not null,
    blob_gas_used bigint not null,
    excess_blob_gas bigint not null,
    primary key (block_number, slot_number)
);

CREATE TABLE IF NOT EXISTS ETHEREUM_BEACON.DEPOSITS (
    block_number bigint not null,
    slot_number bigint not null,
    time_stamp timestamp not null,
    deposit_amount bigint not null,
    pubkey bytea not null,
    signature bytea not null,
    withdrawal_credentials bytea not null,
    epoch_number bigint not null,
    deposit_index int not null,
    primary key (block_number, slot_number, deposit_index)
);

CREATE TABLE IF NOT EXISTS ETHEREUM_BEACON.WITHDRAWALS (
    block_number bigint not null,
    slot_number bigint not null,
    time_stamp timestamp not null,
    withdrawal_index bigint not null,
    validator_index bigint not null,
    address bytea not null,
    amount bigint not null,
    epoch_number bigint not null,
    primary key (block_number, slot_number, withdrawal_index)
);
CREATE TABLE IF NOT EXISTS ETHEREUM_BEACON.ATTESTATIONS (
    block_number bigint not null,
    slot_number bigint not null,
    time_stamp timestamp not null,
    epoch_number bigint not null,
    attestation_index bigint not null,
    aggregation_bits varchar not null,
    beacon_block_root bytea not null,
    source_epoch bigint not null,
    source_root bytea not null,
    target_epoch bigint not null,
    target_root bytea not null,
    attestation_signature bytea not null,
    primary key (block_number, slot_number, attestation_index)
);
CREATE TABLE IF NOT EXISTS ETHEREUM_BEACON.BLOB_SIDECARS (
    block_number bigint not null,
    slot_number bigint not null,
    time_stamp timestamp not null,
    epoch_number bigint not null,
    blob varchar not null,
    blob_index bigint not null,
    kzg_commitment bytea not null,
    kzg_proof bytea not null,
    body_root bytea not null,
    parent_root bytea not null,
    proposer_index bigint not null,
    state_root bytea not null,
    attestation_signature bytea not null,
    primary key (block_number, slot_number, blob_index)
);
CREATE TABLE IF NOT EXISTS ETHEREUM_BEACON.BLOB_SIDECAR_KZG_COMMITMENT_INCLUSION_PROOFS (
    block_number bigint not null,
    slot_number bigint not null,
    time_stamp timestamp not null,
    epoch_number bigint not null,
    blob_index bigint not null,
    kzg_commitment_inclusion_proof_index int not null,
    kzg_commitment_inclusion_proof_value bytea not null,
    primary key (
    block_number,
    slot_number,
    blob_index,
    kzg_commitment_inclusion_proof_index
    )
);
CREATE TABLE IF NOT EXISTS ETHEREUM_BEACON.VALIDATOR_BALANCES (
    block_number bigint not null,
    slot_number bigint not null,
    time_stamp timestamp not null,
    epoch_number bigint not null,
    state_id bytea not null,
    validator_index bigint not null,
    balance bigint not null,
    primary key (block_number, slot_number, validator_index)
);
CREATE TABLE IF NOT EXISTS ETHEREUM_BEACON.ATTESTER_SLASHINGS (
    block_number bigint not null,
    slot_number bigint not null,
    time_stamp timestamp not null,
    epoch_number bigint not null,
    attestation_1_beacon_block_root bytea not null,
    attestation_1_index bigint not null,
    attestation_1_slot bigint not null,
    attestation_1_source_epoch bigint not null,
    attestation_1_source_root bytea not null,
    attestation_1_target_epoch bigint not null,
    attestation_1_target_root bytea not null,
    attestation_1_signature bytea not null,
    attestation_2_beacon_block_root bytea not null,
    attestation_2_index bigint not null,
    attestation_2_slot bigint not null,
    attestation_2_source_epoch bigint not null,
    attestation_2_source_root bytea not null,
    attestation_2_target_epoch bigint not null,
    attestation_2_target_root bytea not null,
    attestation_2_signature bytea not null,
    primary key (block_number, slot_number)
);
CREATE TABLE IF NOT EXISTS ETHEREUM_BEACON.PROPOSER_SLASHINGS (
    block_number bigint not null,
    slot_number bigint not null,
    time_stamp timestamp not null,
    epoch_number bigint not null,
    signed_header_1_body_root bytea not null,
    signed_header_1_parent_root bytea not null,
    signed_header_1_proposer_index bigint not null,
    signed_header_1_slot bigint not null,
    signed_header_1_state_root bytea not null,
    signed_header_1_signature bytea not null,
    signed_header_2_body_root bytea not null,
    signed_header_2_parent_root bytea not null,
    signed_header_2_proposer_index bigint not null,
    signed_header_2_slot bigint not null,
    signed_header_2_state_root bytea not null,
    signed_header_2_signature bytea not null,
    primary key (block_number, slot_number)
);
CREATE TABLE IF NOT EXISTS ETHEREUM_BEACON.BLS_TO_EXECUTION_CHANGES (
    block_number bigint not null,
    slot_number bigint not null,
    time_stamp timestamp not null,
    epoch_number bigint not null,
    from_bls_pubkey bytea not null,
    to_execution_address bytea not null,
    validator_index bigint not null,
    signature bytea not null,
    primary key (
    block_number,
    slot_number,
    validator_index,
    signature
    )
);
CREATE TABLE IF NOT EXISTS ETHEREUM_BEACON.VOLUNTARY_EXITS (
block_number bigint not null,
slot_number bigint not null,
time_stamp timestamp not null,
epoch_number bigint not null,
signature bytea not null,
validator_index bigint not null,
primary key (block_number, slot_number, validator_index)
);
CREATE TABLE IF NOT EXISTS ETHEREUM_BEACON.BLOB_KZG_COMMITMENTS (
block_number bigint not null,
slot_number bigint not null,
time_stamp timestamp not null,
epoch_number bigint not null,
commitment bytea not null,
commitment_index int not null,
primary key (block_number, slot_number, commitment_index)
);
CREATE TABLE IF NOT EXISTS ETHEREUM_BEACON.DEPOSIT_PROOFS (
block_number bigint not null,
slot_number bigint not null,
time_stamp timestamp not null,
epoch_number bigint not null,
proof bytea not null,
deposit_index int not null,
proof_index int not null,
primary key (
block_number,
slot_number,
deposit_index,
proof_index
)
);
CREATE TABLE IF NOT EXISTS ETHEREUM_BEACON.ATTESTER_SLASHINGS_ATTESTING_INDICES (
block_number bigint not null,
slot_number bigint not null,
time_stamp timestamp not null,
epoch_number bigint not null,
attestation_index bigint not null,
attesting_indices_index bigint not null,
attesting_indices_value bigint not null,
primary key (
block_number,
slot_number,
attestation_index,
attesting_indices_index,
attesting_indices_value
)
);