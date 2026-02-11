use axum::{
    extract::{Json, Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::models::{User, UserResponse};
use crate::routes::auth::extract_current_user;

#[derive(Debug, Deserialize)]
pub struct UpdateProfile {
    pub display_name: Option<String>,
    pub bio: Option<String>,
}

pub fn users_routes() -> Router<SqlitePool> {
    Router::new()
        .route("/", get(list_users))
        .route("/me", axum::routing::put(update_profile))
        .route("/{user_id}", get(get_user))
        .route("/{user_id}/posts", get(get_user_posts))
}

async fn list_users(
    State(pool): State<SqlitePool>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let users = sqlx::query_as::<_, User>("SELECT * FROM users LIMIT 20")
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()})))
        })?;

    let responses: Vec<UserResponse> = users.into_iter().map(UserResponse::from).collect();
    Ok(Json(responses))
}

async fn get_user(
    State(pool): State<SqlitePool>,
    Path(user_id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()})))
        })?
        .ok_or_else(|| {
            (StatusCode::NOT_FOUND, Json(serde_json::json!({"detail": "User not found"})))
        })?;

    Ok(Json(UserResponse::from(user)))
}

async fn update_profile(
    State(pool): State<SqlitePool>,
    headers: HeaderMap,
    Json(input): Json<UpdateProfile>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let current_user = extract_current_user(&pool, &headers).await?;

    let display_name = input
        .display_name
        .as_deref()
        .map(|s| s.trim().to_string())
        .or(current_user.display_name.clone());

    let bio = match &input.bio {
        Some(b) => {
            let trimmed = b.trim();
            if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
        }
        None => current_user.bio.clone(),
    };

    let now = chrono::Utc::now();

    sqlx::query("UPDATE users SET display_name = ?, bio = ?, updated_at = ? WHERE id = ?")
        .bind(display_name)
        .bind(&bio)
        .bind(now)
        .bind(current_user.id)
        .execute(&pool)
        .await
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()})))
        })?;

    let updated_user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(current_user.id)
        .fetch_one(&pool)
        .await
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()})))
        })?;

    Ok(Json(UserResponse::from(updated_user)))
}

async fn get_user_posts(
    State(pool): State<SqlitePool>,
    Path(user_id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    // Verify user exists
    let _user = sqlx::query("SELECT id FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()})))
        })?
        .ok_or_else(|| {
            (StatusCode::NOT_FOUND, Json(serde_json::json!({"detail": "User not found"})))
        })?;

    let posts = sqlx::query_as::<_, crate::models::post::Post>(
        "SELECT * FROM posts WHERE author_id = ? ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()})))
    })?;

    // Build responses with author info
    let author = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()})))
        })?;

    let author_resp = UserResponse::from(author);

    let responses: Vec<serde_json::Value> = posts
        .into_iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id,
                "title": p.title,
                "content": p.content,
                "summary": p.summary,
                "category": p.category,
                "file_path": p.file_path,
                "file_name": p.file_name,
                "author_id": p.author_id,
                "author": author_resp,
                "view_count": p.view_count,
                "like_count": p.like_count,
                "created_at": p.created_at,
                "updated_at": p.updated_at,
            })
        })
        .collect();

    Ok(Json(responses))
}
