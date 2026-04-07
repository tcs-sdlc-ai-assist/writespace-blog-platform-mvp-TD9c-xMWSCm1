use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ─── Database Models ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub display_name: String,
    pub username: String,
    pub password_hash: String,
    pub role: String,
    pub is_default_admin: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Post {
    pub id: Uuid,
    pub title: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub author_id: Uuid,
}

// ─── Auth Request/Response DTOs ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub display_name: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: AuthUserInfo,
}

#[derive(Debug, Serialize)]
pub struct AuthUserInfo {
    pub id: Uuid,
    pub display_name: String,
    pub username: String,
    pub role: String,
}

// ─── Post Request/Response DTOs ────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreatePostRequest {
    pub title: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePostRequest {
    pub title: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct PostResponse {
    pub id: Uuid,
    pub title: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub author: PostAuthorInfo,
    pub can_edit: bool,
    pub can_delete: bool,
}

#[derive(Debug, Serialize)]
pub struct PostAuthorInfo {
    pub id: Uuid,
    pub display_name: String,
    pub role: String,
}

// ─── Admin Request/Response DTOs ───────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub display_name: String,
    pub username: String,
    pub role: String,
    pub is_default_admin: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub display_name: String,
    pub username: String,
    pub password: String,
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct AdminStatsResponse {
    pub total_posts: i64,
    pub total_users: i64,
    pub total_admins: i64,
    pub recent_posts: Vec<RecentPostInfo>,
}

#[derive(Debug, Serialize)]
pub struct RecentPostInfo {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
}

// ─── Error Response ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

// ─── JWT Claims ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserClaims {
    pub sub: Uuid,
    pub username: String,
    pub role: String,
    pub exp: usize,
}

// ─── Conversion Helpers ────────────────────────────────────────────────────────

impl From<&User> for AuthUserInfo {
    fn from(user: &User) -> Self {
        AuthUserInfo {
            id: user.id,
            display_name: user.display_name.clone(),
            username: user.username.clone(),
            role: user.role.clone(),
        }
    }
}

impl From<&User> for PostAuthorInfo {
    fn from(user: &User) -> Self {
        PostAuthorInfo {
            id: user.id,
            display_name: user.display_name.clone(),
            role: user.role.clone(),
        }
    }
}

impl From<&User> for UserResponse {
    fn from(user: &User) -> Self {
        UserResponse {
            id: user.id,
            display_name: user.display_name.clone(),
            username: user.username.clone(),
            role: user.role.clone(),
            is_default_admin: user.is_default_admin,
            created_at: user.created_at,
        }
    }
}