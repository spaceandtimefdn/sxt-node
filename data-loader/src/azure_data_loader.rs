use std::collections::HashMap;
use std::error::Error;
use std::time::SystemTime;

use arrow::datatypes::SchemaRef;
use arrow_array::RecordBatch;
use deadpool_postgres::Object;
use futures::future::join_all;
use futures::TryStreamExt;
use log::{debug, info};
use object_store::azure::MicrosoftAzure;
use object_store::path::Path;
use object_store::ObjectStore;
use parquet::arrow::arrow_reader::ParquetRecordBatchReader;
use tokio_postgres::types::ToSql;

use crate::checkpoint::{Checkpoint, CheckpointStatus};
use crate::data_loader::{
    create_client_session,
    extract_schema_and_table,
    extract_year_and_month,
    get_table_columns_and_types,
    process_data_files,
    process_list,
    META_ROW_NUMBER_COLUMN_NAME,
    PRIMARY_KEY_QUERY,
};
use crate::to_pg::{get_pg_values, PgColumn, PgValue};

type Store = MicrosoftAzure;
type ColumnMap = HashMap<String, PgColumn>;

/// `TableLoader` is a struct responsible for loading data into a specific
/// PostgreSQL table within a given schema. It facilitates the interaction
/// between the external data source and the PostgreSQL database by mapping
/// the columns of the source to the table schema and performing the data
/// loading process.
///
struct TableLoader<'a> {
    /// * `store` - A reference to the `Store` which contains the data source.
    ///   This store provides access to the data that needs to be loaded.
    store: &'a Store,
    /// * `schema_name` - A reference to a string slice representing the schema
    ///   name in the PostgreSQL database where the target table resides.
    schema_name: &'a str,
    /// * `table_name` - A reference to a string slice representing the name of
    ///   the PostgreSQL table into which the data will be loaded.
    table_name: &'a str,
    /// * `column_maps` - A `ColumnMap` that stores the mappings between the
    ///   columns of the source data and the columns of the target PostgreSQL
    ///   table. This ensures that data is correctly inserted into the appropriate
    ///   columns.
    column_maps: ColumnMap,
}

/// TableLoadEstimator is a struct used to estimate the data load time
struct TableLoadEstimator<'a> {
    store: &'a Store,
}

impl TableLoadEstimator<'_> {
    async fn estimate_size(&self, prefix: &Path) -> Result<usize, anyhow::Error> {
        // List all years (or directories) under the prefix
        let list_result = self.store.list_with_delimiter(Some(prefix)).await?;

        let mut total_size: usize = 0;
        for year in list_result.common_prefixes {
            debug!("Processing year {}", year);
            // Process each month directory under the year
            let all_months = self.store.list_with_delimiter(Some(&year)).await?;

            // Create futures for processing each month concurrently
            let month_futures: Vec<_> = all_months
                .common_prefixes
                .into_iter()
                .map(|month| async move { self.part_size(&month).await })
                .collect();

            // Await all month processing futures and handle errors
            let results: Result<Vec<usize>, anyhow::Error> =
                join_all(month_futures).await.into_iter().collect();

            match results {
                Ok(sizes) => {
                    for s in sizes {
                        total_size += s;
                    }
                }
                Err(e) => {
                    eprintln!("Error processing months for year {}: {:?}", year, e);
                    continue; // Skip this year if there's an error
                }
            }
        }
        Ok(total_size)
    }

    async fn download_single_file(&self, prefix: &Path) -> Result<(u128, usize), anyhow::Error> {
        let all_years = self.store.list_with_delimiter(Some(prefix)).await?;
        let years: Vec<Path> = all_years.common_prefixes.into_iter().take(1).collect();
        let all_months = self
            .store
            .list_with_delimiter(Some(years.first().unwrap()))
            .await?;
        let months: Vec<Path> = all_months.common_prefixes.into_iter().take(1).collect();

        let all_data_files = self
            .store
            .list(Some(months.first().unwrap()))
            .try_collect::<Vec<_>>()
            .await?;

        let now = SystemTime::now();
        let files: Vec<_> = all_data_files.into_iter().take(1).collect();
        let content = self.store.get(&files.first().unwrap().location).await?;
        let file_contents = content.bytes().await?;
        let time_taken = now.elapsed().unwrap().as_millis();
        Ok((time_taken, file_contents.len()))
    }

    async fn part_size(&self, prefix: &Path) -> Result<usize, anyhow::Error> {
        let part_size = self
            .store
            .list(Some(prefix))
            .try_collect::<Vec<_>>()
            .await?
            .into_iter()
            .map(|p| p.size)
            .sum();
        println!("Part {}, Size {}", prefix, part_size);
        Ok(part_size)
    }
}

impl TableLoader<'_> {
    // Constructor to create a new DataLoader instance

    // Method to process each year directory
    async fn load(&self, prefix: &Path) -> Result<(), anyhow::Error> {
        // List all years (or directories) under the prefix
        let list_result = self.store.list_with_delimiter(Some(prefix)).await?;

        for year in process_list(list_result) {
            info!("Processing year {}", year);
            // Process each month directory under the year
            let all_months = self.store.list_with_delimiter(Some(&year)).await?;

            // Create futures for processing each month concurrently
            let month_futures: Vec<_> = process_list(all_months)
                .into_iter()
                .map(|month| async move { self.process_partition(&month).await })
                .collect();

            // Await all month processing futures
            join_all(month_futures)
                .await
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?;
        }
        Ok(())
    }

    async fn create_index(&self) -> Result<(), anyhow::Error> {
        let client = create_client_session().await?;
        let index_name = format!("{}_{}_HASH", self.schema_name, self.table_name);
        let column_name = META_ROW_NUMBER_COLUMN_NAME;
        let query = format!(
            "CREATE INDEX IF NOT EXISTS  {index_name} on {}.{} ( {column_name})",
            self.schema_name, self.table_name
        );
        debug!("Index creation query {}", query);
        client.execute(&query, &[]).await?;
        Ok(())
    }

    // Method to process each partition (month) and insert records into Postgres
    async fn process_partition(&self, prefix: &Path) -> Result<(), anyhow::Error> {
        info!("processing partition: {}", prefix);
        let (year, month) = extract_year_and_month(prefix.as_ref())
            .ok_or("Failed to extract year and month")
            .map_err(|e| anyhow::anyhow!(e))?;
        let client = create_client_session().await?;
        if Checkpoint::is_completed(&client, self.schema_name, self.table_name, year, month).await?
        {
            debug!(
                "Partition for year {} and month {} is already completed.",
                year, month
            );
            return Ok(());
        }

        let checkpoint =
            Checkpoint::new(self.schema_name.into(), self.table_name.into(), year, month);
        Checkpoint::delete(&client, self.schema_name, self.table_name, year, month).await?;
        checkpoint.insert(&client).await?;

        let now = SystemTime::now();

        let result = self.process_data_files(prefix, &client).await;
        match result {
            Ok(()) => {
                checkpoint
                    .update_status(
                        &client,
                        CheckpointStatus::Completed,
                        None,
                        now.elapsed().unwrap().as_secs() as i64,
                    )
                    .await?
            }
            Err(e) => {
                let root_cause = e
                    .source()
                    .map_or_else(|| e.to_string(), |source| source.to_string());
                info!("failed partition: {}", prefix);
                checkpoint
                    .update_status(
                        &client,
                        CheckpointStatus::Failed,
                        Some(format!("failed due to {}", root_cause)),
                        now.elapsed().unwrap().as_secs() as i64,
                    )
                    .await?
            }
        }
        info!("completed partition: {}", prefix);
        Ok(())
    }

    async fn process_data_files(
        &self,
        prefix: &Path,
        client: &Object,
    ) -> Result<(), anyhow::Error> {
        let all_data_files = self
            .store
            .list(Some(prefix))
            .try_collect::<Vec<_>>()
            .await?;

        for data_file in process_data_files(all_data_files) {
            debug!("Part file location: {}", data_file.location);

            // Fetch the file contents
            let content = self.store.get(&data_file.location).await?;
            let file_contents = content.bytes().await?;

            // Read the file contents as parquet and insert into Postgres
            let mut arrow_reader = ParquetRecordBatchReader::try_new(file_contents, 1024)?;
            while let Some(mut batch) = arrow_reader.next().transpose()? {
                let updated_batch =
                    self.drop_column_from_batch(&mut batch, "sxt_primary_key_binary")?;
                self.insert_record_batch(updated_batch, client).await?;
            }
        }
        Ok(())
    }

    // Function to drop a column from a RecordBatch
    fn drop_column_from_batch<'b>(
        &self,
        batch: &'b mut RecordBatch,
        column_name: &str,
    ) -> Result<&'b mut RecordBatch, anyhow::Error> {
        let schema = batch.schema();

        let batch = match schema.index_of(&column_name.to_uppercase()) {
            Ok(index) => {
                batch.remove_column(index);
                batch
            }
            Err(_) => batch,
        };
        Ok(batch)
    }

    // Helper method to insert a record batch into Postgres
    async fn insert_record_batch(
        &self,
        batch: &mut RecordBatch,
        client: &Object,
    ) -> Result<(), anyhow::Error> {
        self.insert_record_batch_to_postgres(
            client,
            format!("{}.{}", &self.schema_name, &self.table_name).as_str(),
            batch,
            &self.column_maps,
        )
        .await?;
        Ok(())
    }

    async fn insert_record_batch_to_postgres(
        &self,
        client: &Object,
        qualified_table_name: &str,
        batch: &RecordBatch,
        column_map: &HashMap<String, PgColumn>,
    ) -> Result<i64, anyhow::Error> {
        let schema = batch.schema();
        // Collect PostgreSQL values
        let mut pg_values: Vec<PgValue> = Vec::new();
        debug!("Batch size {}", batch.num_rows());
        debug!("schema {}", batch.schema());

        for i in 0..batch.num_rows() {
            let mut val = get_pg_values(batch, i, column_map)?;
            pg_values.append(&mut val);
        }

        // Prepare the insert statement
        let insert_stmt = self
            .build_insert_statement(client, qualified_table_name, &schema, batch.num_rows())
            .await?;

        // Execute the insert statement
        let affected_rows = client
            .execute(
                &insert_stmt,
                &pg_values
                    .iter()
                    .map(|p| p as &(dyn ToSql + Sync))
                    .collect::<Vec<_>>(),
            )
            .await?;

        debug!("Inserted successfully, affected rows {}", affected_rows);

        Ok(affected_rows as i64)
    }

    /// Retrieves primary key columns for a given table dynamically from the Postgres system catalogs.
    async fn get_primary_keys(&self, client: &Object) -> Result<Vec<String>, anyhow::Error> {
        let rows = client
            .query(
                PRIMARY_KEY_QUERY,
                &[
                    &self.table_name.to_lowercase(),
                    &self.schema_name.to_lowercase(),
                ],
            )
            .await?;

        let primary_keys = rows
            .iter()
            .map(|row| row.get::<_, String>("column_name"))
            .collect::<Vec<String>>();

        Ok(primary_keys)
    }

    async fn build_insert_statement(
        &self,
        client: &Object,
        qualified_table_name: &str,
        schema: &SchemaRef,
        num_rows: usize,
    ) -> Result<String, anyhow::Error> {
        let column_list = schema
            .fields()
            .iter()
            .map(|field| field.name().clone())
            .collect::<Vec<String>>()
            .join(", ");

        let mut insert_stmt = format!(
            "INSERT INTO {} ({}) VALUES ",
            qualified_table_name, column_list
        );

        let num_fields = schema.fields().len();
        let mut k = 1;

        for _ in 0..num_rows {
            let place_holder = (k..k + num_fields)
                .map(|i| format!("${}", i))
                .collect::<Vec<String>>()
                .join(", ");
            insert_stmt.push_str(&format!("({}),", place_holder));
            k += num_fields;
        }

        let primary_keys = self.get_primary_keys(client).await?;
        // Step 3: Build the ON CONFLICT statement based on primary keys
        let on_conflict_clause = if !primary_keys.is_empty() {
            let conflict_target = primary_keys.join(", ");
            format!(" ON CONFLICT ({}) DO NOTHING ", conflict_target,)
        } else {
            String::new() // No primary key, no conflict handling
        };

        insert_stmt.pop(); // Remove the trailing comma

        insert_stmt.push_str(&on_conflict_clause);
        Ok(insert_stmt)
    }
}

/// Estimates the load time for data at the specified base path.
///
/// This function initializes a `DataLoader` struct and uses it to
/// estimate the time required to load data from the given `base_path`.
///
/// # Parameters
///
/// - `store`: A reference to a `Store` instance, which is used to interact with the data source.
/// - `base_path`: A `String` representing the base path from which to estimate load time.
///
/// # Errors
///
/// This function will return an error if the estimation process fails,
/// encapsulated in a `anyhow::Error`.
///
pub async fn estimate_load_time(store: &Store, base_path: String) -> Result<(), anyhow::Error> {
    // Initialize the DataLoader struct
    let loader = DataLoader::new(store);
    // Load data from the base path
    loader.estimate(base_path.to_string()).await?;
    Ok(())
}

/// Loads data from Azure at the specified base path.
///
/// This function establishes a connection to the database and initializes
/// a checkpoint before loading data using a `DataLoader` instance.
///
/// # Parameters
///
/// - `store`: A reference to a `Store` instance, which is used to interact with the data source.
/// - `base_path`: A `String` representing the base path from which to load data.
///
/// # Errors
///
/// This function will return an error if any part of the loading process fails,
/// including database connection issues or data loading failures,
/// encapsulated in a `anyhow::Error`.
pub async fn load_data_from_azure(store: &Store, base_path: String) -> Result<(), anyhow::Error> {
    let client = create_client_session().await?; // Establish DB connection
    println!("db and store connected");
    Checkpoint::init_checkpoint(&client).await?;
    // Initialize the DataLoader struct
    let loader = DataLoader::new(store);
    // Load data from the base path
    loader.load_data(base_path.to_string()).await?;
    Ok(())
}

// Entry function to load data
struct DataLoader<'a> {
    store: &'a Store,
}

impl<'a> DataLoader<'a> {
    // Constructor to create a new DataLoader instance
    pub fn new(store: &'a Store) -> Self {
        DataLoader { store }
    }

    // Method to load data from the base path
    async fn load_data(&self, base_path: String) -> Result<(), anyhow::Error> {
        let path = Path::from(base_path.clone());

        // List all tables (or directories) under the base path
        let list_result = self.store.list_with_delimiter(Some(&path)).await?;

        for table in list_result.common_prefixes.clone() {
            info!(
                "Top level directories to be processed {}",
                table.to_string()
            )
        }

        // Process each table (in this case, each year folder)
        for table in list_result.common_prefixes {
            // Process each table and catch any errors
            if let Err(e) = self.process_table(&table).await {
                // Log the error, but continue processing other tables
                eprintln!("Error processing table {}: {:?}", table, e);
                eprintln!("\n\n");
            }
        }

        Ok(())
    }

    async fn estimate(&self, base_path: String) -> Result<(), anyhow::Error> {
        let path = Path::from(base_path.clone());

        // List all tables (or directories) under the base path
        let list_result = self.store.list_with_delimiter(Some(&path)).await?;

        let mut sample_value: Option<(u128, usize)> = None;

        let mut total_size: usize = 0;
        // Process each table (in this case, each year folder)
        for table in list_result.common_prefixes {
            if sample_value.is_none() {
                sample_value = Some(self.sample(&table).await?);
            }

            let estimate_result = self.estimate_table(&table).await;

            match estimate_result {
                Ok(table_size) => total_size += table_size,
                Err(e) => {
                    // Log the error, but continue processing other tables
                    eprintln!("Error processing table {}: {:?}", table, e);
                    eprintln!("\n\n");
                }
            }
        }
        // Handle the case where sample_value might still be None
        if let Some((sample_time, sample_size)) = sample_value {
            println!("sample_time : {} \n", sample_time);
            println!("sample_size : {} \n", sample_size);

            if total_size > 0 {
                // Prevent division by zero
                println!("Total Size : {} \n", total_size);
                println!(
                    "Total Time estimated : {} \n",
                    (sample_size as u128 / total_size as u128) * sample_time
                );
            } else {
                println!("Total Size is 0, cannot estimate time.");
            }
        } else {
            println!("No sample value available to estimate time.");
        }
        Ok(())
    }

    async fn process_table(&self, table: &Path) -> Result<(), anyhow::Error> {
        info!("Processing table {}", table);

        // Extract schema and table name
        let (schema_name, table_name) = extract_schema_and_table(table).map_err(|e| {
            anyhow::anyhow!(format!(
                "Failed to extract schema and table for {}: {}",
                table, e
            ))
        })?;

        let client = create_client_session().await?;
        // Fetch column mappings from the database
        let column_maps = get_table_columns_and_types(&client, &schema_name, &table_name)
            .await
            .map_err(|e| {
                anyhow::anyhow!(format!(
                    "Failed to get column mappings for {}: {}",
                    table_name, e
                ))
            })?;

        let table_loader = TableLoader {
            store: self.store,
            schema_name: &schema_name,
            table_name: &table_name,
            column_maps,
        };

        // Process table data using TableLoader
        table_loader.load(table).await?;

        let now = SystemTime::now();

        // Process index for the table
        table_loader.create_index().await?;
        info!(
            "Time taken to create index {}",
            now.elapsed().unwrap().as_secs()
        );
        Ok(())
    }

    async fn sample(&self, table: &Path) -> Result<(u128, usize), anyhow::Error> {
        info!("Sampling table {}", table);

        let table_estimator = TableLoadEstimator { store: self.store };

        // Process table data using TableLoader
        // This function could include further logic for processing the table
        let result = table_estimator.download_single_file(table).await?;

        Ok(result)
    }

    async fn estimate_table(&self, table: &Path) -> Result<usize, anyhow::Error> {
        info!("Estimating table {}", table);
        let table_estimator = TableLoadEstimator { store: self.store };

        // Process table data using TableLoader
        // This function could include further logic for processing the table
        let result = table_estimator.estimate_size(table).await?;

        Ok(result)
    }
}
