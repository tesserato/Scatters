use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Invalid input path: {0} does not exist or is not a file/directory")]
    InvalidInputPath(PathBuf),

    #[error("Unsupported file format for: {0}")]
    UnsupportedFormat(String),

    #[error("I/O error")]
    Io(#[from] std::io::Error),

    #[error("Data processing error (Polars)")]
    Polars(#[from] polars::prelude::PolarsError),

    #[error("Audio decoding error (Symphonia)")]
    Symphonia(#[from] symphonia::core::errors::Error),

    #[error("Failed to serialize data to JSON")]
    JsonSerialization(#[from] serde_json::Error),

    #[error("Column '{0}' not found in the data")]
    ColumnNotFound(String),

    #[error("No numeric columns found to plot")]
    NoNumericColumns,
}