mod ai_review;
mod db;
mod metrics;
mod models;
mod routes;

use axum::{
    Router,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
};
use std::path::PathBuf;
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use routes::{
    admin_routes, auth_routes, comments_routes, metrics_routes, paper_workflow_routes,
    posts_routes, review_center_routes, reviews_routes, users_routes,
};

fn frontend_dist_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../frontend/dist")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "backend_rust=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load environment variables
    dotenvy::dotenv().ok();

    // Database setup
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://tm_user:tm_pass@127.0.0.1:3306/thought_manifold".to_string());

    let pool = db::init_db(&database_url).await?;
    tracing::info!("Database initialized");

    // Create uploads directory
    tokio::fs::create_dir_all("uploads").await?;

    // Frontend build directory
    let frontend_dir = frontend_dist_dir();

    // CORS layer
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // API routes
    let api_routes = Router::new()
        .nest("/api/auth", auth_routes())
        .nest("/api/users", users_routes())
        .nest("/api/posts", posts_routes())
        .nest("/api/posts", comments_routes())
        .nest("/api/posts", reviews_routes())
        .nest("/api/posts", paper_workflow_routes())
        .nest("/api/reviews", review_center_routes())
        .nest("/api/admin", admin_routes())
        .nest("/api/metrics", metrics_routes())
        .route("/api/health", get(health_check));

    // Build the app
    let app = Router::new()
        .merge(api_routes)
        .nest_service("/uploads", ServeDir::new("uploads"))
        .nest_service("/assets", ServeDir::new(frontend_dir.join("assets")))
        .fallback(serve_spa)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(pool);

    // Run the server
    let addr = "0.0.0.0:8000";
    tracing::info!("Server running on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> impl IntoResponse {
    axum::Json(serde_json::json!({"status": "healthy"}))
}

async fn serve_spa() -> impl IntoResponse {
    let frontend_dir = frontend_dist_dir();
    let index_path = frontend_dir.join("index.html");

    match tokio::fs::read_to_string(&index_path).await {
        Ok(html) => Html(html).into_response(),
        Err(_) => (
            StatusCode::OK,
            axum::Json(serde_json::json!({
                "message": "Welcome to Thought Manifold API (Rust)",
                "docs": "API documentation not available in Rust version"
            })),
        )
            .into_response(),
    }
}
