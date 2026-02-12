use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Multipart, Path, Query, State, multipart::MultipartError},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use chrono::Utc;
use reqwest::Url;
use serde::Deserialize;
use sqlx::{MySql, MySqlPool, QueryBuilder};
use std::{
    collections::{HashMap, HashSet},
    path::{Path as FsPath, PathBuf},
};
use uuid::Uuid;

use crate::ai_review::{ReviewTrigger, schedule_review};
use crate::metrics::{METRIC_VERSION, compute_citation_count, compute_citation_counts_for_posts};
use crate::models::{
    PAPER_STATUS_ACCEPTED, PAPER_STATUS_DRAFT, PAPER_STATUS_PUBLISHED, PAPER_STATUS_SUBMITTED,
    Post, PostListResponse, PostMetrics, PostQuery, PostResponse, User, UserResponse,
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
        COALESCE(ps.view_count, 0) AS view_count,
        COALESCE(ps.like_count, 0) AS like_count,
        p.created_at,
        p.updated_at
"#;
const ALLOWED_UPLOAD_EXTENSIONS: &[&str] = &[
    "pdf", "doc", "docx", "txt", "md", "pptx", "xlsx", "zip", "png", "jpg", "jpeg", "gif",
];

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

    let mut posts_qb = QueryBuilder::<MySql>::new(format!(
        "{}{}",
        POST_SELECT_COLUMNS, POST_SELECT_FROM_CLAUSE
    ));
    let mut posts_has_where = false;
    push_post_filters(&mut posts_qb, &query, &mut posts_has_where);
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
    push_post_filters(&mut count_qb, &query, &mut count_has_where);
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
            view_count: post.view_count,
            like_count: post.like_count,
            user_liked: None,
            metrics: PostMetrics {
                citation_count,
                metric_version: METRIC_VERSION.to_string(),
            },
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
        view_count: post.view_count + 1,
        like_count: post.like_count,
        user_liked,
        metrics: PostMetrics {
            citation_count,
            metric_version: METRIC_VERSION.to_string(),
        },
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

    if category_code == PAPER_CATEGORY && paper_status == PAPER_STATUS_SUBMITTED {
        if let Err(error) = schedule_review(&pool, post_id, ReviewTrigger::AutoCreate).await {
            tracing::error!(
                "Failed to schedule auto AI review on create for post {}: {}",
                post_id,
                error
            );
        }
    }

    let tags_vec = process_tags(&pool, post_id, &tags_str).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"detail": e.to_string()})),
        )
    })?;

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
            view_count: post.view_count,
            like_count: post.like_count,
            user_liked: Some(false),
            metrics: PostMetrics {
                citation_count,
                metric_version: METRIC_VERSION.to_string(),
            },
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
    } else {
        if let Some(ids) = manual_citation_ids {
            replace_post_citations(&pool, post_id, &ids).await?;
        }

        let auto_citation_ids =
            prepare_auto_citations_for_content(&pool, &category_code, &content, Some(post_id))
                .await?;
        replace_post_auto_citations(&pool, post_id, &auto_citation_ids).await?;

        if paper_status == PAPER_STATUS_SUBMITTED {
            if let Err(error) = schedule_review(&pool, post_id, ReviewTrigger::AutoUpdate).await {
                tracing::error!(
                    "Failed to schedule auto AI review on update for post {}: {}",
                    post_id,
                    error
                );
            }
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
        view_count: updated_post.view_count,
        like_count: updated_post.like_count,
        user_liked: Some(user_liked),
        metrics: PostMetrics {
            citation_count,
            metric_version: METRIC_VERSION.to_string(),
        },
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
    query: &PostQuery,
    has_where: &mut bool,
) {
    if let Some(category) = query.category.as_ref() {
        push_condition(query_builder, has_where);
        query_builder.push("c.code = ");
        query_builder.push_bind(category.clone());
    }

    if let Some(search) = query.search.as_ref() {
        let search_pattern = format!("%{}%", search);
        push_condition(query_builder, has_where);
        query_builder.push("(p.title LIKE ");
        query_builder.push_bind(search_pattern.clone());
        query_builder.push(" OR p.content LIKE ");
        query_builder.push_bind(search_pattern);
        query_builder.push(")");
    }

    if let Some(tag) = query.tag.as_ref() {
        push_condition(query_builder, has_where);
        query_builder.push(
            "EXISTS (SELECT 1 FROM post_tags pt JOIN tags t ON t.id = pt.tag_id WHERE pt.post_id = p.id AND t.name = ",
        );
        query_builder.push_bind(tag.clone());
        query_builder.push(")");
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
