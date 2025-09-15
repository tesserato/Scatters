//! This module handles loading data from various file formats into Polars DataFrames.
//!
//! It supports common tabular formats like CSV, Parquet, JSON Lines, and Excel,
//! as well as audio formats like WAV, MP3, and FLAC. The module also includes
//! logic for automatic type inference and casting, such as converting string columns
//! that appear to be numeric or datetime values into their proper types.

use crate::error::AppError;
use polars::prelude::*;
use std::fs::File;
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

/// Loads a supported file into a Polars DataFrame.
///
/// This function inspects the file extension to determine the appropriate loader.
/// After initial loading, it attempts to perform automatic type coercion:
/// 1.  String columns that look entirely numeric are cast to `Float64`.
/// 2.  Remaining string columns that resemble datetime formats are cast to `Datetime`.
///
/// # Arguments
///
/// * `path` - A reference to the path of the file to load.
///
/// # Returns
///
/// A `Result` containing the loaded `DataFrame` on success, or an `AppError`
/// if the file format is unsupported, an I/O error occurs, or parsing fails.
pub fn load_dataframe(path: &Path) -> Result<DataFrame, AppError> {
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_lowercase();

    let mut df = match extension.as_str() {
        "csv" => {
            let file = File::open(path)?;
            CsvReader::new(file).finish().map_err(AppError::from)?
        }
        "parquet" => ParquetReader::new(File::open(path)?)
            .finish()
            .map_err(AppError::from)?,
        "json" | "jsonl" | "ndjson" => {
            let file = File::open(path)?;
            JsonReader::new(file)
                .with_json_format(JsonFormat::JsonLines)
                .finish()
                .map_err(AppError::from)?
        }
        "xlsx" | "xls" => load_excel_dataframe(path)?,
        "wav" | "mp3" | "flac" | "ogg" | "m4a" | "aac" => return load_audio_dataframe(path),
        _ => {
            return Err(AppError::UnsupportedFormat(
                path.to_string_lossy().to_string(),
            ))
        }
    };

    // First, try to coerce string columns that look numeric into Float64.
    // This prevents purely numeric IDs from being misinterpreted as dates.
    try_cast_string_columns_to_numeric(&mut df)?;
    // Next, attempt to auto-coerce remaining string columns that look like datetimes.
    try_cast_string_columns_to_datetime(&mut df)?;

    Ok(df)
}

/// Attempts to cast string columns to `Datetime` if they match common date/time formats.
///
/// This function iterates through string columns and applies two parsing strategies:
/// 1.  Polars' native `cast`, which handles standard formats like RFC3339.
/// 2.  A custom heuristic parser (`parse_string_series_to_datetime`) for other common formats.
///
/// A column is only converted if at least 90% of its non-null values can be successfully parsed,
/// preventing accidental conversion of columns with only a few date-like strings.
fn try_cast_string_columns_to_datetime(df: &mut DataFrame) -> Result<(), AppError> {
    let col_names: Vec<String> = df
        .get_columns()
        .iter()
        .map(|s| s.name().to_string())
        .collect();

    for name in col_names {
        let s = df.column(&name)?.clone();
        if matches!(s.dtype(), DataType::String) {
            let mut accepted = false;
            // Strategy 1: native cast
            if let Ok(parsed) = s.cast(&DataType::Datetime(TimeUnit::Milliseconds, None)) {
                let original_non_nulls = s.len() - s.null_count();
                let parsed_non_nulls = parsed.len() - parsed.null_count();

                // Only accept if the cast is highly successful.
                if original_non_nulls > 0 && parsed_non_nulls * 10 >= original_non_nulls * 9 {
                    df.replace(&name, parsed).map_err(AppError::from)?;
                    accepted = true;
                }
            }
            // Strategy 2: heuristic formats
            if !accepted {
                if let Some(series_dt) = parse_string_series_to_datetime(&s) {
                    df.replace(&name, series_dt).map_err(AppError::from)?;
                }
            }
        }
    }
    Ok(())
}

/// Parses a string `Series` into a `Datetime` `Series` using a variety of format heuristics.
///
/// This function builds a list of common date and datetime format strings (e.g., `YYYY-MM-DD`,
/// `DD/MM/YYYY HH:MM:SS`) and attempts to parse each string value. If successful for a high
/// percentage of values, it returns a new `Series` of type `Datetime`.
fn parse_string_series_to_datetime(s: &Series) -> Option<Series> {
    // Generate candidate format strings
    let seps: &[char] = &['-', '/', '.', ' '];
    let dt_seps: &[&str] = &["T", " "];
    let mut fmts: Vec<String> = Vec::new();
    for &sep in seps {
        let sep_s = sep.to_string();
        let ymd = format!("%Y{sep}%m{sep}%d", sep = sep_s);
        let dmy = format!("%d{sep}%m{sep}%Y", sep = sep_s);
        // date-only
        fmts.push(ymd.clone());
        fmts.push(dmy.clone());
        for &dts in dt_seps {
            // time variants
            for t in ["%H", "%H:%M", "%H:%M:%S"].iter() {
                fmts.push(format!("{}{}{}", ymd, dts, t));
                fmts.push(format!("{}{}{}", dmy, dts, t));
            }
        }
    }

    // Helper to normalize whitespace (trim and collapse runs).
    fn normalize_spaces(input: &str) -> std::borrow::Cow<'_, str> {
        let trimmed = input.trim();
        if trimmed.contains(char::is_whitespace) {
            let s = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
            std::borrow::Cow::Owned(s)
        } else {
            std::borrow::Cow::Borrowed(trimmed)
        }
    }

    // Try parsing using all generated formats; collect milliseconds since epoch.
    let mut parsed: Vec<Option<i64>> = Vec::with_capacity(s.len());
    for av in s.iter() {
        let ms_opt = match av {
            AnyValue::String(v) => {
                let norm = normalize_spaces(v);
                try_parse_many(&norm, &fmts)
            }
            AnyValue::StringOwned(ref v) => {
                let norm = normalize_spaces(v);
                try_parse_many(&norm, &fmts)
            }
            _ => None,
        };
        parsed.push(ms_opt);
    }

    // Apply the 90% threshold for conversion.
    let parsed_non_nulls = parsed.iter().filter(|v| v.is_some()).count();
    let original_non_nulls = s.len() - s.null_count();

    if original_non_nulls > 0 && parsed_non_nulls * 10 >= original_non_nulls * 9 {
        // Build series -> Int64 -> Datetime(ms)
        let ca: Int64Chunked = parsed.into_iter().collect();
        let series = ca.into_series();
        series
            .cast(&DataType::Datetime(TimeUnit::Milliseconds, None))
            .ok()
    } else {
        None
    }
}

/// Helper function to parse a single string with multiple `chrono` format strings.
fn try_parse_many(s: &str, fmts: &[String]) -> Option<i64> {
    use chrono::{NaiveDate, NaiveDateTime};
    // Try datetime formats first
    for f in fmts {
        // If format contains any time specifier, use NaiveDateTime; otherwise NaiveDate
        let has_time = f.contains("%H");
        if has_time {
            if let Ok(dt) = NaiveDateTime::parse_from_str(s, f) {
                let secs = dt.and_utc().timestamp();
                let nanos = dt
                    .and_utc()
                    .timestamp_nanos_opt()
                    .unwrap_or(secs * 1_000_000_000);
                return Some((nanos / 1_000_000) as i64);
            }
        } else {
            if let Ok(d) = NaiveDate::parse_from_str(s, f) {
                let dt = d.and_hms_opt(0, 0, 0)?;
                let secs = dt.and_utc().timestamp();
                return Some(secs * 1000);
            }
        }
    }
    None
}

/// Attempts to cast string columns to `Float64` if they appear to be numeric.
///
/// A column is converted only if *all* of its non-null string values can be successfully
/// parsed as a float. This strict rule helps avoid incorrectly converting mixed-type columns.
/// It also specifically skips columns containing the `|` character, which is reserved
/// for creating vertical marker lines in the plot.
fn try_cast_string_columns_to_numeric(df: &mut DataFrame) -> Result<(), AppError> {
    let col_names: Vec<String> = df
        .get_columns()
        .iter()
        .map(|s| s.name().to_string())
        .collect();

    for name in col_names {
        let s = df.column(&name)?.clone();
        if matches!(s.dtype(), DataType::String) {
            // Check if the column contains the `|` symbol, used as a special marker.
            let contains_pipe = s.iter().any(|av| match av {
                AnyValue::String(t) => t.trim() == "|",
                AnyValue::StringOwned(ref t) => t.trim() == "|",
                _ => false,
            });

            if contains_pipe {
                continue; // Skip numeric conversion for this column.
            }

            // Manually trim and parse floats from string values.
            let parsed_vals: Vec<Option<f64>> = s
                .iter()
                .map(|av| match av {
                    AnyValue::String(t) => t.trim().parse::<f64>().ok(),
                    AnyValue::StringOwned(ref t) => t.trim().parse::<f64>().ok(),
                    _ => None,
                })
                .collect();
            let parsed_series = Series::new(&name, parsed_vals);

            // Only replace if all non-null string values were successfully parsed as numeric.
            let original_non_nulls = s.len() - s.null_count();
            let parsed_numeric_count = parsed_series.len() - parsed_series.null_count();
            if original_non_nulls > 0 && parsed_numeric_count == original_non_nulls {
                df.replace(&name, parsed_series).map_err(AppError::from)?;
            }
        }
    }
    Ok(())
}

/// Loads the first worksheet of an Excel file (`.xlsx`, `.xls`) into a DataFrame.
///
/// Uses the `calamine` crate to read the Excel data. It auto-detects the header row
/// by skipping initial empty rows. All data is initially read as strings and then
/// passed through the same type inference pipeline as other file formats.
fn load_excel_dataframe(path: &Path) -> Result<DataFrame, AppError> {
    use calamine::{open_workbook_auto, DataType as Xl, Reader};

    let mut workbook = open_workbook_auto(path)?;
    let sheet_name = workbook
        .sheet_names()
        .get(0)
        .cloned()
        .ok_or_else(|| AppError::UnsupportedFormat(path.to_string_lossy().to_string()))?;

    let range = workbook
        .worksheet_range(&sheet_name)
        .ok_or_else(|| AppError::UnsupportedFormat(path.to_string_lossy().to_string()))??;

    // Collect all rows to find the header index and maximum column count.
    let rows: Vec<Vec<Xl>> = range.rows().map(|r| r.to_vec()).collect();
    // Find first non-empty row to use as the header.
    let mut header_idx: Option<usize> = None;
    for (i, r) in rows.iter().enumerate() {
        if !r.iter().all(|c| matches!(c, Xl::Empty)) {
            header_idx = Some(i);
            break;
        }
    }
    let header_idx = header_idx
        .ok_or_else(|| AppError::UnsupportedFormat(path.to_string_lossy().to_string()))?;

    // Determine column count from the widest row.
    let col_count = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if col_count == 0 {
        return Err(AppError::UnsupportedFormat(
            path.to_string_lossy().to_string(),
        ));
    }

    // Build header names from the identified header row.
    let mut headers: Vec<String> = Vec::with_capacity(col_count);
    for i in 0..col_count {
        let name = rows
            .get(header_idx)
            .and_then(|r| r.get(i))
            .map(|c| c.to_string())
            .unwrap_or_default();
        let final_name = if name.trim().is_empty() {
            format!("col_{}", i + 1)
        } else {
            name.trim().to_string()
        };
        headers.push(final_name);
    }

    // Initialize column vectors to store data as strings.
    let mut columns: Vec<Vec<Option<String>>> = vec![Vec::new(); col_count];

    // Populate columns with data rows (all converted to strings).
    for (ri, row) in rows.iter().enumerate() {
        if ri <= header_idx {
            continue; // Skip header and any rows above it.
        }
        for ci in 0..col_count {
            let val_str_opt: Option<String> = row.get(ci).and_then(|c| match c {
                Xl::Empty | Xl::Error(_) => None,
                _ => Some(c.to_string()),
            });
            columns[ci].push(val_str_opt);
        }
    }

    // Create a Polars Series for each column and assemble the DataFrame.
    let mut series_vec: Vec<Series> = Vec::with_capacity(col_count);
    for (i, name) in headers.iter().enumerate() {
        let s = Series::new(name.as_str(), &columns[i]);
        series_vec.push(s);
    }
    let df = DataFrame::new(series_vec)?;
    Ok(df)
}

/// Loads an audio file and decodes its default track into a DataFrame.
///
/// Uses the `symphonia` crate to handle various audio codecs and formats.
/// The resulting DataFrame will contain a `sample_index` column and one column for
/// each audio channel (e.g., `channel_0`, `channel_1`).
///
/// # Arguments
///
/// * `path` - A reference to the path of the file to load.
///
/// # Returns
///
/// A `Result` containing a `DataFrame` with separate columns for each audio
/// channel on success, or an `AppError` on failure.
fn load_audio_dataframe(path: &Path) -> Result<DataFrame, AppError> {
    // Setup: Open file and initialize symphonia probe.
    let src = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(src), Default::default());

    let hint = symphonia::core::probe::Hint::new();
    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();
    let probed = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts)?;

    let mut format = probed.format;
    let track = format.default_track().ok_or_else(|| {
        AppError::Symphonia(symphonia::core::errors::Error::Unsupported(
            "No default track found".into(),
        ))
    })?;

    // Get the number of channels from the track's codec parameters.
    let num_channels = track
        .codec_params
        .channels
        .ok_or_else(|| {
            AppError::Symphonia(symphonia::core::errors::Error::Unsupported(
                "Channel count is not available for the track.".into(),
            ))
        })?
        .count();

    let dec_opts: DecoderOptions = Default::default();
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &dec_opts)?;

    // Create a vector of vectors, one for each channel.
    let mut channels_data: Vec<Vec<f32>> = vec![Vec::new(); num_channels];

    // Decoding loop
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::IoError(ref err))
                if err.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(err) => return Err(AppError::from(err)),
        };

        // Decode the packet into an audio buffer.
        let decoded = decoder.decode(&packet)?;

        let mut sample_buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
        sample_buf.copy_planar_ref(decoded);

        // Iterate through each channel (plane) in the decoded buffer.
        for (channel_idx, plane) in sample_buf.planes().planes().iter().enumerate() {
            // Append the samples from the current plane to the correct channel vector.
            channels_data[channel_idx].extend_from_slice(plane);
        }
    }

    // --- Create DataFrame from the separated channel data ---

    // Determine the number of samples from the first channel.
    let num_samples = channels_data.get(0).map_or(0, |v| v.len());
    if num_samples == 0 {
        return Ok(DataFrame::default()); // Return an empty DataFrame if no samples.
    }

    // Create the 'sample_index' series.
    let indices: Vec<u32> = (0..num_samples as u32).collect();
    let mut series_vec = vec![Series::new("sample_index", &indices)];

    // Create a Series for each channel's data.
    for (i, channel_samples) in channels_data.iter().enumerate() {
        // Ensure all channels have the same length. Pad with zeros if necessary.
        let mut samples = channel_samples.clone();
        samples.resize(num_samples, 0.0);

        let name = format!("channel_{}", i + 1);
        series_vec.push(Series::new(&name, samples));
    }

    // Assemble the final DataFrame.
    let df = DataFrame::new(series_vec)?;
    Ok(df)
}
