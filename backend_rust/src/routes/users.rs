use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use sqlx::SqlitePool;

use crate::models::{User, UserResponse};

pub fn users_routes() -> Router<SqlitePool> {
    Router::new()
        .route("/", get(list_users))
        .route("/{user_id}", get(get_user))
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
