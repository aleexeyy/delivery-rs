use axum::{Json, Router, extract::State, routing::get};
use sqlx::Connection;

use crate::{dto::health::HealthResponse, error::ApiError, state::AppState};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .with_state(state)
}

async fn health_check(State(state): State<AppState>) -> Result<Json<HealthResponse>, ApiError> {
    let mut conn = state.pool.acquire().await.map_err(|e| {
        crate::throttled_warn!(60, error = %e, "Health check: failed to acquire DB connection");
        ApiError::InternalServerError
    })?;

    conn.ping().await.map_err(|e| {
        crate::throttled_warn!(60, error = %e, "Health check: DB ping failed");
        ApiError::InternalServerError
    })?;

    Ok(Json(HealthResponse { status: "OK" }))
}
