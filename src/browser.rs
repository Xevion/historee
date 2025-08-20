use anyhow::Result;
use log::{info, warn};
use rusqlite::Connection;
use std::fs;
use std::time::Instant;

use crate::{patterns, sqlite, stats::AnalysisResult, Args};

pub fn analyze_browser_history(args: &Args) -> Result<AnalysisResult> {
    let total_start_time = Instant::now();
    info!("Starting browser history analysis");

    let history_path = sqlite::get_browser_history_path(&args.browser)?;
    let temp_history_path =
        sqlite::copy_history_database(&history_path, args.temp_path.as_deref())?;

    let patterns = if args.no_patterns {
        Vec::new()
    } else {
        patterns::load_domain_patterns(args.patterns.as_deref())?
    };

    let conn = Connection::open(&temp_history_path)?;
    info!("Connected to database");

    let date_range = sqlite::get_date_range(&conn)?;
    let stats = sqlite::extract_domains_from_urls(&conn, &patterns, args.workers)?;

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

pub fn print_analysis_results(result: &AnalysisResult, args: &Args) {
    let (earliest_date, latest_date, days_between) = &result.date_range;

    println!("\n--- {} History Analysis ---", args.browser);

    if *days_between > 0 {
        println!(
            "Date range: {} to {} ({} days)",
            earliest_date,
            latest_date,
            crate::utils::format_number(*days_between as u32)
        );
    } else {
        println!("Date range: {} to {}", earliest_date, latest_date);
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
