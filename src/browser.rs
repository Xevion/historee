use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::Connection;
use std::fs;
use std::time::Instant;
use tracing::{info, warn};

use crate::{args::Browser, patterns, sqlite, stats::AnalysisResult, Args};

pub fn analyze_browser_history(args: &Args) -> Result<AnalysisResult> {
    if args.all_browsers {
        analyze_all_browsers(args)
    } else {
        analyze_single_browser(&args.browser, args)
    }
}

fn analyze_single_browser(browser: &Browser, args: &Args) -> Result<AnalysisResult> {
    let total_start_time = Instant::now();
    info!(
        action = "start",
        component = "browser_analysis",
        browser = ?browser,
        "Starting browser history analysis"
    );

    let history_path = match browser {
        Browser::Firefox => sqlite::get_firefox_history_path()?,
        Browser::Zen => sqlite::get_zen_history_path()?,
        _ => sqlite::get_browser_history_path(browser)?,
    };

    let temp_history_path =
        sqlite::copy_history_database(&history_path, args.temp_path.as_deref())?;

    let patterns = if args.no_patterns {
        Vec::new()
    } else {
        patterns::load_domain_patterns(args.patterns.as_deref())?
    };

    let conn = Connection::open(&temp_history_path)?;
    info!(
        action = "connect",
        component = "database",
        "Connected to database"
    );

    let date_range = match browser {
        Browser::Firefox | Browser::Zen => sqlite::get_firefox_date_range(&conn)?,
        _ => sqlite::get_date_range(&conn)?,
    };

    let stats = match browser {
        Browser::Firefox | Browser::Zen => {
            sqlite::extract_domains_from_firefox_urls(&conn, &patterns, args.workers)?
        }
        _ => sqlite::extract_domains_from_urls(&conn, &patterns, args.workers)?,
    };

    info!(
        action = "disconnect",
        component = "database",
        "Closing database connection"
    );
    drop(conn);

    // Clean up temporary file
    if let Err(e) = fs::remove_file(&temp_history_path) {
        warn!(action = "cleanup", component = "temp_file", error = %e, "Failed to remove temporary file");
    }

    let total_time = total_start_time.elapsed();
    info!(
        action = "complete",
        component = "browser_analysis",
        browser = ?browser,
        duration_ms = total_time.as_millis(),
        "Analysis completed successfully"
    );

    Ok(AnalysisResult { date_range, stats })
}

fn analyze_all_browsers(args: &Args) -> Result<AnalysisResult> {
    let browsers = [
        Browser::Chrome,
        Browser::Edge,
        Browser::Firefox,
        Browser::Vivaldi,
        Browser::Zen,
    ];
    let mut all_stats = crate::stats::DomainStats {
        unique_domains: Vec::new(),
        domain_counts: std::collections::HashMap::new(),
        domains_removed: 0,
    };

    let mut earliest_date_str = None;
    let mut latest_date_str = None;
    let mut earliest_timestamp: Option<DateTime<Utc>> = None;
    let mut latest_timestamp: Option<DateTime<Utc>> = None;

    for browser in &browsers {
        match analyze_single_browser(browser, args) {
            Ok(result) => {
                // Merge stats
                for (domain, count) in &result.stats.domain_counts {
                    *all_stats.domain_counts.entry(domain.clone()).or_insert(0) += count;
                }
                all_stats.domains_removed += result.stats.domains_removed;

                // Update date range - only if we have valid data
                let (earliest, latest, _) = &result.date_range;
                if earliest != "No data available" && latest != "No data available" {
                    // Try to parse the date strings to compare them properly
                    if let (Ok(earliest_parsed), Ok(latest_parsed)) = (
                        chrono::NaiveDate::parse_from_str(earliest, "%B %d, %Y")
                            .or_else(|_| chrono::NaiveDate::parse_from_str(earliest, "%B %-d, %Y")),
                        chrono::NaiveDate::parse_from_str(latest, "%B %d, %Y")
                            .or_else(|_| chrono::NaiveDate::parse_from_str(latest, "%B %-d, %Y")),
                    ) {
                        let earliest_utc = earliest_parsed.and_hms_opt(0, 0, 0).unwrap().and_utc();
                        let latest_utc = latest_parsed.and_hms_opt(0, 0, 0).unwrap().and_utc();

                        if earliest_timestamp.is_none()
                            || earliest_utc < earliest_timestamp.unwrap()
                        {
                            earliest_timestamp = Some(earliest_utc);
                            earliest_date_str = Some(earliest.clone());
                        }
                        if latest_timestamp.is_none() || latest_utc > latest_timestamp.unwrap() {
                            latest_timestamp = Some(latest_utc);
                            latest_date_str = Some(latest.clone());
                        }
                    }
                }
            }
            Err(e) => {
                warn!(browser = ?browser, error = %e, "Failed to analyze browser");
            }
        }
    }

    // Update unique_domains from the final domain_counts
    all_stats.unique_domains = all_stats.domain_counts.keys().cloned().collect();

    // Calculate the total days between earliest and latest
    let total_days = if let (Some(earliest), Some(latest)) = (earliest_timestamp, latest_timestamp)
    {
        (latest - earliest).num_days()
    } else {
        0
    };

    let date_range = (
        earliest_date_str.unwrap_or_else(|| "No data available".to_string()),
        latest_date_str.unwrap_or_else(|| "No data available".to_string()),
        total_days,
    );

    Ok(AnalysisResult {
        date_range,
        stats: all_stats,
    })
}

pub fn print_analysis_results(result: &AnalysisResult, args: &Args) {
    let (earliest_date, latest_date, days_between) = &result.date_range;

    let browser_name = if args.all_browsers {
        "All Browsers".to_string()
    } else {
        args.browser.to_string()
    };

    println!("\n--- {} History Analysis ---", browser_name);

    if *days_between > 0 {
        println!(
            "Date range: {} to {} ({} days)",
            earliest_date,
            latest_date,
            crate::utils::format_number(*days_between as u32)
        );
    } else {
        println!("Date range: {earliest_date} to {latest_date}");
    }

    println!(
        "Total unique domains found: {}",
        crate::utils::format_number(result.stats.unique_domains.len() as u32)
    );
    println!(
        "Domains removed (no valid TLD): {}",
        crate::utils::format_number(result.stats.domains_removed)
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
                crate::utils::redact_domain(domain)
            } else {
                domain.to_string()
            };
            println!(
                "- {}: {} visits",
                display_domain,
                crate::utils::format_number(**count)
            );
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
                crate::utils::redact_domain(domain)
            } else {
                domain.to_string()
            };
            println!(
                "- {}: {} visits",
                display_domain,
                crate::utils::format_number(**count)
            );
        }
    }
}
