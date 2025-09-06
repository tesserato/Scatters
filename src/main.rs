use anyhow::Result;
use clap::Parser;
use data_plotter::cli::Cli;

fn main() -> Result<()> {
    // Parse command-line arguments
    let cli = Cli::parse();

    // Run the main application logic from the library
    if let Err(e) = data_plotter::run(&cli) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}