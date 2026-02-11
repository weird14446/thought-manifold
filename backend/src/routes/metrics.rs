use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use chrono::{Datelike, Utc};
use serde::Deserialize;
use sqlx::MySqlPool;

use crate::metrics::compute_impact_factor;

#[derive(Debug, Deserialize)]
struct JournalMetricsQuery {
    year: Option<i32>,
}

pub fn metrics_routes() -> Router<MySqlPool> {
    Router::new().route("/journal", get(get_journal_metrics))
}

async fn get_journal_metrics(
    State(pool): State<MySqlPool>,
    Query(query): Query<JournalMetricsQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let default_year = Utc::now().year();
    let year = query.year.unwrap_or(default_year);
    if year < 1900 || year > 3000 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"detail": "Year must be between 1900 and 3000"})),
        ));
    }

    let metrics = compute_impact_factor(&pool, year).await.map_err(internal_error)?;
    Ok(Json(metrics))
}

fn internal_error<E: ToString>(error: E) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({"detail": error.to_string()})),
    )
}
