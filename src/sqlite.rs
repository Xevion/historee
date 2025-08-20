use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rayon::prelude::*;
use rusqlite::{Connection, Result as SqliteResult};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tracing::{info, warn};

use crate::args::Browser;

pub fn get_browser_history_path(browser: &Browser) -> Result<PathBuf> {
    let system = env::consts::OS;
    let home = env::var("HOME").or_else(|_| env::var("USERPROFILE"))?;

    let path = match (browser, system) {
        (Browser::Chrome, "windows") => {
            let local_app_data = env::var("LOCALAPPDATA")?;
            PathBuf::from(local_app_data).join("Google/Chrome/User Data/Default/History")
        }
        (Browser::Chrome, "macos") => {
            PathBuf::from(home).join("Library/Application Support/Google/Chrome/Default/History")
        }
        (Browser::Chrome, "linux") => {
            PathBuf::from(home).join(".config/google-chrome/Default/History")
        }

        (Browser::Edge, "windows") => {
            let local_app_data = env::var("LOCALAPPDATA")?;
            PathBuf::from(local_app_data).join("Microsoft/Edge/User Data/Default/History")
        }
        (Browser::Edge, "macos") => {
            PathBuf::from(home).join("Library/Application Support/Microsoft Edge/Default/History")
        }
        (Browser::Edge, "linux") => {
            PathBuf::from(home).join(".config/microsoft-edge/Default/History")
        }

        (Browser::Firefox, "windows") => {
            let app_data = env::var("APPDATA")?;
            PathBuf::from(app_data).join("Mozilla/Firefox")
        }
        (Browser::Firefox, "macos") => {
            PathBuf::from(home).join("Library/Application Support/Firefox/Profiles")
        }
        (Browser::Firefox, "linux") => PathBuf::from(home).join(".mozilla/firefox"),

        (Browser::Zen, "windows") => {
            let app_data = env::var("APPDATA")?;
            PathBuf::from(app_data).join("zen")
        }
        (Browser::Zen, "macos") => {
            PathBuf::from(home).join("Library/Application Support/zen/Profiles")
        }
        (Browser::Zen, "linux") => PathBuf::from(home).join(".zen"),

        (Browser::Vivaldi, "windows") => {
            let local_app_data = env::var("LOCALAPPDATA")?;
            PathBuf::from(local_app_data).join("Vivaldi/User Data/Default/History")
        }
        (Browser::Vivaldi, "macos") => {
            PathBuf::from(home).join("Library/Application Support/Vivaldi/Default/History")
        }
        (Browser::Vivaldi, "linux") => PathBuf::from(home).join(".config/vivaldi/default/History"),

        _ => anyhow::bail!(
            "Unsupported browser '{:?}' or operating system '{}'",
            browser,
            system
        ),
    };

    // Warn users on non-Windows platforms that browser handling hasn't been tested
    if system != "windows" {
        warn!(
            action = "platform_warning",
            component = "browser_path",
            platform = system,
            browser = ?browser,
            "Browser handling has not been tested on this platform. Paths may be incorrect."
        );
    }

    info!(action = "resolve", component = "browser_path", browser = ?browser, path = ?path, "Browser history path resolved");
    Ok(path)
}

pub fn get_firefox_history_path() -> Result<PathBuf> {
    get_firefox_based_history_path(&Browser::Firefox)
}

pub fn get_zen_history_path() -> Result<PathBuf> {
    get_firefox_based_history_path(&Browser::Zen)
}

fn get_firefox_based_history_path(browser: &Browser) -> Result<PathBuf> {
    let profiles_dir = get_browser_history_path(browser)?;

    if !profiles_dir.exists() {
        anyhow::bail!(
            "{} profiles directory not found at {:?}",
            browser,
            profiles_dir
        );
    }

    // Read profiles.ini to find the default profile
    let profiles_ini = profiles_dir.join("profiles.ini");
    if !profiles_ini.exists() {
        anyhow::bail!("{} profiles.ini not found at {:?}", browser, profiles_ini);
    }

    let profiles_content = fs::read_to_string(&profiles_ini)?;
    let mut default_profile_path = None;
    let mut profiles = std::collections::HashMap::new();

    // Parse profiles.ini to find the default profile and all profile paths
    let mut current_profile = None;
    for line in profiles_content.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            // This is a profile section
            current_profile = Some(line[1..line.len() - 1].to_string());
        } else if let Some(profile) = &current_profile {
            if line.starts_with("Path=") {
                let path = line.split('=').nth(1).unwrap_or("").trim();
                profiles.insert(profile.clone(), path.to_string());
            }
            // Note: We'll ignore the Default=1 flag and use our own logic
        }
    }

    info!(action = "debug", component = "profile_parsing", profiles = ?profiles, "Parsed profiles.ini");

    // First, try to find dev-edition profile (this is what actually exists)
    for (profile_name, path) in &profiles {
        if profile_name.contains("Profile0") || path.contains("dev-edition") {
            default_profile_path = Some(path.clone());
            info!(
                action = "debug",
                component = "profile_parsing",
                selected_profile = profile_name,
                path = path,
                "Selected dev-edition profile"
            );
            break;
        }
    }

    // If no dev-edition found, try to find one with "default" in the name
    if default_profile_path.is_none() {
        for (profile_name, path) in &profiles {
            if profile_name.to_lowercase().contains("default") {
                default_profile_path = Some(path.clone());
                info!(
                    action = "debug",
                    component = "profile_parsing",
                    selected_profile = profile_name,
                    path = path,
                    "Selected default profile"
                );
                break;
            }
        }
    }

    // If still no default, use the first profile
    if default_profile_path.is_none() {
        if let Some((profile_name, path)) = profiles.iter().next() {
            default_profile_path = Some(path.clone());
            info!(
                action = "debug",
                component = "profile_parsing",
                selected_profile = profile_name,
                path = path,
                "Selected first available profile"
            );
        }
    }

    let profile_path = default_profile_path.ok_or_else(|| {
        anyhow::anyhow!("Could not find default {} profile in profiles.ini", browser)
    })?;

    // The profile path is relative to the Firefox directory
    let history_path = profiles_dir.join(profile_path).join("places.sqlite");

    info!(action = "debug", component = "profile_parsing", final_path = ?history_path, "Final history path");

    if !history_path.exists() {
        anyhow::bail!(
            "{} history database not found at {:?}",
            browser,
            history_path
        );
    }

    Ok(history_path)
}

pub fn copy_history_database(history_path: &Path, temp_path: Option<&Path>) -> Result<PathBuf> {
    let start_time = Instant::now();
    info!(
        action = "start",
        component = "database_copy",
        "Copying browser history database"
    );

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
    info!(
        action = "complete",
        component = "database_copy",
        duration_ms = copy_time.as_millis(),
        "Database copy completed"
    );
    Ok(temp_path)
}

pub fn get_date_range(conn: &Connection) -> Result<(String, String, i64)> {
    let start_time = Instant::now();
    info!(
        action = "start",
        component = "date_range_query",
        "Querying visit date range"
    );

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

pub fn get_firefox_date_range(conn: &Connection) -> Result<(String, String, i64)> {
    let start_time = Instant::now();
    info!(
        action = "start",
        component = "firefox_date_range_query",
        "Querying Firefox visit date range"
    );

    let (earliest_timestamp, latest_timestamp): (Option<i64>, Option<i64>) = conn
        .query_row(
            "SELECT MIN(visit_date), MAX(visit_date) FROM moz_historyvisits",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .context("Failed to query Firefox visit dates")?;

    if let (Some(earliest), Some(latest)) = (earliest_timestamp, latest_timestamp) {
        // Firefox uses microseconds since 1970-01-01
        let unix_epoch = DateTime::parse_from_rfc3339("1970-01-01T00:00:00Z")?.with_timezone(&Utc);
        let earliest_date = unix_epoch + chrono::Duration::microseconds(earliest);
        let latest_date = unix_epoch + chrono::Duration::microseconds(latest);

        let days_between = (latest_date - earliest_date).num_days();
        let query_time = start_time.elapsed();

        info!(
            action = "complete",
            component = "firefox_date_range_query",
            earliest_date = earliest_date.format("%B %-d, %Y").to_string(),
            latest_date = latest_date.format("%B %-d, %Y").to_string(),
            days_between,
            duration_ms = query_time.as_millis(),
            "Firefox date range query completed"
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
            component = "firefox_date_range_query",
            duration_ms = query_time.as_millis(),
            "No Firefox visit data found"
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
    info!(
        action = "start",
        component = "domain_extraction",
        "Starting domain extraction from URLs"
    );

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

    info!(
        action = "configure",
        component = "domain_extraction",
        worker_count = max_workers,
        "Using workers for processing"
    );

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

pub fn extract_domains_from_firefox_urls(
    conn: &Connection,
    patterns: &[regex::Regex],
    max_workers: Option<usize>,
) -> Result<crate::stats::DomainStats> {
    let start_time = Instant::now();
    info!(
        action = "start",
        component = "firefox_domain_extraction",
        "Starting Firefox domain extraction from URLs"
    );

    let urls: Vec<String> = conn
        .prepare("SELECT url FROM moz_places WHERE url IS NOT NULL")?
        .query_map([], |row| row.get(0))?
        .collect::<SqliteResult<Vec<String>>>()?;

    let query_time = start_time.elapsed();
    info!(
        action = "query",
        component = "firefox_domain_extraction",
        url_count = urls.len(),
        duration_ms = query_time.as_millis(),
        "Found Firefox URLs to process"
    );

    let max_workers = max_workers.unwrap_or_else(|| {
        let cpu_count = num_cpus::get();
        std::cmp::min(cpu_count, 8)
    });

    info!(
        action = "configure",
        component = "firefox_domain_extraction",
        worker_count = max_workers,
        "Using workers for Firefox processing"
    );

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
        component = "firefox_domain_extraction",
        unique_domains = all_stats.unique_domains.len(),
        domains_removed = all_stats.domains_removed,
        "Firefox domain extraction completed"
    );
    info!(
        action = "timing",
        component = "firefox_domain_extraction",
        processing_time_ms = total_processing_time.as_millis(),
        total_time_ms = total_time.as_millis(),
        "Firefox domain extraction timing"
    );

    Ok(all_stats)
}
