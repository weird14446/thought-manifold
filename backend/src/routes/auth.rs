use axum::{
    Router,
    extract::{Json, Query, State},
    http::StatusCode,
    response::{AppendHeaders, IntoResponse, Redirect},
    routing::{get, post},
};
use bcrypt::{DEFAULT_COST, hash, verify};
use chrono::Utc;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sqlx::MySqlPool;

use crate::models::{CreateUser, TokenResponse, User, UserResponse};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
}

pub fn auth_routes() -> Router<MySqlPool> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/me", get(get_me))
        .route("/google", get(google_login))
        .route("/google/callback", get(google_callback))
}

// ============================
// Standard Auth
// ============================

async fn register(
    State(pool): State<MySqlPool>,
    Json(input): Json<CreateUser>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    // Check if user exists
    let existing = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ? OR email = ?")
        .bind(&input.username)
        .bind(&input.email)
        .fetch_optional(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": e.to_string()})),
            )
        })?;

    if existing.is_some() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"detail": "Username or email already registered"})),
        ));
    }

    // Hash password
    let hashed = hash(&input.password, DEFAULT_COST).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"detail": e.to_string()})),
        )
    })?;

    let display_name = input.display_name.unwrap_or_else(|| input.username.clone());
    let now = Utc::now();

    // Insert user
    let result = sqlx::query(
        r#"INSERT INTO users (username, email, hashed_password, display_name, created_at) 
           VALUES (?, ?, ?, ?, ?)"#,
    )
    .bind(&input.username)
    .bind(&input.email)
    .bind(&hashed)
    .bind(&display_name)
    .bind(now)
    .execute(&pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"detail": e.to_string()})),
        )
    })?;

    let user_id = result.last_insert_id() as i64;

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": e.to_string()})),
            )
        })?;

    Ok((StatusCode::CREATED, Json(UserResponse::from(user))))
}

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

async fn login(
    State(pool): State<MySqlPool>,
    axum::Form(input): axum::Form<LoginForm>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
        .bind(&input.username)
        .fetch_optional(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": e.to_string()})),
            )
        })?;

    let user = user.ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"detail": "Incorrect username or password"})),
        )
    })?;

    let hashed = user.hashed_password.as_ref().ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"detail": "This account uses Google login"})),
        )
    })?;

    let valid = verify(&input.password, hashed).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"detail": e.to_string()})),
        )
    })?;

    if !valid {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"detail": "Incorrect username or password"})),
        ));
    }

    let token = generate_jwt(&user.username)?;
    Ok(Json(TokenResponse {
        access_token: token,
        token_type: "bearer".to_string(),
    }))
}

use axum::http::HeaderMap;
use axum::http::header::AUTHORIZATION;

async fn get_me(
    State(pool): State<MySqlPool>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let auth_header = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"detail": "Missing authorization header"})),
            )
        })?;

    let token = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"detail": "Invalid authorization header"})),
        )
    })?;

    let secret = std::env::var("SECRET_KEY").expect("SECRET_KEY must be set in .env");

    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"detail": "Invalid token"})),
        )
    })?;

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
        .bind(&token_data.claims.sub)
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
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"detail": "User not found"})),
            )
        })?;

    Ok(Json(UserResponse::from(user)))
}

pub async fn extract_current_user(
    pool: &MySqlPool,
    headers: &HeaderMap,
) -> Result<User, (StatusCode, Json<serde_json::Value>)> {
    let auth_header = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"detail": "Missing authorization header"})),
            )
        })?;

    let token = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"detail": "Invalid authorization header"})),
        )
    })?;

    let secret = std::env::var("SECRET_KEY").expect("SECRET_KEY must be set in .env");

    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"detail": "Invalid token"})),
        )
    })?;

    sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
        .bind(&token_data.claims.sub)
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
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"detail": "User not found"})),
            )
        })
}

pub async fn extract_optional_user(
    pool: &MySqlPool,
    headers: &HeaderMap,
) -> Result<Option<User>, (StatusCode, Json<serde_json::Value>)> {
    let Some(auth_header) = headers.get(AUTHORIZATION).and_then(|v| v.to_str().ok()) else {
        return Ok(None);
    };

    let Some(token) = auth_header.strip_prefix("Bearer ") else {
        return Ok(None);
    };

    let secret = std::env::var("SECRET_KEY").expect("SECRET_KEY must be set in .env");
    let token_data = match decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    ) {
        Ok(data) => data,
        Err(_) => return Ok(None),
    };

    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
        .bind(&token_data.claims.sub)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": e.to_string()})),
            )
        })?;

    Ok(user)
}

// ============================
// Helper: JWT Generation
// ============================

fn generate_jwt(username: &str) -> Result<String, (StatusCode, Json<serde_json::Value>)> {
    let secret = std::env::var("SECRET_KEY").expect("SECRET_KEY must be set in .env");
    let expiration = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(24))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        sub: username.to_string(),
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"detail": e.to_string()})),
        )
    })
}

// ============================
// Google OAuth 2.1
// ============================

fn generate_pkce() -> (String, String) {
    let mut rng = rand::rng();
    let code_verifier: String = (0..128)
        .map(|_| {
            let idx = rng.random_range(0..66u32);
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~"[idx as usize]
                as char
        })
        .collect();

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let hash = hasher.finalize();

    use base64::Engine;
    let code_challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash);

    (code_verifier, code_challenge)
}

fn generate_state() -> String {
    let mut rng = rand::rng();
    (0..32)
        .map(|_| {
            let idx = rng.random_range(0..16u32);
            b"0123456789abcdef"[idx as usize] as char
        })
        .collect()
}

fn extract_cookie_value(cookie_header: &str, key: &str) -> Option<String> {
    cookie_header
        .split(';')
        .find_map(|cookie| {
            let cookie = cookie.trim();
            cookie.strip_prefix(&format!("{}=", key))
        })
        .map(ToString::to_string)
}

async fn google_login() -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let client_id = std::env::var("GOOGLE_CLIENT_ID").map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"detail": "GOOGLE_CLIENT_ID not configured"})),
        )
    })?;

    let redirect_uri = std::env::var("GOOGLE_REDIRECT_URI")
        .unwrap_or_else(|_| "http://localhost:8000/api/auth/google/callback".to_string());

    let (_code_verifier, code_challenge) = generate_pkce();
    let state = generate_state();

    // In production, store code_verifier in a session/DB keyed by state.
    // For development, we use a simpler approach: pass code_verifier in state cookie.
    // This is acceptable for local development.

    let auth_url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?\
        client_id={}&\
        redirect_uri={}&\
        response_type=code&\
        scope=openid%20email%20profile&\
        code_challenge={}&\
        code_challenge_method=S256&\
        state={}&\
        access_type=offline&\
        prompt=consent",
        client_id,
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(&code_challenge),
        state,
    );

    // Set PKCE verifier/state cookies for callback validation.
    let verifier_cookie = format!(
        "oauth_verifier={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=600",
        _code_verifier
    );
    let state_cookie = format!(
        "oauth_state={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=600",
        state
    );

    Ok((
        AppendHeaders([
            (axum::http::header::SET_COOKIE, verifier_cookie),
            (axum::http::header::SET_COOKIE, state_cookie),
        ]),
        Redirect::temporary(&auth_url),
    ))
}

#[derive(Debug, Deserialize)]
struct GoogleCallbackParams {
    code: String,
    state: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleTokenResponse {
    access_token: String,
    #[allow(dead_code)]
    token_type: Option<String>,
    #[allow(dead_code)]
    expires_in: Option<i64>,
    #[allow(dead_code)]
    id_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleUserInfo {
    #[serde(rename = "sub")]
    google_id: String,
    email: String,
    name: Option<String>,
    picture: Option<String>,
}

async fn google_callback(
    State(pool): State<MySqlPool>,
    headers: HeaderMap,
    Query(params): Query<GoogleCallbackParams>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let client_id = std::env::var("GOOGLE_CLIENT_ID").map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"detail": "GOOGLE_CLIENT_ID not configured"})),
        )
    })?;
    let client_secret = std::env::var("GOOGLE_CLIENT_SECRET").map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"detail": "GOOGLE_CLIENT_SECRET not configured"})),
        )
    })?;
    let redirect_uri = std::env::var("GOOGLE_REDIRECT_URI")
        .unwrap_or_else(|_| "http://localhost:8000/api/auth/google/callback".to_string());

    // Extract code_verifier from cookie
    let cookie_header = headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let code_verifier = extract_cookie_value(cookie_header, "oauth_verifier").unwrap_or_default();
    let cookie_state = extract_cookie_value(cookie_header, "oauth_state").unwrap_or_default();

    let request_state = params.state.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"detail": "Missing OAuth state"})),
        )
    })?;

    if code_verifier.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"detail": "Missing OAuth code verifier"})),
        ));
    }

    if cookie_state.is_empty() || request_state != cookie_state {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"detail": "Invalid OAuth state"})),
        ));
    }

    // Exchange authorization code for access token
    let http_client = reqwest::Client::new();
    let token_response = http_client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("code", params.code.as_str()),
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("grant_type", "authorization_code"),
            ("code_verifier", code_verifier.as_str()),
        ])
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Failed to exchange code: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": "Failed to exchange authorization code"})),
            )
        })?;

    if !token_response.status().is_success() {
        let error_body = token_response.text().await.unwrap_or_default();
        tracing::error!("Google token error: {}", error_body);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"detail": "Failed to get Google access token"})),
        ));
    }

    let google_token: GoogleTokenResponse = token_response.json().await.map_err(|e| {
        tracing::error!("Failed to parse token response: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"detail": "Failed to parse Google token response"})),
        )
    })?;

    // Fetch user info from Google
    let userinfo_response = http_client
        .get("https://www.googleapis.com/oauth2/v3/userinfo")
        .bearer_auth(&google_token.access_token)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch userinfo: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": "Failed to fetch Google user info"})),
            )
        })?;

    let google_user: GoogleUserInfo = userinfo_response.json().await.map_err(|e| {
        tracing::error!("Failed to parse userinfo: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"detail": "Failed to parse Google user info"})),
        )
    })?;

    // Find or create user
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE google_id = ?")
        .bind(&google_user.google_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"detail": e.to_string()})),
            )
        })?;

    let user = match user {
        Some(u) => u,
        None => {
            // Check if email already exists (link accounts)
            let existing = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = ?")
                .bind(&google_user.email)
                .fetch_optional(&pool)
                .await
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"detail": e.to_string()})),
                    )
                })?;

            match existing {
                Some(existing_user) => {
                    // Link Google ID to existing account
                    sqlx::query("UPDATE users SET google_id = ?, avatar_url = COALESCE(avatar_url, ?) WHERE id = ?")
                        .bind(&google_user.google_id)
                        .bind(&google_user.picture)
                        .bind(existing_user.id)
                        .execute(&pool)
                        .await
                        .map_err(|e| {
                            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()})))
                        })?;

                    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
                        .bind(existing_user.id)
                        .fetch_one(&pool)
                        .await
                        .map_err(|e| {
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"detail": e.to_string()})),
                            )
                        })?
                }
                None => {
                    // Create new user with Google info
                    let username = google_user
                        .email
                        .split('@')
                        .next()
                        .unwrap_or("user")
                        .to_string();

                    // Ensure unique username
                    let mut final_username = username.clone();
                    let mut counter = 1u32;
                    loop {
                        let exists = sqlx::query("SELECT id FROM users WHERE username = ?")
                            .bind(&final_username)
                            .fetch_optional(&pool)
                            .await
                            .map_err(|e| {
                                (
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    Json(serde_json::json!({"detail": e.to_string()})),
                                )
                            })?;
                        if exists.is_none() {
                            break;
                        }
                        final_username = format!("{}{}", username, counter);
                        counter += 1;
                    }

                    let display_name = google_user.name.unwrap_or_else(|| final_username.clone());
                    let now = Utc::now();

                    let result = sqlx::query(
                        r#"INSERT INTO users (username, email, google_id, display_name, avatar_url, created_at) 
                           VALUES (?, ?, ?, ?, ?, ?)"#
                    )
                    .bind(&final_username)
                    .bind(&google_user.email)
                    .bind(&google_user.google_id)
                    .bind(&display_name)
                    .bind(&google_user.picture)
                    .bind(now)
                    .execute(&pool)
                    .await
                    .map_err(|e| {
                        (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"detail": e.to_string()})))
                    })?;

                    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
                        .bind(result.last_insert_id() as i64)
                        .fetch_one(&pool)
                        .await
                        .map_err(|e| {
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(serde_json::json!({"detail": e.to_string()})),
                            )
                        })?
                }
            }
        }
    };

    // Generate JWT
    let jwt_token = generate_jwt(&user.username)?;

    // Redirect to frontend with token
    let frontend_url =
        std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:5173".to_string());

    let redirect_url = format!("{}/?token={}", frontend_url, jwt_token);

    // Clear OAuth cookies after successful login.
    let clear_verifier_cookie =
        "oauth_verifier=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0".to_string();
    let clear_state_cookie = "oauth_state=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0".to_string();

    Ok((
        AppendHeaders([
            (axum::http::header::SET_COOKIE, clear_verifier_cookie),
            (axum::http::header::SET_COOKIE, clear_state_cookie),
        ]),
        Redirect::temporary(&redirect_url),
    ))
}
