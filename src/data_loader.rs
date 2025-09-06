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
        "csv" => CsvReader::new(File::open(path)?)
            .finish()
            .map_err(AppError::from)?,
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
        "wav" | "mp3" | "flac" => return load_audio_dataframe(path),
        _ => {
            return Err(AppError::UnsupportedFormat(
                path.to_string_lossy().to_string(),
            ))
        }
    };

    // Attempt to auto-coerce string columns that look like datetimes
    try_cast_string_columns_to_datetime(&mut df)?;
    Ok(df)
}

/// Try to cast string columns into Datetime using Polars' native casting.
fn try_cast_string_columns_to_datetime(df: &mut DataFrame) -> Result<(), AppError> {
    use polars::prelude::*;

    let col_names: Vec<String> = df
        .get_columns()
        .iter()
        .map(|s| s.name().to_string())
        .collect();

    for name in col_names {
        let s = df.column(&name)?.clone();
        if matches!(s.dtype(), DataType::String) {
            if let Ok(parsed) = s.cast(&DataType::Datetime(TimeUnit::Milliseconds, None)) {
                // Replace the column with the parsed datetime series
                df.replace(&name, parsed).map_err(AppError::from)?;
            }
        }
    }
    Ok(())
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
