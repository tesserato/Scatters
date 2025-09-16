//! Defines the command-line interface for the application using `clap`.
//!
//! This module contains the `Cli` struct, which is derived from `clap::Parser`.
//! Each field in the struct corresponds to a command-line argument, option, or flag.
//! The documentation comments on each field are used by `clap` to generate
//! the help messages (`--help`).

use clap::Parser;
use std::path::PathBuf;

/// A tool to generate interactive scatter plots from various data formats.
#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "A tool to generate interactive scatter plots from various data formats."
)]
pub struct Cli {
    /// The input file or folder to scan for data.
    #[arg(required = true)]
    pub input_path: PathBuf,

    /// Directory to save the generated HTML plots.
    /// Defaults to saving next to each input file.
    #[arg(short = 'o', long = "output-dir")]
    pub output: Option<PathBuf>,

    /// Name of the column to use as the index (X-axis).
    /// This has the highest priority for index selection.
    #[arg(short = 'i', long)]
    pub index: Option<String>,

    /// Use the first column of the data as the index (X-axis).
    /// This is overridden by the --index option if both are provided.
    #[arg(short = 'f', long, default_value_t = false)]
    pub use_first_column: bool,

    /// Comma-separated list of columns to plot (Y-axis).
    /// If not provided, all numeric columns will be plotted.
    #[arg(short = 'c', long, use_value_delimiter = true, value_delimiter = ',')]
    pub columns: Option<Vec<String>>,

    /// A custom title for the plot.
    /// Defaults to the input filename.
    #[arg(short = 't', long)]
    pub title: Option<String>,

    /// Downsample series with more than N points using the LTTB algorithm to preserve visual features.
    /// If not provided, no downsampling is performed.
    #[arg(short = 'd', long = "downsample-threshold", default_value_t = 10000)]
    pub downsample_threshold:usize,

    /// Disable dynamic Y-axis autoscaling on zoom.
    /// When disabled, the Y-axis keeps its initial, globally-padded range.
    #[arg(short = 'n', long, default_value_t = false)]
    pub no_autoscale_y: bool,

    /// Enable ECharts animations for a more dynamic feel.
    /// Animations are disabled by default for performance.
    #[arg(short = 'a', long, default_value_t = false)]
    pub animations: bool,

    /// Maximum number of decimal places for numeric formatting in tooltips.
    /// Use -1 for an unlimited number of decimal places.
    #[arg(short = 'm', long = "max-decimals", default_value_t = 2)]
    pub max_decimals: i32,

    /// The character (or string) to recognize as a vertical marker in string columns.
    #[arg(short = 'M', long = "special-marker", default_value_t = String::from("|"))]
    pub vertical_marker: String,

    /// Threshold for ECharts `large` mode. Series with more points than this will be optimized for performance, which may reduce detail.
    #[arg(short = 'l', long = "large-mode-threshold", default_value_t = 2000)]
    pub large_mode_threshold: usize,

    /// Print debug information during processing.
    /// This includes detected columns, data types, and DataFrame shape.
    #[arg(short = 'D', long, default_value_t = false)]
    pub debug: bool,

    /// Use a white (light) theme for the plot instead of the default dark theme.
    #[arg(short = 'w', long = "white-theme", default_value_t = false)]
    pub white_theme: bool,
}