use regex::Regex;

pub fn has_valid_tld(domain: &str) -> bool {
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

pub fn normalize_domain(domain: &str, patterns: &[Regex]) -> String {
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
