use std::collections::HashMap;
use std::env;
use std::time::Duration;

use deadpool_postgres::{Manager, ManagerConfig, Object, Pool, RecyclingMethod};
use lazy_static::lazy_static;
use log::debug;
use object_store::azure::{MicrosoftAzure, MicrosoftAzureBuilder};
use object_store::path::Path;
use object_store::{ClientOptions, ListResult, ObjectMeta};
use regex::Regex;
use tokio::time::sleep;
use tokio_postgres::NoTls;

use crate::azure_data_loader::{estimate_load_time, load_data_from_azure};
use crate::to_pg::PgColumn;

lazy_static! {
    /// Regular expression to capture the year from the object store file path.
    /// The format expected is `SXT_INTERNAL_YEAR=YYYY` where `YYYY` is a 4-digit year.
    static ref YEAR_REGEX: Regex = Regex::new(r"SXT_INTERNAL_YEAR=(\d{4})").unwrap();
    /// Regular expression to capture the month from the object store file path.
    /// The format expected is `SXT_INTERNAL_MONTH=MM` where `MM` is a 1 or 2-digit month.
    static ref MONTH_REGEX: Regex = Regex::new(r"SXT_INTERNAL_MONTH=(\d{1,2})").unwrap();
}

/// Extract the year and month from a given file path using predefined regex patterns.
///
/// # Arguments
/// - `path`: A string slice containing the file path.
///
/// # Returns
/// Returns an `Option<(i32, i32)>` containing the year and month if both are successfully extracted.
pub fn extract_year_and_month(path: &str) -> Option<(i16, i16)> {
    // Extract year and month using the regex patterns
    let year = YEAR_REGEX
        .captures(path)?
        .get(1)?
        .as_str()
        .parse::<i16>()
        .ok();
    let month = MONTH_REGEX
        .captures(path)?
        .get(1)?
        .as_str()
        .parse::<i16>()
        .ok();

    // Return the year and month as a tuple if both are found
    match (year, month) {
        (Some(y), Some(m)) => Some((y, m)),
        _ => None,
    }
}

/// META column used by proof of sql
pub const META_ROW_NUMBER_COLUMN_NAME: &str = "META_ROW_NUMBER";

/// Query to get primary column for a table
pub const PRIMARY_KEY_QUERY: &str = " SELECT
    c.column_name,
    c.data_type
    FROM
    information_schema.table_constraints tc
JOIN
    information_schema.constraint_column_usage AS ccu
    USING (constraint_schema, constraint_name)
JOIN
    information_schema.columns AS c
    ON c.table_schema = tc.constraint_schema
    AND tc.table_name = c.table_name
    AND ccu.column_name = c.column_name
WHERE
    constraint_type = 'PRIMARY KEY'
    AND tc.table_name = $1
    AND tc.table_schema = $2;
    ";

const COLUMN_TYPE_QUERY: &str = "
        SELECT column_name, data_type, numeric_precision, numeric_scale
        FROM information_schema.columns
        WHERE upper(table_name) = $1 and upper(table_schema) = $2
    ";

/// Estimate the processing time for loading data from an Azure object store.
///
/// # Arguments
/// - `base_path`: A string slice that represents the base path in the object store.
///
/// # Returns
/// An `Ok(())` result on success, or an error wrapped in `anyhow::Error` on failure.
pub async fn estimate_time(base_path: &str) -> Result<(), anyhow::Error> {
    let store = get_object_store()?; // Get the object store client
    estimate_load_time(&store, base_path.into()).await?;
    Ok(())
}

/// Run the data loader with retry logic for connecting to the object store and the database.
///
/// # Arguments
/// - `base_path`: The base path of the data to be loaded.
/// - `max_retries`: The maximum number of retry attempts for the operation.
/// - `delay`: Duration to wait between retry attempts.
///
/// # Returns
/// Returns `Ok(())` on success or an error wrapped in `anyhow::Error` if all retries fail.
pub async fn run_data_loader(
    base_path: &str,
    max_retries: u32,
    delay: Duration,
) -> Result<(), anyhow::Error> {
    let mut attempts = 0;

    while attempts < max_retries {
        // Increment the attempt counter
        attempts += 1;

        // Attempt to run the data loader
        match async {
            let store = get_object_store()?; // Get the object store client
            load_data_from_azure(&store, base_path.into()).await?;
            Ok(())
        }
        .await
        {
            Ok(_) => return Ok(()), // Return if successful
            Err(e) => {
                eprintln!("Attempt {} failed: {}", attempts, e);
                if attempts < max_retries {
                    sleep(delay).await; // Wait before retrying
                } else {
                    return Err(e); // Return the last error after max retries
                }
            }
        }
    }

    Ok(())
}

/// Create a connection pool for Postgres using Deadpool with a specified database URL.
///
/// # Arguments
/// - `db_url`: The URL of the Postgres database to connect to.
///
/// # Returns
/// A `Pool` object that allows connections to be retrieved from the pool.
pub fn create_pool(db_url: &str) -> Pool {
    let manager_config = ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    };

    let manager = Manager::from_config(db_url.parse().unwrap(), NoTls, manager_config);
    Pool::builder(manager).max_size(64).build().unwrap()
}

/// Establish a client session with the Postgres database using a connection pool.
///
/// # Returns
/// An `Object` representing the active database connection or an error on failure.
pub async fn create_client_session() -> Result<Object, anyhow::Error> {
    let db_url = env::var("DATABASE_URL").map_err(|_| anyhow::anyhow!("Missing database url"))?;
    let pool = create_pool(&db_url);
    let client = pool
        .get()
        .await
        .map_err(|e| anyhow::anyhow!(format!("Failed to get connection: {}", e)))?;

    Ok(client)
}

/// Retrieve Azure storage configuration from environment variables.
///
/// # Returns
/// A tuple containing the Azure account, access key, and container name, or an error string if any
/// required environment variable is missing.
fn get_azure_config() -> Result<(String, String, String), String> {
    let azure_account = env::var("AZURE_ACCOUNT_NAME").map_err(|_| "Missing AZURE_ACCOUNT_NAME")?;
    let azure_endpoint = env::var("AZURE_ENDPOINT").map_err(|_| "Missing AZURE_ENDPOINT")?;
    let azure_container_name =
        env::var("AZURE_CONTAINER_NAME").map_err(|_| "Missing AZURE_CONTAINER_NAME")?;

    Ok((azure_account, azure_endpoint, azure_container_name))
}

/// Initialize and configure the Microsoft Azure object store client using credentials and settings.
///
/// # Returns
/// An `Ok(MicrosoftAzure)` object on success or an error wrapped in `anyhow::Error` on failure.
fn get_object_store() -> Result<MicrosoftAzure, anyhow::Error> {
    let client_options = ClientOptions::new().with_timeout(Duration::from_secs(1000));

    // Load configuration from environment variables with default values
    let (azure_account, azure_endpoint, azure_container_name) = get_azure_config().unwrap();

    let store = MicrosoftAzureBuilder::from_env()
        .with_account(azure_account)
        .with_container_name(azure_container_name)
        .with_endpoint(azure_endpoint)
        .with_skip_signature(true)
        .with_client_options(client_options)
        .build()?;
    debug!("Store created");
    Ok(store)
}

fn get_process_only_head() -> Result<bool, anyhow::Error> {
    // Read the environment variable
    let process_only_head = env::var("PROCESS_ONLY_HEAD").unwrap_or_else(|_| "false".to_string());

    // Convert the string to a boolean
    match process_only_head.to_lowercase().as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(anyhow::anyhow!(
            "Invalid value for PROCESS_ONLY_HEAD, expected 'true' or 'false'"
        )),
    }
}

/// Process the list of object store prefixes, returning only the first year if the `PROCESS_ONLY_HEAD`
/// environment variable is set to `true`. Otherwise, all years are processed.
///
/// # Arguments
/// - `list_result`: A `ListResult` containing the object store prefixes.
///
/// # Returns
/// A `Vec<Path>` of sorted object store paths to be processed.
pub fn process_list(list_result: ListResult) -> Vec<Path> {
    // Process only the head of the list if the environment variable is set
    let mut list_to_process: Vec<Path> = if get_process_only_head().unwrap() {
        // Take only the first year (if available)
        list_result.common_prefixes.into_iter().take(1).collect()
    } else {
        // Process all years
        list_result.common_prefixes
    };

    // Sort the paths in ascending order
    list_to_process.sort();
    list_to_process
}

/// Process the list of object store files, returning only the first file if the `PROCESS_ONLY_HEAD`
/// environment variable is set to `true`. Otherwise, all files are processed.
///
/// # Arguments
/// - `file_list`: A `Vec<ObjectMeta>` of file metadata from the object store.
///
/// # Returns
/// A `Vec<ObjectMeta>` of sorted file metadata to be processed.
pub fn process_data_files(file_list: Vec<ObjectMeta>) -> Vec<ObjectMeta> {
    // Process only the head of the list if the environment variable is set
    let mut list_to_process: Vec<ObjectMeta> = if get_process_only_head().unwrap() {
        // Take only the first file (if available)
        file_list.into_iter().take(1).collect()
    } else {
        // Process all files
        file_list
    };
    // Sort the paths in ascending order
    list_to_process.sort_by(|a, b| a.location.cmp(&b.location));
    list_to_process
}

/// Extract the schema and table name from a file path in the object store.
///
/// # Arguments
/// - `path`: A `Path` object representing the file path.
///
/// # Returns
/// A `schema and table names extracted`
pub fn extract_schema_and_table(path: &Path) -> Result<(String, String), anyhow::Error> {
    // Extract the file name from the path, handling potential absence of a file name
    if let Some(file_name) = path.filename() {
        // Check if the file name starts with "SQL_" and split it into schema and table
        if let Some((schema, table)) = file_name.trim_start_matches("SQL_").split_once('_') {
            return Ok((schema.to_string(), table.to_string()));
        }
    }

    // Return an error if the schema and table names cannot be extracted
    Err(anyhow::anyhow!("Could not extract schema and table names"))
}

/// Fetches the column metadata for a specific table in a PostgreSQL database.
///
/// This function queries the database using the provided client connection to
/// retrieve the column names, data types, and optional numeric precision and
/// scale for the given table. It returns a `HashMap` where the keys are the
/// column names and the values are `PgColumn` structs containing detailed
/// information about each column.
///
/// # Parameters
/// - `client`: A reference to an active database connection object used to
///   execute the query.
/// - `schema_name`: The name of the schema in which the table resides.
/// - `table_name`: The name of the table whose column metadata is to be fetched.
///
/// # Returns
/// - `Result<HashMap<String, PgColumn>, anyhow::Error>`:
///   - On success, returns a `HashMap` where the key is the column name (as a `String`)
///     and the value is a `PgColumn` struct with information about the column.
///   - On failure, returns a boxed error.
///
/// # Errors
/// This function will return an error if:
/// - The query execution fails.
/// - The provided schema or table names are invalid or non-existent.
///
pub async fn get_table_columns_and_types(
    client: &Object,
    schema_name: &str,
    table_name: &str,
) -> Result<HashMap<String, PgColumn>, anyhow::Error> {
    let rows = client
        .query(
            COLUMN_TYPE_QUERY,
            &[&table_name.to_uppercase(), &schema_name.to_uppercase()],
        )
        .await?;

    let mut column_map = HashMap::new();
    for row in rows {
        let column_name: String = row.get(0);
        let data_type: String = row.get(1);

        // Handle potential null values
        let numeric_precision: Option<i32> = row.get(2);
        let numeric_scale: Option<i32> = row.get(3);

        column_map.insert(
            column_name.clone(),
            PgColumn {
                column_name,
                data_type,
                numeric_precision,
                numeric_scale,
            },
        );
    }

    Ok(column_map)
}
