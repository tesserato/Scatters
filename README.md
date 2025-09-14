# Scatters

[![crates.io](https://img.shields.io/crates/v/scatters.svg)](https://crates.io/crates/scatters)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A CLI to instantly turn tabular data and audio files into interactive, self-contained HTML scatter plots.

`scatters` reads CSV, JSON, Parquet, Excel, and common audio files, then generates beautiful, interactive charts powered by ECharts. It works recursively on directories and saves each plot as a single `.html` file that requires no internet connection or server to view.



## Features

-   **Broad Format Support**: Process CSV, Parquet, JSON/JSONL, Excel (XLSX/XLS), and audio (WAV, MP3, FLAC).
-   **Interactive Plots**: Output includes zoom/pan controls, a draggable legend, a toolbox to save the chart as an image, and tooltips for data points.
-   **Fully Self-Contained**: Generates single HTML files with all necessary JS/CSS included from a CDN. No local dependencies or servers needed to view the plots.
-   **Intelligent Defaults**: Automatically detects the best column for the X-axis (prioritizing datetimes) and plots all other numeric columns.
-   **Powerful Customization**: Use CLI flags to specify X/Y columns, set a title, choose a light or dark theme, enable animations, and more.
-   **Directory Processing**: Point it at a folder to recursively find and process all supported files.
-   **Special Markers**: Use a `|` value in a string column to draw vertical marker lines on your plot for highlighting events.

## Installation

Ensure you have the Rust toolchain installed. You can then install `scatters` using Cargo:
```shell
cargo install scatters
```
## Usage

Once installed, you can use the `scatters` command. If you are running from a cloned repository, you can use cargo run --.

### Basic Examples

#### Plot a CSV file, auto-detecting X/Y axes
`scatters data/measurements.csv`

#### Process an entire directory and save plots to a specific folder
`scatters ./my_data_folder -o ./plots`

#### Plot a WAV file, using sample index as X and amplitude as Y
`scatters audio/sound.wav`

#### Specify the 'timestamp' column as the X-axis
`scatters sensor_log.parquet --index timestamp`

#### Plot only specific columns ('sensor_a', 'sensor_b')
`scatters sensor_log.parquet -c sensor_a,sensor_b`

#### Use the first column as X, add a title, and use the light theme
`scatters data.csv --use-first-column --title "My Custom Plot" --white-theme`

#### Disable dynamic Y-axis autoscaling and enable animations
`scatters data.csv --no-autoscale-y --animations`

## Output Location

By default, each plot is saved as <input_filename>.html next to the original file.

Use the -o or --output-dir option to save all generated plots into a specific directory.

## CLI Reference

A CLI to instantly turn tabular data and audio files into interactive HTML scatter plots.

Usage: scatters [OPTIONS] <INPUT_PATH>

Arguments:
  <INPUT_PATH>  The input file or folder to scan for data

Options:
  -o, --output-dir <OUTPUT_DIR>
          Directory to save the generated HTML plots. Defaults to saving next to each input file
  -i, --index <INDEX>
          Name of the column to use as the index (X-axis). This has the highest priority for index selection
  -f, --use-first-column
          Use the first column of the data as the index (X-axis). This is overridden by the --index option if both are provided
  -c, --columns <COLUMNS>
          Comma-separated list of columns to plot (Y-axis). If not provided, all numeric columns will be plotted
  -t, --title <TITLE>
          A custom title for the plot. Defaults to the input filename
  -n, --no-autoscale-y
          Disable dynamic Y-axis autoscaling on zoom. When disabled, the Y-axis keeps its initial, globally-padded range
  -a, --animations
          Enable ECharts animations for a more dynamic feel. Animations are disabled by default for performance
  -m, --max-decimals <MAX_DECIMALS>
          Maximum number of decimal places for numeric formatting in tooltips. Use -1 for an unlimited number of decimal places [default: 2]
  -l, --large-mode-threshold <LARGE_MODE_THRESHOLD>
          Threshold for ECharts `large` mode. Series with more points than this will be optimized for performance, which may reduce detail [default: 2000]
  -d, --debug
          Print debug information during processing. This includes detected columns, data types, and DataFrame shape
  -w, --white-theme
          Use a white (light) theme for the plot instead of the default dark theme
  -h, --help
          Print help
  -V, --version
          Print version

## X-Axis Selection Priority

scatters selects the X-axis column in the following order:

Column name provided via --index <name>.

The first column if --use-first-column is passed.

A column named sample_index (default for audio files).

The first column with a Datetime or Date data type.

If none of the above match, it falls back to a generated row_index.

## Y-Axis Selection

If -c/--columns is used, only the specified columns are plotted.

Otherwise, all columns with a numeric data type (excluding the chosen X-axis column) are automatically plotted.

## Type Inference

scatters performs automatic type casting to improve plotting:

String to Numeric: Columns where all non-null string values can be parsed as numbers are converted to Float64.

String to Datetime: String columns are converted to Datetime if at least 90% of non-null values match a common date/time format (e.g., YYYY-MM-DD HH:MM:SS, DD/MM/YYYY, etc.).

## Special Markers for Vertical Lines

To highlight specific events or regions, you can add vertical lines to the plot. Create a string column where most values are empty or null, and place a pipe character (|) at the rows where you want a line. scatters will automatically render these as vertical markLines aligned with the corresponding X-axis value.

## Audio File Handling

Audio files (WAV, MP3, FLAC) are decoded into a time series.

The resulting data has two columns: sample_index (the X-axis) and amplitude (the Y-axis).

Multi-channel audio is currently interleaved into a single amplitude series.