use delivery_rs::db;
use delivery_rs::db::buffer::ProximityEventBuffer;
use delivery_rs::routes::router;
use delivery_rs::services::broadcast::run_fleet_broadcast;
use delivery_rs::state::AppState;
use redis::aio::MultiplexedConnection;
use sqlx::PgPool;
use std::time::Duration;
use tokio::sync::broadcast;

use tracing_subscriber::{EnvFilter, fmt, prelude::*};

const HOST_ADDRESS_ENV: &str = "HOST";
const PORT_ENV: &str = "APP_PORT";

#[tokio::main]
async fn main() {
    init_env();
    let _log_guard = init_logger();

    let pool = init_db().await;
    tracing::info!("Connected to DB");

    let redis = init_redis().await;
    tracing::info!("Connected to Redis");

    let (fleet_tx, _) = broadcast::channel::<Vec<u8>>(256);

    tokio::spawn(run_fleet_broadcast(
        pool.clone(),
        redis.clone(),
        fleet_tx.clone(),
    ));

    let proximity_buffer = ProximityEventBuffer::new();

    // Flush buffered proximity events to Postgres every 500 ms as a bulk UNNEST insert.
    tokio::spawn({
        let buffer = proximity_buffer.clone();
        let pool = pool.clone();
        async move {
            let mut interval = tokio::time::interval(Duration::from_millis(100));
            loop {
                interval.tick().await;
                match buffer.flush(&pool).await {
                    Ok(n) if n > 0 => tracing::debug!(count = n, "Flushed proximity events"),
                    Ok(_) => {}
                    Err(e) => {
                        delivery_rs::throttled_error!(60, error = %e, "Proximity flush failed")
                    }
                }
            }
        }
    });

    let state = AppState {
        pool,
        redis,
        fleet_tx,
        proximity_buffer,
    };

    let host = if let Ok(url) = std::env::var(HOST_ADDRESS_ENV) {
        url
    } else {
        tracing::warn!("{HOST_ADDRESS_ENV} is not set, using default");
        "127.0.0.1".to_string()
    };

    let port = if let Ok(port) = std::env::var(PORT_ENV) {
        port.parse::<u16>().unwrap_or(3000)
    } else {
        tracing::warn!("{PORT_ENV} is not set, using default");
        3000
    };

    let app = router(state);

    let listener = tokio::net::TcpListener::bind(format!("{host}:{port}"))
        .await
        .expect("failed to bind TCP listener");

    tracing::info!("Listening on {}:{}", host, port);

    axum::serve(listener, app).await.expect("server crashed");

    tracing::warn!("Server stopped");
}

async fn init_db() -> PgPool {
    db::create_db_pool().await
}

async fn init_redis() -> MultiplexedConnection {
    db::create_redis_pool().await
}

fn init_logger() -> tracing_appender::non_blocking::WorkerGuard {
    let log_dir = std::env::var("LOG_DIR").unwrap_or_else(|_| "./logs".to_string());

    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("warn: could not create log directory {log_dir:?}: {e}");
    }

    let file_appender = tracing_appender::rolling::daily(&log_dir, "backend.log");
    let (non_blocking_writer, guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(
            // File: include file path + line number for post-mortem debugging
            fmt::layer()
                .with_writer(non_blocking_writer)
                .with_ansi(false)
                .with_target(true)
                .with_level(true)
                .with_file(true)
                .with_line_number(true)
                .compact(),
        )
        .with(
            // Stdout: compact output, no file/line to keep it readable
            fmt::layer()
                .with_writer(std::io::stdout)
                .with_ansi(true)
                .with_target(true)
                .with_level(true)
                .compact(),
        )
        .init();

    tracing::info!(log_dir, "Logger initialized");

    guard
}

fn init_env() {
    dotenvy::dotenv().ok();
}
