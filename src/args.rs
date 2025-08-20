use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Browser {
    Chrome,
    Edge,
    Firefox,
    Vivaldi,
    Zen,
}

impl std::fmt::Display for Browser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Browser::Chrome => write!(f, "Chrome"),
            Browser::Edge => write!(f, "Edge"),
            Browser::Firefox => write!(f, "Firefox"),
            Browser::Vivaldi => write!(f, "Vivaldi"),
            Browser::Zen => write!(f, "Zen"),
        }
    }
}

#[derive(Parser, Debug)]
#[command(
    name = "historee",
    about = "Analyze browser history to find unique domains and their visit counts",
    version,
    long_about = None
)]
pub struct Args {
    /// Browser to analyze
    #[arg(short, long, default_value = "vivaldi")]
    pub browser: Browser,

    /// Analyze all supported browsers
    #[arg(long)]
    pub all_browsers: bool,

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
