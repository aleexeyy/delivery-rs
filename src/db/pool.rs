use redis::{self, aio::MultiplexedConnection};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::time::Duration;

pub async fn create_db_pool() -> PgPool {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        tracing::warn!("DATABASE_URL is not set");
        panic!("DATABASE_URL must be set");
    };

    let pool = match PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(10))
        .connect_lazy(database_url.as_str())
    {
        Ok(pool) => pool,
        Err(e) => {
            tracing::error!("Failed to connect to DB: {}", e);
            panic!("Failed to connect to DB: {e}");
        }
    };
    if std::env::var("RUN_MIGRATIONS").is_ok() {
        match sqlx::migrate!("./migrations").run(&pool).await {
            Ok(()) => {
                tracing::info!("Migrations ran successfully");
            }
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
        tracing::warn!("REDIS_URL is not set");
        panic!("REDIS_URL must be set");
    };

    let Ok(redis_client) = redis::Client::open(redis_url.as_str()) else {
        tracing::error!("Failed to connect to Redis: {}", redis_url);
        panic!("Failed to connect to Redis: {redis_url}");
    };

    redis_client
        .get_multiplexed_async_connection()
        .await
        .unwrap()
}
