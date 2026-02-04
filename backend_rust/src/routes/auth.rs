use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::Utc;
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::models::{User, UserResponse, CreateUser, LoginUser, TokenResponse};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

pub fn auth_routes() -> Router<SqlitePool> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/me", get(get_me))
}

async fn register(
    State(pool): State<SqlitePool>,
    Json(input): Json<CreateUser>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    // Check if user exists
    let existing = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE username = ? OR email = ?"
    )
    .bind(&input.username)
    .bind(&input.email)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()})))
    })?;

    if existing.is_some() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"detail": "Username or email already registered"})),
        ));
    }

    // Hash password
    let hashed = hash(&input.password, DEFAULT_COST).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()})))
    })?;

    let display_name = input.display_name.unwrap_or_else(|| input.username.clone());
    let now = Utc::now();

    // Insert user
    let result = sqlx::query(
        r#"INSERT INTO users (username, email, hashed_password, display_name, created_at) 
           VALUES (?, ?, ?, ?, ?)"#
    )
    .bind(&input.username)
    .bind(&input.email)
    .bind(&hashed)
    .bind(&display_name)
    .bind(now)
    .execute(&pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()})))
    })?;

    let user_id = result.last_insert_rowid();

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()})))
        })?;

    Ok((StatusCode::CREATED, Json(UserResponse::from(user))))
}

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

async fn login(
    State(pool): State<SqlitePool>,
    axum::Form(input): axum::Form<LoginForm>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
        .bind(&input.username)
        .fetch_optional(&pool)
        .await
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()})))
        })?;

    let user = user.ok_or_else(|| {
        (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"detail": "Incorrect username or password"})))
    })?;

    let valid = verify(&input.password, &user.hashed_password).map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()})))
    })?;

    if !valid {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"detail": "Incorrect username or password"})),
        ));
    }

    // Generate JWT
    let secret = std::env::var("SECRET_KEY").unwrap_or_else(|_| "your-secret-key".to_string());
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::minutes(30))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        sub: user.username.clone(),
        exp: expiration,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()})))
    })?;

    Ok(Json(TokenResponse {
        access_token: token,
        token_type: "bearer".to_string(),
    }))
}

use axum::http::header::AUTHORIZATION;
use axum::http::HeaderMap;

async fn get_me(
    State(pool): State<SqlitePool>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let auth_header = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"detail": "Missing authorization header"})))
        })?;

    let token = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
        (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"detail": "Invalid authorization header"})))
    })?;

    let secret = std::env::var("SECRET_KEY").unwrap_or_else(|_| "your-secret-key".to_string());
    
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| {
        (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"detail": "Invalid token"})))
    })?;

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
        .bind(&token_data.claims.sub)
        .fetch_optional(&pool)
        .await
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()})))
        })?
        .ok_or_else(|| {
            (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"detail": "User not found"})))
        })?;

    Ok(Json(UserResponse::from(user)))
}

pub async fn extract_current_user(
    pool: &SqlitePool,
    headers: &HeaderMap,
) -> Result<User, (StatusCode, Json<serde_json::Value>)> {
    let auth_header = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"detail": "Missing authorization header"})))
        })?;

    let token = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
        (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"detail": "Invalid authorization header"})))
    })?;

    let secret = std::env::var("SECRET_KEY").unwrap_or_else(|_| "your-secret-key".to_string());
    
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| {
        (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"detail": "Invalid token"})))
    })?;

    sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
        .bind(&token_data.claims.sub)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()})))
        })?
        .ok_or_else(|| {
            (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"detail": "User not found"})))
        })
}
