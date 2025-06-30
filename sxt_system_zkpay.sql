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
    ) WITH (TABLE_UUID=CDBE504DDE8F3C787D0E5DF5FD8802D5);
CREATE TABLE
    IF NOT EXISTS SXT_SYSTEM_ZKPAY.ASSETREMOVED (
        block_number BIGINT NOT NULL,
        transaction_hash BINARY NOT NULL,
        event_index INTEGER NOT NULL,
        time_stamp TIMESTAMP NOT NULL,
        contract_address BINARY NOT NULL,
        asset BINARY NOT NULL,
        PRIMARY KEY (block_number, transaction_hash, event_index)
    ) WITH (TABLE_UUID=DC44271A49BD141245A8A6E186A8D358);
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
    ) WITH (TABLE_UUID=D4D17A155C4600C2E1760C08D742DF2D);
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
    ) WITH (TABLE_UUID=FDADB7CB5F12D32B3D5C941B5AFF10C9);
CREATE TABLE
    IF NOT EXISTS SXT_SYSTEM_ZKPAY.INITIALIZED (
        block_number BIGINT NOT NULL,
        transaction_hash BINARY NOT NULL,
        event_index INTEGER NOT NULL,
        time_stamp TIMESTAMP NOT NULL,
        contract_address BINARY NOT NULL,
        version DECIMAL(20, 0) NOT NULL,
        PRIMARY KEY (block_number, transaction_hash, event_index)
    ) WITH (TABLE_UUID=D36D1E28B48F0669BE6AA7F43BE96DEF);
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
    ) WITH (TABLE_UUID=DDFDF574417B19C39A5E79DF67BCC33A);
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
    ) WITH (TABLE_UUID=D03C0A7C3CDF160A926573CC8D0CBB3A);
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
    ) WITH (TABLE_UUID=E24B3861E3F3379BFE2CB1FDF2C38272);
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
    ) WITH (TABLE_UUID=E536F481502577D1358C24F5E3A4569A);
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
    ) WITH (TABLE_UUID=CFE289807F899C36285C934421D2D219);
CREATE TABLE
    IF NOT EXISTS SXT_SYSTEM_ZKPAY.QUERYFULFILLED (
        block_number BIGINT NOT NULL,
        transaction_hash BINARY NOT NULL,
        event_index INTEGER NOT NULL,
        time_stamp TIMESTAMP NOT NULL,
        contract_address BINARY NOT NULL,
        queryhash BINARY NOT NULL,
        PRIMARY KEY (block_number, transaction_hash, event_index)
    ) WITH (TABLE_UUID=D387D674799F0F15F0AD92016F17AFE9);
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
    ) WITH (TABLE_UUID=DEFE17CFF98747682F405319E0EE3214);
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
    ) WITH (TABLE_UUID=EC0E8790F30ECD01A3CC12FC2734C3A2);
CREATE TABLE
    IF NOT EXISTS SXT_SYSTEM_ZKPAY.TREASURYSET (
        block_number BIGINT NOT NULL,
        transaction_hash BINARY NOT NULL,
        event_index INTEGER NOT NULL,
        time_stamp TIMESTAMP NOT NULL,
        contract_address BINARY NOT NULL,
        treasury BINARY NOT NULL,
        PRIMARY KEY (block_number, transaction_hash, event_index)
    ) WITH (TABLE_UUID=D3DD2C1471A46EFB427CA13680302C08);
