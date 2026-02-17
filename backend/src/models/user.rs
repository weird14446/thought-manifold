use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub email: String,
    #[serde(skip_serializing)]
    pub hashed_password: Option<String>,
    pub google_id: Option<String>,
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub introduction: Option<String>,
    pub hobbies: Option<String>,
    pub interests: Option<String>,
    pub research_areas: Option<String>,
    pub avatar_url: Option<String>,
    pub is_admin: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserResponse {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub display_name: Option<String>,
    pub bio: Option<String>,
    pub introduction: Option<String>,
    pub hobbies: Option<String>,
    pub interests: Option<String>,
    pub research_areas: Option<String>,
    pub avatar_url: Option<String>,
    pub is_admin: bool,
    pub created_at: DateTime<Utc>,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            email: user.email,
            display_name: user.display_name,
            bio: user.bio,
            introduction: user.introduction,
            hobbies: user.hobbies,
            interests: user.interests,
            research_areas: user.research_areas,
            avatar_url: user.avatar_url,
            is_admin: user.is_admin,
            created_at: user.created_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateUser {
    pub username: String,
    pub email: String,
    pub password: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginUser {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
}
