use std::fmt::Display;

use deadpool_postgres::Object;
use tonic::Status;

/// Represents the status of a checkpoint in a processing workflow.
///
/// The `CheckpointStatus` enum defines various states that a checkpoint can be in
/// during processing. Each variant represents a distinct status that can be used
/// to track the progress and outcome of operations.
///
/// This enum can be used in various contexts, such as logging, status reporting,
/// or controlling flow based on the current state of a checkpoint.

#[derive(Debug)]
pub enum CheckpointStatus {
    /// - `Failed`: Indicates that the checkpoint has failed.
    Failed,
    /// - `Completed`: Indicates that the checkpoint has been successfully completed.
    Completed,
    /// - `Processing`: Indicates that the checkpoint is currently being processed.
    Processing,
}

/// Formats the `CheckpointStatus` as a string for display purposes.
///
/// This method converts each variant of the `CheckpointStatus` enum into a
/// human-readable string representation. It is primarily used for logging
/// and user-facing messages.
///
impl Display for CheckpointStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            CheckpointStatus::Failed => "Failed".to_string(),
            CheckpointStatus::Completed => "Completed".to_string(),
            CheckpointStatus::Processing => "Processing".to_string(),
        };
        write!(f, "{}", str)
    }
}

/// Represents a checkpoint in the data loading process.
///
/// The `Checkpoint` struct is used to track the status and metadata of
/// data loading operations for a specific schema and table in a database.
/// It includes information about the year and month of the operation,
/// the current status of the loading process, and the total time taken
/// for the operation.
///
pub struct Checkpoint {
    /// - `schema_name`: A `String` representing the name of the schema
    ///   associated with this checkpoint.
    schema_name: String,
    /// - `table_name`: A `String` representing the name of the table
    ///   associated with this checkpoint.
    table_name: String,

    /// - `year`: An `i16` representing the year during which the data loading
    ///   operation took place.
    year: i16,
    /// - `month`: An `i16` representing the month during which the data loading
    ///   operation took place.
    month: i16,
    /// - `status`: An optional `String` representing the current status of
    ///   the data loading operation (e.g., "completed", "in progress", etc.).
    status: CheckpointStatus,
    /// - `total_time_taken`: An `i64` representing the total time taken
    ///   for the data loading operation in milliseconds.
    total_time_taken: i64,
}

impl Checkpoint {
    /// Creates a new instance of `Checkpoint`.
    ///
    /// # Arguments
    ///
    /// * `schema_name` - The schema name as a `String`.
    /// * `table_name` - The table name as a `String`.
    /// * `year` - The year as an `i32`.
    /// * `month` - The month as an `i32`.
    ///
    /// # Returns
    ///
    /// Returns a new instance of `Checkpoint` with the given schema name, table name, year, and month.
    /// The `status` is initialized as `None` and `total_time_taken` is initialized to `0`.
    pub fn new(schema_name: String, table_name: String, year: i16, month: i16) -> Self {
        Checkpoint {
            schema_name,
            table_name,
            year,
            month,
            status: CheckpointStatus::Processing,
            total_time_taken: 0, // Total time starts at 0
        }
    }

    /// Initializes the checkpoint schema and table in the database.
    ///
    /// This function creates a schema named `SXT` and a table named `checkpoints`
    /// if they do not already exist. It also deletes any previous checkpoints
    /// that are not marked as completed.
    ///
    /// # Parameters
    ///
    /// - `client`: A reference to an `Object` that represents the database connection.
    ///
    /// # Errors
    ///
    /// This function will return a `Status` error if it fails to create the schema,
    /// create the table, or delete previous checkpoints.
    pub async fn init_checkpoint(client: &Object) -> Result<(), Status> {
        let query = "
            CREATE SCHEMA IF NOT EXISTS SXTMETA";
        client
            .execute(query, &[])
            .await
            .map_err(|e| Status::internal(format!("Failed to create sxt schema: {}", e)))?;

        let query = "
            CREATE TABLE IF NOT EXISTS SXTMETA.checkpoints (
                id SERIAL PRIMARY KEY,
                schema_name VARCHAR(64) NOT NULL,
                table_name VARCHAR(64) NOT NULL,
                year SMALLINT NOT NULL,
                month SMALLINT NOT NULL,
                status VARCHAR(50),
                error VARCHAR(500),
                total_time_taken BIGINT NOT NULL
            );
        ";

        // Execute the query
        client
            .execute(query, &[])
            .await
            .map_err(|e| Status::internal(format!("Failed to create checkpoint table: {}", e)))?;

        let query = "
            delete from SXTMETA.checkpoints where status != 'completed'";
        client.execute(query, &[]).await.map_err(|e| {
            Status::internal(format!("Failed to delete from SXTMETA.checkpoints: {}", e))
        })?;

        Ok(())
    }

    /// Inserts a new checkpoint into the database.
    ///
    /// This function inserts a new record into the `checkpoints` table with the
    /// specified details.
    ///
    /// # Parameters
    ///
    /// - `self`: A reference to the struct containing checkpoint details.
    /// - `client`: A reference to an `Object` that represents the database connection.
    ///
    /// # Returns
    ///
    /// Returns the number of rows affected by the insert operation.
    ///
    /// # Errors
    ///
    /// This function will return a `Status` error if it fails to execute the insert query.
    pub async fn insert(&self, client: &Object) -> Result<u64, Status> {
        let query = "
            INSERT INTO SXTMETA.checkpoints (schema_name, table_name, year, month, status, total_time_taken)
            VALUES ($1, $2, $3, $4, $5, $6)";

        let result = client
            .execute(
                query,
                &[
                    &self.schema_name,
                    &self.table_name,
                    &self.year,
                    &self.month,
                    &self.status.to_string(),
                    &self.total_time_taken,
                ],
            )
            .await
            .map_err(|e| Status::internal(format!("Failed to insert checkpoint: {}", e)))?;

        Ok(result)
    }

    /// Updates the status of an existing checkpoint in the database.
    ///
    /// This function updates the status and total time taken for a specific
    /// checkpoint identified by its schema name, table name, year, and month.
    ///
    /// # Parameters
    ///
    /// - `self`: A reference to the struct containing checkpoint details.
    /// - `client`: A reference to an `Object` that represents the database connection.
    /// - `status`: The new status to set for the checkpoint.
    /// - `time_taken`: The total time taken for processing related to this checkpoint.
    ///
    /// # Errors
    ///
    /// This function will return a `Status` error if it fails to execute the update query.
    pub async fn update_status(
        &self,
        client: &Object, // Changed Object to Client for clarity
        status: CheckpointStatus,
        error_message: Option<String>,
        time_taken: i64,
    ) -> Result<(), Status> {
        let query = "
        UPDATE SXTMETA.checkpoints
        SET status = $1, total_time_taken = $2, error = $3
        WHERE schema_name = $4
        AND table_name = $5
        AND year = $6
        AND month = $7
    ";

        // Truncate the error message if it exceeds 500 characters
        let truncated_error_message = match error_message {
            Some(msg) if msg.len() > 500 => Some(msg.chars().take(500).collect::<String>()),
            _ => error_message,
        };

        // Execute the update query
        client
            .execute(
                query,
                &[
                    &status.to_string(), // Convert CheckpointStatus to String here
                    &time_taken,
                    &truncated_error_message, // This will be Some(String) or None
                    &self.schema_name,
                    &self.table_name,
                    &self.year,
                    &self.month,
                ],
            )
            .await
            .map_err(|e| Status::internal(format!("Failed to update status: {}", e)))?;

        Ok(())
    }
    /// Checks if a specific checkpoint is completed.
    ///
    /// This function queries the database to determine if a checkpoint with
    /// the specified schema name, table name, year, and month is marked as completed.
    ///
    /// # Parameters
    ///
    /// - `client`: A reference to an `Object` that represents the database connection.
    /// - `schema_name`: The name of the schema associated with the checkpoint.
    /// - `table_name`: The name of the table associated with the checkpoint.
    /// - `year`: The year associated with the checkpoint.
    /// - `month`: The month associated with the checkpoint.
    ///
    /// # Returns
    ///
    /// Returns `true` if the checkpoint is completed; otherwise, returns `false`.
    ///
    /// # Errors
    ///
    /// This function will return a `Status` error if it fails to execute the query.
    pub async fn is_completed(
        client: &Object,
        schema_name: &str,
        table_name: &str,
        year: i16,
        month: i16,
    ) -> Result<bool, Status> {
        let query = "
            SELECT status
            FROM SXTMETA.checkpoints
            WHERE schema_name = $1
            AND table_name = $2
            AND year = $3
            AND month = $4
            LIMIT 1;
        ";

        let row = client
            .query_opt(query, &[&schema_name, &table_name, &year, &month])
            .await
            .map_err(|e| Status::internal(format!("Failed to update status: {}", e)))?;

        // Check if the row exists and if the status is "completed"
        if let Some(row) = row {
            let status: Option<String> = row.get(0);
            Ok(status.is_some() && status.unwrap() == CheckpointStatus::Completed.to_string())
        } else {
            Ok(false) // No checkpoint found
        }
    }

    /// Deletes previous checkpoints from the database based on specified criteria.
    ///
    /// This function removes checkpoints from the database that match
    /// the given schema name, table name, year, and month.
    ///
    /// # Parameters
    ///
    /// - `client`: A reference to an `Object` that represents the database connection.
    /// - `schema_name`: The name of the schema associated with checkpoints to be deleted.
    /// - `table_name`: The name of the table associated with checkpoints to be deleted.
    /// - `year`: The year associated with checkpoints to be deleted.
    /// - `month`: The month associated with checkpoints to be deleted.
    ///
    /// # Errors
    ///
    /// This function will return a `Status` error if it fails to execute the delete query.
    ///
    pub async fn delete(
        client: &Object,
        schema_name: &str,
        table_name: &str,
        year: i16,
        month: i16,
    ) -> Result<(), Status> {
        let query = "
            delete
            FROM SXTMETA.checkpoints
            WHERE schema_name = $1
            AND table_name = $2
            AND year = $3
            AND month = $4;
        ";

        client
            .query_opt(query, &[&schema_name, &table_name, &year, &month])
            .await
            .map_err(|e| Status::internal(format!("Failed to delete status: {}", e)))?;

        Ok(())
    }
}
