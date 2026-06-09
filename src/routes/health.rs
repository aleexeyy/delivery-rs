use axum::{Json, Router, extract::State, routing::get};
use sqlx::Connection;

use crate::{dto::health::HealthResponse, error::ApiError, state::AppState};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .with_state(state)
}

async fn health_check(State(state): State<AppState>) -> Result<Json<HealthResponse>, ApiError> {
    if let Some(mut conn) = state.pool.try_acquire() {
        conn.ping().await.map_err(|e| {
            tracing::error!("Health check DB ping failed: {e}");
            ApiError::InternalServerError
        })?;

        Ok(Json(HealthResponse { status: "OK" }))
    } else {
        tracing::error!("Health check failed: no DB connection available");

        Err(ApiError::InternalServerError)
    }
}
