CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.UNSTAKEINITIATED(
    block_number bigint not null,
    time_stamp timestamp not null,
    transaction_hash varchar not null,
    event_index int not null,
    contract_address varchar not null,
    decode_error varchar not null,
    staker varchar not null,
    primary key(block_number, event_index)
);

CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.NOMINATED(
    block_number bigint not null,
    time_stamp timestamp not null,
    transaction_hash varchar not null,
    event_index int not null,
    contract_address varchar not null,
    decode_error varchar not null,
    nodesed25519pubkeys varchar not null,
    nominator varchar not null,
    primary key(block_number, event_index)
);

CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.INITIATEUNSTAKECANCELLED(
    block_number bigint not null,
    time_stamp timestamp not null,
    transaction_hash varchar not null,
    event_index int not null,
    contract_address varchar not null,
    decode_error varchar not null,
    staker varchar not null,
    primary key(block_number, event_index)
);

CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.STAKED(
    block_number bigint not null,
    time_stamp timestamp not null,
    transaction_hash varchar not null,
    event_index int not null,
    contract_address varchar not null,
    decode_error varchar not null,
    staker varchar not null,
    amount decimal(75, 0) not null,
    primary key(block_number, event_index)
);

CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.UNSTAKECOMPLETED(
    block_number bigint not null,
    time_stamp timestamp not null,
    transaction_hash varchar not null,
    event_index int not null,
    contract_address varchar not null,
    decode_error varchar not null,
    nodes varchar not null,
    staker varchar not null,
    amount decimal(75, 0) not null,
    sxtblocknumber bigint not null,
    primary key(block_number, event_index)
);

CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.MESSAGE(
    block_number bigint not null,
    time_stamp timestamp not null,
    transaction_hash varchar not null,
    event_index int not null,
    contract_address varchar not null,
    decode_error varchar not null,
    sender varchar not null,
    body varchar not null,
    nonce decimal(75, 0) not null,
    primary key(block_number, event_index)
);
