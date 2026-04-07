use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use sqlx::PgPool;
use uuid::Uuid;

use crate::errors::AppError;
use crate::middleware::auth::{AuthUser, OptionalAuthUser};
use crate::models::{
    CreatePostRequest, PostAuthorInfo, PostResponse, UpdatePostRequest,
};

pub async fn list_posts(
    optional_auth: OptionalAuthUser,
    State(pool): State<PgPool>,
) -> Result<Json<Vec<PostResponse>>, AppError> {
    let rows: Vec<PostRow> = match &optional_auth.0 {
        Some(_) => {
            sqlx::query_as::<_, PostRow>(
                "SELECT p.id, p.title, p.content, p.created_at, p.author_id, \
                 u.display_name as author_display_name, u.role as author_role \
                 FROM posts p \
                 JOIN users u ON p.author_id = u.id \
                 ORDER BY p.created_at DESC"
            )
            .fetch_all(&pool)
            .await?
        }
        None => {
            sqlx::query_as::<_, PostRow>(
                "SELECT p.id, p.title, p.content, p.created_at, p.author_id, \
                 u.display_name as author_display_name, u.role as author_role \
                 FROM posts p \
                 JOIN users u ON p.author_id = u.id \
                 ORDER BY p.created_at DESC \
                 LIMIT 3"
            )
            .fetch_all(&pool)
            .await?
        }
    };

    let posts: Vec<PostResponse> = rows
        .into_iter()
        .map(|row| {
            let (can_edit, can_delete) = compute_permissions(&optional_auth.0, row.author_id);
            PostResponse {
                id: row.id,
                title: row.title,
                content: row.content,
                created_at: row.created_at,
                author: PostAuthorInfo {
                    id: row.author_id,
                    display_name: row.author_display_name,
                    role: row.author_role,
                },
                can_edit,
                can_delete,
            }
        })
        .collect();

    Ok(Json(posts))
}

pub async fn get_post(
    optional_auth: OptionalAuthUser,
    Path(id): Path<Uuid>,
    State(pool): State<PgPool>,
) -> Result<Json<PostResponse>, AppError> {
    let row = sqlx::query_as::<_, PostRow>(
        "SELECT p.id, p.title, p.content, p.created_at, p.author_id, \
         u.display_name as author_display_name, u.role as author_role \
         FROM posts p \
         JOIN users u ON p.author_id = u.id \
         WHERE p.id = $1"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Post not found".to_string()))?;

    let (can_edit, can_delete) = compute_permissions(&optional_auth.0, row.author_id);

    let response = PostResponse {
        id: row.id,
        title: row.title,
        content: row.content,
        created_at: row.created_at,
        author: PostAuthorInfo {
            id: row.author_id,
            display_name: row.author_display_name,
            role: row.author_role,
        },
        can_edit,
        can_delete,
    };

    Ok(Json(response))
}

pub async fn create_post(
    auth_user: AuthUser,
    State(pool): State<PgPool>,
    Json(payload): Json<CreatePostRequest>,
) -> Result<(StatusCode, Json<PostResponse>), AppError> {
    let title = payload.title.trim().to_string();
    let content = payload.content.trim().to_string();

    if title.is_empty() || content.is_empty() {
        return Err(AppError::BadRequest("Title and content required".to_string()));
    }

    if title.len() > 200 {
        return Err(AppError::BadRequest("Title must be 200 characters or less".to_string()));
    }

    let claims = &auth_user.0;

    let row = sqlx::query_as::<_, PostRow>(
        "WITH inserted AS ( \
            INSERT INTO posts (title, content, author_id) \
            VALUES ($1, $2, $3) \
            RETURNING id, title, content, created_at, author_id \
         ) \
         SELECT i.id, i.title, i.content, i.created_at, i.author_id, \
                u.display_name as author_display_name, u.role as author_role \
         FROM inserted i \
         JOIN users u ON i.author_id = u.id"
    )
    .bind(&title)
    .bind(&content)
    .bind(claims.sub)
    .fetch_one(&pool)
    .await?;

    let response = PostResponse {
        id: row.id,
        title: row.title,
        content: row.content,
        created_at: row.created_at,
        author: PostAuthorInfo {
            id: row.author_id,
            display_name: row.author_display_name,
            role: row.author_role,
        },
        can_edit: true,
        can_delete: true,
    };

    Ok((StatusCode::CREATED, Json(response)))
}

pub async fn update_post(
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
    State(pool): State<PgPool>,
    Json(payload): Json<UpdatePostRequest>,
) -> Result<Json<PostResponse>, AppError> {
    let title = payload.title.trim().to_string();
    let content = payload.content.trim().to_string();

    if title.is_empty() || content.is_empty() {
        return Err(AppError::BadRequest("Title and content required".to_string()));
    }

    if title.len() > 200 {
        return Err(AppError::BadRequest("Title must be 200 characters or less".to_string()));
    }

    let claims = &auth_user.0;

    let existing = sqlx::query_as::<_, PostRow>(
        "SELECT p.id, p.title, p.content, p.created_at, p.author_id, \
         u.display_name as author_display_name, u.role as author_role \
         FROM posts p \
         JOIN users u ON p.author_id = u.id \
         WHERE p.id = $1"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Post not found".to_string()))?;

    if claims.role != "admin" && existing.author_id != claims.sub {
        return Err(AppError::Forbidden("Forbidden".to_string()));
    }

    let row = sqlx::query_as::<_, PostRow>(
        "WITH updated AS ( \
            UPDATE posts SET title = $1, content = $2 WHERE id = $3 \
            RETURNING id, title, content, created_at, author_id \
         ) \
         SELECT u2.id, u2.title, u2.content, u2.created_at, u2.author_id, \
                u.display_name as author_display_name, u.role as author_role \
         FROM updated u2 \
         JOIN users u ON u2.author_id = u.id"
    )
    .bind(&title)
    .bind(&content)
    .bind(id)
    .fetch_one(&pool)
    .await?;

    let response = PostResponse {
        id: row.id,
        title: row.title,
        content: row.content,
        created_at: row.created_at,
        author: PostAuthorInfo {
            id: row.author_id,
            display_name: row.author_display_name,
            role: row.author_role,
        },
        can_edit: true,
        can_delete: true,
    };

    Ok(Json(response))
}

pub async fn delete_post(
    auth_user: AuthUser,
    Path(id): Path<Uuid>,
    State(pool): State<PgPool>,
) -> Result<StatusCode, AppError> {
    let claims = &auth_user.0;

    let existing: Option<(Uuid,)> = sqlx::query_as(
        "SELECT author_id FROM posts WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await?;

    let (author_id,) = existing
        .ok_or_else(|| AppError::NotFound("Post not found".to_string()))?;

    if claims.role != "admin" && author_id != claims.sub {
        return Err(AppError::Forbidden("Forbidden".to_string()));
    }

    sqlx::query("DELETE FROM posts WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

// ─── Internal Helpers ──────────────────────────────────────────────────────────

#[derive(Debug, sqlx::FromRow)]
struct PostRow {
    pub id: Uuid,
    pub title: String,
    pub content: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub author_id: Uuid,
    pub author_display_name: String,
    pub author_role: String,
}

fn compute_permissions(
    claims: &Option<crate::models::UserClaims>,
    author_id: Uuid,
) -> (bool, bool) {
    match claims {
        Some(user) => {
            let is_owner = user.sub == author_id;
            let is_admin = user.role == "admin";
            let can_edit = is_owner || is_admin;
            let can_delete = is_owner || is_admin;
            (can_edit, can_delete)
        }
        None => (false, false),
    }
}