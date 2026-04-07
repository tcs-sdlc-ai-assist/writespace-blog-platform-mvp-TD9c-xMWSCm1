use axum::{
    body::Body,
    extract::FromRef,
    http::{header, Request, StatusCode},
    routing::{delete, get, post, put},
    Router,
};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;

use writespace_rust::handlers;
use writespace_rust::middleware::auth::JwtService;

// ---------------------------------------------------------------------------
// AppState (mirrors api/main.rs)
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct AppState {
    pool: PgPool,
    jwt_service: JwtService,
}

impl FromRef<AppState> for PgPool {
    fn from_ref(state: &AppState) -> Self {
        state.pool.clone()
    }
}

impl FromRef<AppState> for JwtService {
    fn from_ref(state: &AppState) -> Self {
        state.jwt_service.clone()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_app(pool: PgPool) -> Router {
    let jwt_service = JwtService::new("test-secret-key-for-integration-tests".to_string());
    let jwt_secret = "test-secret-key-for-integration-tests".to_string();

    let app_state = AppState {
        pool,
        jwt_service,
    };

    Router::new()
        .route("/api/auth/login", post(handlers::login))
        .route("/api/auth/register", post(handlers::register))
        .route(
            "/api/posts",
            get(handlers::list_posts).post(handlers::create_post),
        )
        .route(
            "/api/posts/{id}",
            get(handlers::get_post)
                .put(handlers::update_post)
                .delete(handlers::delete_post),
        )
        .route("/api/admin/stats", get(handlers::stats))
        .route(
            "/api/admin/users",
            get(handlers::list_users).post(handlers::create_user),
        )
        .route("/api/admin/users/{id}", delete(handlers::delete_user))
        .layer(axum::middleware::from_fn(
            move |mut req: axum::extract::Request, next: axum::middleware::Next| {
                let secret = jwt_secret.clone();
                async move {
                    req.extensions_mut().insert(JwtService::new(secret));
                    next.run(req).await
                }
            },
        ))
        .with_state(app_state)
}

async fn setup_db(pool: &PgPool) {
    let migration_sql = include_str!("../migrations/001_initial.sql");
    sqlx::query(migration_sql)
        .execute(pool)
        .await
        .ok();
}

async fn cleanup_db(pool: &PgPool) {
    sqlx::query("DELETE FROM posts")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM users")
        .execute(pool)
        .await
        .ok();
}

async fn create_admin_user(pool: &PgPool) -> (String, String) {
    let username = format!("admin_{}", Uuid::new_v4().to_string().get(..8).unwrap());
    let password = "adminpassword123";
    let password_hash = bcrypt::hash(password, 4).unwrap();

    sqlx::query(
        "INSERT INTO users (display_name, username, password_hash, role, is_default_admin) VALUES ($1, $2, $3, 'admin', TRUE)",
    )
    .bind(&username)
    .bind(&username)
    .bind(&password_hash)
    .execute(pool)
    .await
    .unwrap();

    (username, password.to_string())
}

async fn create_regular_user(pool: &PgPool) -> (String, String) {
    let username = format!("user_{}", Uuid::new_v4().to_string().get(..8).unwrap());
    let password = "userpassword123";
    let password_hash = bcrypt::hash(password, 4).unwrap();

    sqlx::query(
        "INSERT INTO users (display_name, username, password_hash, role, is_default_admin) VALUES ($1, $2, $3, 'user', FALSE)",
    )
    .bind(&username)
    .bind(&username)
    .bind(&password_hash)
    .execute(pool)
    .await
    .unwrap();

    (username, password.to_string())
}

async fn login_user(app: &Router, username: &str, password: &str) -> String {
    let body = json!({
        "username": username,
        "password": password,
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let data: Value = serde_json::from_slice(&bytes).unwrap();
    data["token"].as_str().unwrap().to_string()
}

async fn response_json(resp: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

fn get_db_url() -> String {
    std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests")
}

// ---------------------------------------------------------------------------
// Auth Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_login_success() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let (username, password) = create_admin_user(&pool).await;
    let app = build_app(pool.clone());

    let body = json!({
        "username": username,
        "password": password,
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let data = response_json(resp).await;
    assert!(data["token"].is_string());
    assert_eq!(data["user"]["username"].as_str().unwrap(), username);
    assert_eq!(data["user"]["role"].as_str().unwrap(), "admin");

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_login_invalid_credentials() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let (username, _password) = create_admin_user(&pool).await;
    let app = build_app(pool.clone());

    let body = json!({
        "username": username,
        "password": "wrongpassword",
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let data = response_json(resp).await;
    assert!(data["error"].is_string());

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_login_nonexistent_user() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let app = build_app(pool.clone());

    let body = json!({
        "username": "nonexistent_user",
        "password": "somepassword",
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_login_empty_fields() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let app = build_app(pool.clone());

    let body = json!({
        "username": "",
        "password": "",
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_register_success() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let app = build_app(pool.clone());
    let username = format!("newuser_{}", Uuid::new_v4().to_string().get(..8).unwrap());

    let body = json!({
        "display_name": "New User",
        "username": username,
        "password": "securepassword123",
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/register")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let data = response_json(resp).await;
    assert!(data["token"].is_string());
    assert_eq!(data["user"]["username"].as_str().unwrap(), username);
    assert_eq!(data["user"]["role"].as_str().unwrap(), "user");
    assert_eq!(data["user"]["display_name"].as_str().unwrap(), "New User");

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_register_duplicate_username() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let (username, _password) = create_regular_user(&pool).await;
    let app = build_app(pool.clone());

    let body = json!({
        "display_name": "Duplicate User",
        "username": username,
        "password": "securepassword123",
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/register")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);

    let data = response_json(resp).await;
    assert!(data["error"].as_str().unwrap().contains("already exists"));

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_register_short_username() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let app = build_app(pool.clone());

    let body = json!({
        "display_name": "Short",
        "username": "ab",
        "password": "securepassword123",
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/register")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_register_short_password() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let app = build_app(pool.clone());

    let body = json!({
        "display_name": "Test User",
        "username": "testuser123",
        "password": "short",
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/auth/register")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    cleanup_db(&pool).await;
}

// ---------------------------------------------------------------------------
// Post Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_list_posts_public_limited() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let (username, password) = create_regular_user(&pool).await;
    let app = build_app(pool.clone());
    let token = login_user(&app, &username, &password).await;

    for i in 0..5 {
        let body = json!({
            "title": format!("Post {}", i),
            "content": format!("Content for post {}", i),
        });

        let req = Request::builder()
            .method("POST")
            .uri("/api/posts")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    // Public (no auth) should get at most 3
    let req = Request::builder()
        .method("GET")
        .uri("/api/posts")
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let data = response_json(resp).await;
    let posts = data.as_array().unwrap();
    assert_eq!(posts.len(), 3);

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_list_posts_authenticated_all() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let (username, password) = create_regular_user(&pool).await;
    let app = build_app(pool.clone());
    let token = login_user(&app, &username, &password).await;

    for i in 0..5 {
        let body = json!({
            "title": format!("Post {}", i),
            "content": format!("Content for post {}", i),
        });

        let req = Request::builder()
            .method("POST")
            .uri("/api/posts")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, format!("Bearer {}", token))
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    // Authenticated should get all 5
    let req = Request::builder()
        .method("GET")
        .uri("/api/posts")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let data = response_json(resp).await;
    let posts = data.as_array().unwrap();
    assert_eq!(posts.len(), 5);

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_create_post_success() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let (username, password) = create_regular_user(&pool).await;
    let app = build_app(pool.clone());
    let token = login_user(&app, &username, &password).await;

    let body = json!({
        "title": "My First Post",
        "content": "This is the content of my first post.",
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/posts")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let data = response_json(resp).await;
    assert_eq!(data["title"].as_str().unwrap(), "My First Post");
    assert_eq!(
        data["content"].as_str().unwrap(),
        "This is the content of my first post."
    );
    assert!(data["id"].is_string());
    assert!(data["created_at"].is_string());
    assert_eq!(data["author"]["display_name"].as_str().unwrap(), username);
    assert!(data["can_edit"].as_bool().unwrap());
    assert!(data["can_delete"].as_bool().unwrap());

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_create_post_unauthenticated() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let app = build_app(pool.clone());

    let body = json!({
        "title": "Unauthorized Post",
        "content": "Should fail.",
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/posts")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_create_post_empty_title() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let (username, password) = create_regular_user(&pool).await;
    let app = build_app(pool.clone());
    let token = login_user(&app, &username, &password).await;

    let body = json!({
        "title": "",
        "content": "Content without title.",
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/posts")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_get_post_success() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let (username, password) = create_regular_user(&pool).await;
    let app = build_app(pool.clone());
    let token = login_user(&app, &username, &password).await;

    let body = json!({
        "title": "Fetchable Post",
        "content": "Content to fetch.",
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/posts")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let created = response_json(resp).await;
    let post_id = created["id"].as_str().unwrap();

    // Fetch the post
    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/posts/{}", post_id))
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let data = response_json(resp).await;
    assert_eq!(data["id"].as_str().unwrap(), post_id);
    assert_eq!(data["title"].as_str().unwrap(), "Fetchable Post");

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_get_post_not_found() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let app = build_app(pool.clone());
    let fake_id = Uuid::new_v4();

    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/posts/{}", fake_id))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_update_post_by_owner() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let (username, password) = create_regular_user(&pool).await;
    let app = build_app(pool.clone());
    let token = login_user(&app, &username, &password).await;

    // Create post
    let body = json!({
        "title": "Original Title",
        "content": "Original content.",
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/posts")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    let created = response_json(resp).await;
    let post_id = created["id"].as_str().unwrap();

    // Update post
    let update_body = json!({
        "title": "Updated Title",
        "content": "Updated content.",
    });

    let req = Request::builder()
        .method("PUT")
        .uri(format!("/api/posts/{}", post_id))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(serde_json::to_vec(&update_body).unwrap()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let data = response_json(resp).await;
    assert_eq!(data["title"].as_str().unwrap(), "Updated Title");
    assert_eq!(data["content"].as_str().unwrap(), "Updated content.");

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_update_post_by_admin() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let (user_name, user_pass) = create_regular_user(&pool).await;
    let (admin_name, admin_pass) = create_admin_user(&pool).await;
    let app = build_app(pool.clone());

    let user_token = login_user(&app, &user_name, &user_pass).await;
    let admin_token = login_user(&app, &admin_name, &admin_pass).await;

    // User creates post
    let body = json!({
        "title": "User Post",
        "content": "User content.",
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/posts")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", user_token))
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    let created = response_json(resp).await;
    let post_id = created["id"].as_str().unwrap();

    // Admin updates user's post
    let update_body = json!({
        "title": "Admin Updated Title",
        "content": "Admin updated content.",
    });

    let req = Request::builder()
        .method("PUT")
        .uri(format!("/api/posts/{}", post_id))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token))
        .body(Body::from(serde_json::to_vec(&update_body).unwrap()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let data = response_json(resp).await;
    assert_eq!(data["title"].as_str().unwrap(), "Admin Updated Title");

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_update_post_forbidden_other_user() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let (user1_name, user1_pass) = create_regular_user(&pool).await;
    let (user2_name, user2_pass) = create_regular_user(&pool).await;
    let app = build_app(pool.clone());

    let user1_token = login_user(&app, &user1_name, &user1_pass).await;
    let user2_token = login_user(&app, &user2_name, &user2_pass).await;

    // User1 creates post
    let body = json!({
        "title": "User1 Post",
        "content": "User1 content.",
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/posts")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", user1_token))
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    let created = response_json(resp).await;
    let post_id = created["id"].as_str().unwrap();

    // User2 tries to update user1's post
    let update_body = json!({
        "title": "Hijacked Title",
        "content": "Hijacked content.",
    });

    let req = Request::builder()
        .method("PUT")
        .uri(format!("/api/posts/{}", post_id))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", user2_token))
        .body(Body::from(serde_json::to_vec(&update_body).unwrap()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_delete_post_by_owner() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let (username, password) = create_regular_user(&pool).await;
    let app = build_app(pool.clone());
    let token = login_user(&app, &username, &password).await;

    // Create post
    let body = json!({
        "title": "Deletable Post",
        "content": "Will be deleted.",
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/posts")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    let created = response_json(resp).await;
    let post_id = created["id"].as_str().unwrap();

    // Delete post
    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/api/posts/{}", post_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Verify it's gone
    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/posts/{}", post_id))
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_delete_post_forbidden_other_user() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let (user1_name, user1_pass) = create_regular_user(&pool).await;
    let (user2_name, user2_pass) = create_regular_user(&pool).await;
    let app = build_app(pool.clone());

    let user1_token = login_user(&app, &user1_name, &user1_pass).await;
    let user2_token = login_user(&app, &user2_name, &user2_pass).await;

    // User1 creates post
    let body = json!({
        "title": "User1 Post",
        "content": "User1 content.",
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/posts")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", user1_token))
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    let created = response_json(resp).await;
    let post_id = created["id"].as_str().unwrap();

    // User2 tries to delete user1's post
    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/api/posts/{}", post_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", user2_token))
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_delete_post_by_admin() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let (user_name, user_pass) = create_regular_user(&pool).await;
    let (admin_name, admin_pass) = create_admin_user(&pool).await;
    let app = build_app(pool.clone());

    let user_token = login_user(&app, &user_name, &user_pass).await;
    let admin_token = login_user(&app, &admin_name, &admin_pass).await;

    // User creates post
    let body = json!({
        "title": "User Post To Delete",
        "content": "Admin will delete this.",
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/posts")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", user_token))
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    let created = response_json(resp).await;
    let post_id = created["id"].as_str().unwrap();

    // Admin deletes user's post
    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/api/posts/{}", post_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token))
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    cleanup_db(&pool).await;
}

// ---------------------------------------------------------------------------
// Admin Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_admin_stats() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let (admin_name, admin_pass) = create_admin_user(&pool).await;
    let app = build_app(pool.clone());
    let admin_token = login_user(&app, &admin_name, &admin_pass).await;

    // Create a post
    let body = json!({
        "title": "Stats Post",
        "content": "Content for stats.",
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/posts")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token))
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Get stats
    let req = Request::builder()
        .method("GET")
        .uri("/api/admin/stats")
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token))
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let data = response_json(resp).await;
    assert!(data["total_posts"].as_i64().unwrap() >= 1);
    assert!(data["total_users"].as_i64().unwrap() >= 1);
    assert!(data["total_admins"].as_i64().unwrap() >= 1);
    assert!(data["recent_posts"].is_array());

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_admin_stats_forbidden_for_user() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let (user_name, user_pass) = create_regular_user(&pool).await;
    let app = build_app(pool.clone());
    let user_token = login_user(&app, &user_name, &user_pass).await;

    let req = Request::builder()
        .method("GET")
        .uri("/api/admin/stats")
        .header(header::AUTHORIZATION, format!("Bearer {}", user_token))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_admin_stats_unauthenticated() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let app = build_app(pool.clone());

    let req = Request::builder()
        .method("GET")
        .uri("/api/admin/stats")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_admin_list_users() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let (admin_name, admin_pass) = create_admin_user(&pool).await;
    let (_user_name, _user_pass) = create_regular_user(&pool).await;
    let app = build_app(pool.clone());
    let admin_token = login_user(&app, &admin_name, &admin_pass).await;

    let req = Request::builder()
        .method("GET")
        .uri("/api/admin/users")
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let data = response_json(resp).await;
    let users = data.as_array().unwrap();
    assert!(users.len() >= 2);

    let first_user = &users[0];
    assert!(first_user["id"].is_string());
    assert!(first_user["username"].is_string());
    assert!(first_user["role"].is_string());
    assert!(first_user["display_name"].is_string());

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_admin_list_users_forbidden_for_user() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let (user_name, user_pass) = create_regular_user(&pool).await;
    let app = build_app(pool.clone());
    let user_token = login_user(&app, &user_name, &user_pass).await;

    let req = Request::builder()
        .method("GET")
        .uri("/api/admin/users")
        .header(header::AUTHORIZATION, format!("Bearer {}", user_token))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_admin_create_user() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let (admin_name, admin_pass) = create_admin_user(&pool).await;
    let app = build_app(pool.clone());
    let admin_token = login_user(&app, &admin_name, &admin_pass).await;

    let new_username = format!("created_{}", Uuid::new_v4().to_string().get(..8).unwrap());

    let body = json!({
        "display_name": "Created User",
        "username": new_username,
        "password": "newuserpassword123",
        "role": "user",
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/admin/users")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token))
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let data = response_json(resp).await;
    assert_eq!(data["username"].as_str().unwrap(), new_username);
    assert_eq!(data["role"].as_str().unwrap(), "user");
    assert_eq!(data["display_name"].as_str().unwrap(), "Created User");
    assert_eq!(data["is_default_admin"].as_bool().unwrap(), false);

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_admin_create_user_duplicate_username() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let (admin_name, admin_pass) = create_admin_user(&pool).await;
    let (existing_user, _) = create_regular_user(&pool).await;
    let app = build_app(pool.clone());
    let admin_token = login_user(&app, &admin_name, &admin_pass).await;

    let body = json!({
        "display_name": "Duplicate",
        "username": existing_user,
        "password": "somepassword123",
        "role": "user",
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/admin/users")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token))
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_admin_create_user_forbidden_for_user() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let (user_name, user_pass) = create_regular_user(&pool).await;
    let app = build_app(pool.clone());
    let user_token = login_user(&app, &user_name, &user_pass).await;

    let body = json!({
        "display_name": "Sneaky User",
        "username": "sneaky",
        "password": "sneakypassword123",
        "role": "admin",
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/admin/users")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", user_token))
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_admin_delete_user() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let (admin_name, admin_pass) = create_admin_user(&pool).await;
    let (user_name, _user_pass) = create_regular_user(&pool).await;
    let app = build_app(pool.clone());
    let admin_token = login_user(&app, &admin_name, &admin_pass).await;

    // Get user ID
    let user_row: (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM users WHERE username = $1")
            .bind(&user_name)
            .fetch_one(&pool)
            .await
            .unwrap();
    let user_id = user_row.0;

    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/api/admin/users/{}", user_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Verify user is gone
    let deleted: Option<(uuid::Uuid,)> =
        sqlx::query_as("SELECT id FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&pool)
            .await
            .unwrap();
    assert!(deleted.is_none());

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_admin_delete_default_admin_forbidden() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let (admin_name, admin_pass) = create_admin_user(&pool).await;
    let app = build_app(pool.clone());
    let admin_token = login_user(&app, &admin_name, &admin_pass).await;

    // Get admin's own ID (is_default_admin = TRUE)
    let admin_row: (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM users WHERE username = $1")
            .bind(&admin_name)
            .fetch_one(&pool)
            .await
            .unwrap();
    let admin_id = admin_row.0;

    // Try to delete self (default admin)
    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/api/admin/users/{}", admin_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_admin_delete_user_not_found() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let (admin_name, admin_pass) = create_admin_user(&pool).await;
    let app = build_app(pool.clone());
    let admin_token = login_user(&app, &admin_name, &admin_pass).await;

    let fake_id = Uuid::new_v4();

    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/api/admin/users/{}", fake_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_admin_delete_user_forbidden_for_regular_user() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let (user_name, user_pass) = create_regular_user(&pool).await;
    let (other_user_name, _) = create_regular_user(&pool).await;
    let app = build_app(pool.clone());
    let user_token = login_user(&app, &user_name, &user_pass).await;

    let other_row: (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM users WHERE username = $1")
            .bind(&other_user_name)
            .fetch_one(&pool)
            .await
            .unwrap();
    let other_id = other_row.0;

    let req = Request::builder()
        .method("DELETE")
        .uri(format!("/api/admin/users/{}", other_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", user_token))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_admin_create_user_invalid_role() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let (admin_name, admin_pass) = create_admin_user(&pool).await;
    let app = build_app(pool.clone());
    let admin_token = login_user(&app, &admin_name, &admin_pass).await;

    let body = json!({
        "display_name": "Bad Role User",
        "username": "badrole",
        "password": "password12345",
        "role": "superadmin",
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/admin/users")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", admin_token))
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    cleanup_db(&pool).await;
}

#[tokio::test]
async fn test_post_permissions_in_response() {
    dotenv::dotenv().ok();
    let pool = PgPool::connect(&get_db_url()).await.unwrap();
    setup_db(&pool).await;
    cleanup_db(&pool).await;

    let (user1_name, user1_pass) = create_regular_user(&pool).await;
    let (user2_name, user2_pass) = create_regular_user(&pool).await;
    let app = build_app(pool.clone());

    let user1_token = login_user(&app, &user1_name, &user1_pass).await;
    let user2_token = login_user(&app, &user2_name, &user2_pass).await;

    // User1 creates a post
    let body = json!({
        "title": "Permission Test Post",
        "content": "Testing permissions.",
    });

    let req = Request::builder()
        .method("POST")
        .uri("/api/posts")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", user1_token))
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    let created = response_json(resp).await;
    let post_id = created["id"].as_str().unwrap();

    // User1 sees can_edit and can_delete as true
    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/posts/{}", post_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", user1_token))
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    let data = response_json(resp).await;
    assert!(data["can_edit"].as_bool().unwrap());
    assert!(data["can_delete"].as_bool().unwrap());

    // User2 sees can_edit and can_delete as false
    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/posts/{}", post_id))
        .header(header::AUTHORIZATION, format!("Bearer {}", user2_token))
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    let data = response_json(resp).await;
    assert!(!data["can_edit"].as_bool().unwrap());
    assert!(!data["can_delete"].as_bool().unwrap());

    // Public sees can_edit and can_delete as false
    let req = Request::builder()
        .method("GET")
        .uri(format!("/api/posts/{}", post_id))
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    let data = response_json(resp).await;
    assert!(!data["can_edit"].as_bool().unwrap());
    assert!(!data["can_delete"].as_bool().unwrap());

    cleanup_db(&pool).await;
}