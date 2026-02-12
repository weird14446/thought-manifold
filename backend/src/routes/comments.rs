use axum::{
    Router,
    extract::{Json, Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
};
use chrono::{DateTime, Utc};
use sqlx::FromRow;
use sqlx::MySqlPool;

use crate::models::{Comment, CommentResponse, CreateComment, User, UserResponse};
use crate::routes::auth::extract_current_user;

#[derive(Debug, FromRow)]
struct CommentWithAuthorRow {
    comment_id: i64,
    post_id: i64,
    author_id: i64,
    content: String,
    comment_created_at: DateTime<Utc>,
    comment_updated_at: Option<DateTime<Utc>>,
    user_id: i64,
    username: String,
    email: String,
    display_name: Option<String>,
    bio: Option<String>,
    avatar_url: Option<String>,
    is_admin: bool,
    user_created_at: DateTime<Utc>,
}

pub fn comments_routes() -> Router<MySqlPool> {
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
    State(pool): State<MySqlPool>,
    Path(post_id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    ensure_post_visibility(&pool, post_id).await?;

    let rows = sqlx::query_as::<_, CommentWithAuthorRow>(
        r#"
        SELECT
            c.id AS comment_id,
            c.post_id AS post_id,
            c.author_id AS author_id,
            c.content AS content,
            c.created_at AS comment_created_at,
            c.updated_at AS comment_updated_at,
            u.id AS user_id,
            u.username AS username,
            u.email AS email,
            u.display_name AS display_name,
            u.bio AS bio,
            u.avatar_url AS avatar_url,
            u.is_admin AS is_admin,
            u.created_at AS user_created_at
        FROM comments c
        JOIN users u ON u.id = c.author_id
        WHERE c.post_id = ?
        ORDER BY c.created_at ASC
        "#,
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

    let responses: Vec<CommentResponse> = rows
        .into_iter()
        .map(|row| {
            let author = UserResponse::from(User {
                id: row.user_id,
                username: row.username,
                email: row.email,
                hashed_password: None,
                google_id: None,
                display_name: row.display_name,
                bio: row.bio,
                avatar_url: row.avatar_url,
                is_admin: row.is_admin,
                created_at: row.user_created_at,
                updated_at: None,
            });

            CommentResponse {
                id: row.comment_id,
                post_id: row.post_id,
                author_id: row.author_id,
                author,
                content: row.content,
                created_at: row.comment_created_at,
                updated_at: row.comment_updated_at,
            }
        })
        .collect();

    Ok(Json(responses))
}

async fn create_comment(
    State(pool): State<MySqlPool>,
    headers: HeaderMap,
    Path(post_id): Path<i64>,
    Json(input): Json<CreateComment>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let current_user = extract_current_user(&pool, &headers).await?;
    ensure_post_visibility(&pool, post_id).await?;

    if input.content.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"detail": "Comment content is required"})),
        ));
    }

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
        .bind(result.last_insert_id() as i64)
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
    State(pool): State<MySqlPool>,
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

async fn ensure_post_visibility(
    pool: &MySqlPool,
    post_id: i64,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let post_row = sqlx::query_as::<_, (bool,)>("SELECT is_published FROM posts WHERE id = ?")
        .bind(post_id)
        .fetch_optional(pool)
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

    let (is_published,) = post_row;
    if !is_published {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"detail": "Post not found"})),
        ));
    }

    Ok(())
}
