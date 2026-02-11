use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use super::metrics::PostMetrics;
use super::user::UserResponse;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Post {
    pub id: i64,
    pub title: String,
    pub content: String,
    pub summary: Option<String>,
    pub category: String,
    pub file_path: Option<String>,
    pub file_name: Option<String>,
    pub author_id: i64,
    pub view_count: i64,
    pub like_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct PostResponse {
    pub id: i64,
    pub title: String,
    pub content: String,
    pub summary: Option<String>,
    pub category: String,
    pub file_path: Option<String>,
    pub file_name: Option<String>,
    pub author_id: i64,
    pub author: UserResponse,
    pub view_count: i64,
    pub like_count: i64,
    pub user_liked: Option<bool>,
    pub metrics: PostMetrics,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct PostListResponse {
    pub posts: Vec<PostResponse>,
    pub total: i64,
    pub page: i32,
    pub per_page: i32,
}

#[derive(Debug, Deserialize, Default)]
pub struct PostQuery {
    pub page: Option<i32>,
    pub per_page: Option<i32>,
    pub category: Option<String>,
    pub search: Option<String>,
    pub tag: Option<String>,
}
