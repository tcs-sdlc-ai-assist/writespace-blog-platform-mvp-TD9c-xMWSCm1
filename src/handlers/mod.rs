pub mod auth;
pub mod posts;
pub mod admin;

pub use auth::{login, register};
pub use posts::{list_posts, get_post, create_post, update_post, delete_post};
pub use admin::{stats, list_users, create_user, delete_user};