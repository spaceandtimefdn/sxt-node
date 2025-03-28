create schema if not exists ETHEREUM;
create table if not exists ETHEREUM.BLOCKS (
    time_stamp timestamp not null,
    block_number bigint not null,
    block_hash bytea not null,
    gas_limit decimal(75, 0) not null,
    gas_used decimal(75, 0) not null,
    miner bytea not null,
    parent_hash bytea not null,
    reward decimal(75, 0) not null,
    size bigint not null,
    transaction_count int not null,
    nonce bytea not null,
    receipts_root bytea not null,
    sha3_uncles bytea not null,
    state_root bytea not null,
    transactions_root bytea not null,
    uncles_count bigint not null,
    primary key (block_number)
);
create table if not exists ETHEREUM.TRANSACTIONS (
    time_stamp timestamp not null,
    block_number bigint not null,
    transaction_hash bytea not null,
    transaction_index int not null,
    transaction_fee decimal(75, 0) not null,
    from_address bytea not null,
    to_address bytea not null,
    value_ decimal(75, 0) not null,
    gas bigint not null,
    receipt_cumulative_gas_used bigint not null,
    receipt_status boolean not null,
    primary key (block_number, transaction_hash)
);
create table if not exists ETHEREUM.TRANSACTION_DETAILS (
    time_stamp timestamp not null,
    block_number bigint not null,
    transaction_hash bytea not null,
    transaction_index int not null,
    method_id bytea not null,
    receipt_contract_address bytea not null,
    type_ int not null,
    gas_price decimal(75, 0) not null,
    nonce int not null,
    receipt_gas_used int not null,
    max_fee_per_gas decimal(75, 0) not null,
    max_priority_fee_per_gas decimal(75, 0) not null,
    receipt_effective_gas_price decimal(75, 0) not null,
    logs_count int not null,
    primary key (block_number, transaction_hash)
);
create table if not exists ETHEREUM.NATIVE_WALLETS (
    time_stamp timestamp not null,
    block_number bigint not null,
    wallet_address bytea not null,
    balance decimal(75, 0) not null,
    primary key (
                    block_number,
                    wallet_address,
                    balance
                ),
);
create table if not exists ETHEREUM.NATIVE_TOKEN_TRANSFERS (
    time_stamp timestamp not null,
    block_number bigint not null,
    transaction_hash bytea not null,
    transaction_index int not null,
    from_ bytea not null,
    to_ bytea not null,
    value_ decimal(75, 0) not null,
    primary key (block_number, transaction_hash),
);

create table if not exists ETHEREUM.CONTRACTS (
    time_stamp timestamp not null,
    block_number bigint not null,
    transaction_hash bytea not null,
    transaction_index int not null,
    contract_address bytea not null,
    contract_creator_address bytea not null,
    primary key (contract_address, transaction_hash),
);

create table if not exists ETHEREUM.TOKEN_ERC20_CONTRACTS (
    time_stamp timestamp not null,
    block_number bigint not null,
    transaction_hash bytea not null,
    transaction_index int not null,
    contract_address bytea not null,
    name varchar not null,
    symbol varchar not null,
    decimals int not null,
    primary key (transaction_hash, contract_address),
);

create table if not exists ETHEREUM.TOKEN_ERC20_TRANSFERS (
    time_stamp timestamp not null,
    block_number bigint not null,
    transaction_hash bytea not null,
    transaction_index int not null,
    event_index int not null,
    contract_address bytea not null,
    from_ bytea not null,
    to_ bytea not null,
    value_ decimal(75, 0) not null,
    primary key (block_number, transaction_hash, event_index),
);

create table if not exists ETHEREUM.NFT_ERC721_CONTRACTS (
    time_stamp timestamp not null,
    block_number bigint not null,
    transaction_hash bytea not null,
    transaction_index int not null,
    contract_address bytea not null,
    name varchar not null,
    symbol varchar not null,
    primary key (transaction_hash, contract_address),
);

create table if not exists ETHEREUM.NFT_ERC1155_CONTRACTS (
    time_stamp timestamp not null,
    block_number bigint not null,
    transaction_hash bytea not null,
    transaction_index int not null,
    contract_address bytea not null,
    primary key (transaction_hash, contract_address),
);

create table if not exists ETHEREUM.NFT_ERC1155_TRANSFER (
    time_stamp timestamp not null,
    block_number bigint not null,
    transaction_hash bytea not null,
    transaction_index int not null,
    event_index int not null,
    contract_address bytea not null,
    operator bytea not null,
    from_ bytea not null,
    to_ bytea not null,
    id bytea not null,
    value_ decimal(75, 0) not null,
    primary key (block_number, transaction_hash, event_index),
);

create table if not exists ETHEREUM.NFT_ERC721_APPROVAL (
    time_stamp timestamp not null,
    block_number bigint not null,
    transaction_hash bytea not null,
    transaction_index int not null,
    event_index int not null,
    contract_address bytea not null,
    owner bytea not null,
    approved bytea not null,
    token_id bytea not null,
    primary key (block_number, transaction_hash, event_index),
);

create table if not exists ETHEREUM.NFT_ERC721_APPROVAL_FOR_ALL (
    time_stamp timestamp not null,
    block_number bigint not null,
    transaction_hash bytea not null,
    transaction_index int not null,
    event_index int not null,
    contract_address bytea not null,
    owner bytea not null,
    operator bytea not null,
    approved boolean not null,
    primary key (block_number, transaction_hash, event_index),
);

create table if not exists ETHEREUM.NFT_ERC721_TRANSFER (
    time_stamp timestamp not null,
    block_number bigint not null,
    transaction_hash bytea not null,
    transaction_index int not null,
    event_index int not null,
    contract_address bytea not null,
    from_ bytea not null,
    to_ bytea not null,
    token_id bytea not null,
    primary key (block_number, transaction_hash, event_index),
);

create table if not exists ETHEREUM.ERC173_OWNERSHIP_TRANSFERRED (
    time_stamp timestamp not null,
    block_number bigint not null,
    transaction_hash bytea not null,
    transaction_index int not null,
    event_index int not null,
    contract_address bytea not null,
    previous_owner bytea not null,
    new_owner bytea not null,
    primary key (block_number, transaction_hash, event_index),
);

create table if not exists ETHEREUM.PROXY_ERC1967_UPGRADES (
    time_stamp timestamp not null,
    block_number bigint not null,
    transaction_hash bytea not null,
    transaction_index int not null,
    event_index int not null,
    proxy_contract bytea not null,
    implementation_contract bytea not null,
    primary key (transaction_hash, event_index),
);

create table if not exists ETHEREUM.PROXY_ERC1967_ADMIN_CHANGES (
    time_stamp timestamp not null,
    block_number bigint not null,
    transaction_hash bytea not null,
    transaction_index int not null,
    event_index int not null,
    proxy_contract bytea not null,
    previous_admin bytea not null,
    new_admin bytea not null,
    primary key (transaction_hash, event_index),
);

create table if not exists ETHEREUM.PROXY_NON_ERC_UPGRADES (
    time_stamp timestamp not null,
    block_number bigint not null,
    transaction_hash bytea not null,
    transaction_index int not null,
    event_index int not null,
    proxy_contract bytea not null,
    implementation_contract bytea not null,
    primary key (transaction_hash, event_index),
);

create table if not exists ETHEREUM.TOKEN_ERC20_WALLET_BALANCES (
    time_stamp timestamp not null,
    block_number bigint not null,
    wallet_address bytea not null,
    token_address bytea not null,
    balance decimal(75, 0) not null,
    primary key (
        block_number,
        wallet_address,
        token_address,
        balance
    ),
);

create table if not exists ETHEREUM.TOKEN_ERC20_APPROVAL (
    time_stamp timestamp not null,
    block_number bigint not null,
    transaction_hash bytea not null,
    transaction_index int not null,
    event_index int not null,
    contract_address bytea not null,
    owner bytea not null,
    spender bytea not null,
    value_ decimal(75, 0) not null,
    primary key (block_number, transaction_hash, event_index),
);

create table if not exists ETHEREUM.NFT_ERC721_OWNERS (
    time_stamp timestamp not null,
    block_number bigint not null,
    contract_address bytea not null,
    token_id bytea not null,
    owner bytea not null,
    balance decimal(75, 0) not null,
    primary key (
        block_number,
        contract_address,
        token_id,
        owner,
        balance
    ),
);

create table if not exists ETHEREUM.NFT_ERC1155_OWNERS (
    time_stamp timestamp not null,
    block_number bigint not null,
    contract_address bytea not null,
    owner bytea not null,
    token_id bytea not null,
    balance decimal(75, 0) not null,
    primary key (block_number, contract_address, owner, token_id),
);

create table if not exists ETHEREUM.LOGS (
    time_stamp timestamp not null,
    block_number bigint not null,
    transaction_hash bytea not null,
    transaction_index int not null,
    event_index int not null,
    contract_address bytea not null,
    topic_0 bytea not null,
    topic_1 bytea not null,
    topic_2 bytea not null,
    topic_3 bytea not null,
    status boolean not null,
    raw_data varchar not null,
    primary key (block_number, transaction_hash, event_index),
);

create table if not exists ETHEREUM.STORAGE_SLOT_UPDATES (
    block_number bigint not null,
    time_stamp timestamp not null,
    transaction_hash varchar not null,
    transaction_index int not null,
    contract_address varchar not null,
    slot_position varchar not null,
    slot_value varchar not null,
    primary key (block_number, contract_address),
);

create table if not exists ETHEREUM.SXT_VALUE_OVERFLOW (
    time_stamp timestamp not null,
    block_number bigint not null,
    transaction_hash bytea not null,
    transaction_index int not null,
    schema varchar not null,
    table_name varchar not null,
    column_name varchar not null,
    original_value varchar not null,
    primary key (
        block_number,
        transaction_hash,
        transaction_index,
        schema,
        table_name,
        column_name,
        original_value
    ),
);