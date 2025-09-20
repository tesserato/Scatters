# Scatters

[![crates.io](https://img.shields.io/crates/v/scatters.svg)](https://crates.io/crates/scatters)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Instantly create interactive, single-file HTML scatter plots from tabular data (CSV, Parquet, JSON, Excel) and audio formats (WAV, MP3, FLAC, OGG, M4A, AAC). Built for speed and massive datasets with optional intelligent downsampling.

![Scatters Demo](demo.gif)

`scatters` reads common data and audio files and generates beautiful, interactive charts powered by ECharts. It works recursively on directories and saves each plot as a single `.html` file.



## Features

-   **Broad Format Support**: Process CSV, Parquet, JSON/JSONL, Excel (XLSX/XLS), and audio (WAV, MP3, FLAC).
-   **Interactive Plots**: Output includes zoom/pan controls, a draggable legend, a toolbox to save the chart as an image, and tooltips for data points.
-   **Fully Self-Contained**: Generates single HTML files with all necessary JS/CSS included from a CDN. No local dependencies or servers needed to view the plots.
-   **Intelligent Defaults**: Automatically detects the best column for the X-axis (prioritizing datetimes) and plots all other numeric columns.
-   **Powerful Customization**: Use CLI flags to specify X/Y columns, set a title, choose a light or dark theme, enable animations, and more.
-   **Directory Processing**: Point it at a folder to recursively find and process all supported files.
-   **Special Markers**: Use a `|` value in a string column to draw vertical marker lines on your plot for highlighting events.
-   **Large File Support**: Intelligently downsample massive datasets using the `--downsample` flag to keep plots fast and responsive. Implements the Largest-Triangle-Three-Buckets (LTTB) algorithm for downsampling, to retain meaningful features.

## Installation

Ensure you have the Rust toolchain installed. You can then install `scatters` using Cargo:
```shell
cargo install scatters
```
After installation, run `scatters --help` for a full list of options and usage instructions.
