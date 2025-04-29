CREATE SCHEMA IF NOT EXISTS SXT_META;

CREATE TABLE IF NOT EXISTS SXT_META.SCHEMAS
(
    -- The unique identifier
        ID UUID
            NOT NULL
            DEFAULT gen_random_uuid()
            PRIMARY KEY,
        -- The database/catalog name
        CATALOG_NAME VARCHAR(64)
            NULL,
        -- The schema name
        SCHEMA_NAME VARCHAR(64)
            NOT NULL,
        -- The subscription identifier
        SUBSCRIPTION_ID UUID
            NULL,
        -- The timestamp when the row was inserted
        CREATED TIMESTAMP
            NOT NULL
            DEFAULT NOW(),

        -- Ensure catalog + schema combo is unique
        UNIQUE(CATALOG_NAME, SCHEMA_NAME)
);

CREATE INDEX  IF NOT EXISTS IDX_SCHEMA_NAME ON SXT_META.SCHEMAS(SCHEMA_NAME);

CREATE TABLE IF NOT EXISTS SXT_META.TABLES
(
   -- The unique identifier
   ID UUID
       NOT NULL
       DEFAULT gen_random_uuid()
       PRIMARY KEY,

   -- The SCHEMA table foreign key
   SCHEMA_ID UUID
       NOT NULL,

   -- The database/catalog name
   CATALOG_NAME VARCHAR(64)
       NULL,

   -- The schema name
   SCHEMA_NAME VARCHAR(64)
       NOT NULL,

   -- The table name
   TABLE_NAME VARCHAR(64)
       NOT NULL,

   -- The resource identifier (i.e., concatenation of CATALOG_NAME, SCHEMA_NAME, TABLE_NAME)
   RESOURCEID VARCHAR(192)
       NOT NULL
       UNIQUE,

   -- The table type - defines whether this is actually a table or some type of view
   TABLE_TYPE VARCHAR(10)
       NOT NULL
       CHECK (TABLE_TYPE IN ('TABLE', 'VIEW', 'MAT_VIEW', 'PAR_VIEW')),

   -- The HEX encoded public key (will always be 64 chars for hex encoded ed25519)
   PUBLIC_KEY VARCHAR(64)
       NOT NULL,

   -- The access type for the table - defines the authorization policy
   ACCESS_TYPE VARCHAR(15)
       NOT NULL
       CHECK (ACCESS_TYPE IN ('PERMISSIONED', 'PUBLIC_READ', 'PUBLIC_APPEND', 'PUBLIC_WRITE')),

   -- For `TABLE_TYPE == MAT_VIEW` only, the optional refresh interval
   REFRESH_INTERVAL INTEGER
       NULL
       CHECK (REFRESH_INTERVAL > 0),

   -- Flag: defines whether or not updates/deletes are allowed on the table
   IS_IMMUTABLE BOOLEAN
       NOT NULL
       DEFAULT FALSE,

   -- Flag: defines whether or not the table can be queried with Proof of SQL
   IS_TAMPERPROOF BOOLEAN
       NOT NULL
       DEFAULT FALSE,

   -- Flag: defines whether or not the table exists external to the OLTP DB
   IS_EXTERNAL BOOLEAN
       NOT NULL
       DEFAULT FALSE,

   -- Flag: defines whether or not the table has encrypted data
   IS_ENCRYPTED BOOLEAN
       NOT NULL
       DEFAULT FALSE,

   -- If `IS_ENCRYPTED == TRUE`, defines the encryption dataset identifier
   ENC_DATASET_ID VARCHAR(255)
       NULL,

   -- Stores additional table properties as serialized-JSON data
   PROPERTIES JSONB
       NULL,

   -- Stores the raw SQL text used to create the table
   CREATE_SQL TEXT
       NOT NULL,

   -- The subscription identifier
   SUBSCRIPTION_ID UUID
       NULL,

   -- The timestamp when the row was inserted
   CREATED TIMESTAMP
       NOT NULL
       DEFAULT NOW(),

   -- For views, the underlying view query text
   VIEW_QUERY TEXT
       NULL,

   -- For views, the list of resources referenced in the query text
   VIEW_RESOURCE_LIST TEXT
       NULL,

   -- For `TABLE_TYPE == PAR_VIEW` only, the list of parameters
   VIEW_PARAM_LIST TEXT
       NULL,

   -- Ensure catalog + schema + table combo is unique
   UNIQUE (CATALOG_NAME, SCHEMA_NAME, TABLE_NAME),

   -- SXT_META.SCHEMAS foreign key constraints
   CONSTRAINT FK_SCHEMA_ID FOREIGN KEY (SCHEMA_ID) REFERENCES SXT_META.SCHEMAS(ID),
   CONSTRAINT FK_SCHEMA_CATALOG_SCHEMA FOREIGN KEY (CATALOG_NAME, SCHEMA_NAME) REFERENCES SXT_META.SCHEMAS(CATALOG_NAME, SCHEMA_NAME)
);


CREATE INDEX  IF NOT EXISTS IDX_TABLE_NAME ON SXT_META.TABLES(TABLE_NAME);
CREATE INDEX  IF NOT EXISTS IDX_TABLE_RESOURCEID ON SXT_META.TABLES(RESOURCEID);

-- Stores info about previously deleted tables
CREATE TABLE  IF NOT EXISTS SXT_META.DELETED_TABLES
(
    CATALOG_NAME VARCHAR(64),
    SCHEMA_NAME VARCHAR(64),
    TABLE_NAME VARCHAR(64),
    CREATE_SQL VARCHAR,
    SUBSCRIPTION_ID UUID,
    CREATED TIMESTAMP,
    DELETED TIMESTAMP
        NOT NULL
        DEFAULT NOW()
);

-- Table statistics
CREATE TABLE  IF NOT EXISTS SXT_META.TABLE_STATS
(
    -- The table identifier
    TABLE_ID UUID
        NOT NULL
        PRIMARY KEY,
    -- The table row count
    ROW_COUNT BIGINT
        NOT NULL
        DEFAULT 0,
    -- The table storage size (in bytes)
    SIZE_BYTES BIGINT
        NOT NULL
        DEFAULT 0,
    -- Reserved for additional stat information
    ADDITIONAL_STATS VARCHAR
        NULL,
    -- Defines when the table stats were last updated
    LAST_UPDATED TIMESTAMP
        NOT NULL
        DEFAULT NOW(),

    -- SXT_META.TABLES fk
    CONSTRAINT FK_TABLE_ID FOREIGN KEY (TABLE_ID) REFERENCES SXT_META.TABLES(ID) ON DELETE CASCADE
);

-- Table constraints
CREATE TABLE  IF NOT EXISTS SXT_META.TABLE_CONSTRAINTS
(
    -- The unique identifier
    ID UUID
        NOT NULL
        DEFAULT gen_random_uuid()
        PRIMARY KEY,
    -- The table identifier
    TABLE_ID UUID
        NOT NULL,
    -- The constraint name
    CONSTRAINT_NAME VARCHAR(64)
        NULL,
    -- The constraint type
    CONSTRAINT_TYPE VARCHAR(12)
        NOT NULL
        CHECK(CONSTRAINT_TYPE IN ('CHECK', 'FOREIGN KEY', 'PRIMARY KEY', 'UNIQUE')),
    -- Flag: whether or not the constraint check can be deferred. Today we require this to be false
    IS_DEFERRABLE BOOLEAN
        NOT NULL
        DEFAULT FALSE
        CHECK(IS_DEFERRABLE = FALSE),
    -- Flag: whether or not the constraint check is initially deferred. Today we require this to be false
    IS_INITIALLY_DEFERRED BOOLEAN
        NOT NULL
        DEFAULT FALSE
        CHECK(IS_INITIALLY_DEFERRED = FALSE),
    -- Flag: whether or not the constraint check is enforced
    IS_ENFORCED BOOLEAN
        NOT NULL
        DEFAULT TRUE,
    -- The timestamp when the row was inserted
    CREATED TIMESTAMP
        NOT NULL
        DEFAULT NOW(),

    -- SXT_META.TABLES fk
    CONSTRAINT FK_TABLE_ID FOREIGN KEY (TABLE_ID) REFERENCES SXT_META.TABLES(ID) ON DELETE CASCADE
);

-- Table foreign keys
CREATE TABLE IF NOT EXISTS SXT_META.TABLE_FOREIGN_KEYS
(
    -- The unique identifier
    ID UUID
        NOT NULL
        DEFAULT gen_random_uuid()
        PRIMARY KEY,
    -- The parent constraint identifier (and unique identifier)
    --CONSTRAINT_ID UUID
    --    NOT NULL
    --    PRIMARY KEY,
    -- The origin table ID
    ORIGIN_TABLE_ID UUID
        NOT NULL,
    -- The origin table resource identifier
    ORIGIN_RESOURCEID VARCHAR(192)
        NOT NULL,
    -- The origin columns - i.e., the table foreign key columns (as comma-separated list)
    ORIGIN_COLUMNS VARCHAR
        NOT NULL,
    -- The reference table ID
    REFERENCE_TABLE_ID UUID
        NOT NULL,
    -- The reference table resource identifier
    REFERENCE_RESOURCEID VARCHAR(192)
       NOT NULL,
    -- The reference columns - i.e., the columns in the reference table the origin columns are compared against
    REFERENCE_COLUMNS VARCHAR
        NOT NULL,
    -- The fkey match method
    MATCH_TYPE VARCHAR(8)
        NOT NULL
        DEFAULT 'SIMPLE'
        CHECK(MATCH_TYPE IN ('FULL', 'PARTIAL', 'SIMPLE')),
    -- The update rule
    UPDATE_RULE VARCHAR(255)
        NOT NULL
        DEFAULT 'NO ACTION',
    -- The delete rule
    DELETE_RULE VARCHAR(255)
        NOT NULL
        DEFAULT 'NO ACTION',
    -- Describes the foreign-key relationship
    CARDINALITY VARCHAR(12)
        NOT NULL
        CHECK(CARDINALITY IN ('ONE-TO-ONE', 'ONE-TO-MANY')),
    -- The subscription identifier
    SUBSCRIPTION_ID UUID
        NULL,
    -- The timestamp when the row was inserted
    CREATED TIMESTAMP
        NOT NULL
        DEFAULT NOW(),

    -- SXT_META.TABLE_CONSTRAINTS fk
    --CONSTRAINT FK_CONSTRAINT_ID FOREIGN KEY (CONSTRAINT_ID) REFERENCES SXT_META.TABLE_CONSTRAINTS(ID)
    -- SXT_META.TABLES fk
    CONSTRAINT FK_ORIGIN_TABLE_ID FOREIGN KEY (ORIGIN_TABLE_ID) REFERENCES SXT_META.TABLES(ID) ON DELETE CASCADE,
    CONSTRAINT FK_REFERENCE_TABLE_ID FOREIGN KEY (REFERENCE_TABLE_ID) REFERENCES SXT_META.TABLES(ID) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS SXT_META.COLUMNS
(
    -- The unique identifier
        ID UUID
            NOT NULL
            DEFAULT gen_random_uuid()
            PRIMARY KEY,
        -- The table identifier
        TABLE_ID UUID
            NOT NULL,
        -- The column name
        COLUMN_NAME VARCHAR(64)
            NOT NULL,
        -- The ordinal position (1 based)
        POSITION SMALLINT
            NOT NULL
            CHECK(POSITION >= 1),
        -- The data type
        DATA_TYPE VARCHAR(255)
            NOT NULL,
        -- Flag: whether or not the column can have nulls
        IS_NULLABLE BOOLEAN
            NOT NULL
            DEFAULT TRUE,
        -- The column default value (as string)
        DEFAULT_VALUE VARCHAR(255)
            NULL,
        -- For numeric data types, the precision
        NUMERIC_PRECISION BIGINT
            NULL,
        -- For numeric data types, the scale
        NUMERIC_SCALE BIGINT
            NULL,
        -- The column sequence in the primary key (-1 for non primary keys)
        PRIMARY_KEY_SEQ SMALLINT
            NOT NULL
            DEFAULT -1
            CHECK(PRIMARY_KEY_SEQ >= 1 OR PRIMARY_KEY_SEQ = -1),
        -- Flag: whether or not the column values are auto-generated
        IS_GENERATED BOOLEAN
            NOT NULL
            DEFAULT FALSE,
        -- Flag: whether or not the column values are auto-incremented
        IS_INCREMENT BOOLEAN
            NOT NULL
            DEFAULT FALSE,
        -- Flag: whether or not column stats are enabled
        IS_STAT_ENABLED BOOLEAN
            NOT NULL
            DEFAULT FALSE,
        -- Flag: defines whether or not the table has encrypted data
        IS_ENCRYPTED BOOLEAN
            NOT NULL
            DEFAULT FALSE,
        -- Defines the type of encryption for encrypted columns
        ENC_TYPE VARCHAR(255)
            NULL,
        -- Defines the additional encryption type configuration for encrypted columns
        ENC_OPTION VARCHAR(255)
            NULL,
        -- Defines the column response formatting
        FORMATTING VARCHAR(255)
            NULL,
        -- The subscription identifier
        SUBSCRIPTION_ID UUID
            NULL,
        -- The timestamp when the row was inserted
        CREATED TIMESTAMP
            NOT NULL
            DEFAULT NOW(),

        -- SXT_META.TABLES fk
        CONSTRAINT FK_TABLE_ID FOREIGN KEY (TABLE_ID) REFERENCES SXT_META.TABLES(ID) ON DELETE CASCADE
);

-- Column statistics
CREATE TABLE IF NOT EXISTS SXT_META.COLUMN_STATS
(
    -- The column reference (and unique identifier)
    COLUMN_ID UUID
        NOT NULL
        PRIMARY KEY,
    -- The column min value (not typed)
    MIN_VALUE VARCHAR(255)
        NULL,
    -- The column max value (not typed)
    MAX_VALUE VARCHAR(255)
        NULL,
    -- The total non-null value count
    VALUE_COUNT BIGINT
        NULL,
    -- The count of unique non-null values
    UNIQUE_COUNT BIGINT
        NULL,
    -- The count of null values
    NULL_VALUE_COUNT BIGINT
        NULL,
    -- For numeric types, the average value
    AVERAGE_VALUE FLOAT
        NULL,
    -- For numeric types, the standard deviation
    STANDARD_DEVIATION FLOAT
        NULL,
    -- The 25th percentile value
    PERCENTILE_25 FLOAT
        NULL,
    -- The 50th percentile value
    PERCENTILE_50 FLOAT
        NULL,
    -- The 75th percentile value
    PERCENTILE_75 FLOAT
        NULL,
    -- Contains any additional column stats
    ADDITIONAL_STATS VARCHAR(255)
        NULL,

    -- SXT_META.COLUMNS fk
    CONSTRAINT FK_COLUMN_ID FOREIGN KEY (COLUMN_ID) REFERENCES SXT_META.COLUMNS(ID) ON DELETE CASCADE
);

-- Index metadata
CREATE TABLE IF NOT EXISTS SXT_META.INDEXES
(
    -- The unique identifier
    ID UUID
        NOT NULL
        DEFAULT gen_random_uuid()
        PRIMARY KEY,
    -- The table identifier
    TABLE_ID UUID
        NOT NULL,
    -- The database/catalog name
    CATALOG_NAME VARCHAR(64)
        NULL,
    -- The schema name
    SCHEMA_NAME VARCHAR(64)
        NOT NULL,
    -- The index name
    INDEX_NAME VARCHAR(64)
        NULL,
    -- The resource identifier (i.e., concatenation of CATALOG_NAME, SCHEMA_NAME, INDEX_NAME)
    RESOURCEID VARCHAR(192)
        NOT NULL
        UNIQUE,
    -- The indexed column identifiers (comma-separated for composite)
    COLUMNS VARCHAR
        NOT NULL,
    -- The index type
    INDEX_TYPE VARCHAR(255)
        NOT NULL,
    -- The index sort order (ASC or DESC)
    SORT_ORDER VARCHAR(4)
        NOT NULL
        DEFAULT 'ASC'
        CHECK(SORT_ORDER IN ('ASC', 'DESC')),
    -- Flag: defines whether or not index values must be unique
    IS_UNIQUE BOOLEAN
        NOT NULL
        DEFAULT FALSE,
    -- The subscription identifier
    SUBSCRIPTION_ID UUID
        NULL,
    -- The timestamp when the row was inserted
    CREATED TIMESTAMP
        NOT NULL
        DEFAULT NOW(),

    -- Ensure catalog + schema + index combo is unique
    UNIQUE(CATALOG_NAME, SCHEMA_NAME, INDEX_NAME),
    -- SXT_META.TABLES fk
    CONSTRAINT FK_TABLE_ID FOREIGN KEY (TABLE_ID) REFERENCES SXT_META.TABLES(ID) ON DELETE CASCADE
);