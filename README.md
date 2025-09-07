# data_plotter

A Rust CLI that turns tabular data and audio files into interactive, self-contained HTML scatter plots powered by ECharts.

- Inputs: CSV, Parquet, JSON/JSONL/NDJSON, and audio (WAV/MP3/FLAC)
- Output: One HTML file per input, with zoom/pan, legend, and toolbox — no server required
- Directory mode: Pass a folder to process all supported files recursively

## Build and Run

Prerequisites: Rust toolchain with Cargo

- Build (debug):
```powershell
cargo build
```
- Build (release):
```powershell
cargo build --release
```
- Show CLI help:
```powershell
cargo run -- --help
```

## Usage

Basic examples:
```powershell
# Plot all numeric columns; auto-detect X when possible
cargo run -- sample/sample.csv

# Write outputs to a directory
cargo run -- sample/sample.csv -o plots/

# Set the X-axis explicitly
cargo run -- data.csv --index timestamp

# Choose specific Y columns
cargo run -- data.csv -c sensor_a,sensor_b

# Use first column as X and set a custom title
cargo run -- data.csv --use-first-column --title "My Custom Plot"

# Process an entire directory recursively
cargo run -- path/to/folder -o plots/

# Audio files (mono or multi-channel)
cargo run -- audio.wav

# Disable dynamic Y autoscaling (keep initial padded range)
cargo run -- sample/sample.csv --no-autoscale-y

```

Where outputs go:
- With `-o/--output-dir`, files are saved under that directory as `<stem>.html`.
- Without it, each plot is saved next to its input file.

## CLI
```
A tool to generate interactive scatter plots from various data formats.

Usage: data_plotter.exe [OPTIONS] <INPUT_PATH>

Arguments:
  <INPUT_PATH>  The input file or folder to scan for data

Options:
  -o, --output-dir <OUTPUT_DIR>  Directory to save the generated HTML plots. Defaults to saving next to each input file
      --index <INDEX>            Name of the column to use as the index (X-axis). Highest priority for index selection
      --use-first-column         Use the first column of the data as the index. Overridden by --index
  -c, --columns <COLUMNS>        Comma-separated list of columns to plot (Y-axis). If not provided, all numeric columns will be plotted
      --title <TITLE>            A custom title for the plot. Defaults to the input filename
      --no-autoscale-y           Disable dynamic Y-axis autoscaling on zoom (keeps initial Y range)
  -h, --help                     Print help
  -V, --version                  Print version
```

## How X and Y are chosen

X-axis selection priority:
1) `--index <name>`
2) `--use-first-column`
3) If a column named `sample_index` exists (audio), use it
4) First datetime column (string columns may be auto-cast to datetime)
5) Fallback to a generated `row_index`

Y-axis selection:
- If `-c/--columns` is provided, those columns are used
- Otherwise, all numeric columns except the chosen X column

## Supported formats and behavior

- CSV, Parquet
- JSON, JSONL/NDJSON (JSON is read as JSON Lines; for array-of-objects JSON, convert to NDJSON)
- Audio: WAV/MP3/FLAC via Symphonia
  - The first default track is decoded
  - DataFrame has `sample_index` (X) and `amplitude` (Y)
  - For multi-channel audio, samples are currently interleaved into the single `amplitude` series

## Output HTML

- Self-contained HTML with ECharts loaded from CDN
- Interactive features: zoom/pan (dataZoom), legend scroll, save-as-image, restore
- X-axis type is set automatically (time, category, or value) based on the chosen X series

## Project structure (brief)

- `src/cli.rs` — command-line interface (clap)
- `src/lib.rs` — orchestration (discover files, process each, write output)
- `src/data_loader.rs` — load DataFrames; audio decoding; best-effort datetime casting
- `src/processing.rs` — select X/Y series and plot title
- `src/plotter.rs` — generate HTML and embed series as JSON for ECharts
- `src/error.rs` — error types

For additional repository-specific guidance, see `WARP.md`.
