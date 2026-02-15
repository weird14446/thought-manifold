use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use serde::Deserialize;
use sqlx::MySqlPool;

use crate::ai_review::{
    ReviewTrigger, fetch_latest_review, fetch_post_reviews, fetch_user_review_center,
    schedule_review,
};
use crate::models::PAPER_STATUS_SUBMITTED;
use crate::routes::auth::extract_current_user;

pub fn reviews_routes() -> Router<MySqlPool> {
    Router::new()
        .route("/{post_id}/reviews/latest", get(get_latest_post_review))
        .route("/{post_id}/reviews", get(list_post_reviews))
        .route("/{post_id}/reviews/rerun", post(rerun_post_review))
}

pub fn review_center_routes() -> Router<MySqlPool> {
    Router::new().route("/mine", get(list_my_paper_reviews))
}

#[derive(Debug, Deserialize)]
struct ReviewListQuery {
    limit: Option<i32>,
    offset: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct MyReviewCenterQuery {
    page: Option<i32>,
    per_page: Option<i32>,
}

async fn get_latest_post_review(
    State(pool): State<MySqlPool>,
    headers: HeaderMap,
    Path(post_id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let _ = ensure_review_access(&pool, &headers, post_id).await?;

    let review = fetch_latest_review(&pool, post_id)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"detail": "No AI review found for this post"})),
            )
        })?;

    Ok(Json(review))
}

async fn list_post_reviews(
    State(pool): State<MySqlPool>,
    headers: HeaderMap,
    Path(post_id): Path<i64>,
    Query(query): Query<ReviewListQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let _ = ensure_review_access(&pool, &headers, post_id).await?;

    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let offset = query.offset.unwrap_or(0).max(0);
    let response = fetch_post_reviews(&pool, post_id, limit, offset)
        .await
        .map_err(internal_error)?;

    Ok(Json(response))
}

async fn rerun_post_review(
    State(pool): State<MySqlPool>,
    headers: HeaderMap,
    Path(post_id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let category_code = ensure_review_access(&pool, &headers, post_id).await?;
    if category_code != "paper" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "detail": "AI review is only available for paper category posts"
            })),
        ));
    }

    let latest_version_row =
        sqlx::query_as::<_, (Option<i64>,)>("SELECT latest_paper_version_id FROM posts WHERE id = ?")
            .bind(post_id)
            .fetch_one(&pool)
            .await
            .map_err(internal_error)?;
    let latest_paper_version_id = latest_version_row.0.ok_or_else(|| {
        (
            StatusCode::CONFLICT,
            Json(serde_json::json!({"detail": "No submitted revision available for rerun"})),
        )
    })?;

    let now = chrono::Utc::now();
    sqlx::query(
        r#"
        UPDATE posts
        SET
            paper_status = ?,
            is_published = FALSE,
            published_at = NULL,
            updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(PAPER_STATUS_SUBMITTED)
    .bind(now)
    .bind(post_id)
    .execute(&pool)
    .await
    .map_err(internal_error)?;

    let review_id = schedule_review(&pool, post_id, Some(latest_paper_version_id), ReviewTrigger::Manual)
        .await
        .map_err(internal_error)?;

    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "detail": "AI review scheduled",
            "review_id": review_id
        })),
    ))
}

async fn list_my_paper_reviews(
    State(pool): State<MySqlPool>,
    headers: HeaderMap,
    Query(query): Query<MyReviewCenterQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let current_user = extract_current_user(&pool, &headers).await?;
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).clamp(1, 100);

    let response = fetch_user_review_center(&pool, current_user.id, page, per_page)
        .await
        .map_err(internal_error)?;

    Ok(Json(response))
}

async fn ensure_review_access(
    pool: &MySqlPool,
    headers: &HeaderMap,
    post_id: i64,
) -> Result<String, (StatusCode, Json<serde_json::Value>)> {
    let current_user = extract_current_user(pool, headers).await?;

    let row = sqlx::query_as::<_, (i64, String)>(
        r#"
        SELECT p.author_id, c.code
        FROM posts p
        JOIN post_categories c ON c.id = p.category_id
        WHERE p.id = ?
        "#,
    )
    .bind(post_id)
    .fetch_optional(pool)
    .await
    .map_err(internal_error)?
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"detail": "Post not found"})),
        )
    })?;

    let (author_id, category_code) = row;
    if current_user.id != author_id && !current_user.is_admin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(
                serde_json::json!({"detail": "Not authorized to access AI reviews for this post"}),
            ),
        ));
    }

    Ok(category_code)
}

fn internal_error<E: ToString>(error: E) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({"detail": error.to_string()})),
    )
}
