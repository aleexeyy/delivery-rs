use redis::aio::MultiplexedConnection;
use sqlx::PgPool;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub redis: MultiplexedConnection,
}
