use anyhow::{Context, Result};
use regex::Regex;
use std::fs;
use std::path::Path;
use std::time::Instant;
use tracing::{info, warn};

// Include default patterns at compile time
const DEFAULT_PATTERNS_BYTES: &[u8] = include_bytes!("../default_domain_patterns.txt");

pub fn load_domain_patterns(pattern_file_path: Option<&Path>) -> Result<Vec<Regex>> {
    let start_time = Instant::now();
    info!(
        action = "start",
        component = "pattern_loading",
        "Starting domain pattern loading"
    );

    let mut patterns = Vec::new();

    if let Some(path) = pattern_file_path {
        info!(action = "load", component = "pattern_file", file_path = ?path, "Loading patterns from specified file");
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
        info!(action = "loaded", component = "pattern_file", pattern_count = patterns.len(), file_path = ?path, "Loaded patterns from file");
    } else {
        // Try default file
        let default_file = Path::new("domain_patterns.txt");
        if default_file.exists() {
            info!(action = "load", component = "default_pattern_file", file_path = ?default_file, "Loading patterns from default file");
            let content = fs::read_to_string(default_file)?;
            for (line_num, line) in content.lines().enumerate() {
                let line = line.trim();
                if !line.is_empty() && !line.starts_with('#') {
                    match Regex::new(line) {
                        Ok(regex) => patterns.push(regex),
                        Err(e) => {
                            warn!(action = "parse", component = "regex_pattern", line_number = line_num + 1, error = %e, "Invalid regex pattern")
                        }
                    }
                }
            }
            info!(action = "loaded", component = "default_pattern_file", pattern_count = patterns.len(), file_path = ?default_file, "Loaded patterns from default file");
        }

        // If no patterns loaded, use embedded defaults
        if patterns.is_empty() {
            info!(
                action = "load",
                component = "embedded_patterns",
                "Using embedded default patterns"
            );
            let default_content = std::str::from_utf8(DEFAULT_PATTERNS_BYTES)
                .context("Failed to decode embedded default patterns")?;

            for (line_num, line) in default_content.lines().enumerate() {
                let line = line.trim();
                if !line.is_empty() && !line.starts_with('#') {
                    match Regex::new(line) {
                        Ok(regex) => patterns.push(regex),
                        Err(e) => {
                            warn!(action = "parse", component = "embedded_regex_pattern", line_number = line_num + 1, error = %e, "Invalid regex pattern")
                        }
                    }
                }
            }
            info!(
                action = "loaded",
                component = "embedded_patterns",
                pattern_count = patterns.len(),
                "Loaded patterns from embedded defaults"
            );
        }
    }

    let pattern_time = start_time.elapsed();
    info!(
        action = "complete",
        component = "pattern_loading",
        pattern_count = patterns.len(),
        duration_ms = pattern_time.as_millis(),
        "Successfully compiled patterns"
    );
    Ok(patterns)
}

pub fn init_default_patterns() -> Result<()> {
    let default_file = Path::new("domain_patterns.txt");

    if default_file.exists() {
        anyhow::bail!(
            "domain_patterns.txt already exists. Remove it first if you want to reinitialize."
        );
    }

    let default_content = std::str::from_utf8(DEFAULT_PATTERNS_BYTES)
        .context("Failed to decode embedded default patterns")?;

    fs::write(default_file, default_content)?;
    println!("Created domain_patterns.txt with default patterns");

    Ok(())
}
