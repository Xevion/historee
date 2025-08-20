use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use tracing::{info, warn};
use rayon::prelude::*;
use rusqlite::{Connection, Result as SqliteResult};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub fn get_browser_history_path(browser: &str) -> Result<PathBuf> {
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

    info!(action = "resolve", component = "browser_path", browser = browser, path = ?path, "Browser history path resolved");
    Ok(path)
}

pub fn copy_history_database(history_path: &Path, temp_path: Option<&Path>) -> Result<PathBuf> {
    let start_time = Instant::now();
    info!(action = "start", component = "database_copy", "Copying browser history database");

    let temp_path = temp_path.map(|p| p.to_path_buf()).unwrap_or_else(|| {
        PathBuf::from(&format!(
            "{}/browser_history_copy.db",
            env::var("HOME").unwrap_or_else(|_| env::var("USERPROFILE").unwrap_or_default())
        ))
    });

    info!(action = "copy", component = "database_copy", source = ?history_path, destination = ?temp_path, "Database copy paths");

    if !history_path.exists() {
        anyhow::bail!("History file not found at {:?}", history_path);
    }

    fs::copy(history_path, &temp_path)?;

    let copy_time = start_time.elapsed();
    info!(action = "complete", component = "database_copy", duration_ms = copy_time.as_millis(), "Database copy completed");
    Ok(temp_path)
}

pub fn get_date_range(conn: &Connection) -> Result<(String, String, i64)> {
    let start_time = Instant::now();
    info!(action = "start", component = "date_range_query", "Querying visit date range");

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
            action = "complete",
            component = "date_range_query",
            earliest_date = earliest_date.format("%B %-d, %Y").to_string(),
            latest_date = latest_date.format("%B %-d, %Y").to_string(),
            days_between,
            duration_ms = query_time.as_millis(),
            "Date range query completed"
        );

        Ok((
            earliest_date.format("%B %-d, %Y").to_string(),
            latest_date.format("%B %-d, %Y").to_string(),
            days_between,
        ))
    } else {
        let query_time = start_time.elapsed();
        warn!(
            action = "complete",
            component = "date_range_query",
            duration_ms = query_time.as_millis(),
            "No visit data found"
        );
        Ok((
            "No data available".to_string(),
            "No data available".to_string(),
            0,
        ))
    }
}

pub fn extract_domains_from_urls(
    conn: &Connection,
    patterns: &[regex::Regex],
    max_workers: Option<usize>,
) -> Result<crate::stats::DomainStats> {
    let start_time = Instant::now();
    info!(action = "start", component = "domain_extraction", "Starting domain extraction from URLs");

    let urls: Vec<String> = conn
        .prepare("SELECT url FROM urls")?
        .query_map([], |row| row.get(0))?
        .collect::<SqliteResult<Vec<String>>>()?;

    let query_time = start_time.elapsed();
    info!(
        action = "query",
        component = "domain_extraction",
        url_count = urls.len(),
        duration_ms = query_time.as_millis(),
        "Found URLs to process"
    );

    let max_workers = max_workers.unwrap_or_else(|| {
        let cpu_count = num_cpus::get();
        std::cmp::min(cpu_count, 8)
    });

    info!(action = "configure", component = "domain_extraction", worker_count = max_workers, "Using workers for processing");

    let processing_start = Instant::now();

    // Use Rayon's built-in parallel iterator with automatic work-stealing
    let batch_stats: Vec<crate::stats::DomainStats> = urls
        .into_par_iter()
        .fold(
            || crate::stats::DomainStats {
                unique_domains: Vec::new(),
                domain_counts: std::collections::HashMap::new(),
                domains_removed: 0,
            },
            |mut acc, url_str| {
                if let Ok(url) = url::Url::parse(&url_str) {
                    if let Some(host) = url.host_str() {
                        if !crate::domain::has_valid_tld(host) {
                            acc.domains_removed += 1;
                        } else {
                            let normalized_domain = crate::domain::normalize_domain(host, patterns);

                            if !crate::domain::has_valid_tld(&normalized_domain) {
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
    let mut all_stats = crate::stats::DomainStats {
        unique_domains: Vec::new(),
        domain_counts: std::collections::HashMap::new(),
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
        action = "complete",
        component = "domain_extraction",
        unique_domains = all_stats.unique_domains.len(),
        domains_removed = all_stats.domains_removed,
        "Domain extraction completed"
    );
    info!(
        action = "timing",
        component = "domain_extraction",
        processing_time_ms = total_processing_time.as_millis(),
        total_time_ms = total_time.as_millis(),
        "Domain extraction timing"
    );

    Ok(all_stats)
}
