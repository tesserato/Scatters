use crate::cli::Cli;
use crate::error::AppError;
use polars::prelude::*;
use std::path::Path;

/// Holds the prepared data series ready for plotting.
pub struct PlotData {
    pub title: String,
    pub x_series: Series,
    pub y_series_list: Vec<Series>,
    pub autoscale_y: bool,
    pub animations: bool,
}

/// Selects the X and Y series from a DataFrame based on user preferences.
pub fn prepare_plot_data(df: DataFrame, cli: &Cli, file_path: &Path) -> Result<PlotData, AppError> {
    // 1. Determine the X-axis (index) series based on priority
    let (x_series, x_name) = select_x_series(&df, cli)?;

    // 2. Determine the Y-axis series
    let y_series_list = select_y_series(&df, cli, &x_name)?;

    // 3. Determine the plot title
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
        animations: !cli.no_animations,
    })
}

fn select_x_series(df: &DataFrame, cli: &Cli) -> Result<(Series, String), AppError> {
    // Priority 1: --index flag
    if let Some(index_name) = &cli.index {
        let series = df
            .column(index_name)
            .map_err(|_| AppError::ColumnNotFound(index_name.clone()))?
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
            .clone();
        let name = series.name().to_string();
        return Ok((series, name));
    }

    // Priority 3: Audio-friendly default — use 'sample_index' if present
    if df.get_column_names().iter().any(|&n| n == "sample_index") {
        let series = df
            .column("sample_index")
            .map_err(|_| AppError::ColumnNotFound("sample_index".to_string()))?
            .clone();
        return Ok((series, "sample_index".to_string()));
    }

    // Priority 4: Auto-detect first datetime column
    for series in df.get_columns() {
        if matches!(series.dtype(), DataType::Datetime(_, _) | DataType::Date) {
            let name = series.name().to_string();
            return Ok((series.clone(), name));
        }
    }

    // Priority 5: Fallback to row numbers
    println!("  -> Warning: No index specified and no datetime column found. Using row numbers as index.");
    let row_count = df.height() as u32;
    let series = Series::new("row_index", (0..row_count).collect::<Vec<u32>>());
    Ok((series, "row_index".to_string()))
}

fn select_y_series(df: &DataFrame, cli: &Cli, x_name: &str) -> Result<Vec<Series>, AppError> {
    let mut y_series_list = Vec::new();

    // Case 1: --columns flag is used
    if let Some(columns) = &cli.columns {
        for col_name in columns {
            let series = df
                .column(col_name)
                .map_err(|_| AppError::ColumnNotFound(col_name.clone()))?
                .clone();
            y_series_list.push(series);
        }
    }
    // Case 2: Default - use all numeric columns
    else {
        for series in df.get_columns() {
            if series.name() != x_name && series.dtype().is_numeric() {
                y_series_list.push(series.clone());
            }
        }
    }

    if y_series_list.is_empty() {
        Err(AppError::NoNumericColumns)
    } else {
        Ok(y_series_list)
    }
}
