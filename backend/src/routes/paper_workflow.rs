use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{delete, get},
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::{FromRow, MySqlPool};

use crate::models::{
    CreateReviewComment, PaperVersion, PaperVersionListResponse, PaperVersionResponse,
    ReviewComment, ReviewCommentListResponse, ReviewCommentResponse, User, UserResponse,
};
use crate::routes::auth::extract_current_user;

#[derive(Debug, Deserialize)]
struct VersionListQuery {
    limit: Option<i32>,
    offset: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct ReviewCommentListQuery {
    paper_version_id: Option<i64>,
    limit: Option<i32>,
    offset: Option<i32>,
}

#[derive(Debug, FromRow)]
struct PostAccessRow {
    author_id: i64,
    is_published: bool,
    category_code: String,
    latest_paper_version_id: Option<i64>,
}

#[derive(Debug, FromRow)]
struct ReviewCommentWithAuthorRow {
    comment_id: i64,
    post_id: i64,
    paper_version_id: Option<i64>,
    author_id: i64,
    parent_comment_id: Option<i64>,
    content: String,
    is_deleted: bool,
    deleted_at: Option<DateTime<Utc>>,
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

#[derive(Debug, FromRow)]
struct ReviewCommentDeleteTarget {
    id: i64,
    post_id: i64,
    author_id: i64,
    parent_comment_id: Option<i64>,
}

#[derive(Debug, Clone, Copy)]
enum DeleteReviewCommentMode {
    Soft,
    Hard,
}

impl DeleteReviewCommentMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Soft => "soft",
            Self::Hard => "hard",
        }
    }
}

pub fn paper_workflow_routes() -> Router<MySqlPool> {
    Router::new()
        .route("/{post_id}/versions", get(list_paper_versions))
        .route("/{post_id}/versions/latest", get(get_latest_paper_version))
        .route("/{post_id}/review-comments", get(list_review_comments).post(create_review_comment))
        .route(
            "/{post_id}/review-comments/{comment_id}",
            delete(delete_review_comment),
        )
}

async fn list_paper_versions(
    State(pool): State<MySqlPool>,
    headers: HeaderMap,
    Path(post_id): Path<i64>,
    Query(query): Query<VersionListQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let current_user = extract_current_user(&pool, &headers).await?;
    let post_access = fetch_post_access(&pool, post_id).await?;
    ensure_paper_author_or_admin(&current_user, &post_access)?;

    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let offset = query.offset.unwrap_or(0).max(0);

    let rows = sqlx::query_as::<_, PaperVersion>(
        r#"
        SELECT
            id,
            post_id,
            CAST(version_number AS SIGNED) AS version_number,
            title,
            content,
            summary,
            github_url,
            file_path,
            file_name,
            CAST(tags_json AS CHAR) AS tags_json,
            CAST(citations_json AS CHAR) AS citations_json,
            submitted_by,
            submitted_at,
            created_at
        FROM paper_versions
        WHERE post_id = ?
        ORDER BY version_number DESC, id DESC
        LIMIT ? OFFSET ?
        "#,
    )
    .bind(post_id)
    .bind(i64::from(limit))
    .bind(i64::from(offset))
    .fetch_all(&pool)
    .await
    .map_err(internal_error)?;

    let (total,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM paper_versions WHERE post_id = ?")
        .bind(post_id)
        .fetch_one(&pool)
        .await
        .map_err(internal_error)?;

    let versions = rows.into_iter().map(map_paper_version).collect();
    Ok(Json(PaperVersionListResponse {
        versions,
        total,
        limit,
        offset,
    }))
}

async fn get_latest_paper_version(
    State(pool): State<MySqlPool>,
    headers: HeaderMap,
    Path(post_id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let current_user = extract_current_user(&pool, &headers).await?;
    let post_access = fetch_post_access(&pool, post_id).await?;
    ensure_paper_author_or_admin(&current_user, &post_access)?;

    let row = sqlx::query_as::<_, PaperVersion>(
        r#"
        SELECT
            id,
            post_id,
            CAST(version_number AS SIGNED) AS version_number,
            title,
            content,
            summary,
            github_url,
            file_path,
            file_name,
            CAST(tags_json AS CHAR) AS tags_json,
            CAST(citations_json AS CHAR) AS citations_json,
            submitted_by,
            submitted_at,
            created_at
        FROM paper_versions
        WHERE post_id = ?
        ORDER BY version_number DESC, id DESC
        LIMIT 1
        "#,
    )
    .bind(post_id)
    .fetch_optional(&pool)
    .await
    .map_err(internal_error)?
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"detail": "No paper version found"})),
        )
    })?;

    Ok(Json(map_paper_version(row)))
}

async fn list_review_comments(
    State(pool): State<MySqlPool>,
    headers: HeaderMap,
    Path(post_id): Path<i64>,
    Query(query): Query<ReviewCommentListQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let current_user = extract_current_user(&pool, &headers).await?;
    let post_access = fetch_post_access(&pool, post_id).await?;
    ensure_review_comment_access(&current_user, &post_access)?;

    let target_version_id =
        resolve_target_version_id(&pool, post_id, post_access.latest_paper_version_id, query.paper_version_id)
            .await?;
    let limit = query.limit.unwrap_or(100).clamp(1, 200);
    let offset = query.offset.unwrap_or(0).max(0);

    let rows = sqlx::query_as::<_, ReviewCommentWithAuthorRow>(
        r#"
        SELECT
            rc.id AS comment_id,
            rc.post_id AS post_id,
            rc.paper_version_id AS paper_version_id,
            rc.author_id AS author_id,
            rc.parent_comment_id AS parent_comment_id,
            rc.content AS content,
            rc.is_deleted AS is_deleted,
            rc.deleted_at AS deleted_at,
            rc.created_at AS comment_created_at,
            rc.updated_at AS comment_updated_at,
            u.id AS user_id,
            u.username AS username,
            u.email AS email,
            u.display_name AS display_name,
            u.bio AS bio,
            u.avatar_url AS avatar_url,
            u.is_admin AS is_admin,
            u.created_at AS user_created_at
        FROM paper_review_comments rc
        JOIN users u ON u.id = rc.author_id
        WHERE rc.post_id = ? AND rc.paper_version_id <=> ?
        ORDER BY rc.created_at ASC
        LIMIT ? OFFSET ?
        "#,
    )
    .bind(post_id)
    .bind(target_version_id)
    .bind(i64::from(limit))
    .bind(i64::from(offset))
    .fetch_all(&pool)
    .await
    .map_err(internal_error)?;

    let (total,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM paper_review_comments WHERE post_id = ? AND paper_version_id <=> ?",
    )
    .bind(post_id)
    .bind(target_version_id)
    .fetch_one(&pool)
    .await
    .map_err(internal_error)?;

    let comments = rows.into_iter().map(map_review_comment_row).collect();
    Ok(Json(ReviewCommentListResponse {
        comments,
        total,
        limit,
        offset,
    }))
}

async fn create_review_comment(
    State(pool): State<MySqlPool>,
    headers: HeaderMap,
    Path(post_id): Path<i64>,
    Json(input): Json<CreateReviewComment>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let current_user = extract_current_user(&pool, &headers).await?;
    let post_access = fetch_post_access(&pool, post_id).await?;
    ensure_review_comment_access(&current_user, &post_access)?;

    let content = input.content.trim();
    if content.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"detail": "Comment content is required"})),
        ));
    }

    let target_version_id =
        resolve_target_version_id(&pool, post_id, post_access.latest_paper_version_id, input.paper_version_id)
            .await?;

    if let Some(parent_comment_id) = input.parent_comment_id {
        let parent_row = sqlx::query_as::<_, (i64, Option<i64>)>(
            "SELECT post_id, paper_version_id FROM paper_review_comments WHERE id = ?",
        )
        .bind(parent_comment_id)
        .fetch_optional(&pool)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"detail": "Parent review comment not found"})),
            )
        })?;

        if parent_row.0 != post_id {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"detail": "Parent comment does not belong to this post"})),
            ));
        }

        if parent_row.1 != target_version_id {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"detail": "Parent comment belongs to a different paper version"})),
            ));
        }
    }

    let now = Utc::now();
    let insert = sqlx::query(
        r#"
        INSERT INTO paper_review_comments (
            post_id,
            paper_version_id,
            author_id,
            parent_comment_id,
            content,
            is_deleted,
            deleted_at,
            created_at
        ) VALUES (?, ?, ?, ?, ?, FALSE, NULL, ?)
        "#,
    )
    .bind(post_id)
    .bind(target_version_id)
    .bind(current_user.id)
    .bind(input.parent_comment_id)
    .bind(content)
    .bind(now)
    .execute(&pool)
    .await
    .map_err(internal_error)?;

    let comment = sqlx::query_as::<_, ReviewComment>("SELECT * FROM paper_review_comments WHERE id = ?")
        .bind(insert.last_insert_id() as i64)
        .fetch_one(&pool)
        .await
        .map_err(internal_error)?;

    Ok((
        StatusCode::CREATED,
        Json(ReviewCommentResponse {
            id: comment.id,
            post_id: comment.post_id,
            paper_version_id: comment.paper_version_id,
            author_id: comment.author_id,
            parent_comment_id: comment.parent_comment_id,
            author: UserResponse::from(current_user),
            content: comment.content,
            is_deleted: comment.is_deleted,
            deleted_at: comment.deleted_at,
            created_at: comment.created_at,
            updated_at: comment.updated_at,
        }),
    ))
}

async fn delete_review_comment(
    State(pool): State<MySqlPool>,
    headers: HeaderMap,
    Path((post_id, comment_id)): Path<(i64, i64)>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let current_user = extract_current_user(&pool, &headers).await?;
    let post_access = fetch_post_access(&pool, post_id).await?;

    let comment = find_review_comment_target(&pool, comment_id, post_id)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"detail": "Review comment not found"})),
            )
        })?;

    let can_delete = if post_access.is_published {
        current_user.is_admin || current_user.id == comment.author_id
    } else {
        current_user.is_admin
            || current_user.id == post_access.author_id
            || current_user.id == comment.author_id
    };

    if !can_delete {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"detail": "Not authorized to delete this review comment"})),
        ));
    }

    let delete_mode = apply_review_comment_delete_policy(&pool, &comment)
        .await
        .map_err(internal_error)?;

    Ok(Json(serde_json::json!({
        "message": "Review comment deleted successfully",
        "delete_mode": delete_mode.as_str()
    })))
}

async fn fetch_post_access(
    pool: &MySqlPool,
    post_id: i64,
) -> Result<PostAccessRow, (StatusCode, Json<serde_json::Value>)> {
    let row = sqlx::query_as::<_, PostAccessRow>(
        r#"
        SELECT
            p.author_id AS author_id,
            p.is_published AS is_published,
            c.code AS category_code,
            p.latest_paper_version_id AS latest_paper_version_id
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

    if row.category_code != "paper" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"detail": "Paper workflow is only available for paper posts"})),
        ));
    }

    Ok(row)
}

fn ensure_paper_author_or_admin(
    current_user: &User,
    post_access: &PostAccessRow,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    if current_user.id == post_access.author_id || current_user.is_admin {
        return Ok(());
    }

    Err((
        StatusCode::FORBIDDEN,
        Json(serde_json::json!({"detail": "Not authorized to access paper versions"})),
    ))
}

fn ensure_review_comment_access(
    current_user: &User,
    post_access: &PostAccessRow,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    if post_access.is_published {
        return Ok(());
    }

    if current_user.id == post_access.author_id || current_user.is_admin {
        return Ok(());
    }

    Err((
        StatusCode::FORBIDDEN,
        Json(serde_json::json!({"detail": "Not authorized to access review comments for this paper"})),
    ))
}

async fn resolve_target_version_id(
    pool: &MySqlPool,
    post_id: i64,
    latest_version_id: Option<i64>,
    requested_version_id: Option<i64>,
) -> Result<Option<i64>, (StatusCode, Json<serde_json::Value>)> {
    let target_version_id = requested_version_id.or(latest_version_id);
    let Some(version_id) = target_version_id else {
        return Err((
            StatusCode::CONFLICT,
            Json(serde_json::json!({"detail": "No submitted revision available"})),
        ));
    };

    let exists = sqlx::query("SELECT id FROM paper_versions WHERE id = ? AND post_id = ?")
        .bind(version_id)
        .bind(post_id)
        .fetch_optional(pool)
        .await
        .map_err(internal_error)?;

    if exists.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"detail": "Paper version not found"})),
        ));
    }

    Ok(Some(version_id))
}

fn map_paper_version(version: PaperVersion) -> PaperVersionResponse {
    PaperVersionResponse {
        id: version.id,
        post_id: version.post_id,
        version_number: version.version_number,
        title: version.title,
        content: version.content,
        summary: version.summary,
        github_url: version.github_url,
        file_path: version.file_path,
        file_name: version.file_name,
        tags: parse_string_list_json(version.tags_json),
        citations: parse_i64_list_json(version.citations_json),
        submitted_by: version.submitted_by,
        submitted_at: version.submitted_at,
        created_at: version.created_at,
    }
}

fn map_review_comment_row(row: ReviewCommentWithAuthorRow) -> ReviewCommentResponse {
    let author = UserResponse::from(User {
        id: row.user_id,
        username: row.username,
        email: row.email,
        hashed_password: None,
        google_id: None,
        display_name: row.display_name,
        bio: row.bio,
        introduction: None,
        hobbies: None,
        interests: None,
        research_areas: None,
        avatar_url: row.avatar_url,
        is_admin: row.is_admin,
        created_at: row.user_created_at,
        updated_at: None,
    });

    ReviewCommentResponse {
        id: row.comment_id,
        post_id: row.post_id,
        paper_version_id: row.paper_version_id,
        author_id: row.author_id,
        parent_comment_id: row.parent_comment_id,
        author,
        content: if row.is_deleted {
            String::new()
        } else {
            row.content
        },
        is_deleted: row.is_deleted,
        deleted_at: row.deleted_at,
        created_at: row.comment_created_at,
        updated_at: row.comment_updated_at,
    }
}

async fn find_review_comment_target(
    pool: &MySqlPool,
    comment_id: i64,
    post_id: i64,
) -> Result<Option<ReviewCommentDeleteTarget>, sqlx::Error> {
    let row = sqlx::query_as::<_, ReviewCommentDeleteTarget>(
        "SELECT id, post_id, author_id, parent_comment_id FROM paper_review_comments WHERE id = ?",
    )
    .bind(comment_id)
    .fetch_optional(pool)
    .await?;

    let Some(target) = row else {
        return Ok(None);
    };

    if target.post_id != post_id {
        return Ok(None);
    }

    Ok(Some(target))
}

async fn apply_review_comment_delete_policy(
    pool: &MySqlPool,
    target: &ReviewCommentDeleteTarget,
) -> Result<DeleteReviewCommentMode, sqlx::Error> {
    let (child_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM paper_review_comments WHERE parent_comment_id = ?")
            .bind(target.id)
            .fetch_one(pool)
            .await?;

    if child_count > 0 {
        let now = Utc::now();
        sqlx::query(
            "UPDATE paper_review_comments SET is_deleted = TRUE, deleted_at = COALESCE(deleted_at, ?), content = '', updated_at = ? WHERE id = ?",
        )
        .bind(now)
        .bind(now)
        .bind(target.id)
        .execute(pool)
        .await?;

        Ok(DeleteReviewCommentMode::Soft)
    } else {
        sqlx::query("DELETE FROM paper_review_comments WHERE id = ?")
            .bind(target.id)
            .execute(pool)
            .await?;
        prune_soft_deleted_review_comment_ancestors(pool, target.parent_comment_id).await?;

        Ok(DeleteReviewCommentMode::Hard)
    }
}

async fn prune_soft_deleted_review_comment_ancestors(
    pool: &MySqlPool,
    mut current_comment_id: Option<i64>,
) -> Result<(), sqlx::Error> {
    while let Some(comment_id) = current_comment_id {
        let row = sqlx::query_as::<_, (Option<i64>, bool)>(
            "SELECT parent_comment_id, is_deleted FROM paper_review_comments WHERE id = ?",
        )
        .bind(comment_id)
        .fetch_optional(pool)
        .await?;

        let Some((parent_comment_id, is_deleted)) = row else {
            break;
        };

        let (child_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM paper_review_comments WHERE parent_comment_id = ?",
        )
        .bind(comment_id)
        .fetch_one(pool)
        .await?;

        if is_deleted && child_count == 0 {
            sqlx::query("DELETE FROM paper_review_comments WHERE id = ?")
                .bind(comment_id)
                .execute(pool)
                .await?;
            current_comment_id = parent_comment_id;
        } else {
            break;
        }
    }

    Ok(())
}

fn parse_string_list_json(raw: Option<String>) -> Vec<String> {
    raw.and_then(|json_text| serde_json::from_str::<Vec<String>>(&json_text).ok())
        .unwrap_or_default()
}

fn parse_i64_list_json(raw: Option<String>) -> Vec<i64> {
    raw.and_then(|json_text| serde_json::from_str::<Vec<i64>>(&json_text).ok())
        .unwrap_or_default()
}

fn internal_error<E: ToString>(error: E) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({"detail": error.to_string()})),
    )
}
