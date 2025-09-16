//! This module handles the logic for selecting and preparing data for plotting.
//!
//! It takes a raw DataFrame and the parsed command-line arguments to determine
//! which column should be used for the X-axis and which columns for the Y-axis.
//! It also resolves the plot title and other plot-specific configurations.

use crate::cli::Cli;
use crate::error::AppError;
use polars::prelude::*;
use std::path::Path;

/// A container for all the data and configuration needed to generate a plot.
///
/// This struct is the output of the `prepare_plot_data` function and serves as the
/// input for the `plotter` module.
pub struct PlotData {
    /// The title of the plot.
    pub title: String,
    /// The Polars `Series` to be used for the X-axis.
    pub x_series: Series,
    /// A list of Polars `Series` to be plotted on the Y-axis.
    pub y_series_list: Vec<Series>,
    /// Whether to enable dynamic Y-axis rescaling on zoom.
    pub autoscale_y: bool,
    /// Whether to enable ECharts animations.
    pub animations: bool,
    /// The maximum number of decimal places for numeric tooltips.
    pub max_decimals: i32,
    /// Whether to use the white (light) theme.
    pub use_white_theme: bool,
    /// The threshold for enabling ECharts' high-performance `large` mode.
    pub large_mode_threshold: usize,
}

/// Selects the X and Y series from a DataFrame and packages them for plotting.
///
/// This function encapsulates the core logic for interpreting user intent from the CLI
/// arguments and applying it to the loaded data.
///
/// # Arguments
///
/// * `df` - The input `DataFrame` loaded from a file.
/// * `cli` - A reference to the parsed command-line arguments (`Cli` struct).
/// * `file_path` - The path of the input file, used to generate a default title.
///
/// # Returns
///
/// A `Result` containing a `PlotData` struct ready for the plotting engine,
/// or an `AppError` if an appropriate X or Y series cannot be determined.
pub fn prepare_plot_data(df: DataFrame, cli: &Cli, file_path: &Path) -> Result<PlotData, AppError> {
    // 1. Print DataFrame info if debug is enabled
    if cli.debug {
        println!("  -> DataFrame shape: {} rows × {} columns", df.height(), df.width());
        for col in df.get_columns() {
            println!("  -> Column '{}': {} values, dtype: {}, non-null: {}", 
                col.name(), col.len(), col.dtype(), col.len() - col.null_count());
        }
    }

    // 2. Determine the X-axis (index) series based on priority.
    let (x_series, x_name) = select_x_series(&df, cli)?;
    
    if cli.debug {
        println!("  -> Selected X-axis column: '{}' with {} values", x_name, x_series.len());
    }

    // 3. Determine the Y-axis series.
    let y_series_list = select_y_series(&df, cli, &x_name)?;

    // 4. Determine the plot title.
    let title = cli.title.clone().unwrap_or_else(|| {
        file_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    });

    Ok(PlotData {
        title,
        x_series,
        y_series_list,
        autoscale_y: !cli.no_autoscale_y,
        animations: cli.animations,
        max_decimals: cli.max_decimals,
        use_white_theme: cli.white_theme,
        large_mode_threshold: cli.large_mode_threshold,
    })
}

/// Safely check a string series for any values containing the `|` marker.
/// Returns false if series iteration fails or length mismatches.
fn check_string_series_for_pipe(series: &Series, cli: &Cli) -> bool {
    let mut count = 0;
    let len = series.len();
    let mut has_pipe = false;

    // Only process if there are enough non-null values
    let non_null_count = len - series.null_count();
    if non_null_count < 1 || (non_null_count as f64 / len as f64) < 0.5 {
        if cli.debug {
            println!("  -> Warning: Skipping mostly null column '{}'", series.name());
        }
        return false;
    }

    // Count values and look for pipes
    for (i, av) in series.iter().enumerate() {
        count = i + 1;
        if cli.debug && i < 5 {
            println!("  -> Value [{}] in '{}': {:?}", i, series.name(), av);
        }
        match av {
            AnyValue::String(s) => {
                if s.trim() == "|" {
                    has_pipe = true;
                    break;
                }
            }
            AnyValue::StringOwned(s) => {
                if s.trim() == "|" {
                    has_pipe = true;
                    break;
                }
            }
            _ => {}
        }
    }

    // Check for length mismatch
    if count != len {
        if cli.debug {
            println!("  -> Warning: Iterator length mismatch in '{}': expected {}, got {}",
                series.name(), len, count);
        }
        false
    } else {
        has_pipe
    }
}

/// Selects the X-axis series based on a predefined priority order.
///
/// The selection priority is as follows:
/// 1.  The column specified by the `--index` flag.
/// 2.  The first column of the DataFrame if `--use-first-column` is specified.
/// 3.  A column named `sample_index` (common for audio data).
/// 4.  The first `Datetime` or `Date` column found.
/// 5.  A fallback generated series of row numbers named `row_index`.
///
/// # Returns
///
/// A tuple containing the selected `Series` and its name.
fn select_x_series(df: &DataFrame, cli: &Cli) -> Result<(Series, String), AppError> {
    // Priority 1: --index flag
    if let Some(index_name) = &cli.index {
        let series = df
            .column(index_name)
            .map_err(|_| AppError::ColumnNotFound(index_name.clone()))?
            .as_series()
            .unwrap()
            .clone();
        return Ok((series, index_name.clone()));
    }

    // Priority 2: --use-first-column flag
    if cli.use_first_column {
        let series = df
            .get_columns()
            .get(0)
            .ok_or(AppError::Polars(PolarsError::NoData(
                "DataFrame is empty".into(),
            )))?
            .as_series()
            .unwrap()
            .clone();
        let name = series.name().to_string();
        return Ok((series, name));
    }

    // Priority 3: Audio-friendly default — use 'sample_index' if present.
    if df.get_column_names().iter().any(|&n| n == "sample_index") {
        let series = df.column("sample_index")?.as_series().unwrap().clone();
        return Ok((series, "sample_index".to_string()));
    }

    // Priority 4: Auto-detect first datetime column.
    for column in df.get_columns() {
        if matches!(column.dtype(), DataType::Datetime(_, _) | DataType::Date) {
            // Skip columns that are entirely null after casting attempts.
            if column.null_count() < column.len() {
                let name = column.name().to_string();
                return Ok((column.as_series().unwrap().clone(), name));
            }
        }
    }

    // Priority 5: Fallback to row numbers.
    println!("  -> Warning: No index specified and no datetime column found. Using row numbers as index.");
    let row_count = df.height() as u32;
    let series = Series::new("row_index".into(), (0..row_count).collect::<Vec<u32>>());
    Ok((series, "row_index".to_string()))
}

/// Selects the Y-axis series to be plotted.
///
/// Two main cases are handled:
/// 1.  If the `--columns` flag is provided, only the specified columns are used.
/// 2.  Otherwise, all numeric columns (excluding the selected X-axis column) are used.
///     String columns containing the special `|` marker are also included.
///
/// # Errors
///
/// Returns `AppError::NoNumericColumns` if no suitable Y-axis columns can be found.
fn select_y_series(df: &DataFrame, cli: &Cli, x_name: &str) -> Result<Vec<Series>, AppError> {
    let mut y_series_list: Vec<Series> = Vec::new();

    if cli.debug {
        println!("  -> Scanning columns for Y-axis data...");
    }

    // Case 1: --columns flag is used.
    if let Some(columns) = &cli.columns {
        for col_name in columns {
            if cli.debug {
                println!("  -> Processing specified column '{}'", col_name);
            }

            let series = df
                .column(col_name)
                .map_err(|_| AppError::ColumnNotFound(col_name.clone()))?
                .as_series()
                .unwrap()
                .clone();
            y_series_list.push(series);
        }
    }
    // Case 2: Default - use all numeric columns and special string columns.
    else {
        for column in df.get_columns() {
            if column.name() != x_name {
                let is_numeric = column.dtype().is_numeric();
                let series = column.as_series().unwrap();

                let should_include = if let DataType::String = column.dtype() {
                    check_string_series_for_pipe(&series, cli)
                } else {
                    is_numeric
                };

                if should_include {
                    if cli.debug {
                        println!("  -> Including Y-axis column '{}' ({} values)", series.name(), series.len());
                    }
                    y_series_list.push(series.clone());
                } else if cli.debug {
                    println!("  -> Skipping column '{}'", series.name());
                }
            }
        }
    }

    if y_series_list.is_empty() {
        Err(AppError::NoNumericColumns)
    } else {
        Ok(y_series_list)
    }
}