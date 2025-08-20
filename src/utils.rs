use std::env;

pub fn setup_logging(verbose: bool) {
    if verbose {
        env::set_var("RUST_LOG", "info");
    } else {
        env::set_var("RUST_LOG", "error");
    }
    env_logger::init();
}

pub fn format_number(num: u32) -> String {
    num.to_string()
        .as_bytes()
        .rchunks(3)
        .rev()
        .map(|chunk| std::str::from_utf8(chunk).unwrap())
        .collect::<Vec<_>>()
        .join(",")
}

pub fn redact_domain(domain: &str) -> String {
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

pub fn validate_args(args: &crate::args::Args) -> anyhow::Result<()> {
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

    Ok(())
}
