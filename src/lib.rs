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

/// Main library entry point.
pub fn run(cli: &Cli) -> Result<()> {
    let input_path = &cli.input_path;

    // Discover files to process
    let files_to_process = find_supported_files(input_path)?;
    if files_to_process.is_empty() {
        println!("No supported files found in the specified path.");
        return Ok(());
    }

    println!("Found {} files to process...", files_to_process.len());

    // Process each file
    for file_path in files_to_process {
        println!("Processing '{}'...", file_path.display());
        process_single_file(&file_path, cli)
            .with_context(|| format!("Failed to process file: {}", file_path.display()))?;
    }

    println!("Done.");
    Ok(())
}

/// Orchestrates the loading, processing, and plotting for a single file.
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

/// Finds all supported files in the given path (file or directory).
fn find_supported_files(path: &Path) -> Result<Vec<std::path::PathBuf>, AppError> {
    let mut files = Vec::new();
    let supported_extensions: Vec<&str> = vec![
        "csv", "parquet", "json", "jsonl", "ndjson", "xlsx", "xls", "wav", "mp3", "flac",
    ];

    if path.is_file() {
        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
            if supported_extensions.contains(&ext) {
                files.push(path.to_path_buf());
            }
        }
    } else if path.is_dir() {
        for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                if let Some(ext) = entry.path().extension().and_then(|s| s.to_str()) {
                    if supported_extensions.contains(&ext) {
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

/// Generates a unique output path for each input file.
fn generate_output_path(input_path: &Path, cli: &Cli) -> std::path::PathBuf {
    let stem = input_path.file_stem().unwrap_or_default().to_string_lossy();
    let default_output_name = format!("{}.html", stem);

    if let Some(output) = &cli.output {
        output.join(default_output_name)
    } else {
        // Default to saving next to the input file
        input_path.with_file_name(default_output_name)
    }
}
