use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::AppError;
use crate::middleware::auth::AuthUser;
use crate::models::{
    AdminStatsResponse, CreateUserRequest, RecentPostInfo, UserResponse,
};

pub async fn stats(
    AuthUser(claims): AuthUser,
    State(pool): State<PgPool>,
) -> Result<Json<AdminStatsResponse>, AppError> {
    if claims.role != "admin" {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    let total_posts: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM posts")
        .fetch_one(&pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to count posts: {:?}", e);
            AppError::InternalServerError("Failed to fetch stats".to_string())
        })?;

    let total_users: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to count users: {:?}", e);
            AppError::InternalServerError("Failed to fetch stats".to_string())
        })?;

    let total_admins: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM users WHERE role = 'admin'")
            .fetch_one(&pool)
            .await
            .map_err(|e| {
                tracing::error!("Failed to count admins: {:?}", e);
                AppError::InternalServerError("Failed to fetch stats".to_string())
            })?;

    let recent_posts: Vec<RecentPostInfo> = sqlx::query_as::<_, RecentPostInfo>(
        "SELECT id, title, created_at FROM posts ORDER BY created_at DESC LIMIT 5",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch recent posts: {:?}", e);
        AppError::InternalServerError("Failed to fetch stats".to_string())
    })?;

    Ok(Json(AdminStatsResponse {
        total_posts: total_posts.0,
        total_users: total_users.0,
        total_admins: total_admins.0,
        recent_posts,
    }))
}

pub async fn list_users(
    AuthUser(claims): AuthUser,
    State(pool): State<PgPool>,
) -> Result<Json<Vec<UserResponse>>, AppError> {
    if claims.role != "admin" {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    let users: Vec<UserResponse> = sqlx::query_as::<_, UserResponse>(
        "SELECT id, display_name, username, role, is_default_admin, created_at FROM users ORDER BY created_at ASC",
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list users: {:?}", e);
        AppError::InternalServerError("Failed to list users".to_string())
    })?;

    Ok(Json(users))
}

pub async fn create_user(
    AuthUser(claims): AuthUser,
    State(pool): State<PgPool>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<UserResponse>), AppError> {
    if claims.role != "admin" {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    let username = payload.username.trim().to_string();
    let display_name = payload.display_name.trim().to_string();
    let password = payload.password.clone();
    let role = payload.role.trim().to_lowercase();

    if username.len() < 3 || username.len() > 50 {
        return Err(AppError::BadRequest(
            "Username must be between 3 and 50 characters".to_string(),
        ));
    }

    if !username.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        return Err(AppError::BadRequest(
            "Username must contain only alphanumeric characters, underscores, or hyphens".to_string(),
        ));
    }

    if display_name.is_empty() || display_name.len() > 100 {
        return Err(AppError::BadRequest(
            "Display name must be between 1 and 100 characters".to_string(),
        ));
    }

    if password.len() < 8 {
        return Err(AppError::BadRequest(
            "Password must be at least 8 characters".to_string(),
        ));
    }

    if role != "admin" && role != "user" {
        return Err(AppError::BadRequest(
            "Role must be 'admin' or 'user'".to_string(),
        ));
    }

    let password_hash = bcrypt::hash(&password, 12).map_err(|e| {
        tracing::error!("Failed to hash password: {:?}", e);
        AppError::InternalServerError("Failed to hash password".to_string())
    })?;

    let user: UserResponse = sqlx::query_as::<_, UserResponse>(
        "INSERT INTO users (display_name, username, password_hash, role, is_default_admin) VALUES ($1, $2, $3, $4, FALSE) RETURNING id, display_name, username, role, is_default_admin, created_at",
    )
    .bind(&display_name)
    .bind(&username)
    .bind(&password_hash)
    .bind(&role)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        let err_str = format!("{}", e);
        if err_str.contains("23505") || err_str.contains("duplicate key") {
            return AppError::Conflict("Username already exists".to_string());
        }
        tracing::error!("Failed to create user: {:?}", e);
        AppError::InternalServerError("Failed to create user".to_string())
    })?;

    tracing::info!("Admin '{}' created user '{}'", claims.username, username);

    Ok((StatusCode::CREATED, Json(user)))
}

pub async fn delete_user(
    AuthUser(claims): AuthUser,
    State(pool): State<PgPool>,
    Path(user_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    if claims.role != "admin" {
        return Err(AppError::Forbidden("Admin access required".to_string()));
    }

    if user_id == claims.sub {
        return Err(AppError::Forbidden(
            "Cannot delete default admin or self".to_string(),
        ));
    }

    let target_user: Option<(Uuid, bool)> =
        sqlx::query_as("SELECT id, is_default_admin FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| {
                tracing::error!("Failed to fetch user for deletion: {:?}", e);
                AppError::InternalServerError("Failed to delete user".to_string())
            })?;

    match target_user {
        None => {
            return Err(AppError::NotFound("User not found".to_string()));
        }
        Some((_id, is_default_admin)) => {
            if is_default_admin {
                return Err(AppError::Forbidden(
                    "Cannot delete default admin or self".to_string(),
                ));
            }
        }
    }

    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete user: {:?}", e);
            AppError::InternalServerError("Failed to delete user".to_string())
        })?;

    tracing::info!(
        "Admin '{}' deleted user '{}'",
        claims.username,
        user_id
    );

    Ok(StatusCode::NO_CONTENT)
}