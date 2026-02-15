use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PaperVersion {
    pub id: i64,
    pub post_id: i64,
    pub version_number: i32,
    pub title: String,
    pub content: String,
    pub summary: Option<String>,
    pub github_url: Option<String>,
    pub file_path: Option<String>,
    pub file_name: Option<String>,
    pub tags_json: Option<String>,
    pub citations_json: Option<String>,
    pub submitted_by: Option<i64>,
    pub submitted_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperVersionResponse {
    pub id: i64,
    pub post_id: i64,
    pub version_number: i32,
    pub title: String,
    pub content: String,
    pub summary: Option<String>,
    pub github_url: Option<String>,
    pub file_path: Option<String>,
    pub file_name: Option<String>,
    pub tags: Vec<String>,
    pub citations: Vec<i64>,
    pub submitted_by: Option<i64>,
    pub submitted_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperVersionListResponse {
    pub versions: Vec<PaperVersionResponse>,
    pub total: i64,
    pub limit: i32,
    pub offset: i32,
}
