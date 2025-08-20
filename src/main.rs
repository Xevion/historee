use anyhow::Result;
use clap::Parser;
use log::error;

use browser_unique_domains::{browser, utils, Args};

fn main() -> Result<()> {
    let args = Args::parse();
    utils::setup_logging(args.verbose);

    // Validate arguments
    utils::validate_args(&args)?;

    match browser::analyze_browser_history(&args) {
        Ok(result) => {
            browser::print_analysis_results(&result, &args);
            Ok(())
        }
        Err(e) => {
            error!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
