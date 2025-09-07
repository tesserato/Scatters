use clap::Parser;
use std::path::PathBuf;

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
    #[arg(short, long)]
    pub output_dir: Option<PathBuf>,

    /// Name of the column to use as the index (X-axis).
    /// Highest priority for index selection.
    #[arg(long)]
    pub index: Option<String>,

    /// Use the first column of the data as the index.
    /// Overridden by --index.
    #[arg(long, default_value_t = false)]
    pub use_first_column: bool,

    /// Comma-separated list of columns to plot (Y-axis).
    /// If not provided, all numeric columns will be plotted.
    #[arg(short, long, use_value_delimiter = true, value_delimiter = ',')]
    pub columns: Option<Vec<String>>,

    /// A custom title for the plot.
    /// Defaults to the input filename.
    #[arg(long)]
    pub title: Option<String>,

    /// Disable dynamic Y-axis autoscaling (keep initial 10% padded range)
    #[arg(long, default_value_t = false)]
    pub no_autoscale_y: bool,

    /// Enable ECharts animations
    #[arg(long, default_value_t = false)]
    pub animations: bool,

    /// Print debug info about detected columns and types
    #[arg(long, default_value_t = false)]
    pub debug: bool,
}
