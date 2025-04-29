//! The data-loader crate
//! Module responsible for handling data loading from Azure Blob Storage.
//!
/// Module responsible for handling data loading from Azure Blob Storage.
///
/// This module provides functionality to interact with Azure's storage
/// infrastructure, enabling the loading of data from blob containers into
/// your application. It includes utilities for connecting to Azure,
/// processing blobs, and managing data transfers.
pub mod azure_data_loader;

/// Module for managing checkpoints in data processing workflows.
///
/// The `checkpoint` module provides utilities to record and manage
/// checkpoints, allowing for resuming long-running data load
pub mod checkpoint;

/// Module for error handling and custom error types.
///
/// This module defines custom error types and utility functions for handling
/// errors that occur throughout the application. It provides a standardized
/// way to report and manage errors across different components of the system.
pub mod error;

/// Module for PostgreSQL table and column mapping.
///
/// The `to_pg` module handles mapping between external data sources and
/// PostgreSQL tables. It provides data structures and functions to manage
/// PostgreSQL column metadata (such as column names, data types, and numeric
/// precision) and execute queries related to database schema.
pub mod to_pg;

/// Module for data loading from an object store.
pub mod data_loader;
