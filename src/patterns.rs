use anyhow::{Context, Result};
use log::{info, warn};
use regex::Regex;
use std::fs;
use std::path::Path;
use std::time::Instant;

// Include default patterns at compile time
const DEFAULT_PATTERNS_BYTES: &[u8] = include_bytes!("../default_domain_patterns.txt");

pub fn load_domain_patterns(pattern_file_path: Option<&Path>) -> Result<Vec<Regex>> {
    let start_time = Instant::now();
    info!("Starting domain pattern loading");

    let mut patterns = Vec::new();

    if let Some(path) = pattern_file_path {
        info!("Loading patterns from specified file: {path:?}");
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
            info!("Loading patterns from default file: {default_file:?}");
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

        // If no patterns loaded, use embedded defaults
        if patterns.is_empty() {
            info!("Using embedded default patterns");
            let default_content = std::str::from_utf8(DEFAULT_PATTERNS_BYTES)
                .context("Failed to decode embedded default patterns")?;

            for (line_num, line) in default_content.lines().enumerate() {
                let line = line.trim();
                if !line.is_empty() && !line.starts_with('#') {
                    match Regex::new(line) {
                        Ok(regex) => patterns.push(regex),
                        Err(e) => warn!("Invalid regex pattern at line {}: {}", line_num + 1, e),
                    }
                }
            }
            info!("Loaded {} patterns from embedded defaults", patterns.len());
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
