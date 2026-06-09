use redis::{self, aio::MultiplexedConnection};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::time::Duration;

pub async fn create_db_pool() -> PgPool {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        panic!("DATABASE_URL must be set");
    };

    // Tune via DB_POOL_SIZE env var. At 500 vehicles × 5 Hz × 2 DB ops each
    // the pool needs headroom well beyond the old default of 10.
    let max_connections: u32 = std::env::var("DB_POOL_SIZE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(50);

    let pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .acquire_timeout(Duration::from_secs(30))
        .connect(database_url.as_str())
        .await
        .unwrap_or_else(|e| {
            tracing::error!("Failed to connect to DB: {}", e);
            panic!("Failed to connect to DB: {e}");
        });

    if std::env::var("RUN_MIGRATIONS").is_ok() {
        match sqlx::migrate!("./migrations").run(&pool).await {
            Ok(()) => tracing::info!("Migrations ran successfully"),
            Err(e) => {
                tracing::error!("Failed to run migrations: {}", e);
                panic!("Failed to run migrations: {e}");
            }
        }
    }

    pool
}

pub async fn create_redis_pool() -> MultiplexedConnection {
    let Ok(redis_url) = std::env::var("REDIS_URL") else {
        panic!("REDIS_URL must be set");
    };

    let Ok(redis_client) = redis::Client::open(redis_url.as_str()) else {
        panic!("Invalid REDIS_URL: {redis_url}");
    };

    redis_client
        .get_multiplexed_async_connection()
        .await
        .unwrap_or_else(|e| panic!("Failed to connect to Redis: {e}"))
}
