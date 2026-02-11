pub mod auth;
pub mod posts;
pub mod users;
pub mod comments;
pub mod admin;
pub mod metrics;

pub use auth::auth_routes;
pub use posts::posts_routes;
pub use users::users_routes;
pub use comments::comments_routes;
pub use admin::admin_routes;
pub use metrics::metrics_routes;
