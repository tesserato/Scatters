//! Defines the custom error type for the application.
//!
//! This module centralizes all possible errors that can occur during the
//! execution of the program into a single `AppError` enum. Using `thiserror`,
//! it provides convenient conversions from underlying library errors
//! (like `std::io::Error`, `polars::prelude::PolarsError`, etc.) and
//! descriptive error messages.

use std::path::PathBuf;
use thiserror::Error;

/// The primary error type for the application.
#[derive(Error, Debug)]
pub enum AppError {
    /// Error indicating that the provided input path does not exist or is not a valid file/directory.
    #[error("Invalid input path: {0} does not exist or is not a file/directory")]
    InvalidInputPath(PathBuf),

    /// Error for when a file has an extension that is not supported by any of the data loaders.
    #[error("Unsupported file format for: {0}")]
    UnsupportedFormat(String),

    /// An I/O error occurred, typically while reading a file or writing the output plot.
    #[error("I/O error")]
    Io(#[from] std::io::Error),

    /// An error originating from the Polars data manipulation library.
    #[error("Data processing error (Polars)")]
    Polars(#[from] polars::prelude::PolarsError),

    /// An error from the Symphonia library, occurring during audio file decoding.
    #[error("Audio decoding error (Symphonia)")]
    Symphonia(#[from] symphonia::core::errors::Error),

    /// An error that occurred during JSON serialization of the plot data.
    #[error("Failed to serialize data to JSON")]
    JsonSerialization(#[from] serde_json::Error),

    /// An error from the Calamine library, occurring during Excel file parsing.
    #[error("Excel parsing error (Calamine)")]
    Calamine(#[from] calamine::Error),

    /// Error for when a user-specified column name is not found in the DataFrame.
    #[error("Column '{0}' not found in the data")]
    ColumnNotFound(String),

    /// Error indicating that no plottable (numeric) columns were found after selecting the X-axis.
    #[error("No numeric columns found to plot")]
    NoNumericColumns,
}
