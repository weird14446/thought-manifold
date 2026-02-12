pub mod admin;
pub mod auth;
pub mod comments;
pub mod metrics;
pub mod posts;
pub mod reviews;
pub mod users;

pub use admin::admin_routes;
pub use auth::auth_routes;
pub use comments::comments_routes;
pub use metrics::metrics_routes;
pub use posts::posts_routes;
pub use reviews::{review_center_routes, reviews_routes};
pub use users::users_routes;
