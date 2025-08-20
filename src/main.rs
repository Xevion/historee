use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use clap::Parser;
use log::{error, info, warn};
use rayon::prelude::*;
use regex::Regex;
use rusqlite::{Connection, Result as SqliteResult};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;
use url::Url;

#[derive(Parser, Debug)]
#[command(
    name = "browser-unique-domains",
    about = "Analyze browser history to find unique domains and their visit counts",
    version,
    long_about = None
)]
struct Args {
    /// Browser to analyze
    #[arg(short, long, default_value = "Vivaldi")]
    browser: String,

    /// Number of top domains to display
    #[arg(short, long)]
    top: Option<usize>,

    /// Number of bottom domains to display
    #[arg(long)]
    bottom: Option<usize>,

    /// Path to custom domain pattern file
    #[arg(short, long)]
    patterns: Option<PathBuf>,

    /// Disable pattern-based domain normalization
    #[arg(long)]
    no_patterns: bool,

    /// Custom temporary file path for database copy
    #[arg(long)]
    temp_path: Option<PathBuf>,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Number of worker threads
    #[arg(short, long)]
    workers: Option<usize>,

    /// Redact domain names for privacy
    #[arg(long)]
    redact: bool,
}

#[derive(Debug)]
struct DomainStats {
    unique_domains: Vec<String>,
    domain_counts: HashMap<String, u32>,
    domains_removed: u32,
}

#[derive(Debug)]
struct AnalysisResult {
    date_range: (String, String, i64),
    stats: DomainStats,
}

fn setup_logging(verbose: bool) {
    if verbose {
        env::set_var("RUST_LOG", "info");
    } else {
        env::set_var("RUST_LOG", "error");
    }
    env_logger::init();
}

fn get_browser_history_path(browser: &str) -> Result<PathBuf> {
    let system = env::consts::OS;
    let home = env::var("HOME").or_else(|_| env::var("USERPROFILE"))?;

    let path = match (browser.to_lowercase().as_str(), system) {
        ("vivaldi", "windows") => {
            let local_app_data = env::var("LOCALAPPDATA")?;
            PathBuf::from(local_app_data).join("Vivaldi/User Data/Default/History")
        }
        ("vivaldi", "macos") => {
            PathBuf::from(home).join("Library/Application Support/Vivaldi/Default/History")
        }
        ("vivaldi", "linux") => PathBuf::from(home).join(".config/vivaldi/default/History"),
        _ => anyhow::bail!(
            "Unsupported browser '{}' or operating system '{}'",
            browser,
            system
        ),
    };

    info!("Browser history path: {:?}", path);
    Ok(path)
}

fn copy_history_database(history_path: &Path, temp_path: Option<&Path>) -> Result<PathBuf> {
    let start_time = Instant::now();
    info!("Copying browser history database");

    let temp_path = temp_path.map(|p| p.to_path_buf()).unwrap_or_else(|| {
        PathBuf::from(&format!(
            "{}/browser_history_copy.db",
            env::var("HOME").unwrap_or_else(|_| env::var("USERPROFILE").unwrap_or_default())
        ))
    });

    info!("Source: {:?}", history_path);
    info!("Destination: {:?}", temp_path);

    if !history_path.exists() {
        anyhow::bail!("History file not found at {:?}", history_path);
    }

    fs::copy(history_path, &temp_path)?;

    let copy_time = start_time.elapsed();
    info!("Database copy completed in {:.1}ms", copy_time.as_millis());
    Ok(temp_path)
}

fn get_date_range(conn: &Connection) -> Result<(String, String, i64)> {
    let start_time = Instant::now();
    info!("Querying visit date range");

    let (earliest_timestamp, latest_timestamp): (Option<i64>, Option<i64>) = conn
        .query_row(
            "SELECT MIN(visit_time), MAX(visit_time) FROM visits",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .context("Failed to query visit dates")?;

    if let (Some(earliest), Some(latest)) = (earliest_timestamp, latest_timestamp) {
        // Chrome uses microseconds since 1601-01-01
        let chrome_epoch =
            DateTime::parse_from_rfc3339("1601-01-01T00:00:00Z")?.with_timezone(&Utc);
        let earliest_date = chrome_epoch + chrono::Duration::microseconds(earliest);
        let latest_date = chrome_epoch + chrono::Duration::microseconds(latest);

        let days_between = (latest_date - earliest_date).num_days();
        let query_time = start_time.elapsed();

        info!(
            "Date range: {} to {} ({} days) in {:.1}ms",
            earliest_date.format("%B %-d, %Y"),
            latest_date.format("%B %-d, %Y"),
            days_between,
            query_time.as_millis()
        );

        Ok((
            earliest_date.format("%B %-d, %Y").to_string(),
            latest_date.format("%B %-d, %Y").to_string(),
            days_between,
        ))
    } else {
        let query_time = start_time.elapsed();
        warn!(
            "No visit data found (query took {:.1}ms)",
            query_time.as_millis()
        );
        Ok((
            "No data available".to_string(),
            "No data available".to_string(),
            0,
        ))
    }
}

fn load_domain_patterns(pattern_file_path: Option<&Path>) -> Result<Vec<Regex>> {
    let start_time = Instant::now();
    info!("Starting domain pattern loading");

    let default_patterns = vec![
        r"^.+\.(cloudfront\.net)$",
        r"^.+\.(amazonaws\.com)$",
        r"^.+\.(herokuapp\.com)$",
        r"^.+\.(netlify\.app)$",
        r"^.+\.(vercel\.app)$",
        r"^.+\.(github\.io)$",
        r"^.+\.(firebaseapp\.com)$",
        r"^.+\.(appspot\.com)$",
        r"^.+\.(azurewebsites\.net)$",
        r"^.+\.(cloudflare\.com)$",
        r"^.+\.(fastly\.com)$",
        r"^.+\.(cdn\.com)$",
        r"^.+\.(cdn\.net)$",
        r"^.+\.(cdn\.org)$",
        r"^.+\.(s3\.amazonaws\.com)$",
        r"^.+\.(s3-website-[^.]+\.amazonaws\.com)$",
        r"^.+\.(elasticbeanstalk\.com)$",
        r"^.+\.(railway\.app)$",
        r"^.+\.(render\.com)$",
        r"^.+\.(fly\.io)$",
        r"^.+\.(digitaloceanspaces\.com)$",
        r"^.+\.(bunnycdn\.com)$",
        r"^.+\.(stackpathcdn\.com)$",
        r"^.+\.(keycdn\.com)$",
    ];

    let mut patterns = Vec::new();

    if let Some(path) = pattern_file_path {
        info!("Loading patterns from specified file: {:?}", path);
        if !path.exists() {
            anyhow::bail!("Pattern file not found: {:?}", path);
        }

        let content = fs::read_to_string(path)?;
        for (line_num, line) in content.lines().enumerate() {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') {
                match Regex::new(line) {
                    Ok(regex) => patterns.push(regex),
                    Err(e) => {
                        anyhow::bail!("Invalid regex pattern at line {}: {}", line_num + 1, e)
                    }
                }
            }
        }
        info!("Loaded {} patterns from {:?}", patterns.len(), path);
    } else {
        // Try default file
        let default_file = Path::new("domain_patterns.txt");
        if default_file.exists() {
            info!("Loading patterns from default file: {:?}", default_file);
            let content = fs::read_to_string(default_file)?;
            for (line_num, line) in content.lines().enumerate() {
                let line = line.trim();
                if !line.is_empty() && !line.starts_with('#') {
                    match Regex::new(line) {
                        Ok(regex) => patterns.push(regex),
                        Err(e) => warn!("Invalid regex pattern at line {}: {}", line_num + 1, e),
                    }
                }
            }
            info!("Loaded {} patterns from {:?}", patterns.len(), default_file);
        }

        // If no patterns loaded, use defaults
        if patterns.is_empty() {
            info!("Using default patterns");
            for pattern_str in default_patterns {
                patterns.push(Regex::new(pattern_str)?);
            }
        }
    }

    let pattern_time = start_time.elapsed();
    info!(
        "Successfully compiled {} patterns in {:.1}ms",
        patterns.len(),
        pattern_time.as_millis()
    );
    Ok(patterns)
}

fn has_valid_tld(domain: &str) -> bool {
    if domain.is_empty() || domain.len() < 3 || !domain.contains('.') {
        return false;
    }

    if let Some(last_dot) = domain.rfind('.') {
        if last_dot == domain.len() - 1 {
            return false;
        }
        let tld = &domain[last_dot + 1..];
        tld.len() >= 2
            && tld
                .chars()
                .all(|c| c.is_ascii_lowercase() && c.is_ascii_alphabetic())
    } else {
        false
    }
}

fn normalize_domain(domain: &str, patterns: &[Regex]) -> String {
    if domain.is_empty() {
        return domain.to_string();
    }

    // Optimize: avoid unnecessary string allocation for simple cases
    let normalized_domain = if domain.matches('.').count() <= 2 {
        domain.to_string()
    } else {
        let parts: Vec<&str> = domain.split('.').collect();
        if parts.len() > 3 {
            parts[parts.len() - 3..].join(".")
        } else {
            domain.to_string()
        }
    };

    // Apply pattern normalization
    for pattern in patterns {
        if let Some(captures) = pattern.captures(&normalized_domain) {
            if let Some(matched) = captures.get(1) {
                return matched.as_str().to_string();
            }
        }
    }

    normalized_domain
}

fn extract_domains_from_urls(
    conn: &Connection,
    patterns: &[Regex],
    max_workers: Option<usize>,
) -> Result<DomainStats> {
    let start_time = Instant::now();
    info!("Starting domain extraction from URLs");

    let urls: Vec<String> = conn
        .prepare("SELECT url FROM urls")?
        .query_map([], |row| row.get(0))?
        .collect::<SqliteResult<Vec<String>>>()?;

    let query_time = start_time.elapsed();
    info!(
        "Found {} URLs to process (query took {:.1}ms)",
        urls.len(),
        query_time.as_millis()
    );

    let max_workers = max_workers.unwrap_or_else(|| {
        let cpu_count = num_cpus::get();
        std::cmp::min(cpu_count, 8)
    });

    info!("Using {} workers", max_workers);

    let processing_start = Instant::now();

    // Use Rayon's built-in parallel iterator with automatic work-stealing
    let batch_stats: Vec<DomainStats> = urls
        .into_par_iter()
        .fold(
            || DomainStats {
                unique_domains: Vec::new(),
                domain_counts: HashMap::new(),
                domains_removed: 0,
            },
            |mut acc, url_str| {
                if let Ok(url) = Url::parse(&url_str) {
                    if let Some(host) = url.host_str() {
                        if !has_valid_tld(host) {
                            acc.domains_removed += 1;
                        } else {
                            let normalized_domain = normalize_domain(host, patterns);

                            if !has_valid_tld(&normalized_domain) {
                                acc.domains_removed += 1;
                            } else {
                                *acc.domain_counts.entry(normalized_domain).or_insert(0) += 1;
                            }
                        }
                    }
                }
                acc
            },
        )
        .collect();

    // Merge all results from fold operations
    let mut all_stats = DomainStats {
        unique_domains: Vec::new(),
        domain_counts: HashMap::new(),
        domains_removed: 0,
    };

    for stats in batch_stats {
        all_stats.unique_domains.extend(stats.unique_domains);
        for (domain, count) in stats.domain_counts {
            *all_stats.domain_counts.entry(domain).or_insert(0) += count;
        }
        all_stats.domains_removed += stats.domains_removed;
    }

    // Update unique_domains from the final domain_counts
    all_stats.unique_domains = all_stats.domain_counts.keys().cloned().collect();

    let total_processing_time = processing_start.elapsed();
    let total_time = start_time.elapsed();
    info!(
        "Domain extraction completed: {} unique domains, {} removed",
        all_stats.unique_domains.len(),
        all_stats.domains_removed
    );
    info!(
        "Processing time: {:.1}ms, Total time: {:.1}ms",
        total_processing_time.as_millis(),
        total_time.as_millis()
    );

    Ok(all_stats)
}

fn analyze_browser_history(args: &Args) -> Result<AnalysisResult> {
    let total_start_time = Instant::now();
    info!("Starting browser history analysis");

    let history_path = get_browser_history_path(&args.browser)?;
    let temp_history_path = copy_history_database(&history_path, args.temp_path.as_deref())?;

    let patterns = if args.no_patterns {
        Vec::new()
    } else {
        load_domain_patterns(args.patterns.as_deref())?
    };

    let conn = Connection::open(&temp_history_path)?;
    info!("Connected to database");

    let date_range = get_date_range(&conn)?;
    let stats = extract_domains_from_urls(&conn, &patterns, args.workers)?;

    info!("Closing database connection");
    drop(conn);

    // Clean up temporary file
    if let Err(e) = fs::remove_file(&temp_history_path) {
        warn!("Failed to remove temporary file: {}", e);
    }

    let total_time = total_start_time.elapsed();
    info!(
        "Analysis completed successfully in {:.1}ms",
        total_time.as_millis()
    );

    Ok(AnalysisResult { date_range, stats })
}

fn format_number(num: u32) -> String {
    num.to_string()
        .as_bytes()
        .rchunks(3)
        .rev()
        .map(|chunk| std::str::from_utf8(chunk).unwrap())
        .collect::<Vec<_>>()
        .join(",")
}

fn redact_domain(domain: &str) -> String {
    let parts: Vec<&str> = domain.split('.').collect();
    if parts.len() <= 1 {
        return domain.to_string();
    }

    if parts.len() >= 2 && parts[parts.len() - 2].len() <= 3 {
        return format!("???.{}", parts[parts.len() - 1]);
    }

    let redacted_parts: Vec<String> = parts[..parts.len() - 1]
        .iter()
        .map(|part| "*".repeat(part.len()))
        .collect();

    let mut result = redacted_parts.join(".");
    result.push('.');
    result.push_str(parts[parts.len() - 1]);
    result
}

fn print_analysis_results(result: &AnalysisResult, args: &Args) {
    let (earliest_date, latest_date, days_between) = &result.date_range;

    println!("\n--- {} History Analysis ---", args.browser);

    if *days_between > 0 {
        println!(
            "Date range: {} to {} ({} days)",
            earliest_date,
            latest_date,
            format_number(*days_between as u32)
        );
    } else {
        println!("Date range: {} to {}", earliest_date, latest_date);
    }

    println!(
        "Total unique domains found: {}",
        format_number(result.stats.unique_domains.len() as u32)
    );
    println!(
        "Domains removed (no valid TLD): {}",
        format_number(result.stats.domains_removed)
    );

    // Sort domains by count
    let mut sorted_domains: Vec<(&String, &u32)> = result.stats.domain_counts.iter().collect();
    sorted_domains.sort_by(|a, b| b.1.cmp(a.1));

    if let Some(top_count) = args.top {
        println!(
            "\nTop {} most visited domains:",
            std::cmp::min(top_count, sorted_domains.len())
        );
        for (domain, count) in sorted_domains.iter().take(top_count) {
            let display_domain = if args.redact {
                redact_domain(domain)
            } else {
                domain.to_string()
            };
            println!("- {}: {} visits", display_domain, format_number(**count));
        }
    }

    if let Some(bottom_count) = args.bottom {
        let mut bottom_sorted = sorted_domains.clone();
        bottom_sorted.sort_by(|a, b| a.1.cmp(b.1));

        println!(
            "\nBottom {} least visited domains:",
            std::cmp::min(bottom_count, bottom_sorted.len())
        );
        for (domain, count) in bottom_sorted.iter().take(bottom_count) {
            let display_domain = if args.redact {
                redact_domain(domain)
            } else {
                domain.to_string()
            };
            println!("- {}: {} visits", display_domain, format_number(**count));
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    setup_logging(args.verbose);

    // Validate arguments
    if let Some(top) = args.top {
        if top == 0 {
            anyhow::bail!("--top must be greater than 0");
        }
    }

    if let Some(bottom) = args.bottom {
        if bottom == 0 {
            anyhow::bail!("--bottom must be greater than 0");
        }
    }

    if let Some(workers) = args.workers {
        if workers == 0 {
            anyhow::bail!("--workers must be greater than 0");
        }
    }

    match analyze_browser_history(&args) {
        Ok(result) => {
            print_analysis_results(&result, &args);
            Ok(())
        }
        Err(e) => {
            error!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
