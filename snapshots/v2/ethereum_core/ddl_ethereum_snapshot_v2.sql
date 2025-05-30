CREATE TABLE IF NOT EXISTS ETHEREUM.BLOCKS(
  BLOCK_NUMBER BIGINT NOT NULL,
  TIME_STAMP TIMESTAMP,
  BLOCK_HASH VARCHAR,
  MINER VARCHAR,
  REWARD DECIMAL(78, 0),
  SIZE_ INT,
  GAS_USED INT,
  GAS_LIMIT INT,
  BASE_FEE_PER_GAS DECIMAL(78, 0),
  TRANSACTION_COUNT INT,
  PARENT_HASH VARCHAR,
  PRIMARY KEY(BLOCK_NUMBER)
);

CREATE TABLE IF NOT EXISTS ETHEREUM.BLOCK_DETAILS(
  BLOCK_NUMBER BIGINT NOT NULL,
  TIME_STAMP TIMESTAMP,
  SHA3_UNCLES VARCHAR,
  STATE_ROOT VARCHAR,
  TRANSACTIONS_ROOT VARCHAR,
  RECEIPTS_ROOT VARCHAR,
  UNCLES_COUNT INT,
  VERSION VARCHAR,
  LOGS_BLOOM VARCHAR,
  NONCE VARCHAR,
  PRIMARY KEY(BLOCK_NUMBER)
);

CREATE TABLE IF NOT EXISTS ETHEREUM.TRANSACTIONS(
  BLOCK_NUMBER BIGINT NOT NULL,
  TIME_STAMP TIMESTAMP,
  TRANSACTION_HASH VARCHAR NOT NULL,
  TRANSACTION_FEE DECIMAL(78, 0),
  FROM_ADDRESS VARCHAR,
  TO_ADDRESS VARCHAR,
  VALUE_ DECIMAL(78, 0),
  GAS DECIMAL(78, 0),
  RECEIPT_CUMULATIVE_GAS_USED INT,
  RECEIPT_STATUS INT,
  PRIMARY KEY(BLOCK_NUMBER, TRANSACTION_HASH)
);

CREATE TABLE IF NOT EXISTS ETHEREUM.TRANSACTION_DETAILS(
  BLOCK_NUMBER BIGINT NOT NULL,
  TIME_STAMP TIMESTAMP NOT NULL,
  TRANSACTION_HASH VARCHAR NOT NULL,
  CHAIN_ID VARCHAR,
  FUNCTION_NAME VARCHAR,
  METHOD_ID VARCHAR,
  TRANSACTION_INDEX INT,
  RECEIPT_CONTRACT_ADDRESS VARCHAR,
  TYPE_ VARCHAR,
  GAS_PRICE DECIMAL(78, 0),
  NONCE INT,
  RECEIPT_GAS_USED INT,
  MAX_FEE_PER_GAS DECIMAL(78, 0),
  MAX_PRIORITY_FEE_PER_GAS DECIMAL(78, 0),
  RECEIPT_EFFECTIVE_GAS_PRICE DECIMAL(78, 0),
  LOGS_COUNT INT,
  PRIMARY KEY(BLOCK_NUMBER, TRANSACTION_HASH)
);

CREATE TABLE IF NOT EXISTS ETHEREUM.CONTRACTS(
  BLOCK_NUMBER BIGINT,
  TIME_STAMP TIMESTAMP,
  CONTRACT_CREATOR_ADDRESS VARCHAR,
  PROXY_CONTRACT_IMPL_ADDRESS VARCHAR,
  CONTRACT_ADDRESS VARCHAR NOT NULL,
  TRANSACTION_HASH VARCHAR,
  PRIMARY KEY(BLOCK_NUMBER, CONTRACT_ADDRESS)
);

CREATE TABLE IF NOT EXISTS ETHEREUM.TOKENS(
  CONTRACT_ADDRESS VARCHAR NOT NULL,
  NAME VARCHAR,
  DECIMALS DECIMAL(78, 0) NOT NULL,
  SYMBOL VARCHAR,
  BLOCK_NUMBER BIGINT NOT NULL,
  TIME_STAMP TIMESTAMP NOT NULL,
  PRIMARY KEY(BLOCK_NUMBER, CONTRACT_ADDRESS)
);

CREATE TABLE IF NOT EXISTS ETHEREUM.NFT_COLLECTIONS(
  CONTRACT_ADDRESS VARCHAR NOT NULL,
  NAME VARCHAR,
  TOKEN_STANDARD VARCHAR,
  SYMBOL VARCHAR,
  BLOCK_NUMBER BIGINT NOT NULL,
  TIME_STAMP TIMESTAMP NOT NULL,
  PRIMARY KEY(BLOCK_NUMBER, CONTRACT_ADDRESS)
);

CREATE TABLE IF NOT EXISTS ETHEREUM.NFTS(
  CONTRACT_ADDRESS VARCHAR NOT NULL,
  TOKEN_ID VARCHAR NOT NULL,
  TIME_STAMP TIMESTAMP NOT NULL,
  TOKEN_URI VARCHAR,
  BLOCK_NUMBER BIGINT NOT NULL,
  PRIMARY KEY(BLOCK_NUMBER, CONTRACT_ADDRESS, TOKEN_ID)
);

CREATE TABLE IF NOT EXISTS ETHEREUM.NATIVETOKEN_TRANSFERS(
  TRANSACTION_HASH VARCHAR NOT NULL,
  BLOCK_NUMBER BIGINT NOT NULL,
  TIME_STAMP TIMESTAMP,
  FROM_ VARCHAR,
  TO_ VARCHAR,
  VALUE_ DECIMAL(78, 0),
  PRIMARY KEY(BLOCK_NUMBER, TRANSACTION_HASH)
);

CREATE TABLE IF NOT EXISTS ETHEREUM.ERC20_EVT_TRANSFER(
  TRANSACTION_HASH VARCHAR NOT NULL,
  EVENT_INDEX INT NOT NULL,
  BLOCK_NUMBER BIGINT,
  TIME_STAMP TIMESTAMP,
  FROM_ VARCHAR,
  TO_ VARCHAR,
  VALUE_ DECIMAL(78, 0),
  CONTRACT_ADDRESS VARCHAR,
  PRIMARY KEY(BLOCK_NUMBER, TRANSACTION_HASH, EVENT_INDEX)
);

CREATE TABLE IF NOT EXISTS ETHEREUM.ERC20_EVT_APPROVAL(
  BLOCK_NUMBER BIGINT NOT NULL,
  TIME_STAMP TIMESTAMP,
  TRANSACTION_HASH VARCHAR NOT NULL,
  EVENT_INDEX INT NOT NULL,
  OWNER VARCHAR,
  SPENDER VARCHAR,
  VALUE_ DECIMAL(78, 0),
  CONTRACT_ADDRESS VARCHAR,
  PRIMARY KEY(BLOCK_NUMBER, TRANSACTION_HASH, EVENT_INDEX)
);

CREATE TABLE IF NOT EXISTS ETHEREUM.ERC721_EVT_TRANSFER(
  TRANSACTION_HASH VARCHAR NOT NULL,
  EVENT_INDEX INT NOT NULL,
  TOKEN_ID VARCHAR NOT NULL,
  BLOCK_NUMBER BIGINT,
  TIME_STAMP TIMESTAMP,
  FROM_ VARCHAR,
  TO_ VARCHAR,
  CONTRACT_ADDRESS VARCHAR,
  PRIMARY KEY(BLOCK_NUMBER, TRANSACTION_HASH, EVENT_INDEX)
);

CREATE TABLE IF NOT EXISTS ETHEREUM.ERC721_EVT_APPROVAL(
  BLOCK_NUMBER BIGINT NOT NULL,
  TIME_STAMP TIMESTAMP,
  TRANSACTION_HASH VARCHAR NOT NULL,
  EVENT_INDEX INT NOT NULL,
  TOKEN_ID VARCHAR NOT NULL,
  OWNER VARCHAR,
  APPROVED VARCHAR,
  CONTRACT_ADDRESS VARCHAR,
  PRIMARY KEY(BLOCK_NUMBER, TRANSACTION_HASH, EVENT_INDEX)
);

CREATE TABLE IF NOT EXISTS ETHEREUM.ERC1155_EVT_TRANSFER(
  TRANSACTION_HASH VARCHAR NOT NULL,
  EVENT_INDEX INT NOT NULL,
  OPERATOR VARCHAR,
  BLOCK_NUMBER BIGINT,
  TIME_STAMP TIMESTAMP,
  FROM_ VARCHAR,
  TO_ VARCHAR,
  CONTRACT_ADDRESS VARCHAR,
  VALUE_ DECIMAL(78, 0),
  ID VARCHAR,
  PRIMARY KEY(BLOCK_NUMBER, TRANSACTION_HASH, EVENT_INDEX)
);

CREATE TABLE IF NOT EXISTS ETHEREUM.ERC1155_EVT_TRANSFERBATCH(
  TRANSACTION_HASH VARCHAR NOT NULL,
  EVENT_INDEX INT NOT NULL,
  OPERATOR VARCHAR,
  BLOCK_NUMBER BIGINT,
  TIME_STAMP TIMESTAMP,
  FROM_ VARCHAR,
  TO_ VARCHAR,
  CONTRACT_ADDRESS VARCHAR,
  VALUES_ VARCHAR,
  IDS VARCHAR,
  PRIMARY KEY(BLOCK_NUMBER, TRANSACTION_HASH, EVENT_INDEX)
);

CREATE TABLE IF NOT EXISTS ETHEREUM.CONTRACT_EVT_APPROVALFORALL(
  BLOCK_NUMBER BIGINT NOT NULL,
  TIME_STAMP TIMESTAMP,
  TRANSACTION_HASH VARCHAR NOT NULL,
  EVENT_INDEX INT NOT NULL,
  OPERATOR VARCHAR,
  ACCOUNT VARCHAR,
  APPROVED BOOLEAN,
  CONTRACT_ADDRESS VARCHAR,
  PRIMARY KEY(BLOCK_NUMBER, TRANSACTION_HASH, EVENT_INDEX)
);

CREATE TABLE IF NOT EXISTS ETHEREUM.CONTRACT_EVT_OWNERSHIPTRANSFERRED(
  BLOCK_NUMBER BIGINT NOT NULL,
  TIME_STAMP TIMESTAMP,
  TRANSACTION_HASH VARCHAR NOT NULL,
  EVENT_INDEX INT NOT NULL,
  PREVIOUSOWNER VARCHAR,
  NEWOWNER VARCHAR,
  CONTRACT_ADDRESS VARCHAR,
  PRIMARY KEY(BLOCK_NUMBER, TRANSACTION_HASH, EVENT_INDEX)
);

CREATE TABLE IF NOT EXISTS ETHEREUM.NATIVE_WALLETS(
  WALLET_ADDRESS VARCHAR NOT NULL,
  BLOCK_NUMBER BIGINT NOT NULL,
  BALANCE DECIMAL(78, 0),
  TIME_STAMP TIMESTAMP,
  PRIMARY KEY(WALLET_ADDRESS, BLOCK_NUMBER)
);

CREATE TABLE IF NOT EXISTS ETHEREUM.FUNGIBLETOKEN_WALLETS(
  WALLET_ADDRESS VARCHAR NOT NULL,
  TOKEN_ADDRESS VARCHAR NOT NULL,
  BLOCK_NUMBER BIGINT NOT NULL,
  BALANCE DECIMAL(78, 0),
  TIME_STAMP TIMESTAMP,
  PRIMARY KEY(WALLET_ADDRESS, TOKEN_ADDRESS, BLOCK_NUMBER)
);

CREATE TABLE IF NOT EXISTS ETHEREUM.ERC721_OWNERS(
  BLOCK_NUMBER BIGINT NOT NULL,
  TIME_STAMP TIMESTAMP,
  CONTRACT_ADDRESS VARCHAR NOT NULL,
  TOKEN_ID VARCHAR NOT NULL,
  OWNER VARCHAR,
  BALANCE DECIMAL(78, 0),
  PRIMARY KEY(BLOCK_NUMBER, CONTRACT_ADDRESS, TOKEN_ID)
);

CREATE TABLE IF NOT EXISTS ETHEREUM.ERC1155_OWNERS(
  BLOCK_NUMBER BIGINT NOT NULL,
  TIME_STAMP TIMESTAMP,
  CONTRACT_ADDRESS VARCHAR NOT NULL,
  TOKEN_ID VARCHAR NOT NULL,
  OWNER VARCHAR,
  BALANCE DECIMAL(78, 0),
  PRIMARY KEY(BLOCK_NUMBER, CONTRACT_ADDRESS, TOKEN_ID, OWNER)
);
