use snafu::Snafu;
/// A module for error handling in the application.
///
/// This module defines a custom error type `FPGError` that is used throughout
/// the application to represent various error conditions. It also provides
/// a type alias for `Result` that simplifies error handling by using
/// `FPGError` as the error type. Additionally, it includes macros for
/// creating custom error messages and status responses.
///
/// # Type Aliases
///
/// - `Result<T>`: A type alias for `std::result::Result<T, FPGError>`,
///   which represents the result of an operation that can succeed with a value
///   of type `T` or fail with an `FPGError`.
pub type Result<T> = std::result::Result<T, FPGError>;

/// Represents errors that can occur in the application.
///
/// The `FPGError` enum defines various error types that can be encountered
/// during execution. Each variant represents a different source of error.

#[derive(Debug, Snafu)]
#[allow(missing_docs)]
pub enum FPGError {
    /// Represents an error from the Arrow library.
    #[snafu(display("Arrow error: {}", source))]
    Arrow { source: arrow::error::ArrowError },

    /// Represents an I/O error.
    #[snafu(display("I/O error: {}", source))]
    Io { source: std::io::Error },

    /// Represents a custom error with a descriptive message.
    #[snafu(display("Invalid argument: {}", message))]
    Custom { message: String },

    /// Represents an arrow record batch to postgres type conversion error with a descriptive message.
    #[snafu(display("Failed during type conversion: {}", message))]
    RecordBatchToPostgresError { message: String },

    /// Represents an error from the Tonic library.
    #[snafu(display("Tonic error: {}", source))]
    Tonic { source: tonic::transport::Error },

    /// Represents an error from the Tokio Postgres library.
    #[snafu(display("Postgres error: {}", source))]
    Postgres { source: tokio_postgres::Error },
}

/// Creates custom errors with context.
///
/// This macro generates a custom error of type `FPGError::Custom` with a
/// formatted message that includes the file and line number where the
/// macro was invoked.
#[macro_export]
macro_rules! err {
    ($desc:expr) => {
        $crate::error::FPGError::Custom {
            message: format!("{} at {}:{}", $desc, file!(), line!()),
        }
    };
    ($desc:expr, $err:expr) => {
        $crate::error::FPGError::Custom {
            message: format!(
                "{} caused by '{:?}' at {}:{}",
                $desc,
                $err,
                file!(),
                line!()
            ),
        }
    };
}

/// Creates internal status errors with context.
///
/// This macro generates a status response containing an internal error message
/// formatted with details about where it was called.
#[macro_export]
macro_rules! status {
    ($err:expr) => {
        Status::internal(format!("error: '{:?}' at {}:{}", $err, file!(), line!()))
    };
}
