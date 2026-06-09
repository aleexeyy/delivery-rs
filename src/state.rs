use crate::db::buffer::ProximityEventBuffer;
use redis::aio::MultiplexedConnection;
use sqlx::PgPool;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub redis: MultiplexedConnection,
    pub fleet_tx: broadcast::Sender<Vec<u8>>,
    pub proximity_buffer: ProximityEventBuffer,
}
