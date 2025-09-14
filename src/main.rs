//! The binary entry point for the `scatters` command-line application.
//!
//! This file is responsible for:
//! 1. Parsing command-line arguments using the `clap` crate.
//! 2. Calling the main application logic in the `scatters` library.
//! 3. Handling and printing any errors that occur during execution.

use anyhow::Result;
use clap::Parser;
use scatters::cli::Cli;

/// The main function of the executable.
///
/// Parses command-line arguments and invokes the library's `run` function.
/// If an error occurs, it is printed to stderr and the process exits with a non-zero status code.
fn main() -> Result<()> {
    // Parse command-line arguments
    let cli = Cli::parse();

    // Run the main application logic from the library
    if let Err(e) = scatters::run(&cli) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}
