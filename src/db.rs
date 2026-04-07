use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use crate::config::Config;
use crate::errors::AppError;

pub async fn create_pool(database_url: &str) -> Result<PgPool, AppError> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .idle_timeout(std::time::Duration::from_secs(300))
        .connect(database_url)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create database pool: {:?}", e);
            AppError::InternalServerError(format!("Failed to connect to database: {}", e))
        })?;

    tracing::info!("Database connection pool created successfully");
    Ok(pool)
}

pub async fn run_migrations(pool: &PgPool) -> Result<(), AppError> {
    let migration_sql = include_str!("../migrations/001_initial.sql");

    sqlx::query(migration_sql)
        .execute(pool)
        .await
        .map_err(|e| {
            let err_str = format!("{}", e);
            if err_str.contains("already exists") || err_str.contains("42710") || err_str.contains("42P07") {
                tracing::info!("Migrations already applied, skipping");
                return AppError::InternalServerError("".to_string());
            }
            tracing::error!("Failed to run migrations: {:?}", e);
            AppError::InternalServerError(format!("Failed to run migrations: {}", e))
        })
        .ok();

    tracing::info!("Database migrations completed");
    Ok(())
}

pub async fn seed_admin(pool: &PgPool, config: &Config) -> Result<(), AppError> {
    let existing_admin: Option<(uuid::Uuid,)> = sqlx::query_as(
        "SELECT id FROM users WHERE username = $1"
    )
    .bind(&config.default_admin_username)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to check for existing admin: {:?}", e);
        AppError::InternalServerError(format!("Failed to check for existing admin: {}", e))
    })?;

    if existing_admin.is_some() {
        tracing::info!("Default admin user already exists, skipping seed");
        return Ok(());
    }

    let password_hash = bcrypt::hash(&config.default_admin_password, 12)
        .map_err(|e| {
            tracing::error!("Failed to hash admin password: {:?}", e);
            AppError::InternalServerError(format!("Failed to hash admin password: {}", e))
        })?;

    sqlx::query(
        "INSERT INTO users (display_name, username, password_hash, role, is_default_admin) VALUES ($1, $2, $3, 'admin', TRUE)"
    )
    .bind(&config.default_admin_username)
    .bind(&config.default_admin_username)
    .bind(&password_hash)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to seed default admin user: {:?}", e);
        AppError::InternalServerError(format!("Failed to seed default admin user: {}", e))
    })?;

    tracing::info!("Default admin user '{}' created successfully", config.default_admin_username);
    Ok(())
}