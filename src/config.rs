use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub default_admin_username: String,
    pub default_admin_password: String,
    pub rust_log: String,
}

impl Config {
    pub fn from_env() -> Self {
        let database_url = env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set");

        let jwt_secret = env::var("JWT_SECRET")
            .expect("JWT_SECRET must be set");

        let default_admin_username = env::var("DEFAULT_ADMIN_USERNAME")
            .unwrap_or_else(|_| "admin".to_string());

        let default_admin_password = env::var("DEFAULT_ADMIN_PASSWORD")
            .unwrap_or_else(|_| "change-me-in-production".to_string());

        let rust_log = env::var("RUST_LOG")
            .unwrap_or_else(|_| "writespace=info,tower_http=debug".to_string());

        Self {
            database_url,
            jwt_secret,
            default_admin_username,
            default_admin_password,
            rust_log,
        }
    }
}