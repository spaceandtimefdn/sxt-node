CREATE TABLE
    IF NOT EXISTS SXT_SYSTEM_ZKPAY.ASSETADDED (
        block_number BIGINT NOT NULL,
        transaction_hash BINARY NOT NULL,
        event_index INTEGER NOT NULL,
        time_stamp TIMESTAMP NOT NULL,
        contract_address BINARY NOT NULL,
        asset BINARY NOT NULL,
        allowedpaymenttypes BINARY NOT NULL,
        pricefeed BINARY NOT NULL,
        tokendecimals SMALLINT NOT NULL,
        stalepricethresholdinseconds DECIMAL(20, 0) NOT NULL,
        PRIMARY KEY (block_number, transaction_hash, event_index)
    );

CREATE TABLE
    IF NOT EXISTS SXT_SYSTEM_ZKPAY.ASSETREMOVED (
        block_number BIGINT NOT NULL,
        transaction_hash BINARY NOT NULL,
        event_index INTEGER NOT NULL,
        time_stamp TIMESTAMP NOT NULL,
        contract_address BINARY NOT NULL,
        asset BINARY NOT NULL,
        PRIMARY KEY (block_number, transaction_hash, event_index)
    );

CREATE TABLE
    IF NOT EXISTS SXT_SYSTEM_ZKPAY.CALLBACKFAILED (
        block_number BIGINT NOT NULL,
        transaction_hash BINARY NOT NULL,
        event_index INTEGER NOT NULL,
        time_stamp TIMESTAMP NOT NULL,
        contract_address BINARY NOT NULL,
        queryhash BINARY NOT NULL,
        callbackclientcontractaddress BINARY NOT NULL,
        PRIMARY KEY (block_number, transaction_hash, event_index)
    );

CREATE TABLE
    IF NOT EXISTS SXT_SYSTEM_ZKPAY.CALLBACKSUCCEEDED (
        block_number BIGINT NOT NULL,
        transaction_hash BINARY NOT NULL,
        event_index INTEGER NOT NULL,
        time_stamp TIMESTAMP NOT NULL,
        contract_address BINARY NOT NULL,
        queryhash BINARY NOT NULL,
        callbackclientcontractaddress BINARY NOT NULL,
        PRIMARY KEY (block_number, transaction_hash, event_index)
    );

CREATE TABLE
    IF NOT EXISTS SXT_SYSTEM_ZKPAY.INITIALIZED (
        block_number BIGINT NOT NULL,
        transaction_hash BINARY NOT NULL,
        event_index INTEGER NOT NULL,
        time_stamp TIMESTAMP NOT NULL,
        contract_address BINARY NOT NULL,
        version DECIMAL(20, 0) NOT NULL,
        PRIMARY KEY (block_number, transaction_hash, event_index)
    );

CREATE TABLE
    IF NOT EXISTS SXT_SYSTEM_ZKPAY.NEWQUERYPAYMENT (
        block_number BIGINT NOT NULL,
        transaction_hash BINARY NOT NULL,
        event_index INTEGER NOT NULL,
        time_stamp TIMESTAMP NOT NULL,
        contract_address BINARY NOT NULL,
        queryhash BINARY NOT NULL,
        asset BINARY NOT NULL,
        amount DECIMAL(75, 0) NOT NULL,
        source_ BINARY NOT NULL,
        amountinusd DECIMAL(75, 0) NOT NULL,
        PRIMARY KEY (block_number, transaction_hash, event_index)
    );

CREATE TABLE
    IF NOT EXISTS SXT_SYSTEM_ZKPAY.OWNERSHIPTRANSFERRED (
        block_number BIGINT NOT NULL,
        transaction_hash BINARY NOT NULL,
        event_index INTEGER NOT NULL,
        time_stamp TIMESTAMP NOT NULL,
        contract_address BINARY NOT NULL,
        previousowner BINARY NOT NULL,
        newowner BINARY NOT NULL,
        PRIMARY KEY (block_number, transaction_hash, event_index)
    );

CREATE TABLE
    IF NOT EXISTS SXT_SYSTEM_ZKPAY.PAYMENTREFUNDED (
        block_number BIGINT NOT NULL,
        transaction_hash BINARY NOT NULL,
        event_index INTEGER NOT NULL,
        time_stamp TIMESTAMP NOT NULL,
        contract_address BINARY NOT NULL,
        queryhash BINARY NOT NULL,
        asset BINARY NOT NULL,
        source_ BINARY NOT NULL,
        amount DECIMAL(75, 0) NOT NULL,
        PRIMARY KEY (block_number, transaction_hash, event_index)
    );

CREATE TABLE
    IF NOT EXISTS SXT_SYSTEM_ZKPAY.PAYMENTSETTLED (
        block_number BIGINT NOT NULL,
        transaction_hash BINARY NOT NULL,
        event_index INTEGER NOT NULL,
        time_stamp TIMESTAMP NOT NULL,
        contract_address BINARY NOT NULL,
        queryhash BINARY NOT NULL,
        usedamount DECIMAL(75, 0) NOT NULL,
        remainingamount DECIMAL(75, 0) NOT NULL,
        PRIMARY KEY (block_number, transaction_hash, event_index)
    );

CREATE TABLE
    IF NOT EXISTS SXT_SYSTEM_ZKPAY.QUERYCANCELED (
        block_number BIGINT NOT NULL,
        transaction_hash BINARY NOT NULL,
        event_index INTEGER NOT NULL,
        time_stamp TIMESTAMP NOT NULL,
        contract_address BINARY NOT NULL,
        queryhash BINARY NOT NULL,
        caller BINARY NOT NULL,
        PRIMARY KEY (block_number, transaction_hash, event_index)
    );

CREATE TABLE
    IF NOT EXISTS SXT_SYSTEM_ZKPAY.QUERYFULFILLED (
        block_number BIGINT NOT NULL,
        transaction_hash BINARY NOT NULL,
        event_index INTEGER NOT NULL,
        time_stamp TIMESTAMP NOT NULL,
        contract_address BINARY NOT NULL,
        queryhash BINARY NOT NULL,
        PRIMARY KEY (block_number, transaction_hash, event_index)
    );

CREATE TABLE
    IF NOT EXISTS SXT_SYSTEM_ZKPAY.QUERYRECEIVED (
        block_number BIGINT NOT NULL,
        transaction_hash BINARY NOT NULL,
        event_index INTEGER NOT NULL,
        time_stamp TIMESTAMP NOT NULL,
        contract_address BINARY NOT NULL,
        querynonce DECIMAL(75, 0) NOT NULL,
        sender BINARY NOT NULL,
        query BINARY NOT NULL,
        queryparameters BINARY NOT NULL,
        timeout DECIMAL(20, 0) NOT NULL,
        callbackclientcontractaddress BINARY NOT NULL,
        callbackgaslimit DECIMAL(20, 0) NOT NULL,
        callbackdata BINARY NOT NULL,
        customlogiccontractaddress BINARY NOT NULL,
        queryhash BINARY NOT NULL,
        PRIMARY KEY (block_number, transaction_hash, event_index)
    );

CREATE TABLE
    IF NOT EXISTS SXT_SYSTEM_ZKPAY.SENDPAYMENT (
        block_number BIGINT NOT NULL,
        transaction_hash BINARY NOT NULL,
        event_index INTEGER NOT NULL,
        time_stamp TIMESTAMP NOT NULL,
        contract_address BINARY NOT NULL,
        asset BINARY NOT NULL,
        amount DECIMAL(75, 0) NOT NULL,
        onbehalfof BINARY NOT NULL,
        target BINARY NOT NULL,
        memo BINARY NOT NULL,
        amountinusd DECIMAL(75, 0) NOT NULL,
        sender BINARY NOT NULL,
        PRIMARY KEY (block_number, transaction_hash, event_index)
    );

CREATE TABLE
    IF NOT EXISTS SXT_SYSTEM_ZKPAY.TREASURYSET (
        block_number BIGINT NOT NULL,
        transaction_hash BINARY NOT NULL,
        event_index INTEGER NOT NULL,
        time_stamp TIMESTAMP NOT NULL,
        contract_address BINARY NOT NULL,
        treasury BINARY NOT NULL,
        PRIMARY KEY (block_number, transaction_hash, event_index)
    );