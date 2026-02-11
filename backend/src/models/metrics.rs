use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostMetrics {
    pub citation_count: i64,
    pub metric_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorMetrics {
    pub user_id: i64,
    pub g_index: i64,
    pub total_citations: i64,
    pub paper_count: i64,
    pub formula: String,
    pub metric_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalMetrics {
    pub year: i32,
    pub impact_factor: Option<f64>,
    pub numerator_citations: i64,
    pub denominator_papers: i64,
    pub formula: String,
    pub metric_version: String,
}
