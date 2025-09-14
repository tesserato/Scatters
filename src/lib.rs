//! The main library for the `scatters` application.
//!
//! This crate provides the core logic for finding, loading, processing, and plotting data.
//! It orchestrates the flow from command-line arguments to the final HTML output file.
//! The primary entry point is the `run` function, which takes the parsed CLI arguments
//! and executes the plotting process.
//!
//! The library is structured into several modules:
//! - `cli`: Defines the command-line interface.
//! - `data_loader`: Handles reading various file formats into DataFrames.
//! - `processing`: Logic for selecting X and Y axes and preparing data for plotting.
//! - `plotter`: Generates the final HTML/JavaScript plot from the prepared data.
//! - `error`: Defines the application's custom error type.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

pub mod cli;
pub mod data_loader;
pub mod error;
pub mod plotter;
pub mod processing;

use crate::cli::Cli;
use crate::error::AppError;

/// The main entry point for the application logic.
///
/// This function orchestrates the entire process:
/// 1.  It finds all supported files based on the input path (which can be a file or directory).
/// 2.  It iterates through each file, calling `process_single_file` to handle the plotting.
/// 3.  It prints progress and completion messages to the console.
///
/// # Arguments
///
/// * `cli` - A reference to the `Cli` struct containing parsed command-line arguments.
///
/// # Errors
///
/// Returns an error if file discovery or processing fails for any of the files.
pub fn run(cli: &Cli) -> Result<()> {
    // 1. Discover files to process
    let files_to_process = find_supported_files(&cli.input_path)?;
    if files_to_process.is_empty() {
        println!("No supported files found in the specified path.");
        return Ok(());
    }

    println!("Found {} files to process...", files_to_process.len());

    // 2. Process each file
    for file_path in files_to_process {
        println!("Processing '{}'...", file_path.display());
        process_single_file(&file_path, cli)
            .with_context(|| format!("Failed to process file: {}", file_path.display()))?;
    }

    println!("Done.");
    Ok(())
}

/// Orchestrates the loading, processing, and plotting for a single file.
///
/// # Arguments
///
/// * `file_path` - The path to the data file to process.
/// * `cli` - A reference to the parsed command-line arguments.
///
/// # Errors
///
/// Returns an error if any step (loading, processing, plotting, or saving) fails.
fn process_single_file(file_path: &Path, cli: &Cli) -> Result<()> {
    // 1. Load data into a DataFrame
    let df = data_loader::load_dataframe(file_path)?;

    if cli.debug {
        println!("  -> Detected columns:");
        for s in df.get_columns() {
            println!("     - {}: {:?}", s.name(), s.dtype());
        }
        println!("  -> Shape: {} rows x {} cols", df.height(), df.width());
    }

    // 2. Prepare data for plotting (select X and Y series)
    let plot_data = processing::prepare_plot_data(df, cli, file_path)?;

    // 3. Generate the HTML plot
    let html_content = plotter::generate_html_plot(&plot_data)?;

    // 4. Save the output
    let output_path = generate_output_path(file_path, cli);
    fs::create_dir_all(output_path.parent().unwrap_or(Path::new(".")))?;
    fs::write(&output_path, html_content)
        .with_context(|| format!("Failed to write output to {}", output_path.display()))?;

    println!("  -> Plot saved to '{}'", output_path.display());

    Ok(())
}

/// Finds all supported files based on a given path.
///
/// If the path is a file, it checks if its extension is supported.
/// If the path is a directory, it recursively walks the directory and collects all
/// files with supported extensions.
///
/// # Arguments
///
/// * `path` - The input path, which can be a file or a directory.
///
/// # Returns
///
/// A `Result` containing a vector of `PathBuf`s for all supported files found,
/// or an `AppError::InvalidInputPath` if the path doesn't exist.
fn find_supported_files(path: &Path) -> Result<Vec<std::path::PathBuf>, AppError> {
    let mut files = Vec::new();
    let supported_extensions: Vec<&str> = vec![
        "csv", "parquet", "json", "jsonl", "ndjson", "xlsx", "xls", "wav", "mp3", "flac",
    ];

    if path.is_file() {
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            if supported_extensions.contains(&ext.to_lowercase().as_str()) {
                files.push(path.to_path_buf());
            }
        }
    } else if path.is_dir() {
        for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                if let Some(ext) = entry.path().extension().and_then(|s| s.to_str()) {
                    if supported_extensions.contains(&ext.to_lowercase().as_str()) {
                        files.push(entry.path().to_path_buf());
                    }
                }
            }
        }
    } else {
        return Err(AppError::InvalidInputPath(path.to_path_buf()));
    }
    Ok(files)
}

/// Determines the output path for a generated HTML plot.
///
/// If an output directory is specified via CLI arguments, the plot is saved inside that
/// directory with the name `<input_stem>.html`.
/// Otherwise, it is saved next to the input file with the same name.
///
/// # Arguments
///
/// * `input_path` - The path of the original input file.
/// * `cli` - A reference to the parsed command-line arguments.
///
/// # Returns
///
/// A `PathBuf` representing the full path for the output HTML file.
fn generate_output_path(input_path: &Path, cli: &Cli) -> std::path::PathBuf {
    let stem = input_path.file_stem().unwrap_or_default().to_string_lossy();
    let default_output_name = format!("{}.html", stem);

    if let Some(output_dir) = &cli.output {
        output_dir.join(default_output_name)
    } else {
        // Default to saving next to the input file
        input_path.with_file_name(default_output_name)
    }
}
