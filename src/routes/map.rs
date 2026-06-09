use axum::{
    Router,
    extract::ws::{Message, WebSocket},
    extract::{State, WebSocketUpgrade},
    http::{StatusCode, header},
    response::{Html, IntoResponse},
    routing::get,
};
use tokio::sync::broadcast;

use crate::db::vehicle::PostgresVehicleRepo;
use crate::dto::fleet::VehicleDestination;
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(map_page_handler))
        .route("/ws", get(ws_handler))
        .route("/fleet/assignments", get(fleet_assignments_handler))
        .with_state(state)
}

async fn map_page_handler() -> Html<&'static str> {
    Html(include_str!("../../static/index.html"))
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state.fleet_tx))
}

async fn handle_socket(mut socket: WebSocket, tx: broadcast::Sender<Vec<u8>>) {
    let mut rx = tx.subscribe();

    loop {
        match rx.recv().await {
            Ok(bin) => {
                if socket.send(Message::Binary(bin.into())).await.is_err() {
                    break;
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!("WS client lagged by {n} snapshots — skipping missed frames");
            }
            Err(broadcast::error::RecvError::Closed) => {
                break;
            }
        }
    }
}

pub async fn fleet_assignments_handler(State(state): State<AppState>) -> impl IntoResponse {
    let repo = PostgresVehicleRepo::new(state.pool);

    match repo.get_all_active_destinations().await {
        Ok(map) => {
            let list: Vec<VehicleDestination> = map
                .into_iter()
                .map(|(vid, coords)| VehicleDestination {
                    vehicle_id: vid,
                    coords,
                })
                .collect();

            match rmp_serde::to_vec_named(&list) {
                Ok(bin) => {
                    // Return raw bytes with the correct MessagePack header
                    let headers = [(header::CONTENT_TYPE, "application/msgpack")];
                    (StatusCode::OK, headers, bin).into_response()
                }
                Err(err) => {
                    tracing::error!("Failed to compress vehicle destinations into binary: {err}");
                    StatusCode::INTERNAL_SERVER_ERROR.into_response()
                }
            }
        }
        Err(err) => {
            tracing::error!("Failed to fetch fleet assignments: {err}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
