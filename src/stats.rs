use std::collections::HashMap;

#[derive(Debug)]
pub struct DomainStats {
    pub unique_domains: Vec<String>,
    pub domain_counts: HashMap<String, u32>,
    pub domains_removed: u32,
}

#[derive(Debug)]
pub struct AnalysisResult {
    pub date_range: (String, String, i64),
    pub stats: DomainStats,
}
