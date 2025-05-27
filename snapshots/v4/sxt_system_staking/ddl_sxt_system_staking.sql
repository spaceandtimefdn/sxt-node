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

CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.ASSETADDED (
    block_number BIGINT NOT NULL,
    transaction_hash BINARY NOT NULL,
    event_index INTEGER NOT NULL,
    time_stamp TIMESTAMP NOT NULL,
    contract_address BINARY NOT NULL,
    ASSET BINARY NOT NULL,
    ALLOWEDPAYMENTTYPES BINARY NOT NULL,
    PRICEFEED BINARY NOT NULL,
    TOKENDECIMALS SMALLINT NOT NULL,
    STALEPRICETHRESHOLDINSECONDS DECIMAL(20,0) NOT NULL,
    PRIMARY KEY (block_number, transaction_hash, event_index)
);


CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.ASSETREMOVED (
    block_number BIGINT NOT NULL,
    transaction_hash BINARY NOT NULL,
    event_index INTEGER NOT NULL,
    time_stamp TIMESTAMP NOT NULL,
    contract_address BINARY NOT NULL,
    ASSET BINARY NOT NULL,
    PRIMARY KEY (block_number, transaction_hash, event_index)
);


CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.CALLBACKFAILED (
    block_number BIGINT NOT NULL,
    transaction_hash BINARY NOT NULL,
    event_index INTEGER NOT NULL,
    time_stamp TIMESTAMP NOT NULL,
    contract_address BINARY NOT NULL,
    QUERYHASH BINARY NOT NULL,
    CALLBACKCLIENTCONTRACTADDRESS BINARY NOT NULL,
    PRIMARY KEY (block_number, transaction_hash, event_index)
);


CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.CALLBACKSUCCEEDED (
    block_number BIGINT NOT NULL,
    transaction_hash BINARY NOT NULL,
    event_index INTEGER NOT NULL,
    time_stamp TIMESTAMP NOT NULL,
    contract_address BINARY NOT NULL,
    QUERYHASH BINARY NOT NULL,
    CALLBACKCLIENTCONTRACTADDRESS BINARY NOT NULL,
    PRIMARY KEY (block_number, transaction_hash, event_index)
);


CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.INITIALIZED (
    block_number BIGINT NOT NULL,
    transaction_hash BINARY NOT NULL,
    event_index INTEGER NOT NULL,
    time_stamp TIMESTAMP NOT NULL,
    contract_address BINARY NOT NULL,
    VERSION DECIMAL(20,0) NOT NULL,
    PRIMARY KEY (block_number, transaction_hash, event_index)
);


CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.NEWQUERYPAYMENT (
    block_number BIGINT NOT NULL,
    transaction_hash BINARY NOT NULL,
    event_index INTEGER NOT NULL,
    time_stamp TIMESTAMP NOT NULL,
    contract_address BINARY NOT NULL,
    QUERYHASH BINARY NOT NULL,
    ASSET BINARY NOT NULL,
    AMOUNT DECIMAL(75,0) NOT NULL,
    SOURCE_ BINARY NOT NULL,
    AMOUNTINUSD DECIMAL(75,0) NOT NULL,
    PRIMARY KEY (block_number, transaction_hash, event_index)
);


CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.OWNERSHIPTRANSFERRED (
    block_number BIGINT NOT NULL,
    transaction_hash BINARY NOT NULL,
    event_index INTEGER NOT NULL,
    time_stamp TIMESTAMP NOT NULL,
    contract_address BINARY NOT NULL,
    PREVIOUSOWNER BINARY NOT NULL,
    NEWOWNER BINARY NOT NULL,
    PRIMARY KEY (block_number, transaction_hash, event_index)
);


CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.PAYMENTREFUNDED (
    block_number BIGINT NOT NULL,
    transaction_hash BINARY NOT NULL,
    event_index INTEGER NOT NULL,
    time_stamp TIMESTAMP NOT NULL,
    contract_address BINARY NOT NULL,
    QUERYHASH BINARY NOT NULL,
    ASSET BINARY NOT NULL,
    SOURCE_ BINARY NOT NULL,
    AMOUNT DECIMAL(75,0) NOT NULL,
    PRIMARY KEY (block_number, transaction_hash, event_index)
);


CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.PAYMENTSETTLED (
    block_number BIGINT NOT NULL,
    transaction_hash BINARY NOT NULL,
    event_index INTEGER NOT NULL,
    time_stamp TIMESTAMP NOT NULL,
    contract_address BINARY NOT NULL,
    QUERYHASH BINARY NOT NULL,
    USEDAMOUNT DECIMAL(75,0) NOT NULL,
    REMAININGAMOUNT DECIMAL(75,0) NOT NULL,
    PRIMARY KEY (block_number, transaction_hash, event_index)
);


CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.QUERYCANCELED (
    block_number BIGINT NOT NULL,
    transaction_hash BINARY NOT NULL,
    event_index INTEGER NOT NULL,
    time_stamp TIMESTAMP NOT NULL,
    contract_address BINARY NOT NULL,
    QUERYHASH BINARY NOT NULL,
    CALLER BINARY NOT NULL,
    PRIMARY KEY (block_number, transaction_hash, event_index)
);


CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.QUERYFULFILLED (
    block_number BIGINT NOT NULL,
    transaction_hash BINARY NOT NULL,
    event_index INTEGER NOT NULL,
    time_stamp TIMESTAMP NOT NULL,
    contract_address BINARY NOT NULL,
    QUERYHASH BINARY NOT NULL,
    PRIMARY KEY (block_number, transaction_hash, event_index)
);


CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.QUERYRECEIVED (
    block_number BIGINT NOT NULL,
    transaction_hash BINARY NOT NULL,
    event_index INTEGER NOT NULL,
    time_stamp TIMESTAMP NOT NULL,
    contract_address BINARY NOT NULL,
    QUERYNONCE DECIMAL(75,0) NOT NULL,
    SENDER BINARY NOT NULL,
    QUERY BINARY NOT NULL,
    QUERYPARAMETERS BINARY NOT NULL,
    TIMEOUT DECIMAL(20,0) NOT NULL,
    CALLBACKCLIENTCONTRACTADDRESS BINARY NOT NULL,
    CALLBACKGASLIMIT DECIMAL(20,0) NOT NULL,
    CALLBACKDATA BINARY NOT NULL,
    CUSTOMLOGICCONTRACTADDRESS BINARY NOT NULL,
    QUERYHASH BINARY NOT NULL,
    PRIMARY KEY (block_number, transaction_hash, event_index)
);


CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.SENDPAYMENT (
    block_number BIGINT NOT NULL,
    transaction_hash BINARY NOT NULL,
    event_index INTEGER NOT NULL,
    time_stamp TIMESTAMP NOT NULL,
    contract_address BINARY NOT NULL,
    ASSET BINARY NOT NULL,
    AMOUNT DECIMAL(75,0) NOT NULL,
    ONBEHALFOF BINARY NOT NULL,
    TARGET BINARY NOT NULL,
    MEMO BINARY NOT NULL,
    AMOUNTINUSD DECIMAL(75,0) NOT NULL,
    SENDER BINARY NOT NULL,
    PRIMARY KEY (block_number, transaction_hash, event_index)
);


CREATE TABLE IF NOT EXISTS SXT_SYSTEM_STAKING.TREASURYSET (
    block_number BIGINT NOT NULL,
    transaction_hash BINARY NOT NULL,
    event_index INTEGER NOT NULL,
    time_stamp TIMESTAMP NOT NULL,
    contract_address BINARY NOT NULL,
    TREASURY BINARY NOT NULL,
    PRIMARY KEY (block_number, transaction_hash, event_index)
);
