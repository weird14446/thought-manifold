use axum::{
    extract::{Multipart, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use sqlx::SqlitePool;
use std::path::PathBuf;
use uuid::Uuid;
use chrono::Utc;

use crate::models::{Post, PostResponse, PostListResponse, PostQuery, User, UserResponse};
use crate::routes::auth::extract_current_user;

pub fn posts_routes() -> Router<SqlitePool> {
    Router::new()
        .route("/", get(list_posts).post(create_post))
        .route("/{post_id}", get(get_post).delete(delete_post))
        .route("/{post_id}/like", post(like_post))
}

async fn list_posts(
    State(pool): State<SqlitePool>,
    Query(query): Query<PostQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let page = query.page.unwrap_or(1);
    let per_page = query.per_page.unwrap_or(10);
    let offset = (page - 1) * per_page;

    // Build query based on filters
    let (posts, total): (Vec<Post>, i64) = if let Some(ref category) = query.category {
        let posts = sqlx::query_as::<_, Post>(
            "SELECT * FROM posts WHERE category = ? ORDER BY created_at DESC LIMIT ? OFFSET ?"
        )
        .bind(category)
        .bind(per_page)
        .bind(offset)
        .fetch_all(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()}))))?;

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM posts WHERE category = ?")
            .bind(category)
            .fetch_one(&pool)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()}))))?;
        
        (posts, count.0)
    } else if let Some(ref search) = query.search {
        let search_pattern = format!("%{}%", search);
        let posts = sqlx::query_as::<_, Post>(
            "SELECT * FROM posts WHERE title LIKE ? OR content LIKE ? ORDER BY created_at DESC LIMIT ? OFFSET ?"
        )
        .bind(&search_pattern)
        .bind(&search_pattern)
        .bind(per_page)
        .bind(offset)
        .fetch_all(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()}))))?;

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM posts WHERE title LIKE ? OR content LIKE ?")
            .bind(&search_pattern)
            .bind(&search_pattern)
            .fetch_one(&pool)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()}))))?;
        
        (posts, count.0)
    } else {
        let posts = sqlx::query_as::<_, Post>(
            "SELECT * FROM posts ORDER BY created_at DESC LIMIT ? OFFSET ?"
        )
        .bind(per_page)
        .bind(offset)
        .fetch_all(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()}))))?;

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM posts")
            .fetch_one(&pool)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()}))))?;
        
        (posts, count.0)
    };

    // Get authors for each post
    let mut post_responses = Vec::new();
    for post in posts {
        let author = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
            .bind(post.author_id)
            .fetch_one(&pool)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()}))))?;

        post_responses.push(PostResponse {
            id: post.id,
            title: post.title,
            content: post.content,
            summary: post.summary,
            category: post.category,
            file_path: post.file_path,
            file_name: post.file_name,
            author_id: post.author_id,
            author: UserResponse::from(author),
            view_count: post.view_count,
            like_count: post.like_count,
            created_at: post.created_at,
            updated_at: post.updated_at,
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
    Path(post_id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let post = sqlx::query_as::<_, Post>("SELECT * FROM posts WHERE id = ?")
        .bind(post_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()}))))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, Json(serde_json::json!({"detail": "Post not found"}))))?;

    // Increment view count
    sqlx::query("UPDATE posts SET view_count = view_count + 1 WHERE id = ?")
        .bind(post_id)
        .execute(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()}))))?;

    let author = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(post.author_id)
        .fetch_one(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()}))))?;

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
        created_at: post.created_at,
        updated_at: post.updated_at,
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

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (StatusCode::BAD_REQUEST, Json(serde_json::json!({"detail": e.to_string()})))
    })? {
        let name = field.name().unwrap_or_default().to_string();
        
        match name.as_str() {
            "title" => {
                title = field.text().await.map_err(|e| {
                    (StatusCode::BAD_REQUEST, Json(serde_json::json!({"detail": e.to_string()})))
                })?;
            }
            "content" => {
                content = field.text().await.map_err(|e| {
                    (StatusCode::BAD_REQUEST, Json(serde_json::json!({"detail": e.to_string()})))
                })?;
            }
            "summary" => {
                summary = Some(field.text().await.map_err(|e| {
                    (StatusCode::BAD_REQUEST, Json(serde_json::json!({"detail": e.to_string()})))
                })?);
            }
            "category" => {
                category = field.text().await.map_err(|e| {
                    (StatusCode::BAD_REQUEST, Json(serde_json::json!({"detail": e.to_string()})))
                })?;
            }
            "file" => {
                if let Some(original_name) = field.file_name() {
                    let original_name = original_name.to_string();
                    let ext = std::path::Path::new(&original_name)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("bin");
                    
                    let unique_name = format!("{}.{}", Uuid::new_v4(), ext);
                    let upload_path = PathBuf::from("uploads").join(&unique_name);
                    
                    let data = field.bytes().await.map_err(|e| {
                        (StatusCode::BAD_REQUEST, Json(serde_json::json!({"detail": e.to_string()})))
                    })?;
                    
                    tokio::fs::write(&upload_path, &data).await.map_err(|e| {
                        (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()})))
                    })?;
                    
                    file_path = Some(upload_path.to_string_lossy().to_string());
                    file_name = Some(original_name);
                }
            }
            _ => {}
        }
    }

    if title.is_empty() || content.is_empty() {
        return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({"detail": "Title and content are required"}))));
    }

    let now = Utc::now();
    let result = sqlx::query(
        r#"INSERT INTO posts (title, content, summary, category, file_path, file_name, author_id, created_at) 
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#
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
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()}))))?;

    let post_id = result.last_insert_rowid();
    let post = sqlx::query_as::<_, Post>("SELECT * FROM posts WHERE id = ?")
        .bind(post_id)
        .fetch_one(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()}))))?;

    Ok((StatusCode::CREATED, Json(PostResponse {
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
        created_at: post.created_at,
        updated_at: post.updated_at,
    })))
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
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()}))))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, Json(serde_json::json!({"detail": "Post not found"}))))?;

    if post.author_id != current_user.id {
        return Err((StatusCode::FORBIDDEN, Json(serde_json::json!({"detail": "Not authorized to delete this post"}))));
    }

    // Delete file if exists
    if let Some(ref path) = post.file_path {
        let _ = tokio::fs::remove_file(path).await;
    }

    sqlx::query("DELETE FROM posts WHERE id = ?")
        .bind(post_id)
        .execute(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()}))))?;

    Ok(Json(serde_json::json!({"message": "Post deleted successfully"})))
}

async fn like_post(
    State(pool): State<SqlitePool>,
    headers: HeaderMap,
    Path(post_id): Path<i64>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let _ = extract_current_user(&pool, &headers).await?;

    let post = sqlx::query_as::<_, Post>("SELECT * FROM posts WHERE id = ?")
        .bind(post_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()}))))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, Json(serde_json::json!({"detail": "Post not found"}))))?;

    let new_count = post.like_count + 1;
    sqlx::query("UPDATE posts SET like_count = ? WHERE id = ?")
        .bind(new_count)
        .bind(post_id)
        .execute(&pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()}))))?;

    Ok(Json(serde_json::json!({"message": "Post liked", "like_count": new_count})))
}
