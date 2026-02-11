use axum::{
    extract::{Json, Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use sqlx::SqlitePool;
use chrono::Utc;

use crate::models::{Comment, CommentResponse, CreateComment, User, UserResponse};
use crate::routes::auth::extract_current_user;

pub fn comments_routes() -> Router<SqlitePool> {
    Router::new()
        .route(
            "/{post_id}/comments",
            get(list_comments).post(create_comment),
        )
        .route(
            "/{post_id}/comments/{comment_id}",
            axum::routing::delete(delete_comment),
        )
}

async fn list_comments(
    State(pool): State<SqlitePool>,
    Path(post_id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let comments = sqlx::query_as::<_, Comment>(
        "SELECT * FROM comments WHERE post_id = ? ORDER BY created_at ASC",
    )
    .bind(post_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"detail": e.to_string()})),
        )
    })?;

    let mut responses = Vec::new();
    for comment in comments {
        let author = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
            .bind(comment.author_id)
            .fetch_one(&pool)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"detail": e.to_string()})),
                )
            })?;

        responses.push(CommentResponse {
            id: comment.id,
            post_id: comment.post_id,
            author_id: comment.author_id,
            author: UserResponse::from(author),
            content: comment.content,
            created_at: comment.created_at,
            updated_at: comment.updated_at,
        });
    }

    Ok(Json(responses))
}

async fn create_comment(
    State(pool): State<SqlitePool>,
    headers: HeaderMap,
    Path(post_id): Path<i64>,
    Json(input): Json<CreateComment>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let current_user = extract_current_user(&pool, &headers).await?;

    if input.content.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"detail": "Comment content is required"})),
        ));
    }

    // Verify post exists
    let _post = sqlx::query("SELECT id FROM posts WHERE id = ?")
        .bind(post_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": e.to_string()})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"detail": "Post not found"})),
            )
        })?;

    let now = Utc::now();
    let result = sqlx::query(
        "INSERT INTO comments (post_id, author_id, content, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(post_id)
    .bind(current_user.id)
    .bind(input.content.trim())
    .bind(now)
    .execute(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"detail": e.to_string()})),
        )
    })?;

    let comment = sqlx::query_as::<_, Comment>("SELECT * FROM comments WHERE id = ?")
        .bind(result.last_insert_rowid())
        .fetch_one(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": e.to_string()})),
            )
        })?;

    Ok((
        StatusCode::CREATED,
        Json(CommentResponse {
            id: comment.id,
            post_id: comment.post_id,
            author_id: comment.author_id,
            author: UserResponse::from(current_user),
            content: comment.content,
            created_at: comment.created_at,
            updated_at: comment.updated_at,
        }),
    ))
}

async fn delete_comment(
    State(pool): State<SqlitePool>,
    headers: HeaderMap,
    Path((post_id, comment_id)): Path<(i64, i64)>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let current_user = extract_current_user(&pool, &headers).await?;

    let comment =
        sqlx::query_as::<_, Comment>("SELECT * FROM comments WHERE id = ? AND post_id = ?")
            .bind(comment_id)
            .bind(post_id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"detail": e.to_string()})),
                )
            })?
            .ok_or_else(|| {
                (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({"detail": "Comment not found"})),
                )
            })?;

    if comment.author_id != current_user.id {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"detail": "Not authorized to delete this comment"})),
        ));
    }

    sqlx::query("DELETE FROM comments WHERE id = ?")
        .bind(comment_id)
        .execute(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": e.to_string()})),
            )
        })?;

    Ok(Json(
        serde_json::json!({"message": "Comment deleted successfully"}),
    ))
}
