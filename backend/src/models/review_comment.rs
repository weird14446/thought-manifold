use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::models::UserResponse;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ReviewComment {
    pub id: i64,
    pub post_id: i64,
    pub paper_version_id: Option<i64>,
    pub author_id: i64,
    pub parent_comment_id: Option<i64>,
    pub content: String,
    pub is_deleted: bool,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewCommentResponse {
    pub id: i64,
    pub post_id: i64,
    pub paper_version_id: Option<i64>,
    pub author_id: i64,
    pub parent_comment_id: Option<i64>,
    pub author: UserResponse,
    pub content: String,
    pub is_deleted: bool,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateReviewComment {
    pub content: String,
    pub parent_comment_id: Option<i64>,
    pub paper_version_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReviewCommentListResponse {
    pub comments: Vec<ReviewCommentResponse>,
    pub total: i64,
    pub limit: i32,
    pub offset: i32,
}
