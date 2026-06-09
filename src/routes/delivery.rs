use crate::dto::delivery::CreateDeliveryPayload;
use crate::services::delivery::DeliveryService;
use crate::state::AppState;
use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::post};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/deliveries", post(create_delivery_handler))
        .with_state(state)
}

async fn create_delivery_handler(
    State(state): State<AppState>,
    Json(payload): Json<CreateDeliveryPayload>,
) -> impl IntoResponse {
    let service = DeliveryService::new(state.pool, state.redis);

    match service.create_delivery(payload.lat, payload.lng).await {
        Ok(delivery) => (StatusCode::CREATED, Json(delivery)).into_response(),
        Err(err) => {
            tracing::error!(error = %err, "Failed to create delivery");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
