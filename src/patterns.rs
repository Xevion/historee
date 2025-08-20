use anyhow::Result;
use log::{info, warn};
use regex::Regex;
use std::fs;
use std::path::Path;
use std::time::Instant;

pub fn load_domain_patterns(pattern_file_path: Option<&Path>) -> Result<Vec<Regex>> {
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
