use axum::{
    Router,
    extract::{Json, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{delete, get, put},
};
use chrono::{Datelike, Utc};
use serde::Deserialize;
use sqlx::MySqlPool;

use crate::ai_review::{fetch_admin_reviews, fetch_ai_review_metrics, parse_status_filter};
use crate::metrics::compute_impact_factor;
use crate::models::{User, UserResponse};
use crate::routes::auth::extract_current_user;
use crate::routes::comments::{apply_comment_delete_policy, find_comment_target};

// ============================
// Helper: Extract Admin User
// ============================
pub async fn extract_admin_user(
    pool: &MySqlPool,
    headers: &HeaderMap,
) -> Result<User, (StatusCode, Json<serde_json::Value>)> {
    let user = extract_current_user(pool, headers).await?;
    if !user.is_admin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"detail": "Admin access required"})),
        ));
    }
    Ok(user)
}

pub fn admin_routes() -> Router<MySqlPool> {
    Router::new()
        .route("/stats", get(admin_stats))
        .route("/users", get(admin_list_users))
        .route("/reviews", get(admin_list_reviews))
        .route("/users/{user_id}/role", put(admin_update_role))
        .route("/users/{user_id}", delete(admin_delete_user))
        .route("/posts/{post_id}", delete(admin_delete_post))
        .route("/comments/{comment_id}", delete(admin_delete_comment))
}

// ============================
// GET /admin/stats
// ============================
async fn admin_stats(
    State(pool): State<MySqlPool>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let _admin = extract_admin_user(&pool, &headers).await?;

    let user_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": e.to_string()})),
            )
        })?;

    let post_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM posts")
        .fetch_one(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": e.to_string()})),
            )
        })?;

    let comment_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM comments")
        .fetch_one(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": e.to_string()})),
            )
        })?;

    let total_views: (i64,) =
        sqlx::query_as("SELECT CAST(COALESCE(SUM(view_count), 0) AS SIGNED) FROM post_stats")
            .fetch_one(&pool)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"detail": e.to_string()})),
                )
            })?;

    let total_likes: (i64,) =
        sqlx::query_as("SELECT CAST(COALESCE(SUM(like_count), 0) AS SIGNED) FROM post_stats")
            .fetch_one(&pool)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"detail": e.to_string()})),
                )
            })?;

    let journal_metrics = compute_impact_factor(&pool, Utc::now().year())
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": e.to_string()})),
            )
        })?;

    let ai_review_metrics = fetch_ai_review_metrics(&pool).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"detail": e.to_string()})),
        )
    })?;

    Ok(Json(serde_json::json!({
        "total_users": user_count.0,
        "total_posts": post_count.0,
        "total_comments": comment_count.0,
        "total_views": total_views.0,
        "total_likes": total_likes.0,
        "journal_metrics": journal_metrics,
        "ai_review_metrics": ai_review_metrics,
    })))
}

#[derive(Debug, Deserialize)]
struct AdminReviewQuery {
    status: Option<String>,
    page: Option<i32>,
    per_page: Option<i32>,
}

async fn admin_list_reviews(
    State(pool): State<MySqlPool>,
    headers: HeaderMap,
    Query(query): Query<AdminReviewQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let _admin = extract_admin_user(&pool, &headers).await?;

    let status_filter = if let Some(status_raw) = query.status.as_deref() {
        Some(parse_status_filter(status_raw).ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"detail": "Invalid status filter. Use pending|completed|failed"})),
            )
        })?)
    } else {
        None
    };

    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).clamp(1, 100);

    let response = fetch_admin_reviews(&pool, status_filter, page, per_page)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": e.to_string()})),
            )
        })?;

    Ok(Json(response))
}

// ============================
// GET /admin/users
// ============================
async fn admin_list_users(
    State(pool): State<MySqlPool>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let _admin = extract_admin_user(&pool, &headers).await?;

    let users = sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY created_at DESC")
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": e.to_string()})),
            )
        })?;

    // Return full user info with post counts
    let mut user_list = Vec::new();
    for u in users {
        let post_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM posts WHERE author_id = ?")
            .bind(u.id)
            .fetch_one(&pool)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"detail": e.to_string()})),
                )
            })?;

        let comment_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM comments WHERE author_id = ?")
                .bind(u.id)
                .fetch_one(&pool)
                .await
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"detail": e.to_string()})),
                    )
                })?;

        let resp = UserResponse::from(u);
        user_list.push(serde_json::json!({
            "id": resp.id,
            "username": resp.username,
            "email": resp.email,
            "display_name": resp.display_name,
            "bio": resp.bio,
            "avatar_url": resp.avatar_url,
            "is_admin": resp.is_admin,
            "created_at": resp.created_at,
            "post_count": post_count.0,
            "comment_count": comment_count.0,
        }));
    }

    Ok(Json(serde_json::json!(user_list)))
}

// ============================
// PUT /admin/users/:id/role
// ============================
#[derive(Debug, Deserialize)]
struct UpdateRole {
    is_admin: bool,
}

async fn admin_update_role(
    State(pool): State<MySqlPool>,
    headers: HeaderMap,
    Path(user_id): Path<i64>,
    Json(input): Json<UpdateRole>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let admin = extract_admin_user(&pool, &headers).await?;

    // Prevent self-demotion
    if admin.id == user_id && !input.is_admin {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"detail": "Cannot remove your own admin role"})),
        ));
    }

    // Verify target user exists
    let _target = sqlx::query("SELECT id FROM users WHERE id = ?")
        .bind(user_id)
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
                Json(serde_json::json!({"detail": "User not found"})),
            )
        })?;

    sqlx::query("UPDATE users SET is_admin = ? WHERE id = ?")
        .bind(input.is_admin)
        .bind(user_id)
        .execute(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": e.to_string()})),
            )
        })?;

    let updated_user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": e.to_string()})),
            )
        })?;

    Ok(Json(UserResponse::from(updated_user)))
}

// ============================
// DELETE /admin/users/:id
// ============================
async fn admin_delete_user(
    State(pool): State<MySqlPool>,
    headers: HeaderMap,
    Path(user_id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let admin = extract_admin_user(&pool, &headers).await?;

    // Prevent self-deletion
    if admin.id == user_id {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"detail": "Cannot delete your own account"})),
        ));
    }

    // Delete user's comments, post_likes, posts, then user
    sqlx::query("DELETE FROM comments WHERE author_id = ?")
        .bind(user_id)
        .execute(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": e.to_string()})),
            )
        })?;

    sqlx::query("DELETE FROM post_likes WHERE user_id = ?")
        .bind(user_id)
        .execute(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": e.to_string()})),
            )
        })?;

    sqlx::query(
        r#"
        DELETE FROM post_citations
        WHERE citing_post_id IN (SELECT id FROM posts WHERE author_id = ?)
           OR cited_post_id IN (SELECT id FROM posts WHERE author_id = ?)
        "#,
    )
    .bind(user_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"detail": e.to_string()})),
        )
    })?;

    sqlx::query("DELETE FROM posts WHERE author_id = ?")
        .bind(user_id)
        .execute(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": e.to_string()})),
            )
        })?;

    let result = sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id)
        .execute(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": e.to_string()})),
            )
        })?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"detail": "User not found"})),
        ));
    }

    Ok(Json(serde_json::json!({"detail": "User deleted"})))
}

// ============================
// DELETE /admin/posts/:id
// ============================
async fn admin_delete_post(
    State(pool): State<MySqlPool>,
    headers: HeaderMap,
    Path(post_id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let _admin = extract_admin_user(&pool, &headers).await?;

    // Delete associated data
    sqlx::query("DELETE FROM comments WHERE post_id = ?")
        .bind(post_id)
        .execute(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": e.to_string()})),
            )
        })?;

    sqlx::query("DELETE FROM post_likes WHERE post_id = ?")
        .bind(post_id)
        .execute(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": e.to_string()})),
            )
        })?;

    sqlx::query("DELETE FROM post_citations WHERE citing_post_id = ? OR cited_post_id = ?")
        .bind(post_id)
        .bind(post_id)
        .execute(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": e.to_string()})),
            )
        })?;

    let result = sqlx::query("DELETE FROM posts WHERE id = ?")
        .bind(post_id)
        .execute(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": e.to_string()})),
            )
        })?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"detail": "Post not found"})),
        ));
    }

    Ok(Json(serde_json::json!({"detail": "Post deleted"})))
}

// ============================
// DELETE /admin/comments/:id
// ============================
async fn admin_delete_comment(
    State(pool): State<MySqlPool>,
    headers: HeaderMap,
    Path(comment_id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let _admin = extract_admin_user(&pool, &headers).await?;

    let comment = find_comment_target(&pool, comment_id, None)
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

    let delete_mode = apply_comment_delete_policy(&pool, &comment)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": e.to_string()})),
            )
        })?;

    Ok(Json(serde_json::json!({
        "detail": "Comment deleted",
        "delete_mode": delete_mode.as_str()
    })))
}
