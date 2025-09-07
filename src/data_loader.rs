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
pub fn load_dataframe(path: &Path) -> Result<DataFrame, AppError> {
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_lowercase();

    let mut df = match extension.as_str() {
        "csv" => {
            // First, try reading assuming a header is present (Polars default)
            let file = File::open(path)?;
            let df = CsvReader::new(file).finish().map_err(AppError::from)?;
            df
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
        "wav" | "mp3" | "flac" => return load_audio_dataframe(path),
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

/// Try to cast string columns into Datetime using multiple strategies.
/// This will only convert the column if a high percentage of non-null values can be parsed.
/// 1) Polars native cast (RFC-like formats)
/// 2) Heuristic parser for many common formats: YYYY{sep}MM{sep}DD or DD{sep}MM{sep}YYYY,
///    optionally followed by time (HH, HH:MM, HH:MM:SS) and date-time separators of 'T' or space.
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

                // Only accept the cast if it's successful for at least 90% of non-null values.
                // This avoids converting columns with only a few date-like strings.
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

/// Attempt to parse a String Series into Datetime (ms) with many formats.
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
    // Helper: normalize whitespace (collapse runs)
    fn normalize_spaces(input: &str) -> std::borrow::Cow<'_, str> {
        let trimmed = input.trim();
        if trimmed.contains(char::is_whitespace) {
            let s = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
            std::borrow::Cow::Owned(s)
        } else {
            std::borrow::Cow::Borrowed(trimmed)
        }
    }

    // Try parsing using fmts; collect ms since epoch
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

    // Apply the 90% threshold here as well.
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

/// Try to cast string columns into Float64 when they look numeric.
fn try_cast_string_columns_to_numeric(df: &mut DataFrame) -> Result<(), AppError> {
    let col_names: Vec<String> = df
        .get_columns()
        .iter()
        .map(|s| s.name().to_string())
        .collect();

    for name in col_names {
        let s = df.column(&name)?.clone();
        if matches!(s.dtype(), DataType::String) {
            // Manually trim and parse floats from string values
            let parsed_vals: Vec<Option<f64>> = s
                .iter()
                .map(|av| match av {
                    AnyValue::String(t) => t.trim().parse::<f64>().ok(),
                    AnyValue::StringOwned(ref t) => t.trim().parse::<f64>().ok(),
                    _ => None,
                })
                .collect();
            let parsed_series = Series::new(&name, parsed_vals);

            // Only replace if the number of successfully parsed values is greater than
            // the number of failed values (nulls). This avoids converting columns
            // that are mostly non-numeric but contain some numbers.
            if parsed_series.null_count() * 2 < parsed_series.len() {
                df.replace(&name, parsed_series).map_err(AppError::from)?;
            }
        }
    }
    Ok(())
}

/// Loads an Excel file (first worksheet) into a DataFrame.
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

    // Collect rows as Vec<Vec<Xl>>
    let rows: Vec<Vec<Xl>> = range.rows().map(|r| r.to_vec()).collect();
    // Find first non-empty row for header
    let mut header_idx: Option<usize> = None;
    for (i, r) in rows.iter().enumerate() {
        let all_empty = r.iter().all(|c| matches!(c, Xl::Empty));
        if !all_empty {
            header_idx = Some(i);
            break;
        }
    }
    let header_idx = header_idx
        .ok_or_else(|| AppError::UnsupportedFormat(path.to_string_lossy().to_string()))?;

    // Determine column count as max row length
    let col_count = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if col_count == 0 {
        return Err(AppError::UnsupportedFormat(
            path.to_string_lossy().to_string(),
        ));
    }

    // Build header names from the header row
    let mut headers: Vec<String> = Vec::with_capacity(col_count);
    for i in 0..col_count {
        let name = rows
            .get(header_idx)
            .and_then(|r| r.get(i))
            .map(|c| match c {
                Xl::String(s) => s.trim().to_string(),
                Xl::Float(v) => v.to_string(),
                Xl::Int(v) => v.to_string(),
                Xl::Bool(v) => v.to_string(),
                Xl::DateTime(v) => v.to_string(),
                _ => String::new(),
            })
            .unwrap_or_default();
        let final_name = if name.is_empty() {
            format!("col_{}", i + 1)
        } else {
            name
        };
        headers.push(final_name);
    }

    // Initialize column vectors
    let mut columns: Vec<Vec<Option<String>>> = vec![Vec::new(); col_count];

    for (ri, row) in rows.iter().enumerate() {
        if ri <= header_idx {
            continue;
        } // skip header and any leading rows before it
        for ci in 0..col_count {
            let val_str_opt: Option<String> = row
                .get(ci)
                .map(|c| match c {
                    Xl::Empty => None,
                    Xl::String(s) => Some(s.trim().to_string()),
                    Xl::Float(v) => Some(v.to_string()),
                    Xl::Int(v) => Some(v.to_string()),
                    Xl::Bool(v) => Some(v.to_string()),
                    Xl::DateTime(v) => Some(v.to_string()),
                    Xl::Duration(v) => Some(v.to_string()),
                    Xl::DateTimeIso(s) => Some(s.trim().to_string()),
                    Xl::DurationIso(s) => Some(s.trim().to_string()),
                    Xl::Error(_) => None,
                })
                .flatten();
            columns[ci].push(val_str_opt);
        }
    }

    let mut series_vec: Vec<Series> = Vec::with_capacity(col_count);
    for (i, name) in headers.iter().enumerate() {
        let s = Series::new(name.as_str(), &columns[i]);
        series_vec.push(s);
    }
    let df = DataFrame::new(series_vec)?;
    Ok(df)
}

/// Loads an audio file and converts its first track to a DataFrame.
fn load_audio_dataframe(path: &Path) -> Result<DataFrame, AppError> {
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

    let dec_opts: DecoderOptions = Default::default();
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &dec_opts)?;

    let mut samples_f32: Vec<f32> = Vec::new();

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

        let decoded = decoder.decode(&packet)?;
        let mut sample_buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
        sample_buf.copy_interleaved_ref(decoded);
        samples_f32.extend_from_slice(sample_buf.samples());
    }

    let indices: Vec<u32> = (0..samples_f32.len() as u32).collect();
    let df = df! {
        "sample_index" => &indices,
        "amplitude" => &samples_f32,
    }?;
    Ok(df)
}
