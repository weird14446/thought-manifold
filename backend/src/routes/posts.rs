use axum::{
    extract::{multipart::MultipartError, DefaultBodyLimit, Multipart, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use sqlx::{QueryBuilder, Sqlite, SqlitePool};
use std::{
    collections::{HashMap, HashSet},
    path::{Path as FsPath, PathBuf},
};
use uuid::Uuid;

use crate::models::{Post, PostListResponse, PostQuery, PostResponse, User, UserResponse};
use crate::routes::auth::{extract_current_user, extract_optional_user};

const MAX_UPLOAD_SIZE_BYTES: usize = 10 * 1024 * 1024;
const MULTIPART_BODY_LIMIT_BYTES: usize = 12 * 1024 * 1024;
const ALLOWED_UPLOAD_EXTENSIONS: &[&str] = &[
    "pdf", "doc", "docx", "txt", "md", "pptx", "xlsx", "zip", "png", "jpg", "jpeg", "gif",
];

pub fn posts_routes() -> Router<SqlitePool> {
    Router::new()
        .route("/", get(list_posts).post(create_post))
        .route("/{post_id}", get(get_post).put(update_post).delete(delete_post))
        .route("/{post_id}/like", post(like_post))
        // Keep multipart parsing above the 10MB policy threshold so route-level validation can return a precise 413.
        .layer(DefaultBodyLimit::max(MULTIPART_BODY_LIMIT_BYTES))
}

async fn list_posts(
    State(pool): State<SqlitePool>,
    Query(query): Query<PostQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(10).clamp(1, 100);
    let offset = i64::from(page - 1) * i64::from(per_page);

    let mut posts_qb = QueryBuilder::<Sqlite>::new("SELECT p.* FROM posts p");
    push_post_filters(&mut posts_qb, &query);
    posts_qb.push(" ORDER BY p.created_at DESC LIMIT ");
    posts_qb.push_bind(i64::from(per_page));
    posts_qb.push(" OFFSET ");
    posts_qb.push_bind(offset);

    let posts = posts_qb
        .build_query_as::<Post>()
        .fetch_all(&pool)
        .await
        .map_err(internal_error)?;

    let mut count_qb = QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM posts p");
    push_post_filters(&mut count_qb, &query);
    let (total,): (i64,) = count_qb
        .build_query_as()
        .fetch_one(&pool)
        .await
        .map_err(internal_error)?;

    let author_map = fetch_authors_map(&pool, &posts)
        .await
        .map_err(internal_error)?;
    let tags_map = fetch_tags_map(&pool, &posts).await.map_err(internal_error)?;

    let mut post_responses = Vec::with_capacity(posts.len());
    for post in posts {
        let author = author_map.get(&post.author_id).cloned().ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": "Post author not found"})),
            )
        })?;

        let tags = tags_map.get(&post.id).cloned().unwrap_or_default();

        post_responses.push(PostResponse {
            id: post.id,
            title: post.title,
            content: post.content,
            summary: post.summary,
            category: post.category,
            file_path: post.file_path,
            file_name: post.file_name,
            author_id: post.author_id,
            author,
            view_count: post.view_count,
            like_count: post.like_count,
            user_liked: None,
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
    State(pool): State<SqlitePool>,
    headers: HeaderMap,
    Path(post_id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let post = sqlx::query_as::<_, Post>("SELECT * FROM posts WHERE id = ?")
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

    sqlx::query("UPDATE posts SET view_count = view_count + 1 WHERE id = ?")
        .bind(post_id)
        .execute(&pool)
        .await
        .map_err(internal_error)?;

    let author = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(post.author_id)
        .fetch_one(&pool)
        .await
        .map_err(internal_error)?;

    let tags = fetch_tags(&pool, post.id).await.unwrap_or_default();
    let current_user = extract_optional_user(&pool, &headers).await?;
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
        category: post.category,
        file_path: post.file_path,
        file_name: post.file_name,
        author_id: post.author_id,
        author: UserResponse::from(author),
        view_count: post.view_count + 1,
        like_count: post.like_count,
        user_liked,
        created_at: post.created_at,
        updated_at: post.updated_at,
        tags,
    }))
}

async fn create_post(
    State(pool): State<SqlitePool>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let current_user = extract_current_user(&pool, &headers).await?;

    let mut title = String::new();
    let mut content = String::new();
    let mut summary: Option<String> = None;
    let mut category = "other".to_string();
    let mut file_path: Option<String> = None;
    let mut file_name: Option<String> = None;
    let mut tags_str = String::new();

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
            "category" => {
                category = field.text().await.map_err(multipart_error)?;
            }
            "tags" => {
                tags_str = field.text().await.map_err(multipart_error)?;
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

    let now = Utc::now();
    let result = sqlx::query(
        r#"INSERT INTO posts (title, content, summary, category, file_path, file_name, author_id, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&title)
    .bind(&content)
    .bind(&summary)
    .bind(&category)
    .bind(&file_path)
    .bind(&file_name)
    .bind(current_user.id)
    .bind(now)
    .execute(&pool)
    .await
    .map_err(internal_error)?;

    let post_id = result.last_insert_rowid();

    let tags_vec = process_tags(&pool, post_id, &tags_str).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"detail": e.to_string()})),
        )
    })?;

    let post = sqlx::query_as::<_, Post>("SELECT * FROM posts WHERE id = ?")
        .bind(post_id)
        .fetch_one(&pool)
        .await
        .map_err(internal_error)?;

    Ok((
        StatusCode::CREATED,
        Json(PostResponse {
            id: post.id,
            title: post.title,
            content: post.content,
            summary: post.summary,
            category: post.category,
            file_path: post.file_path,
            file_name: post.file_name,
            author_id: post.author_id,
            author: UserResponse::from(current_user),
            view_count: post.view_count,
            like_count: post.like_count,
            user_liked: Some(false),
            created_at: post.created_at,
            updated_at: post.updated_at,
            tags: tags_vec,
        }),
    ))
}

async fn update_post(
    State(pool): State<SqlitePool>,
    headers: HeaderMap,
    Path(post_id): Path<i64>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let current_user = extract_current_user(&pool, &headers).await?;

    let post = sqlx::query_as::<_, Post>("SELECT * FROM posts WHERE id = ?")
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
    let mut category = post.category.clone();
    let mut file_path = post.file_path.clone();
    let mut file_name = post.file_name.clone();
    let mut remove_file = false;
    let mut tags_str: Option<String> = None;
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
            "category" => {
                let val = field.text().await.map_err(multipart_error)?;
                if !val.is_empty() {
                    category = val;
                }
            }
            "tags" => {
                tags_str = Some(field.text().await.map_err(multipart_error)?);
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
    } else if remove_file && file_path.is_some() {
        if let Some(ref path) = post.file_path {
            let _ = tokio::fs::remove_file(path).await;
        }
        file_path = None;
        file_name = None;
    }

    let now = Utc::now();
    sqlx::query(
        "UPDATE posts SET title = ?, content = ?, summary = ?, category = ?, file_path = ?, file_name = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&title)
    .bind(&content)
    .bind(&summary)
    .bind(&category)
    .bind(&file_path)
    .bind(&file_name)
    .bind(now)
    .bind(post_id)
    .execute(&pool)
    .await
    .map_err(internal_error)?;

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

    let updated_post = sqlx::query_as::<_, Post>("SELECT * FROM posts WHERE id = ?")
        .bind(post_id)
        .fetch_one(&pool)
        .await
        .map_err(internal_error)?;

    let user_liked = fetch_user_liked(&pool, current_user.id, post_id)
        .await
        .map_err(internal_error)?;

    Ok(Json(PostResponse {
        id: updated_post.id,
        title: updated_post.title,
        content: updated_post.content,
        summary: updated_post.summary,
        category: updated_post.category,
        file_path: updated_post.file_path,
        file_name: updated_post.file_name,
        author_id: updated_post.author_id,
        author: UserResponse::from(current_user),
        view_count: updated_post.view_count,
        like_count: updated_post.like_count,
        user_liked: Some(user_liked),
        created_at: updated_post.created_at,
        updated_at: updated_post.updated_at,
        tags: tags_vec,
    }))
}

async fn delete_post(
    State(pool): State<SqlitePool>,
    headers: HeaderMap,
    Path(post_id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let current_user = extract_current_user(&pool, &headers).await?;

    let post = sqlx::query_as::<_, Post>("SELECT * FROM posts WHERE id = ?")
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

    sqlx::query("DELETE FROM posts WHERE id = ?")
        .bind(post_id)
        .execute(&pool)
        .await
        .map_err(internal_error)?;

    Ok(Json(
        serde_json::json!({"message": "Post deleted successfully"}),
    ))
}

async fn like_post(
    State(pool): State<SqlitePool>,
    headers: HeaderMap,
    Path(post_id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let current_user = extract_current_user(&pool, &headers).await?;

    let _post = sqlx::query("SELECT id FROM posts WHERE id = ?")
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

    sqlx::query("UPDATE posts SET like_count = ? WHERE id = ?")
        .bind(new_count)
        .bind(post_id)
        .execute(&pool)
        .await
        .map_err(internal_error)?;

    Ok(Json(serde_json::json!({
        "message": if user_liked { "Post liked" } else { "Post unliked" },
        "like_count": new_count,
        "user_liked": user_liked
    })))
}

fn push_post_filters(query_builder: &mut QueryBuilder<Sqlite>, query: &PostQuery) {
    let mut has_where = false;

    if let Some(category) = query.category.as_ref() {
        push_condition(query_builder, &mut has_where);
        query_builder.push("p.category = ");
        query_builder.push_bind(category.clone());
    }

    if let Some(search) = query.search.as_ref() {
        let search_pattern = format!("%{}%", search);
        push_condition(query_builder, &mut has_where);
        query_builder.push("(p.title LIKE ");
        query_builder.push_bind(search_pattern.clone());
        query_builder.push(" OR p.content LIKE ");
        query_builder.push_bind(search_pattern);
        query_builder.push(")");
    }

    if let Some(tag) = query.tag.as_ref() {
        push_condition(query_builder, &mut has_where);
        query_builder.push(
            "EXISTS (SELECT 1 FROM post_tags pt JOIN tags t ON t.id = pt.tag_id WHERE pt.post_id = p.id AND t.name = ",
        );
        query_builder.push_bind(tag.clone());
        query_builder.push(")");
    }
}

fn push_condition(query_builder: &mut QueryBuilder<Sqlite>, has_where: &mut bool) {
    if *has_where {
        query_builder.push(" AND ");
    } else {
        query_builder.push(" WHERE ");
        *has_where = true;
    }
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

async fn fetch_user_liked(pool: &SqlitePool, user_id: i64, post_id: i64) -> Result<bool, sqlx::Error> {
    let liked = sqlx::query("SELECT 1 FROM post_likes WHERE user_id = ? AND post_id = ?")
        .bind(user_id)
        .bind(post_id)
        .fetch_optional(pool)
        .await?;

    Ok(liked.is_some())
}

async fn fetch_authors_map(
    pool: &SqlitePool,
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

    let mut query_builder = QueryBuilder::<Sqlite>::new("SELECT * FROM users WHERE id IN (");
    {
        let mut separated = query_builder.separated(", ");
        for author_id in &author_ids {
            separated.push_bind(author_id);
        }
    }
    query_builder.push(")");

    let users = query_builder.build_query_as::<User>().fetch_all(pool).await?;

    Ok(users
        .into_iter()
        .map(|user| (user.id, UserResponse::from(user)))
        .collect())
}

async fn fetch_tags_map(pool: &SqlitePool, posts: &[Post]) -> Result<HashMap<i64, Vec<String>>, sqlx::Error> {
    let post_ids: Vec<i64> = posts.iter().map(|post| post.id).collect();

    if post_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut query_builder = QueryBuilder::<Sqlite>::new(
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

async fn fetch_tags(pool: &SqlitePool, post_id: i64) -> Result<Vec<String>, sqlx::Error> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT t.name FROM tags t JOIN post_tags pt ON t.id = pt.tag_id WHERE pt.post_id = ?",
    )
    .bind(post_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|(name,)| name).collect())
}

async fn process_tags(
    pool: &SqlitePool,
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
        let tag_id: i64 =
            if let Some(row) = sqlx::query_as::<_, (i64,)>("SELECT id FROM tags WHERE name = ?")
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
                res.last_insert_rowid()
            };

        let _ = sqlx::query("INSERT OR IGNORE INTO post_tags (post_id, tag_id) VALUES (?, ?)")
            .bind(post_id)
            .bind(tag_id)
            .execute(pool)
            .await;

        final_tags.push(tag);
    }

    Ok(final_tags)
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
