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
    /// A list of series to plot, each as a (name, x_series, y_series) tuple.
    pub series_list: Vec<(String, Series, Series)>,
    /// The special string used to identify vertical markers.
    pub special_marker: String,
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
    /// True if any series was downsampled.
    pub downsampled: bool,
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
    // 1. Determine the X-axis (index) series based on priority.
    let (x_series, x_name) = select_x_series(&df, cli)?;

    if cli.debug {
        println!(
            "  -> Selected X-axis column: '{}' with {} values",
            x_name,
            x_series.len()
        );
    }

    // 2. Determine the Y-axis series.
    let y_series_list = select_y_series(&df, cli, &x_name)?;

    let mut final_series_list = Vec::new();
    let mut downsampled = false;

    // 3. Process each series, applying downsampling if necessary.
    for y_series in y_series_list {
        let y_name = y_series.name().to_string();
        if let Some(threshold) = cli.downsample {
            if y_series.len() > threshold {
                println!(
                    "  -> Downsampling '{}' from {} to {} points...",
                    y_name,
                    y_series.len(),
                    threshold
                );
                let (ds_x, ds_y) = downsample_series(&x_series, &y_series, threshold);
                final_series_list.push((y_name, ds_x, ds_y));
                downsampled = true;
                continue;
            }
        }
        // If not downsampling, use the original series.
        final_series_list.push((y_name, x_series.clone(), y_series));
    }

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
        series_list: final_series_list,
        special_marker: cli.special_marker.clone(),
        autoscale_y: !cli.no_autoscale_y,
        animations: cli.animations,
        max_decimals: cli.max_decimals,
        use_white_theme: cli.white_theme,
        large_mode_threshold: cli.large_mode_threshold,
        downsampled,
    })
}

/// Downsamples a pair of X/Y series using the LTTB algorithm.
///
/// Note: This converts the data to `f64` for processing, so original types like
/// Datetime are lost and become numeric representations (e.g., milliseconds).
fn downsample_series(x_series: &Series, y_series: &Series, threshold: usize) -> (Series, Series) {
    let points: Vec<lttb::DataPoint> = x_series
        .iter()
        .zip(y_series.iter())
        .filter_map(|(x_val, y_val)| {
            if let (Some(x), Some(y)) = (any_value_to_f64(&x_val), any_value_to_f64(&y_val)) {
                Some(lttb::DataPoint::new(x, y))
            } else {
                None
            }
        })
        .collect();

    if points.is_empty() {
        return (
            Series::new_empty(x_series.name().clone(), &DataType::Float64),
            Series::new_empty(y_series.name().clone(), &DataType::Float64),
        );
    }

    let downsampled_points = lttb::lttb(points, threshold);

    let mut x_builder = PrimitiveChunkedBuilder::<Float64Type>::new(
        "x_downsampled".into(),
        downsampled_points.len(),
    );
    let mut y_builder = PrimitiveChunkedBuilder::<Float64Type>::new(
        "y_downsampled".into(),
        downsampled_points.len(),
    );

    for p in downsampled_points {
        x_builder.append_value(p.x);
        y_builder.append_value(p.y);
    }

    (
        x_builder.finish().into_series(),
        y_builder.finish().into_series(),
    )
}

/// Safely check a string series for any values containing the special marker.
/// Returns true if the marker is found.
fn check_string_series_for_marker(series: &Series, cli: &Cli) -> bool {
    for av in series.iter() {
        match av {
            AnyValue::String(s) => {
                if s.trim() == cli.special_marker {
                    return true;
                }
            }
            AnyValue::StringOwned(s) => {
                if s.trim() == cli.special_marker {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
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
///     String columns containing the special marker are also included.
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
                    check_string_series_for_marker(series, cli)
                } else {
                    is_numeric
                };

                if should_include {
                    if cli.debug {
                        println!(
                            "  -> Including Y-axis column '{}' ({} values)",
                            series.name(),
                            series.len()
                        );
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

/// Converts a Polars `AnyValue` to an `Option<f64>`.
///
/// This helper is used for calculating min/max ranges for axes and for downsampling.
/// It returns `Some(f64)` for numeric, date, and datetime types, and `None` for all others.
/// Dates and datetimes are converted to milliseconds since the epoch as a float.
pub fn any_value_to_f64(av: &AnyValue) -> Option<f64> {
    match av {
        AnyValue::String(s) => s.trim().replace(',', "").parse::<f64>().ok(),
        AnyValue::StringOwned(s) => s.trim().replace(',', "").parse::<f64>().ok(),
        AnyValue::UInt8(v) => Some(*v as f64),
        AnyValue::UInt16(v) => Some(*v as f64),
        AnyValue::UInt32(v) => Some(*v as f64),
        AnyValue::UInt64(v) => Some(*v as f64),
        AnyValue::Int8(v) => Some(*v as f64),
        AnyValue::Int16(v) => Some(*v as f64),
        AnyValue::Int32(v) => Some(*v as f64),
        AnyValue::Int64(v) => Some(*v as f64),
        AnyValue::Float32(v) => Some(*v as f64),
        AnyValue::Float64(v) => Some(*v),
        AnyValue::Date(days) => Some((*days as i64 as f64) * 86_400_000.0),
        AnyValue::Datetime(v, unit, _) => Some(match unit {
            polars::prelude::TimeUnit::Nanoseconds => (*v as f64) / 1_000_000.0,
            polars::prelude::TimeUnit::Microseconds => (*v as f64) / 1_000.0,
            polars::prelude::TimeUnit::Milliseconds => *v as f64,
        }),
        _ => None,
    }
}
