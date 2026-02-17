use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State, multipart::MultipartError},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use chrono::{DateTime, Datelike, Utc};
use regex::Regex;
use reqwest::{Client, Url};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::{MySql, MySqlPool, QueryBuilder};
use std::{
    collections::{HashMap, HashSet},
    path::{Path as FsPath, PathBuf},
    time::Duration,
};
use uuid::Uuid;

use crate::ai_review::{ReviewTrigger, schedule_review};
use crate::metrics::{METRIC_VERSION, compute_citation_count, compute_citation_counts_for_posts};
use crate::models::{
    PAPER_STATUS_ACCEPTED, PAPER_STATUS_DRAFT, PAPER_STATUS_PUBLISHED, PAPER_STATUS_REJECTED,
    PAPER_STATUS_REVISION, PAPER_STATUS_SUBMITTED, Post, PostDoiMetadata, PostListResponse,
    PostMetrics, PostQuery, PostResponse, User, UserResponse,
};
use crate::routes::auth::{extract_current_user, extract_optional_user};

const MAX_UPLOAD_SIZE_BYTES: usize = 10 * 1024 * 1024;
const MULTIPART_BODY_LIMIT_BYTES: usize = 12 * 1024 * 1024;
const PAPER_CATEGORY: &str = "paper";
const CITATION_SOURCE_MANUAL: u8 = 1;
const CITATION_SOURCE_AUTO: u8 = 2;
const POST_SELECT_FROM_CLAUSE: &str = r#"
    FROM posts p
    JOIN post_categories c ON c.id = p.category_id
    LEFT JOIN post_files pf ON pf.post_id = p.id
    LEFT JOIN post_stats ps ON ps.post_id = p.id
"#;
const POST_SELECT_COLUMNS: &str = r#"
    SELECT
        p.id,
        p.title,
        p.content,
        p.summary,
        p.github_url,
        c.code AS category,
        pf.file_path,
        pf.file_name,
        p.author_id,
        p.is_published,
        p.published_at,
        p.paper_status,
        CAST(p.current_revision AS SIGNED) AS current_revision,
        COALESCE(ps.view_count, 0) AS view_count,
        COALESCE(ps.like_count, 0) AS like_count,
        p.created_at,
        p.updated_at
"#;
const ALLOWED_UPLOAD_EXTENSIONS: &[&str] = &[
    "pdf", "doc", "docx", "txt", "md", "pptx", "xlsx", "zip", "png", "jpg", "jpeg", "gif",
];
const CROSSREF_API_BASE: &str = "https://api.crossref.org/works/";
const DOI_PATTERN: &str = r#"(?i)\b10\.\d{4,9}/[-._;()/:A-Z0-9]+"#;
const DEFAULT_CROSSREF_TIMEOUT_SECS: u64 = 8;
const DEFAULT_CROSSREF_MAX_DOIS: usize = 10;
const INTERNAL_DOI_PREFIX: &str = "TM";
const INTERNAL_DOI_HASH_LENGTH: usize = 12;

pub fn posts_routes() -> Router<MySqlPool> {
    Router::new()
        .route("/", get(list_posts).post(create_post))
        .route(
            "/{post_id}",
            get(get_post).put(update_post).delete(delete_post),
        )
        .route("/{post_id}/publish", post(publish_post))
        .route("/{post_id}/like", post(like_post))
        // Keep multipart parsing above the 10MB policy threshold so route-level validation can return a precise 413.
        .layer(DefaultBodyLimit::max(MULTIPART_BODY_LIMIT_BYTES))
}

async fn list_posts(
    State(pool): State<MySqlPool>,
    Query(query): Query<PostQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(10).clamp(1, 100);
    let offset = i64::from(page - 1) * i64::from(per_page);
    let filters = resolve_post_filters(&query)?;

    let mut posts_qb = QueryBuilder::<MySql>::new(format!(
        "{}{}",
        POST_SELECT_COLUMNS, POST_SELECT_FROM_CLAUSE
    ));
    let mut posts_has_where = false;
    push_post_filters(&mut posts_qb, &filters, &mut posts_has_where);
    push_visibility_filter(&mut posts_qb, &mut posts_has_where);
    posts_qb.push(" ORDER BY p.created_at DESC LIMIT ");
    posts_qb.push_bind(i64::from(per_page));
    posts_qb.push(" OFFSET ");
    posts_qb.push_bind(offset);

    let posts = posts_qb
        .build_query_as::<Post>()
        .fetch_all(&pool)
        .await
        .map_err(internal_error)?;

    let mut count_qb = QueryBuilder::<MySql>::new(
        "SELECT COUNT(*) FROM posts p JOIN post_categories c ON c.id = p.category_id",
    );
    let mut count_has_where = false;
    push_post_filters(&mut count_qb, &filters, &mut count_has_where);
    push_visibility_filter(&mut count_qb, &mut count_has_where);
    let (total,): (i64,) = count_qb
        .build_query_as()
        .fetch_one(&pool)
        .await
        .map_err(internal_error)?;

    let author_map = fetch_authors_map(&pool, &posts)
        .await
        .map_err(internal_error)?;
    let tags_map = fetch_tags_map(&pool, &posts)
        .await
        .map_err(internal_error)?;
    let post_ids: Vec<i64> = posts.iter().map(|post| post.id).collect();
    let citation_count_map = compute_citation_counts_for_posts(&pool, &post_ids)
        .await
        .map_err(internal_error)?;

    let mut post_responses = Vec::with_capacity(posts.len());
    for post in posts {
        let author = author_map.get(&post.author_id).cloned().ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": "Post author not found"})),
            )
        })?;

        let tags = tags_map.get(&post.id).cloned().unwrap_or_default();
        let citation_count = *citation_count_map.get(&post.id).unwrap_or(&0);

        post_responses.push(PostResponse {
            id: post.id,
            title: post.title,
            content: post.content,
            summary: post.summary,
            github_url: post.github_url,
            category: post.category,
            file_path: post.file_path,
            file_name: post.file_name,
            author_id: post.author_id,
            author,
            is_published: post.is_published,
            published_at: post.published_at,
            paper_status: post.paper_status,
            current_revision: post.current_revision,
            view_count: post.view_count,
            like_count: post.like_count,
            user_liked: None,
            metrics: PostMetrics {
                citation_count,
                metric_version: METRIC_VERSION.to_string(),
            },
            doi_metadata: Vec::new(),
            created_at: post.created_at,
            updated_at: post.updated_at,
            tags,
        });
    }

    Ok(Json(PostListResponse {
        posts: post_responses,
        total,
        page,
        per_page,
    }))
}

async fn get_post(
    State(pool): State<MySqlPool>,
    headers: HeaderMap,
    Path(post_id): Path<i64>,
    Query(query): Query<PostDetailQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let post_query = format!(
        "{}{} WHERE p.id = ?",
        POST_SELECT_COLUMNS, POST_SELECT_FROM_CLAUSE
    );
    let post = sqlx::query_as::<_, Post>(&post_query)
        .bind(post_id)
        .fetch_optional(&pool)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"detail": "Post not found"})),
            )
        })?;

    let current_user = extract_optional_user(&pool, &headers).await?;
    if !post.is_published {
        let allow_review_center_access = query.source.as_deref() == Some("review_center");
        let has_private_access = current_user
            .as_ref()
            .map(|user| user.id == post.author_id || user.is_admin)
            .unwrap_or(false);
        if !allow_review_center_access || !has_private_access {
            return Err((
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"detail": "Post not found"})),
            ));
        }
    }

    sqlx::query(
        r#"
        INSERT INTO post_stats (post_id, view_count, like_count, updated_at)
        VALUES (?, 1, 0, ?)
        ON DUPLICATE KEY UPDATE view_count = view_count + 1, updated_at = VALUES(updated_at)
        "#,
    )
    .bind(post_id)
    .bind(Utc::now())
    .execute(&pool)
    .await
    .map_err(internal_error)?;

    let author = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(post.author_id)
        .fetch_one(&pool)
        .await
        .map_err(internal_error)?;

    let tags = fetch_tags(&pool, post.id).await.unwrap_or_default();
    let citation_count = compute_citation_count(&pool, post.id)
        .await
        .map_err(internal_error)?;
    if let Err(error) = ensure_internal_doi_metadata(&pool, post.id).await {
        tracing::warn!(
            "Failed to ensure internal DOI for post {}: {}",
            post.id,
            error
        );
    }
    let doi_metadata = fetch_post_doi_metadata(&pool, post.id)
        .await
        .map_err(internal_error)?;
    let user_liked = if let Some(user) = current_user {
        Some(
            fetch_user_liked(&pool, user.id, post_id)
                .await
                .map_err(internal_error)?,
        )
    } else {
        None
    };

    Ok(Json(PostResponse {
        id: post.id,
        title: post.title,
        content: post.content,
        summary: post.summary,
        github_url: post.github_url,
        category: post.category,
        file_path: post.file_path,
        file_name: post.file_name,
        author_id: post.author_id,
        author: UserResponse::from(author),
        is_published: post.is_published,
        published_at: post.published_at,
        paper_status: post.paper_status,
        current_revision: post.current_revision,
        view_count: post.view_count + 1,
        like_count: post.like_count,
        user_liked,
        metrics: PostMetrics {
            citation_count,
            metric_version: METRIC_VERSION.to_string(),
        },
        doi_metadata,
        created_at: post.created_at,
        updated_at: post.updated_at,
        tags,
    }))
}

async fn create_post(
    State(pool): State<MySqlPool>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let current_user = extract_current_user(&pool, &headers).await?;

    let mut title = String::new();
    let mut content = String::new();
    let mut summary: Option<String> = None;
    let mut github_url: Option<String> = None;
    let mut category = "other".to_string();
    let mut file_path: Option<String> = None;
    let mut file_name: Option<String> = None;
    let mut tags_str = String::new();
    let mut citations_str: Option<String> = None;
    let mut requested_paper_status: Option<String> = None;

    while let Some(field) = multipart.next_field().await.map_err(multipart_error)? {
        let name = field.name().unwrap_or_default().to_string();

        match name.as_str() {
            "title" => {
                title = field.text().await.map_err(multipart_error)?;
            }
            "content" => {
                content = field.text().await.map_err(multipart_error)?;
            }
            "summary" => {
                summary = Some(field.text().await.map_err(multipart_error)?);
            }
            "github_url" => {
                let value = field.text().await.map_err(multipart_error)?;
                github_url = validate_github_url(&value)?;
            }
            "category" => {
                category = field.text().await.map_err(multipart_error)?;
            }
            "tags" => {
                tags_str = field.text().await.map_err(multipart_error)?;
            }
            "citations" => {
                citations_str = Some(field.text().await.map_err(multipart_error)?);
            }
            "paper_status" => {
                requested_paper_status = Some(field.text().await.map_err(multipart_error)?);
            }
            "file" => {
                if let Some(original_name) = field.file_name() {
                    let original_name = original_name.to_string();
                    if !original_name.is_empty() {
                        let data = field.bytes().await.map_err(multipart_error)?;
                        validate_upload_file(&original_name, data.len())?;

                        let ext = normalized_extension(&original_name).ok_or_else(|| {
                            (
                                StatusCode::BAD_REQUEST,
                                Json(serde_json::json!({"detail": "Invalid file extension"})),
                            )
                        })?;

                        let unique_name = format!("{}.{}", Uuid::new_v4(), ext);
                        let upload_path = PathBuf::from("uploads").join(&unique_name);

                        tokio::fs::write(&upload_path, &data)
                            .await
                            .map_err(internal_error)?;

                        file_path = Some(upload_path.to_string_lossy().to_string());
                        file_name = Some(original_name);
                    }
                }
            }
            _ => {}
        }
    }

    if title.is_empty() || content.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"detail": "Title and content are required"})),
        ));
    }

    let (category_id, category_code) = resolve_or_create_category(&pool, &category).await?;
    let manual_citation_ids =
        prepare_citations_for_create(&pool, &category_code, citations_str.as_deref()).await?;
    let auto_citation_ids =
        prepare_auto_citations_for_content(&pool, &category_code, &content, None).await?;

    let now = Utc::now();
    let paper_status =
        resolve_create_paper_status(&category_code, requested_paper_status.as_deref())?;
    let is_published = paper_status == PAPER_STATUS_PUBLISHED;
    let published_at = if is_published { Some(now) } else { None };
    let result = sqlx::query(
        r#"INSERT INTO posts (title, content, summary, github_url, category_id, author_id, is_published, published_at, paper_status, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&title)
    .bind(&content)
    .bind(&summary)
    .bind(&github_url)
    .bind(category_id)
    .bind(current_user.id)
    .bind(is_published)
    .bind(published_at)
    .bind(&paper_status)
    .bind(now)
    .execute(&pool)
    .await
    .map_err(internal_error)?;

    let post_id = result.last_insert_id() as i64;

    sqlx::query(
        "INSERT INTO post_stats (post_id, view_count, like_count, updated_at) VALUES (?, 0, 0, ?)",
    )
    .bind(post_id)
    .bind(now)
    .execute(&pool)
    .await
    .map_err(internal_error)?;

    if let (Some(saved_path), Some(saved_name)) = (file_path.as_ref(), file_name.as_ref()) {
        sqlx::query(
            "INSERT INTO post_files (post_id, file_path, file_name, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(post_id)
        .bind(saved_path)
        .bind(saved_name)
        .bind(now)
        .bind(now)
        .execute(&pool)
        .await
        .map_err(internal_error)?;
    }

    replace_post_citations(&pool, post_id, &manual_citation_ids).await?;
    replace_post_auto_citations(&pool, post_id, &auto_citation_ids).await?;
    if let Err(error) = sync_post_doi_metadata(
        &pool,
        post_id,
        &category_code,
        &title,
        summary.as_deref(),
        &content,
    )
    .await
    {
        tracing::warn!(
            "Failed to auto-collect DOI metadata for post {} on create: {}",
            post_id,
            error
        );
    }

    let tags_vec = process_tags(&pool, post_id, &tags_str).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"detail": e.to_string()})),
        )
    })?;

    if category_code == PAPER_CATEGORY && paper_status == PAPER_STATUS_SUBMITTED {
        let (paper_version_id, _) =
            create_paper_version_snapshot(&pool, post_id, current_user.id).await?;
        if let Err(error) = schedule_review(
            &pool,
            post_id,
            Some(paper_version_id),
            ReviewTrigger::AutoCreate,
        )
        .await
        {
            tracing::error!(
                "Failed to schedule auto AI review on create for post {}: {}",
                post_id,
                error
            );
        }
    }

    let post_query = format!(
        "{}{} WHERE p.id = ?",
        POST_SELECT_COLUMNS, POST_SELECT_FROM_CLAUSE
    );
    let post = sqlx::query_as::<_, Post>(&post_query)
        .bind(post_id)
        .fetch_one(&pool)
        .await
        .map_err(internal_error)?;
    let citation_count = compute_citation_count(&pool, post_id)
        .await
        .map_err(internal_error)?;
    let doi_metadata = fetch_post_doi_metadata(&pool, post_id)
        .await
        .map_err(internal_error)?;

    Ok((
        StatusCode::CREATED,
        Json(PostResponse {
            id: post.id,
            title: post.title,
            content: post.content,
            summary: post.summary,
            github_url: post.github_url,
            category: post.category,
            file_path: post.file_path,
            file_name: post.file_name,
            author_id: post.author_id,
            author: UserResponse::from(current_user),
            is_published: post.is_published,
            published_at: post.published_at,
            paper_status: post.paper_status,
            current_revision: post.current_revision,
            view_count: post.view_count,
            like_count: post.like_count,
            user_liked: Some(false),
            metrics: PostMetrics {
                citation_count,
                metric_version: METRIC_VERSION.to_string(),
            },
            doi_metadata,
            created_at: post.created_at,
            updated_at: post.updated_at,
            tags: tags_vec,
        }),
    ))
}

async fn update_post(
    State(pool): State<MySqlPool>,
    headers: HeaderMap,
    Path(post_id): Path<i64>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let current_user = extract_current_user(&pool, &headers).await?;

    let post_query = format!(
        "{}{} WHERE p.id = ?",
        POST_SELECT_COLUMNS, POST_SELECT_FROM_CLAUSE
    );
    let post = sqlx::query_as::<_, Post>(&post_query)
        .bind(post_id)
        .fetch_optional(&pool)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"detail": "Post not found"})),
            )
        })?;

    if post.author_id != current_user.id {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"detail": "Not authorized to edit this post"})),
        ));
    }

    let mut title = post.title.clone();
    let mut content = post.content.clone();
    let mut summary = post.summary.clone();
    let mut github_url = post.github_url.clone();
    let mut category = post.category.clone();
    let mut file_path = post.file_path.clone();
    let mut file_name = post.file_name.clone();
    let mut remove_file = false;
    let mut file_changed = false;
    let mut tags_str: Option<String> = None;
    let mut citations_str: Option<String> = None;
    let mut requested_paper_status: Option<String> = None;
    let mut replacement_file: Option<(String, Vec<u8>)> = None;

    while let Some(field) = multipart.next_field().await.map_err(multipart_error)? {
        let name = field.name().unwrap_or_default().to_string();

        match name.as_str() {
            "title" => {
                let val = field.text().await.map_err(multipart_error)?;
                if !val.is_empty() {
                    title = val;
                }
            }
            "content" => {
                let val = field.text().await.map_err(multipart_error)?;
                if !val.is_empty() {
                    content = val;
                }
            }
            "summary" => {
                summary = Some(field.text().await.map_err(multipart_error)?);
            }
            "github_url" => {
                let value = field.text().await.map_err(multipart_error)?;
                github_url = validate_github_url(&value)?;
            }
            "category" => {
                let val = field.text().await.map_err(multipart_error)?;
                if !val.is_empty() {
                    category = val;
                }
            }
            "tags" => {
                tags_str = Some(field.text().await.map_err(multipart_error)?);
            }
            "citations" => {
                citations_str = Some(field.text().await.map_err(multipart_error)?);
            }
            "paper_status" => {
                requested_paper_status = Some(field.text().await.map_err(multipart_error)?);
            }
            "remove_file" => {
                let val = field.text().await.map_err(multipart_error)?;
                remove_file = val == "true";
            }
            "file" => {
                if let Some(original_name) = field.file_name() {
                    let original_name = original_name.to_string();
                    if !original_name.is_empty() {
                        let data = field.bytes().await.map_err(multipart_error)?;
                        validate_upload_file(&original_name, data.len())?;
                        replacement_file = Some((original_name, data.to_vec()));
                    }
                }
            }
            _ => {}
        }
    }

    if let Some((new_original_name, new_data)) = replacement_file {
        let ext = normalized_extension(&new_original_name).ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"detail": "Invalid file extension"})),
            )
        })?;
        let unique_name = format!("{}.{}", Uuid::new_v4(), ext);
        let upload_path = PathBuf::from("uploads").join(&unique_name);

        tokio::fs::write(&upload_path, &new_data)
            .await
            .map_err(internal_error)?;

        if let Some(ref old_path) = post.file_path {
            let _ = tokio::fs::remove_file(old_path).await;
        }

        file_path = Some(upload_path.to_string_lossy().to_string());
        file_name = Some(new_original_name);
        file_changed = true;
    } else if remove_file && file_path.is_some() {
        if let Some(ref path) = post.file_path {
            let _ = tokio::fs::remove_file(path).await;
        }
        file_path = None;
        file_name = None;
        file_changed = true;
    }

    let (category_id, category_code) = resolve_or_create_category(&pool, &category).await?;
    let manual_citation_ids = if let Some(raw) = citations_str.as_deref() {
        Some(prepare_citations_for_update(&pool, post_id, &category_code, raw).await?)
    } else {
        None
    };

    let now = Utc::now();
    let paper_status = resolve_update_paper_status(
        &category_code,
        post.paper_status.as_str(),
        requested_paper_status.as_deref(),
    )?;
    let is_published = paper_status == PAPER_STATUS_PUBLISHED;
    let published_at = if is_published { Some(now) } else { None };
    sqlx::query(
        "UPDATE posts SET title = ?, content = ?, summary = ?, github_url = ?, category_id = ?, is_published = ?, published_at = ?, paper_status = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&title)
    .bind(&content)
    .bind(&summary)
    .bind(&github_url)
    .bind(category_id)
    .bind(is_published)
    .bind(published_at)
    .bind(&paper_status)
    .bind(now)
    .bind(post_id)
    .execute(&pool)
    .await
    .map_err(internal_error)?;

    if file_changed {
        if let (Some(saved_path), Some(saved_name)) = (file_path.as_ref(), file_name.as_ref()) {
            sqlx::query(
                r#"
                INSERT INTO post_files (post_id, file_path, file_name, created_at, updated_at)
                VALUES (?, ?, ?, ?, ?)
                ON DUPLICATE KEY UPDATE
                    file_path = VALUES(file_path),
                    file_name = VALUES(file_name),
                    updated_at = VALUES(updated_at)
                "#,
            )
            .bind(post_id)
            .bind(saved_path)
            .bind(saved_name)
            .bind(now)
            .bind(now)
            .execute(&pool)
            .await
            .map_err(internal_error)?;
        } else {
            sqlx::query("DELETE FROM post_files WHERE post_id = ?")
                .bind(post_id)
                .execute(&pool)
                .await
                .map_err(internal_error)?;
        }
    }

    let tags_vec = if let Some(t_str) = tags_str {
        process_tags(&pool, post_id, &t_str).await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": e.to_string()})),
            )
        })?
    } else {
        fetch_tags(&pool, post_id).await.unwrap_or_default()
    };

    if category_code != PAPER_CATEGORY {
        clear_all_post_citations(&pool, post_id).await?;
        sqlx::query("UPDATE posts SET current_revision = 0, latest_paper_version_id = NULL WHERE id = ?")
            .bind(post_id)
            .execute(&pool)
            .await
            .map_err(internal_error)?;
    } else {
        if let Some(ids) = manual_citation_ids {
            replace_post_citations(&pool, post_id, &ids).await?;
        }

        let auto_citation_ids =
            prepare_auto_citations_for_content(&pool, &category_code, &content, Some(post_id))
                .await?;
        replace_post_auto_citations(&pool, post_id, &auto_citation_ids).await?;
    }

    if let Err(error) = sync_post_doi_metadata(
        &pool,
        post_id,
        &category_code,
        &title,
        summary.as_deref(),
        &content,
    )
    .await
    {
        tracing::warn!(
            "Failed to auto-collect DOI metadata for post {} on update: {}",
            post_id,
            error
        );
    }

    if category_code == PAPER_CATEGORY && paper_status == PAPER_STATUS_SUBMITTED {
        let (paper_version_id, _) =
            create_paper_version_snapshot(&pool, post_id, current_user.id).await?;
        if let Err(error) = schedule_review(
            &pool,
            post_id,
            Some(paper_version_id),
            ReviewTrigger::AutoUpdate,
        )
        .await
        {
            tracing::error!(
                "Failed to schedule auto AI review on update for post {}: {}",
                post_id,
                error
            );
        }
    }

    let updated_post = sqlx::query_as::<_, Post>(&post_query)
        .bind(post_id)
        .fetch_one(&pool)
        .await
        .map_err(internal_error)?;

    let user_liked = fetch_user_liked(&pool, current_user.id, post_id)
        .await
        .map_err(internal_error)?;
    let citation_count = compute_citation_count(&pool, post_id)
        .await
        .map_err(internal_error)?;
    let doi_metadata = fetch_post_doi_metadata(&pool, post_id)
        .await
        .map_err(internal_error)?;

    Ok(Json(PostResponse {
        id: updated_post.id,
        title: updated_post.title,
        content: updated_post.content,
        summary: updated_post.summary,
        github_url: updated_post.github_url,
        category: updated_post.category,
        file_path: updated_post.file_path,
        file_name: updated_post.file_name,
        author_id: updated_post.author_id,
        author: UserResponse::from(current_user),
        is_published: updated_post.is_published,
        published_at: updated_post.published_at,
        paper_status: updated_post.paper_status,
        current_revision: updated_post.current_revision,
        view_count: updated_post.view_count,
        like_count: updated_post.like_count,
        user_liked: Some(user_liked),
        metrics: PostMetrics {
            citation_count,
            metric_version: METRIC_VERSION.to_string(),
        },
        doi_metadata,
        created_at: updated_post.created_at,
        updated_at: updated_post.updated_at,
        tags: tags_vec,
    }))
}

async fn delete_post(
    State(pool): State<MySqlPool>,
    headers: HeaderMap,
    Path(post_id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let current_user = extract_current_user(&pool, &headers).await?;

    let post_query = format!(
        "{}{} WHERE p.id = ?",
        POST_SELECT_COLUMNS, POST_SELECT_FROM_CLAUSE
    );
    let post = sqlx::query_as::<_, Post>(&post_query)
        .bind(post_id)
        .fetch_optional(&pool)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"detail": "Post not found"})),
            )
        })?;

    if post.author_id != current_user.id {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"detail": "Not authorized to delete this post"})),
        ));
    }

    if let Some(ref path) = post.file_path {
        let _ = tokio::fs::remove_file(path).await;
    }

    clear_all_post_citations(&pool, post_id).await?;

    sqlx::query("DELETE FROM posts WHERE id = ?")
        .bind(post_id)
        .execute(&pool)
        .await
        .map_err(internal_error)?;

    Ok(Json(
        serde_json::json!({"message": "Post deleted successfully"}),
    ))
}

async fn publish_post(
    State(pool): State<MySqlPool>,
    headers: HeaderMap,
    Path(post_id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let current_user = extract_current_user(&pool, &headers).await?;

    let row = sqlx::query_as::<_, (i64, String, String)>(
        r#"
        SELECT p.author_id, c.code AS category_code, p.paper_status
        FROM posts p
        JOIN post_categories c ON c.id = p.category_id
        WHERE p.id = ?
        "#,
    )
    .bind(post_id)
    .fetch_optional(&pool)
    .await
    .map_err(internal_error)?
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"detail": "Post not found"})),
        )
    })?;

    let (author_id, category_code, paper_status) = row;
    if current_user.id != author_id && !current_user.is_admin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"detail": "Not authorized to publish this post"})),
        ));
    }

    if category_code != PAPER_CATEGORY {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"detail": "Only paper posts can use publish transition"})),
        ));
    }

    if paper_status == PAPER_STATUS_PUBLISHED {
        return Ok(Json(serde_json::json!({
            "detail": "Post is already published",
            "paper_status": PAPER_STATUS_PUBLISHED,
            "is_published": true
        })));
    }

    if paper_status != PAPER_STATUS_ACCEPTED {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "detail": "Only accepted papers can be published",
                "paper_status": paper_status
            })),
        ));
    }

    let now = Utc::now();
    sqlx::query(
        r#"
        UPDATE posts
        SET
            paper_status = ?,
            is_published = TRUE,
            published_at = COALESCE(published_at, ?),
            updated_at = ?
        WHERE id = ?
        "#,
    )
    .bind(PAPER_STATUS_PUBLISHED)
    .bind(now)
    .bind(now)
    .bind(post_id)
    .execute(&pool)
    .await
    .map_err(internal_error)?;

    Ok(Json(serde_json::json!({
        "detail": "Paper published successfully",
        "paper_status": PAPER_STATUS_PUBLISHED,
        "is_published": true,
        "published_at": now
    })))
}

async fn like_post(
    State(pool): State<MySqlPool>,
    headers: HeaderMap,
    Path(post_id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let current_user = extract_current_user(&pool, &headers).await?;

    let post_row = sqlx::query_as::<_, (bool,)>("SELECT is_published FROM posts WHERE id = ?")
        .bind(post_id)
        .fetch_optional(&pool)
        .await
        .map_err(internal_error)?
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

    let existing = sqlx::query("SELECT id FROM post_likes WHERE user_id = ? AND post_id = ?")
        .bind(current_user.id)
        .bind(post_id)
        .fetch_optional(&pool)
        .await
        .map_err(internal_error)?;

    let user_liked = if existing.is_some() {
        sqlx::query("DELETE FROM post_likes WHERE user_id = ? AND post_id = ?")
            .bind(current_user.id)
            .bind(post_id)
            .execute(&pool)
            .await
            .map_err(internal_error)?;
        false
    } else {
        sqlx::query("INSERT INTO post_likes (user_id, post_id, created_at) VALUES (?, ?, ?)")
            .bind(current_user.id)
            .bind(post_id)
            .bind(Utc::now())
            .execute(&pool)
            .await
            .map_err(internal_error)?;
        true
    };

    let (new_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM post_likes WHERE post_id = ?")
        .bind(post_id)
        .fetch_one(&pool)
        .await
        .map_err(internal_error)?;

    sqlx::query(
        r#"
        INSERT INTO post_stats (post_id, view_count, like_count, updated_at)
        VALUES (?, 0, ?, ?)
        ON DUPLICATE KEY UPDATE like_count = VALUES(like_count), updated_at = VALUES(updated_at)
        "#,
    )
    .bind(post_id)
    .bind(new_count)
    .bind(Utc::now())
    .execute(&pool)
    .await
    .map_err(internal_error)?;

    Ok(Json(serde_json::json!({
        "message": if user_liked { "Post liked" } else { "Post unliked" },
        "like_count": new_count,
        "user_liked": user_liked
    })))
}

fn push_post_filters(
    query_builder: &mut QueryBuilder<MySql>,
    filters: &ResolvedPostFilters,
    has_where: &mut bool,
) {
    if let Some(category) = filters.category.as_ref() {
        push_condition(query_builder, has_where);
        query_builder.push("c.code = ");
        query_builder.push_bind(category.clone());
    }

    if let Some(search_pattern) = filters.search_pattern.as_ref() {
        push_condition(query_builder, has_where);
        query_builder.push("(p.title LIKE ");
        query_builder.push_bind(search_pattern.clone());
        query_builder.push(" OR p.content LIKE ");
        query_builder.push_bind(search_pattern.clone());
        query_builder.push(")");
    }

    if let Some(tag) = filters.tag.as_ref() {
        push_condition(query_builder, has_where);
        query_builder.push(
            "EXISTS (SELECT 1 FROM post_tags pt JOIN tags t ON t.id = pt.tag_id WHERE pt.post_id = p.id AND t.name = ",
        );
        query_builder.push_bind(tag.clone());
        query_builder.push(")");
    }

    if let Some(author_pattern) = filters.author_pattern.as_ref() {
        push_condition(query_builder, has_where);
        query_builder.push(
            "EXISTS (SELECT 1 FROM users u WHERE u.id = p.author_id AND (u.username LIKE ",
        );
        query_builder.push_bind(author_pattern.clone());
        query_builder.push(" OR COALESCE(u.display_name, '') LIKE ");
        query_builder.push_bind(author_pattern.clone());
        query_builder.push("))");
    }

    if let Some(year) = filters.year {
        push_condition(query_builder, has_where);
        query_builder.push("YEAR(COALESCE(p.published_at, p.created_at)) = ");
        query_builder.push_bind(year);
    }

    if let Some(paper_status) = filters.paper_status.as_ref() {
        push_condition(query_builder, has_where);
        query_builder.push("p.paper_status = ");
        query_builder.push_bind(paper_status.clone());
    }

    if let Some(ai_decision) = filters.ai_decision.as_ref() {
        push_condition(query_builder, has_where);
        query_builder.push(
            "EXISTS (SELECT 1 FROM post_ai_reviews r JOIN ai_review_decisions d ON d.id = r.decision_id WHERE r.post_id = p.id AND r.status_id = 2 AND r.id = (SELECT MAX(r2.id) FROM post_ai_reviews r2 WHERE r2.post_id = p.id AND r2.status_id = 2) AND d.code = ",
        );
        query_builder.push_bind(ai_decision.clone());
        query_builder.push(")");
    }

    if let Some(min_citations) = filters.min_citation_count {
        push_condition(query_builder, has_where);
        query_builder.push(
            "(SELECT COUNT(*) FROM (SELECT DISTINCT pc.citing_post_id, pc.cited_post_id FROM post_citations pc) citation_edges WHERE citation_edges.cited_post_id = p.id) >= ",
        );
        query_builder.push_bind(min_citations);
    }

    if let Some(max_citations) = filters.max_citation_count {
        push_condition(query_builder, has_where);
        query_builder.push(
            "(SELECT COUNT(*) FROM (SELECT DISTINCT pc.citing_post_id, pc.cited_post_id FROM post_citations pc) citation_edges WHERE citation_edges.cited_post_id = p.id) <= ",
        );
        query_builder.push_bind(max_citations);
    }

    if let Some(min_author_g_index) = filters.min_author_g_index {
        push_condition(query_builder, has_where);
        query_builder.push(
            r#"
            (
                SELECT COALESCE(MAX(gcalc.rn), 0)
                FROM (
                    SELECT ranked.rn, ranked.cum_citations
                    FROM (
                        SELECT
                            ROW_NUMBER() OVER (ORDER BY author_papers.citation_count DESC, author_papers.post_id ASC) AS rn,
                            SUM(author_papers.citation_count) OVER (
                                ORDER BY author_papers.citation_count DESC, author_papers.post_id ASC
                                ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
                            ) AS cum_citations
                        FROM (
                            SELECT
                                ap.id AS post_id,
                                COALESCE(citation_counts.citation_count, 0) AS citation_count
                            FROM posts ap
                            JOIN post_categories apc ON apc.id = ap.category_id
                            LEFT JOIN (
                                SELECT distinct_edges.cited_post_id, COUNT(*) AS citation_count
                                FROM (
                                    SELECT DISTINCT citing_post_id, cited_post_id
                                    FROM post_citations
                                ) distinct_edges
                                GROUP BY distinct_edges.cited_post_id
                            ) citation_counts ON citation_counts.cited_post_id = ap.id
                            WHERE ap.author_id = p.author_id AND apc.code = 'paper'
                        ) author_papers
                    ) ranked
                    WHERE ranked.cum_citations >= (ranked.rn * ranked.rn)
                ) gcalc
            ) >= 
            "#,
        );
        query_builder.push_bind(min_author_g_index);
    }
}

fn push_visibility_filter(query_builder: &mut QueryBuilder<MySql>, has_where: &mut bool) {
    push_condition(query_builder, has_where);
    query_builder.push("p.is_published = TRUE");
}

#[derive(Debug, Deserialize, Default)]
struct PostDetailQuery {
    source: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct ResolvedPostFilters {
    category: Option<String>,
    search_pattern: Option<String>,
    tag: Option<String>,
    author_pattern: Option<String>,
    year: Option<i32>,
    paper_status: Option<String>,
    ai_decision: Option<String>,
    min_citation_count: Option<i64>,
    max_citation_count: Option<i64>,
    min_author_g_index: Option<i64>,
}

#[derive(Debug, Clone)]
struct DoiMetadataRecord {
    doi: String,
    title: Option<String>,
    journal: Option<String>,
    publisher: Option<String>,
    published_at: Option<String>,
    source_url: Option<String>,
    raw_json: Option<String>,
}

fn normalize_query_value(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
}

fn resolve_post_filters(
    query: &PostQuery,
) -> Result<ResolvedPostFilters, (StatusCode, Json<serde_json::Value>)> {
    let category = normalize_query_value(&query.category).map(|value| value.to_ascii_lowercase());
    let search_pattern = normalize_query_value(&query.search).map(|value| format!("%{}%", value));
    let tag = normalize_query_value(&query.tag);
    let author_pattern = normalize_query_value(&query.author).map(|value| format!("%{}%", value));
    let year = query.year;
    let paper_status = normalize_query_value(&query.paper_status)
        .map(|value| value.to_ascii_lowercase())
        .map(|status| validate_paper_status_filter(&status))
        .transpose()?;
    let ai_decision = normalize_query_value(&query.ai_decision)
        .map(|value| value.to_ascii_lowercase())
        .map(|decision| validate_ai_decision_filter(&decision))
        .transpose()?;
    let min_citation_count = query.min_citation_count;
    let max_citation_count = query.max_citation_count;
    let min_author_g_index = query.min_author_g_index;

    if let Some(filter_year) = year {
        if !(1900..=2100).contains(&filter_year) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "detail": "year must be between 1900 and 2100"
                })),
            ));
        }
    }

    if let Some(min_value) = min_citation_count {
        if min_value < 0 {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "detail": "min_citation_count must be 0 or greater"
                })),
            ));
        }
    }

    if let Some(max_value) = max_citation_count {
        if max_value < 0 {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "detail": "max_citation_count must be 0 or greater"
                })),
            ));
        }
    }

    if let (Some(min_value), Some(max_value)) = (min_citation_count, max_citation_count) {
        if min_value > max_value {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "detail": "min_citation_count cannot be greater than max_citation_count"
                })),
            ));
        }
    }

    if let Some(min_g_index) = min_author_g_index {
        if min_g_index < 0 {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "detail": "min_author_g_index must be 0 or greater"
                })),
            ));
        }
    }

    Ok(ResolvedPostFilters {
        category,
        search_pattern,
        tag,
        author_pattern,
        year,
        paper_status,
        ai_decision,
        min_citation_count,
        max_citation_count,
        min_author_g_index,
    })
}

fn validate_paper_status_filter(
    raw: &str,
) -> Result<String, (StatusCode, Json<serde_json::Value>)> {
    let valid = [
        PAPER_STATUS_DRAFT,
        PAPER_STATUS_SUBMITTED,
        PAPER_STATUS_REVISION,
        PAPER_STATUS_ACCEPTED,
        PAPER_STATUS_PUBLISHED,
        PAPER_STATUS_REJECTED,
    ];

    if valid.contains(&raw) {
        return Ok(raw.to_string());
    }

    Err((
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({
            "detail": "paper_status must be one of: draft, submitted, revision, accepted, published, rejected"
        })),
    ))
}

fn validate_ai_decision_filter(
    raw: &str,
) -> Result<String, (StatusCode, Json<serde_json::Value>)> {
    let valid = ["accept", "minor_revision", "major_revision", "reject"];
    if valid.contains(&raw) {
        return Ok(raw.to_string());
    }

    Err((
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({
            "detail": "ai_decision must be one of: accept, minor_revision, major_revision, reject"
        })),
    ))
}

fn push_condition(query_builder: &mut QueryBuilder<MySql>, has_where: &mut bool) {
    if *has_where {
        query_builder.push(" AND ");
    } else {
        query_builder.push(" WHERE ");
        *has_where = true;
    }
}

fn normalize_paper_status(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

fn resolve_create_paper_status(
    category_code: &str,
    requested_status: Option<&str>,
) -> Result<String, (StatusCode, Json<serde_json::Value>)> {
    let requested = requested_status
        .map(normalize_paper_status)
        .filter(|value| !value.is_empty());

    if category_code != PAPER_CATEGORY {
        if let Some(value) = requested {
            if value != PAPER_STATUS_PUBLISHED {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "detail": "paper_status can only be set to 'published' for non-paper posts"
                    })),
                ));
            }
        }
        return Ok(PAPER_STATUS_PUBLISHED.to_string());
    }

    match requested.as_deref() {
        None | Some(PAPER_STATUS_SUBMITTED) => Ok(PAPER_STATUS_SUBMITTED.to_string()),
        Some(PAPER_STATUS_DRAFT) => Ok(PAPER_STATUS_DRAFT.to_string()),
        Some(other) => Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "detail": format!(
                    "Invalid paper_status '{}' for paper create. Allowed: draft, submitted",
                    other
                )
            })),
        )),
    }
}

fn resolve_update_paper_status(
    category_code: &str,
    _current_status: &str,
    requested_status: Option<&str>,
) -> Result<String, (StatusCode, Json<serde_json::Value>)> {
    let requested = requested_status
        .map(normalize_paper_status)
        .filter(|value| !value.is_empty());

    if category_code != PAPER_CATEGORY {
        if let Some(value) = requested {
            if value != PAPER_STATUS_PUBLISHED {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "detail": "paper_status can only be set to 'published' for non-paper posts"
                    })),
                ));
            }
        }
        return Ok(PAPER_STATUS_PUBLISHED.to_string());
    }

    match requested.as_deref() {
        None | Some(PAPER_STATUS_SUBMITTED) => Ok(PAPER_STATUS_SUBMITTED.to_string()),
        Some(PAPER_STATUS_DRAFT) => Ok(PAPER_STATUS_DRAFT.to_string()),
        Some(other) => Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "detail": format!(
                    "Invalid paper_status '{}' for paper update. Allowed: draft, submitted",
                    other
                )
            })),
        )),
    }
}

fn validate_github_url(raw: &str) -> Result<Option<String>, (StatusCode, Json<serde_json::Value>)> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let parsed = Url::parse(trimmed).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "detail": "github_url must be a valid URL"
            })),
        )
    })?;

    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "detail": "github_url must use http or https"
            })),
        ));
    }

    let host = parsed
        .host_str()
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    let is_github_host =
        host == "github.com" || host == "www.github.com" || host.ends_with(".github.com");

    if !is_github_host {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "detail": "github_url must point to github.com"
            })),
        ));
    }

    Ok(Some(parsed.to_string()))
}

fn normalized_extension(filename: &str) -> Option<String> {
    FsPath::new(filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
}

fn validate_upload_file(
    original_name: &str,
    file_size_bytes: usize,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    if file_size_bytes > MAX_UPLOAD_SIZE_BYTES {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(serde_json::json!({
                "detail": format!("File too large. Max size is {}MB", MAX_UPLOAD_SIZE_BYTES / 1024 / 1024)
            })),
        ));
    }

    let extension = normalized_extension(original_name).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"detail": "File extension is required"})),
        )
    })?;

    if !ALLOWED_UPLOAD_EXTENSIONS.contains(&extension.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "detail": "Unsupported file type. Allowed types: pdf, doc, docx, txt, md, pptx, xlsx, zip, png, jpg, jpeg, gif"
            })),
        ));
    }

    Ok(())
}

async fn fetch_user_liked(
    pool: &MySqlPool,
    user_id: i64,
    post_id: i64,
) -> Result<bool, sqlx::Error> {
    let liked = sqlx::query("SELECT 1 FROM post_likes WHERE user_id = ? AND post_id = ?")
        .bind(user_id)
        .bind(post_id)
        .fetch_optional(pool)
        .await?;

    Ok(liked.is_some())
}

async fn fetch_authors_map(
    pool: &MySqlPool,
    posts: &[Post],
) -> Result<HashMap<i64, UserResponse>, sqlx::Error> {
    let author_ids: Vec<i64> = posts
        .iter()
        .map(|post| post.author_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    if author_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut query_builder = QueryBuilder::<MySql>::new("SELECT * FROM users WHERE id IN (");
    {
        let mut separated = query_builder.separated(", ");
        for author_id in &author_ids {
            separated.push_bind(author_id);
        }
    }
    query_builder.push(")");

    let users = query_builder
        .build_query_as::<User>()
        .fetch_all(pool)
        .await?;

    Ok(users
        .into_iter()
        .map(|user| (user.id, UserResponse::from(user)))
        .collect())
}

async fn fetch_tags_map(
    pool: &MySqlPool,
    posts: &[Post],
) -> Result<HashMap<i64, Vec<String>>, sqlx::Error> {
    let post_ids: Vec<i64> = posts.iter().map(|post| post.id).collect();

    if post_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut query_builder = QueryBuilder::<MySql>::new(
        "SELECT pt.post_id, t.name FROM post_tags pt JOIN tags t ON t.id = pt.tag_id WHERE pt.post_id IN (",
    );
    {
        let mut separated = query_builder.separated(", ");
        for post_id in &post_ids {
            separated.push_bind(post_id);
        }
    }
    query_builder.push(") ORDER BY pt.post_id, t.name");

    let rows: Vec<(i64, String)> = query_builder.build_query_as().fetch_all(pool).await?;

    let mut tags_by_post = HashMap::<i64, Vec<String>>::new();
    for (post_id, tag_name) in rows {
        tags_by_post.entry(post_id).or_default().push(tag_name);
    }

    Ok(tags_by_post)
}

async fn fetch_tags(pool: &MySqlPool, post_id: i64) -> Result<Vec<String>, sqlx::Error> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT t.name FROM tags t JOIN post_tags pt ON t.id = pt.tag_id WHERE pt.post_id = ?",
    )
    .bind(post_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|(name,)| name).collect())
}

async fn process_tags(
    pool: &MySqlPool,
    post_id: i64,
    tags_str: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    sqlx::query("DELETE FROM post_tags WHERE post_id = ?")
        .bind(post_id)
        .execute(pool)
        .await?;

    let tags: Vec<String> = tags_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let mut final_tags = Vec::new();

    for tag in tags {
        let tag_id: i64 = if let Some(row) =
            sqlx::query_as::<_, (i64,)>("SELECT id FROM tags WHERE name = ?")
                .bind(&tag)
                .fetch_optional(pool)
                .await?
        {
            row.0
        } else {
            let res = sqlx::query("INSERT INTO tags (name) VALUES (?)")
                .bind(&tag)
                .execute(pool)
                .await?;
            res.last_insert_id() as i64
        };

        let _ = sqlx::query("INSERT IGNORE INTO post_tags (post_id, tag_id) VALUES (?, ?)")
            .bind(post_id)
            .bind(tag_id)
            .execute(pool)
            .await;

        final_tags.push(tag);
    }

    Ok(final_tags)
}

async fn prepare_citations_for_create(
    pool: &MySqlPool,
    category: &str,
    citations_raw: Option<&str>,
) -> Result<Vec<i64>, (StatusCode, Json<serde_json::Value>)> {
    if category != PAPER_CATEGORY {
        if citations_raw.unwrap_or_default().trim().is_empty() {
            return Ok(Vec::new());
        }

        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "detail": "Citations are only allowed for paper category posts"
            })),
        ));
    }

    let citation_ids = parse_citation_ids(citations_raw.unwrap_or_default())?;
    validate_citation_targets(pool, &citation_ids).await?;
    Ok(citation_ids)
}

async fn prepare_citations_for_update(
    pool: &MySqlPool,
    post_id: i64,
    category: &str,
    citations_raw: &str,
) -> Result<Vec<i64>, (StatusCode, Json<serde_json::Value>)> {
    if category != PAPER_CATEGORY {
        if citations_raw.trim().is_empty() {
            return Ok(Vec::new());
        }

        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "detail": "Citations are only allowed for paper category posts"
            })),
        ));
    }

    let citation_ids = parse_citation_ids(citations_raw)?;
    if citation_ids.contains(&post_id) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"detail": "Self-citation is not allowed"})),
        ));
    }

    validate_citation_targets(pool, &citation_ids).await?;
    Ok(citation_ids)
}

async fn prepare_auto_citations_for_content(
    pool: &MySqlPool,
    category: &str,
    content: &str,
    current_post_id: Option<i64>,
) -> Result<Vec<i64>, (StatusCode, Json<serde_json::Value>)> {
    if category != PAPER_CATEGORY {
        return Ok(Vec::new());
    }

    let mut citation_ids = extract_auto_citation_ids(content);
    if let Some(post_id) = current_post_id {
        citation_ids.retain(|id| *id != post_id);
    }

    validate_citation_targets(pool, &citation_ids).await?;
    Ok(citation_ids)
}

fn parse_citation_ids(raw: &str) -> Result<Vec<i64>, (StatusCode, Json<serde_json::Value>)> {
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }

    let mut seen = HashSet::new();
    let mut citation_ids = Vec::new();

    for token in raw.split(',') {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }

        let parsed = trimmed.parse::<i64>().map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "detail": "Citations must be comma-separated numeric post IDs"
                })),
            )
        })?;

        if parsed <= 0 {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"detail": "Citation post IDs must be positive integers"})),
            ));
        }

        if seen.insert(parsed) {
            citation_ids.push(parsed);
        }
    }

    Ok(citation_ids)
}

fn extract_auto_citation_ids(content: &str) -> Vec<i64> {
    let mut ids = HashSet::new();
    extract_ids_after_pattern(content, "/posts/", &mut ids);

    let lowered = content.to_ascii_lowercase();
    for marker in ["cite:", "citation:", "post:", "cite#", "citation#", "post#"] {
        extract_ids_after_pattern(&lowered, marker, &mut ids);
    }

    let mut result: Vec<i64> = ids.into_iter().collect();
    result.sort_unstable();
    result
}

fn extract_ids_after_pattern(content: &str, pattern: &str, target: &mut HashSet<i64>) {
    let bytes = content.as_bytes();
    let mut cursor = 0usize;

    while cursor < content.len() {
        let Some(found) = content[cursor..].find(pattern) else {
            break;
        };

        let start = cursor + found + pattern.len();
        let mut end = start;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }

        if end > start {
            if let Ok(id_str) = std::str::from_utf8(&bytes[start..end]) {
                if let Ok(id) = id_str.parse::<i64>() {
                    if id > 0 {
                        target.insert(id);
                    }
                }
            }
        }

        cursor = start;
    }
}

async fn validate_citation_targets(
    pool: &MySqlPool,
    citation_ids: &[i64],
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    if citation_ids.is_empty() {
        return Ok(());
    }

    let mut query_builder = QueryBuilder::<MySql>::new(
        "SELECT p.id FROM posts p JOIN post_categories c ON c.id = p.category_id WHERE c.code = 'paper' AND p.id IN (",
    );
    {
        let mut separated = query_builder.separated(", ");
        for citation_id in citation_ids {
            separated.push_bind(citation_id);
        }
    }
    query_builder.push(")");

    let rows: Vec<(i64,)> = query_builder
        .build_query_as()
        .fetch_all(pool)
        .await
        .map_err(internal_error)?;
    let valid_ids: HashSet<i64> = rows.into_iter().map(|(id,)| id).collect();

    if valid_ids.len() != citation_ids.len() {
        let invalid_ids: Vec<String> = citation_ids
            .iter()
            .filter(|id| !valid_ids.contains(id))
            .map(|id| id.to_string())
            .collect();

        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "detail": format!("Invalid citation target post IDs: {}", invalid_ids.join(", "))
            })),
        ));
    }

    Ok(())
}

fn normalize_category_code(raw: &str) -> String {
    let normalized = raw.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        "other".to_string()
    } else {
        normalized
    }
}

fn category_display_name(code: &str) -> String {
    code.split('_')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => {
                    let mut titled = String::new();
                    titled.extend(first.to_uppercase());
                    titled.push_str(chars.as_str());
                    titled
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

async fn resolve_or_create_category(
    pool: &MySqlPool,
    raw_category: &str,
) -> Result<(i64, String), (StatusCode, Json<serde_json::Value>)> {
    let code = normalize_category_code(raw_category);

    if let Some((id, existing_code)) = sqlx::query_as::<_, (i64, String)>(
        "SELECT CAST(id AS SIGNED) AS id, code FROM post_categories WHERE code = ?",
    )
    .bind(&code)
    .fetch_optional(pool)
    .await
    .map_err(internal_error)?
    {
        return Ok((id, existing_code));
    }

    let display_name = category_display_name(&code);
    let insert_result =
        sqlx::query("INSERT INTO post_categories (code, display_name) VALUES (?, ?)")
            .bind(&code)
            .bind(&display_name)
            .execute(pool)
            .await;

    if let Err(error) = insert_result {
        match &error {
            sqlx::Error::Database(db_error) if db_error.code().as_deref() == Some("1062") => {}
            _ => return Err(internal_error(error)),
        }
    }

    let (id, existing_code): (i64, String) =
        sqlx::query_as("SELECT CAST(id AS SIGNED) AS id, code FROM post_categories WHERE code = ?")
            .bind(&code)
            .fetch_one(pool)
            .await
            .map_err(internal_error)?;

    Ok((id, existing_code))
}

async fn clear_all_post_citations(
    pool: &MySqlPool,
    post_id: i64,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    sqlx::query("DELETE FROM post_citations WHERE citing_post_id = ? OR cited_post_id = ?")
        .bind(post_id)
        .bind(post_id)
        .execute(pool)
        .await
        .map_err(internal_error)?;

    Ok(())
}

async fn replace_post_citations(
    pool: &MySqlPool,
    post_id: i64,
    citation_ids: &[i64],
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    sqlx::query("DELETE FROM post_citations WHERE citing_post_id = ? AND citation_source_id = ?")
        .bind(post_id)
        .bind(CITATION_SOURCE_MANUAL)
        .execute(pool)
        .await
        .map_err(internal_error)?;

    for cited_post_id in citation_ids {
        if *cited_post_id == post_id {
            continue;
        }
        sqlx::query(
            "INSERT IGNORE INTO post_citations (citing_post_id, cited_post_id, citation_source_id, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind(post_id)
        .bind(cited_post_id)
        .bind(CITATION_SOURCE_MANUAL)
        .bind(Utc::now())
        .execute(pool)
        .await
        .map_err(internal_error)?;
    }

    Ok(())
}

async fn replace_post_auto_citations(
    pool: &MySqlPool,
    post_id: i64,
    citation_ids: &[i64],
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    sqlx::query("DELETE FROM post_citations WHERE citing_post_id = ? AND citation_source_id = ?")
        .bind(post_id)
        .bind(CITATION_SOURCE_AUTO)
        .execute(pool)
        .await
        .map_err(internal_error)?;

    for cited_post_id in citation_ids {
        if *cited_post_id == post_id {
            continue;
        }
        sqlx::query(
            "INSERT IGNORE INTO post_citations (citing_post_id, cited_post_id, citation_source_id, created_at) VALUES (?, ?, ?, ?)",
        )
        .bind(post_id)
        .bind(cited_post_id)
        .bind(CITATION_SOURCE_AUTO)
        .bind(Utc::now())
        .execute(pool)
        .await
        .map_err(internal_error)?;
    }

    Ok(())
}

async fn fetch_post_created_at(pool: &MySqlPool, post_id: i64) -> Result<Option<DateTime<Utc>>, sqlx::Error> {
    sqlx::query_scalar::<_, DateTime<Utc>>("SELECT created_at FROM posts WHERE id = ?")
        .bind(post_id)
        .fetch_optional(pool)
        .await
}

fn normalize_internal_doi_category(raw: &str) -> String {
    let mut normalized = String::new();
    let mut previous_was_separator = false;

    for ch in normalize_category_code(raw).chars() {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
            previous_was_separator = false;
            continue;
        }

        if !normalized.is_empty() && !previous_was_separator {
            normalized.push('_');
            previous_was_separator = true;
        }
    }

    while normalized.ends_with('_') {
        normalized.pop();
    }

    if normalized.is_empty() {
        "other".to_string()
    } else {
        normalized
    }
}

fn generate_internal_doi(post_id: i64, created_at: DateTime<Utc>, category: &str) -> String {
    let year = created_at.year();
    let normalized_category = normalize_internal_doi_category(category);

    let mut hasher = Sha256::new();
    hasher.update(INTERNAL_DOI_PREFIX.as_bytes());
    hasher.update(b":");
    hasher.update(year.to_string().as_bytes());
    hasher.update(b":");
    hasher.update(normalized_category.as_bytes());
    hasher.update(b":");
    hasher.update(post_id.to_string().as_bytes());
    hasher.update(b":");
    hasher.update(created_at.timestamp_micros().to_string().as_bytes());

    let hash = format!("{:X}", hasher.finalize());
    let hash_id = &hash[..INTERNAL_DOI_HASH_LENGTH.min(hash.len())];

    format!(
        "{}.{}.{}/{}",
        INTERNAL_DOI_PREFIX, year, normalized_category, hash_id
    )
}

fn build_internal_doi_record(
    post_id: i64,
    category: &str,
    created_at: DateTime<Utc>,
    title: Option<&str>,
) -> DoiMetadataRecord {
    let normalized_category = normalize_internal_doi_category(category);
    let doi = generate_internal_doi(post_id, created_at, &normalized_category);
    let year = created_at.year();

    DoiMetadataRecord {
        doi,
        title: title
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        journal: Some("Thought Manifold".to_string()),
        publisher: Some("Thought Manifold".to_string()),
        published_at: Some(created_at.format("%Y-%m-%d").to_string()),
        source_url: Some(format!("/posts/{}", post_id)),
        raw_json: Some(
            serde_json::json!({
                "source": "thought_manifold_internal_hash",
                "format": "TM.{year}.{category}/{hashID}",
                "year": year,
                "category": normalized_category,
            })
            .to_string(),
        ),
    }
}

async fn upsert_post_doi_metadata(
    pool: &MySqlPool,
    post_id: i64,
    record: &DoiMetadataRecord,
) -> Result<(), sqlx::Error> {
    let now = Utc::now();
    sqlx::query(
        r#"
        INSERT INTO post_doi_metadata (
            post_id,
            doi,
            title,
            journal,
            publisher,
            published_at,
            source_url,
            raw_json,
            created_at,
            updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON DUPLICATE KEY UPDATE
            title = VALUES(title),
            journal = VALUES(journal),
            publisher = VALUES(publisher),
            published_at = VALUES(published_at),
            source_url = VALUES(source_url),
            raw_json = VALUES(raw_json),
            updated_at = VALUES(updated_at)
        "#,
    )
    .bind(post_id)
    .bind(&record.doi)
    .bind(&record.title)
    .bind(&record.journal)
    .bind(&record.publisher)
    .bind(&record.published_at)
    .bind(&record.source_url)
    .bind(&record.raw_json)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(())
}

async fn ensure_internal_doi_metadata(pool: &MySqlPool, post_id: i64) -> anyhow::Result<()> {
    let Some(created_at) = fetch_post_created_at(pool, post_id).await? else {
        return Ok(());
    };

    let (category_code, title): (String, String) = sqlx::query_as(
        r#"
        SELECT c.code, p.title
        FROM posts p
        JOIN post_categories c ON c.id = p.category_id
        WHERE p.id = ?
        "#,
    )
    .bind(post_id)
    .fetch_one(pool)
    .await?;

    let internal_doi = generate_internal_doi(post_id, created_at, &category_code);
    let existing: Option<String> =
        sqlx::query_scalar("SELECT doi FROM post_doi_metadata WHERE post_id = ? AND doi = ? LIMIT 1")
            .bind(post_id)
            .bind(&internal_doi)
            .fetch_optional(pool)
            .await?;

    if existing.is_some() {
        return Ok(());
    }

    let internal_record = build_internal_doi_record(post_id, &category_code, created_at, Some(&title));
    upsert_post_doi_metadata(pool, post_id, &internal_record).await?;
    Ok(())
}

async fn sync_post_doi_metadata(
    pool: &MySqlPool,
    post_id: i64,
    category: &str,
    title: &str,
    summary: Option<&str>,
    content: &str,
) -> anyhow::Result<()> {
    let mut records = Vec::new();
    if let Some(created_at) = fetch_post_created_at(pool, post_id).await? {
        records.push(build_internal_doi_record(
            post_id,
            category,
            created_at,
            Some(title),
        ));
    }

    if category != PAPER_CATEGORY {
        replace_post_doi_metadata(pool, post_id, &records).await?;
        return Ok(());
    }

    let max_dois = std::env::var("CROSSREF_MAX_DOIS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_CROSSREF_MAX_DOIS);
    let timeout_secs = std::env::var("CROSSREF_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_CROSSREF_TIMEOUT_SECS);

    let dois = extract_doi_candidates(title, summary, content, max_dois);
    if dois.is_empty() {
        replace_post_doi_metadata(pool, post_id, &records).await?;
        return Ok(());
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .user_agent("ThoughtManifold/1.0 (mailto:admin@thought-manifold.local)")
        .build()?;

    records.reserve(dois.len());
    for doi in dois {
        match fetch_crossref_metadata_for_doi(&client, &doi).await {
            Ok(Some(mut record)) => {
                record.doi = doi;
                records.push(record);
            }
            Ok(None) => records.push(DoiMetadataRecord {
                doi,
                title: None,
                journal: None,
                publisher: None,
                published_at: None,
                source_url: None,
                raw_json: None,
            }),
            Err(error) => {
                tracing::warn!("Crossref lookup failed for DOI {}: {}", doi, error);
                records.push(DoiMetadataRecord {
                    doi,
                    title: None,
                    journal: None,
                    publisher: None,
                    published_at: None,
                    source_url: None,
                    raw_json: None,
                });
            }
        }
    }

    replace_post_doi_metadata(pool, post_id, &records).await?;
    Ok(())
}

async fn replace_post_doi_metadata(
    pool: &MySqlPool,
    post_id: i64,
    records: &[DoiMetadataRecord],
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM post_doi_metadata WHERE post_id = ?")
        .bind(post_id)
        .execute(&mut *tx)
        .await?;

    let now = Utc::now();
    for record in records {
        sqlx::query(
            r#"
            INSERT INTO post_doi_metadata (
                post_id,
                doi,
                title,
                journal,
                publisher,
                published_at,
                source_url,
                raw_json,
                created_at,
                updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(post_id)
        .bind(&record.doi)
        .bind(&record.title)
        .bind(&record.journal)
        .bind(&record.publisher)
        .bind(&record.published_at)
        .bind(&record.source_url)
        .bind(&record.raw_json)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

fn collapse_bibtex_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn escape_bibtex_value(value: &str) -> String {
    collapse_bibtex_whitespace(value)
        .replace('\\', "\\\\")
        .replace('{', "\\{")
        .replace('}', "\\}")
}

fn sanitize_bibtex_key_fragment(raw: &str) -> String {
    let mut key = String::new();
    let mut previous_was_separator = false;

    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            key.push(ch.to_ascii_lowercase());
            previous_was_separator = false;
        } else if !previous_was_separator {
            key.push('_');
            previous_was_separator = true;
        }
    }

    while key.starts_with('_') {
        key.remove(0);
    }
    while key.ends_with('_') {
        key.pop();
    }

    if key.len() > 64 {
        key.truncate(64);
    }

    key
}

fn extract_bibtex_year(doi: &str, published_at: Option<&str>) -> Option<String> {
    if let Some(value) = published_at {
        let trimmed = value.trim();
        let year: String = trimmed.chars().take(4).collect();
        if year.len() == 4 && year.chars().all(|ch| ch.is_ascii_digit()) {
            return Some(year);
        }
    }

    let mut parts = doi.splitn(2, '/');
    let prefix = parts.next().unwrap_or_default();
    let segments: Vec<&str> = prefix.split('.').collect();
    if segments.len() >= 3
        && segments[0].eq_ignore_ascii_case(INTERNAL_DOI_PREFIX)
        && segments[1].chars().all(|ch| ch.is_ascii_digit())
    {
        return Some(segments[1].to_string());
    }

    None
}

fn extract_bibtex_month(published_at: Option<&str>) -> Option<String> {
    let value = published_at?.trim();
    let month = value.split('-').nth(1)?;
    let normalized: String = month.chars().take(2).collect();
    (normalized.len() == 2 && normalized.chars().all(|ch| ch.is_ascii_digit()))
        .then_some(normalized)
}

fn frontend_base_url_for_links() -> String {
    std::env::var("FRONTEND_URL")
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "http://localhost:5173".to_string())
}

fn resolve_bibtex_link(post_id: i64, doi: &str, source_url: Option<&str>) -> String {
    if let Some(source) = source_url.map(str::trim).filter(|value| !value.is_empty()) {
        if source.starts_with("http://") || source.starts_with("https://") {
            return source.to_string();
        }

        let base = frontend_base_url_for_links();
        if source.starts_with('/') {
            return format!("{}{}", base, source);
        }
        return format!("{}/{}", base, source);
    }

    if doi
        .split('.')
        .next()
        .map(|segment| segment.eq_ignore_ascii_case(INTERNAL_DOI_PREFIX))
        .unwrap_or(false)
    {
        return format!("{}/posts/{}", frontend_base_url_for_links(), post_id);
    }

    format!("https://doi.org/{}", doi)
}

async fn fetch_post_bibtex_author(pool: &MySqlPool, post_id: i64) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT COALESCE(NULLIF(TRIM(u.display_name), ''), u.username)
        FROM posts p
        JOIN users u ON u.id = p.author_id
        WHERE p.id = ?
        LIMIT 1
        "#,
    )
    .bind(post_id)
    .fetch_optional(pool)
    .await
}

fn build_bibtex_from_doi_metadata(
    post_id: i64,
    doi: &str,
    title: Option<&str>,
    author: Option<&str>,
    journal: Option<&str>,
    publisher: Option<&str>,
    published_at: Option<&str>,
    source_url: Option<&str>,
) -> String {
    let entry_type = if journal.is_some() { "article" } else { "misc" };
    let mut key = sanitize_bibtex_key_fragment(doi);
    if key.is_empty() {
        key = format!("tm_post_{}", post_id);
    } else if key
        .chars()
        .next()
        .map(|ch| ch.is_ascii_digit())
        .unwrap_or(false)
    {
        key = format!("tm_{}", key);
    }

    let mut fields: Vec<(&str, String)> = Vec::new();
    let resolved_title = title
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("Thought Manifold Post {}", post_id));
    fields.push(("title", resolved_title));
    if let Some(value) = author.map(str::trim).filter(|value| !value.is_empty()) {
        fields.push(("author", value.to_string()));
    }

    if let Some(value) = journal.map(str::trim).filter(|value| !value.is_empty()) {
        fields.push(("journal", value.to_string()));
    }
    if let Some(value) = publisher.map(str::trim).filter(|value| !value.is_empty()) {
        fields.push(("publisher", value.to_string()));
    }
    if let Some(value) = extract_bibtex_year(doi, published_at) {
        fields.push(("year", value));
    }
    if let Some(value) = extract_bibtex_month(published_at) {
        fields.push(("month", value));
    }

    fields.push(("doi", doi.to_string()));
    let resolved_link = resolve_bibtex_link(post_id, doi, source_url);
    fields.push(("url", resolved_link.clone()));
    fields.push(("link", resolved_link));

    fields.push((
        "note",
        "Auto-generated by Thought Manifold DOI service".to_string(),
    ));

    let mut bibtex = String::new();
    bibtex.push_str(&format!("@{}{{{},\n", entry_type, key));
    for (name, value) in fields {
        bibtex.push_str(&format!("  {} = {{{}}},\n", name, escape_bibtex_value(&value)));
    }
    bibtex.push('}');
    bibtex
}

async fn fetch_post_doi_metadata(
    pool: &MySqlPool,
    post_id: i64,
) -> Result<Vec<PostDoiMetadata>, sqlx::Error> {
    let bibtex_author = fetch_post_bibtex_author(pool, post_id).await?;

    let rows: Vec<(
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        r#"
        SELECT doi, title, journal, publisher, published_at, source_url
        FROM post_doi_metadata
        WHERE post_id = ?
        ORDER BY created_at DESC, id DESC
        "#,
    )
    .bind(post_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(doi, title, journal, publisher, published_at, source_url)| PostDoiMetadata {
                bibtex: build_bibtex_from_doi_metadata(
                    post_id,
                    &doi,
                    title.as_deref(),
                    bibtex_author.as_deref(),
                    journal.as_deref(),
                    publisher.as_deref(),
                    published_at.as_deref(),
                    source_url.as_deref(),
                ),
                doi,
                title,
                journal,
                publisher,
                published_at,
                source_url,
            },
        )
        .collect())
}

fn extract_doi_candidates(
    title: &str,
    summary: Option<&str>,
    content: &str,
    max_dois: usize,
) -> Vec<String> {
    let mut joined = String::with_capacity(
        title.len() + summary.map(|value| value.len()).unwrap_or(0) + content.len() + 8,
    );
    joined.push_str(title);
    joined.push('\n');
    if let Some(value) = summary {
        joined.push_str(value);
        joined.push('\n');
    }
    joined.push_str(content);

    let regex = match Regex::new(DOI_PATTERN) {
        Ok(compiled) => compiled,
        Err(error) => {
            tracing::error!("Failed to compile DOI regex: {}", error);
            return Vec::new();
        }
    };

    let mut seen = HashSet::new();
    let mut dois = Vec::new();

    for matched in regex.find_iter(&joined) {
        let Some(normalized) = normalize_doi(matched.as_str()) else {
            continue;
        };

        if seen.insert(normalized.clone()) {
            dois.push(normalized);
            if dois.len() >= max_dois {
                break;
            }
        }
    }

    dois
}

fn normalize_doi(raw: &str) -> Option<String> {
    let trimmed = raw
        .trim()
        .trim_matches(|ch: char| {
            matches!(
                ch,
                '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | ',' | '.' | ';' | ':'
            )
        })
        .trim();

    if trimmed.is_empty() {
        return None;
    }

    Some(trimmed.to_ascii_lowercase())
}

async fn fetch_crossref_metadata_for_doi(
    client: &Client,
    doi: &str,
) -> anyhow::Result<Option<DoiMetadataRecord>> {
    let url = format!("{}{}", CROSSREF_API_BASE, urlencoding::encode(doi));
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let payload = response.json::<serde_json::Value>().await?;
    let message = payload
        .get("message")
        .and_then(|value| value.as_object())
        .cloned()
        .unwrap_or_default();
    let message_value = serde_json::Value::Object(message);

    Ok(Some(DoiMetadataRecord {
        doi: doi.to_string(),
        title: extract_crossref_title(&message_value),
        journal: extract_crossref_first_array_text(&message_value, "container-title"),
        publisher: extract_crossref_text(&message_value, "publisher"),
        published_at: extract_crossref_published_at(&message_value),
        source_url: extract_crossref_text(&message_value, "URL")
            .or_else(|| Some(format!("https://doi.org/{}", doi))),
        raw_json: Some(payload.to_string()),
    }))
}

fn extract_crossref_text(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|item| item.as_str())
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
}

fn extract_crossref_first_array_text(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|item| item.as_array())
        .and_then(|items| items.iter().find_map(|entry| entry.as_str()))
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
}

fn extract_crossref_title(value: &serde_json::Value) -> Option<String> {
    extract_crossref_first_array_text(value, "title").or_else(|| extract_crossref_text(value, "title"))
}

fn extract_crossref_published_at(value: &serde_json::Value) -> Option<String> {
    for key in ["published-print", "published-online", "issued"] {
        let Some(date_parts) = value
            .get(key)
            .and_then(|entry| entry.get("date-parts"))
            .and_then(|entry| entry.as_array())
            .and_then(|outer| outer.first())
            .and_then(|entry| entry.as_array())
        else {
            continue;
        };

        let year = date_parts.first().and_then(|value| value.as_i64());
        let month = date_parts.get(1).and_then(|value| value.as_i64());
        let day = date_parts.get(2).and_then(|value| value.as_i64());

        if let Some(year_value) = year {
            if let (Some(month_value), Some(day_value)) = (month, day) {
                return Some(format!(
                    "{:04}-{:02}-{:02}",
                    year_value, month_value, day_value
                ));
            }
            if let Some(month_value) = month {
                return Some(format!("{:04}-{:02}", year_value, month_value));
            }
            return Some(format!("{:04}", year_value));
        }
    }

    None
}

async fn create_paper_version_snapshot(
    pool: &MySqlPool,
    post_id: i64,
    submitted_by: i64,
) -> Result<(i64, i32), (StatusCode, Json<serde_json::Value>)> {
    let mut tx = pool.begin().await.map_err(internal_error)?;

    let (next_version,): (i32,) = sqlx::query_as(
        "SELECT CAST(COALESCE(MAX(version_number), 0) + 1 AS SIGNED) FROM paper_versions WHERE post_id = ?",
    )
    .bind(post_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(internal_error)?;

    let source = sqlx::query_as::<
        _,
        (
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        ),
    >(
        r#"
        SELECT
            p.title,
            p.content,
            p.summary,
            p.github_url,
            pf.file_path,
            pf.file_name
        FROM posts p
        LEFT JOIN post_files pf ON pf.post_id = p.id
        WHERE p.id = ?
        "#,
    )
    .bind(post_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(internal_error)?;

    let tags: Vec<String> = sqlx::query_as::<_, (String,)>(
        r#"
        SELECT t.name
        FROM post_tags pt
        JOIN tags t ON t.id = pt.tag_id
        WHERE pt.post_id = ?
        ORDER BY t.name
        "#,
    )
    .bind(post_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(internal_error)?
    .into_iter()
    .map(|(name,)| name)
    .collect();

    let citations: Vec<i64> =
        sqlx::query_as::<_, (i64,)>("SELECT DISTINCT cited_post_id FROM post_citations WHERE citing_post_id = ? ORDER BY cited_post_id")
            .bind(post_id)
            .fetch_all(&mut *tx)
            .await
            .map_err(internal_error)?
            .into_iter()
            .map(|(id,)| id)
            .collect();

    let now = Utc::now();
    let tags_json = if tags.is_empty() {
        None
    } else {
        Some(serde_json::to_string(&tags).map_err(internal_error)?)
    };
    let citations_json = if citations.is_empty() {
        None
    } else {
        Some(serde_json::to_string(&citations).map_err(internal_error)?)
    };

    let result = sqlx::query(
        r#"
        INSERT INTO paper_versions (
            post_id,
            version_number,
            title,
            content,
            summary,
            github_url,
            file_path,
            file_name,
            tags_json,
            citations_json,
            submitted_by,
            submitted_at,
            created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(post_id)
    .bind(next_version)
    .bind(&source.0)
    .bind(&source.1)
    .bind(&source.2)
    .bind(&source.3)
    .bind(&source.4)
    .bind(&source.5)
    .bind(&tags_json)
    .bind(&citations_json)
    .bind(submitted_by)
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(internal_error)?;

    let version_id = result.last_insert_id() as i64;

    sqlx::query(
        "UPDATE posts SET current_revision = ?, latest_paper_version_id = ?, updated_at = ? WHERE id = ?",
    )
    .bind(next_version)
    .bind(version_id)
    .bind(now)
    .bind(post_id)
    .execute(&mut *tx)
    .await
    .map_err(internal_error)?;

    tx.commit().await.map_err(internal_error)?;
    Ok((version_id, next_version))
}

fn internal_error<E: ToString>(error: E) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({"detail": error.to_string()})),
    )
}

fn multipart_error(error: MultipartError) -> (StatusCode, Json<serde_json::Value>) {
    (
        error.status(),
        Json(serde_json::json!({"detail": error.body_text()})),
    )
}
