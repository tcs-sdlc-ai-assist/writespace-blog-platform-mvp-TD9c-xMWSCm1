use axum::{extract::State, http::StatusCode, Json};
use sqlx::PgPool;

use crate::errors::AppError;
use crate::middleware::auth::JwtService;
use crate::models::{
    AuthResponse, AuthUserInfo, LoginRequest, RegisterRequest, User,
};

pub async fn login(
    State(pool): State<PgPool>,
    State(jwt_service): State<JwtService>,
    Json(payload): Json<LoginRequest>,
) -> Result<(StatusCode, Json<AuthResponse>), AppError> {
    if payload.username.is_empty() || payload.password.is_empty() {
        return Err(AppError::BadRequest(
            "Username and password are required".to_string(),
        ));
    }

    let user: User = sqlx::query_as::<_, User>(
        "SELECT id, display_name, username, password_hash, role, is_default_admin, created_at FROM users WHERE username = $1",
    )
    .bind(&payload.username)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::Unauthorized("Invalid credentials".to_string()))?;

    let password_valid = bcrypt::verify(&payload.password, &user.password_hash)
        .map_err(|e| {
            tracing::error!("Bcrypt verify error: {:?}", e);
            AppError::InternalServerError("Password verification error".to_string())
        })?;

    if !password_valid {
        return Err(AppError::Unauthorized("Invalid credentials".to_string()));
    }

    let token = jwt_service.sign(&user)?;

    let response = AuthResponse {
        token,
        user: AuthUserInfo::from(&user),
    };

    Ok((StatusCode::OK, Json(response)))
}

pub async fn register(
    State(pool): State<PgPool>,
    State(jwt_service): State<JwtService>,
    Json(payload): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<AuthResponse>), AppError> {
    if payload.username.is_empty() {
        return Err(AppError::BadRequest("Username is required".to_string()));
    }

    if payload.username.len() < 3 || payload.username.len() > 50 {
        return Err(AppError::BadRequest(
            "Username must be between 3 and 50 characters".to_string(),
        ));
    }

    if payload.password.len() < 8 {
        return Err(AppError::BadRequest(
            "Password must be at least 8 characters".to_string(),
        ));
    }

    if payload.display_name.is_empty() {
        return Err(AppError::BadRequest(
            "Display name is required".to_string(),
        ));
    }

    if payload.display_name.len() > 100 {
        return Err(AppError::BadRequest(
            "Display name must be at most 100 characters".to_string(),
        ));
    }

    let password_hash = bcrypt::hash(&payload.password, 12)?;

    let user: User = sqlx::query_as::<_, User>(
        "INSERT INTO users (display_name, username, password_hash, role, is_default_admin) VALUES ($1, $2, $3, 'user', FALSE) RETURNING id, display_name, username, password_hash, role, is_default_admin, created_at",
    )
    .bind(&payload.display_name)
    .bind(&payload.username)
    .bind(&password_hash)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        match &e {
            sqlx::Error::Database(db_err) => {
                if let Some(code) = db_err.code() {
                    if code.as_ref() == "23505" {
                        return AppError::Conflict("Username already exists".to_string());
                    }
                }
                tracing::error!("Database error during registration: {:?}", e);
                AppError::InternalServerError("Database error".to_string())
            }
            _ => {
                tracing::error!("Database error during registration: {:?}", e);
                AppError::InternalServerError("Database error".to_string())
            }
        }
    })?;

    let token = jwt_service.sign(&user)?;

    let response = AuthResponse {
        token,
        user: AuthUserInfo::from(&user),
    };

    Ok((StatusCode::CREATED, Json(response)))
}