use crate::dto::telemetry::IngestTelemetryPayload;
use crate::services::telemetry::FleetService;
use crate::state::AppState;
use axum::Router;
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse, routing::post};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/telemetry", post(ingest_telemetry_handler))
        .with_state(state)
}

async fn ingest_telemetry_handler(
    State(state): State<AppState>,
    Json(payload): Json<IngestTelemetryPayload>,
) -> impl IntoResponse {
    let fleet_service = FleetService::new(state.pool, state.redis, state.proximity_buffer);

    match fleet_service
        .process_telemetry(payload.vehicle_id, payload.position)
        .await
    {
        Ok(_) => {
            // 202 Accepted is idiomatic for fast-ingest streams
            StatusCode::ACCEPTED
        }
        Err(err) => {
            tracing::error!(vehicle_id = payload.vehicle_id.0, error = %err, "Telemetry ingestion failed");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}
