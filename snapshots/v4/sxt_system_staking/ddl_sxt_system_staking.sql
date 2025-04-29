CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.UNSTAKEINITIATED(
    block_number bigint not null,
    time_stamp timestamp not null,
    transaction_hash binary not null,
    event_index int not null,
    contract_address binary not null,
    staker binary not null,
    amount decimal(75,0) not null,
    primary key(block_number, event_index)
);

CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.UNSTAKED(
    block_number bigint not null,
    time_stamp timestamp not null,
    transaction_hash binary not null,
    event_index int not null,
    contract_address binary not null,
    staker binary not null,
    amount decimal(75,0) not null,
    primary key(block_number, event_index)
);

CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.UNSTAKECLAIMED(
    block_number bigint not null,
    time_stamp timestamp not null,
    transaction_hash binary not null,
    event_index int not null,
    contract_address binary not null,
    staker binary not null,
    primary key(block_number, event_index)
);

CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.NOMINATED(
    block_number bigint not null,
    time_stamp timestamp not null,
    transaction_hash binary not null,
    event_index int not null,
    contract_address binary not null,
    nodesed25519pubkeys varchar not null,
    nominator binary not null,
    primary key(block_number, event_index)
);

CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.INITIATEUNSTAKECANCELLED(
    block_number bigint not null,
    time_stamp timestamp not null,
    transaction_hash binary not null,
    event_index int not null,
    contract_address binary not null,
    staker binary not null,
    primary key(block_number, event_index)
);

CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.STAKED(
    block_number bigint not null,
    time_stamp timestamp not null,
    transaction_hash binary not null,
    event_index int not null,
    contract_address binary not null,
    staker binary not null,
    amount decimal(75, 0) not null,
    primary key(block_number, event_index)
);

CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.MESSAGE(
    block_number bigint not null,
    time_stamp timestamp not null,
    transaction_hash binary not null,
    event_index int not null,
    contract_address binary not null,
    sender binary not null,
    body binary not null,
    nonce decimal(75, 0) not null,
    primary key(block_number, event_index)
);

CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.UNPAUSED(
    block_number bigint not null,
    transaction_hash binary not null,
    event_index integer not null,
    time_stamp timestamp not null,
    contract_address binary not null,
    account binary not null,
    primary key (block_number, transaction_hash, event_index)
);

CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.UNSTAKINGUNBONDINGPERIODSET(
    block_number bigint not null,
    transaction_hash binary not null,
    event_index integer not null,
    time_stamp timestamp not null,
    contract_address binary not null,
    unstakingunbondingperiod decimal(20,0) not null,
    primary key (block_number, transaction_hash, event_index)
);

CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.STAKINGPOOLSET(
    block_number bigint not null,
    transaction_hash binary not null,
    event_index integer not null,
    time_stamp timestamp not null,
    contract_address binary not null,
    stakingpool binary not null,
    primary key (block_number, transaction_hash, event_index)
);

CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.OWNERSHIPTRANSFERRED(
    block_number bigint not null,
    transaction_hash binary not null,
    event_index integer not null,
    time_stamp timestamp not null,
    contract_address binary not null,
    previousowner binary not null,
    newowner binary not null,
    primary key (block_number, transaction_hash, event_index)
);

CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.PAUSED(
    block_number bigint not null,
    transaction_hash binary not null,
    event_index integer not null,
    time_stamp timestamp not null,
    contract_address binary not null,
    account binary not null,
    primary key (block_number, transaction_hash, event_index)
);

CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.SUBSTRATESIGNATUREVALIDATORSET(
    block_number bigint not null,
    transaction_hash binary not null,
    event_index integer not null,
    time_stamp timestamp not null,
    contract_address binary not null,
    substratesignaturevalidator binary not null,
    primary key (block_number, transaction_hash, event_index)
);

CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.STAKINGTOKENSET(
    block_number bigint not null,
    transaction_hash binary not null,
    event_index integer not null,
    time_stamp timestamp not null,
    contract_address binary not null,
    token binary not null,
    primary key (block_number, transaction_hash, event_index)
);
