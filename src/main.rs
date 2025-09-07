use anyhow::Result;
use clap::Parser;
use scatters::cli::Cli;

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