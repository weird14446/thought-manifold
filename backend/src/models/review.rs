use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiReviewStatus {
    Pending,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiReviewDecision {
    Accept,
    MinorRevision,
    MajorRevision,
    Reject,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiReviewScores {
    pub overall_score: Option<i32>,
    pub novelty_score: Option<i32>,
    pub methodology_score: Option<i32>,
    pub clarity_score: Option<i32>,
    pub citation_integrity_score: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiReviewEditorial {
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiReviewPeer {
    pub summary: Option<String>,
    pub major_issues: Vec<String>,
    pub minor_issues: Vec<String>,
    pub required_revisions: Vec<String>,
    pub strengths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiReviewResponse {
    pub id: i64,
    pub post_id: i64,
    pub paper_version_id: Option<i64>,
    pub version_number: Option<i32>,
    pub status: AiReviewStatus,
    pub trigger: String,
    pub decision: Option<AiReviewDecision>,
    pub model: Option<String>,
    pub prompt_version: Option<String>,
    pub language_code: Option<String>,
    pub scores: AiReviewScores,
    pub editorial: AiReviewEditorial,
    pub peer: AiReviewPeer,
    pub input_snapshot: Option<Value>,
    pub raw_response: Option<Value>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiReviewListResponse {
    pub reviews: Vec<AiReviewResponse>,
    pub total: i64,
    pub page: i32,
    pub per_page: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiReviewSummary {
    pub id: i64,
    pub paper_version_id: Option<i64>,
    pub version_number: Option<i32>,
    pub status: AiReviewStatus,
    pub decision: Option<AiReviewDecision>,
    pub trigger: String,
    pub overall_score: Option<i32>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyPaperReviewItem {
    pub post_id: i64,
    pub title: String,
    pub category: String,
    pub paper_status: String,
    pub current_revision: i32,
    pub is_published: bool,
    pub published_at: Option<DateTime<Utc>>,
    pub latest_review: Option<AiReviewSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyPaperReviewListResponse {
    pub items: Vec<MyPaperReviewItem>,
    pub total: i64,
    pub page: i32,
    pub per_page: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiReviewMetricsSummary {
    pub total_reviews: i64,
    pub pending_reviews: i64,
    pub completed_reviews: i64,
    pub failed_reviews: i64,
}
