# WARP.md

This file provides guidance to WARP (warp.dev) when working with code in this repository.

## Project Overview

Data Plotter is a Rust CLI tool that generates interactive scatter plots from various data formats (CSV, Parquet, JSON, JSONL, and audio files like WAV/MP3/FLAC). It outputs self-contained HTML files with ECharts-powered visualizations.

## Common Commands

### Building and Development
```powershell
# Build the project
cargo build

# Build and run with arguments
cargo run -- <input_path> [options]

# Build release version
cargo build --release

# Run tests
cargo test

# Check code without building
cargo check

# Format code
cargo fmt

# Run clippy for lints
cargo clippy
```

### Running the Tool
```powershell
# Basic usage - plot all numeric columns from a file
cargo run -- sample/sample.csv

# Specify output directory
cargo run -- sample/sample.csv -o plots/

# Use custom index column
cargo run -- data.csv --index timestamp

# Plot specific columns
cargo run -- data.csv -c sensor_a,sensor_b

# Use first column as index with custom title
cargo run -- data.csv --use-first-column --title "My Custom Plot"
```

### Testing with Sample Data
```powershell
# Test with provided sample CSV
cargo run -- sample/sample.csv

# The sample contains timestamp, sensor_a, and sensor_b columns
```

## Code Architecture

### Core Flow
1. **CLI Parsing** (`cli.rs`) - Uses clap to define command-line interface
2. **File Discovery** (`lib.rs`) - Recursively finds supported files (CSV, Parquet, JSON, audio)
3. **Data Loading** (`data_loader.rs`) - Loads files into Polars DataFrames, with special handling for audio files using Symphonia
4. **Data Processing** (`processing.rs`) - Selects X/Y series based on user preferences with intelligent fallbacks
5. **Plot Generation** (`plotter.rs`) - Creates self-contained HTML with embedded ECharts JavaScript
6. **Output** - Saves HTML files alongside input files or in specified directory

### Key Components

**Main Library (`lib.rs`)**
- Entry point function `run()` orchestrates the entire pipeline
- File discovery supports multiple formats: CSV, Parquet, JSON/JSONL, WAV/MP3/FLAC
- Processes files in batch, creating one HTML plot per input file

**Data Loading (`data_loader.rs`)**
- Uses Polars for structured data (CSV, Parquet, JSON)
- Uses Symphonia for audio files, converting to amplitude over sample index
- Audio files become DataFrames with `sample_index` and `amplitude` columns

**Processing Logic (`processing.rs`)**
- **X-axis selection priority**: 1) `--index` flag, 2) `--use-first-column`, 3) auto-detect datetime columns, 4) fallback to row numbers
- **Y-axis selection**: Uses specified columns via `--columns` or defaults to all numeric columns (excluding X-axis)
- Returns `PlotData` struct with title, x_series, and y_series_list

**Plot Generation (`plotter.rs`)**
- Uses hypertext crate for type-safe HTML generation
- Embeds ECharts from CDN with scatter plot configuration
- Includes interactive features: zoom, pan, toolbox, legend scrolling
- Converts Polars data to JSON format suitable for ECharts

**Error Handling (`error.rs`)**
- Custom error types using thiserror for different failure modes
- Covers I/O, data processing, audio decoding, and user input errors

### Dependencies Strategy
- **Polars**: High-performance DataFrame operations with lazy evaluation features
- **Symphonia**: Comprehensive audio format support (WAV, MP3, FLAC, etc.)
- **Clap**: Modern CLI with derive macros for type-safe argument parsing
- **Hypertext**: Type-safe HTML generation using maud-like syntax
- **Anyhow/Thiserror**: Ergonomic error handling patterns

### Supported File Formats
- **Structured Data**: CSV, Parquet, JSON, JSONL/NDJSON
- **Audio Files**: WAV, MP3, FLAC (converted to amplitude plots)
- **Output**: Self-contained HTML with embedded JavaScript (no external dependencies)

## File Structure
```
src/
├── main.rs          # Binary entry point
├── lib.rs           # Main library logic and file discovery
├── cli.rs           # Command-line interface definition
├── data_loader.rs   # File loading (structured data + audio)
├── processing.rs    # Data preparation and series selection
├── plotter.rs       # HTML generation with ECharts
└── error.rs         # Custom error types
```
