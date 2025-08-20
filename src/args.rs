use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "historee",
    about = "Analyze browser history to find unique domains and their visit counts",
    version,
    long_about = None
)]
pub struct Args {
    /// Browser to analyze
    #[arg(short, long, default_value = "Vivaldi")]
    pub browser: String,

    /// Number of top domains to display
    #[arg(short, long)]
    pub top: Option<usize>,

    /// Number of bottom domains to display
    #[arg(long)]
    pub bottom: Option<usize>,

    /// Path to custom domain pattern file
    #[arg(short, long)]
    pub patterns: Option<PathBuf>,

    /// Disable pattern-based domain normalization
    #[arg(long)]
    pub no_patterns: bool,

    /// Custom temporary file path for database copy
    #[arg(long)]
    pub temp_path: Option<PathBuf>,

    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,

    /// Number of worker threads
    #[arg(short, long)]
    pub workers: Option<usize>,

    /// Redact domain names for privacy
    #[arg(long)]
    pub redact: bool,

    /// Initialize domain_patterns.txt with default patterns
    #[arg(long)]
    pub init: bool,
}
