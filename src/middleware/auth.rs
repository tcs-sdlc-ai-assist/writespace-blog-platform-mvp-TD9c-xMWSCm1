use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde_json::json;

use crate::errors::AppError;
use crate::models::{User, UserClaims};

pub struct JwtService {
    secret: String,
}

impl JwtService {
    pub fn new(secret: String) -> Self {
        Self { secret }
    }

    pub fn sign(&self, user: &User) -> Result<String, AppError> {
        let expiration = chrono::Utc::now()
            .checked_add_signed(chrono::Duration::hours(24))
            .expect("valid timestamp")
            .timestamp() as usize;

        let claims = UserClaims {
            sub: user.id,
            username: user.username.clone(),
            role: user.role.clone(),
            exp: expiration,
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret.as_bytes()),
        )?;

        Ok(token)
    }

    pub fn verify(&self, token: &str) -> Result<UserClaims, AppError> {
        let token_data = decode::<UserClaims>(
            token,
            &DecodingKey::from_secret(self.secret.as_bytes()),
            &Validation::default(),
        )?;

        Ok(token_data.claims)
    }
}

#[derive(Debug, Clone)]
pub struct AuthUser(pub UserClaims);

#[derive(Debug, Clone)]
pub struct OptionalAuthUser(pub Option<UserClaims>);

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let jwt_secret = parts
            .extensions
            .get::<JwtService>()
            .ok_or_else(|| AppError::InternalServerError("JWT service not configured".to_string()))?;

        let auth_header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| AppError::Unauthorized("Missing authorization header".to_string()))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| AppError::Unauthorized("Invalid authorization header format".to_string()))?;

        let claims = jwt_secret.verify(token)?;

        Ok(AuthUser(claims))
    }
}

impl<S> FromRequestParts<S> for OptionalAuthUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let jwt_service = match parts.extensions.get::<JwtService>() {
            Some(service) => service,
            None => return Ok(OptionalAuthUser(None)),
        };

        let auth_header = match parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
        {
            Some(header) => header,
            None => return Ok(OptionalAuthUser(None)),
        };

        let token = match auth_header.strip_prefix("Bearer ") {
            Some(t) => t,
            None => return Ok(OptionalAuthUser(None)),
        };

        match jwt_service.verify(token) {
            Ok(claims) => Ok(OptionalAuthUser(Some(claims))),
            Err(_) => Ok(OptionalAuthUser(None)),
        }
    }
}

pub async fn require_auth(
    auth_user: AuthUser,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let _user = auth_user.0;
    next.run(request).await
}

pub async fn require_admin(
    auth_user: AuthUser,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    if auth_user.0.role != "admin" {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Forbidden: admin access required"})),
        )
            .into_response();
    }
    next.run(request).await
}