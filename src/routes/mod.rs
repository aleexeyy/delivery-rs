use crate::state::AppState;
use axum::Router;
use tower_http::trace::TraceLayer;

mod health;
mod telemetry;

pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(health::router(state.clone()))
        .merge(telemetry::router(state.clone()))
        .layer(TraceLayer::new_for_http())
}
