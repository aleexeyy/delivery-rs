use delivery_rs::db;
use delivery_rs::routes::router;
use delivery_rs::state::AppState;
use redis::aio::MultiplexedConnection;
use sqlx::PgPool;

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

    let state = AppState { pool, redis };

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
    println!("--- RUST DEBUG: init_logger() has started executing! ---");

    let log_dir = std::env::var("LOG_DIR").unwrap_or_else(|_| "./logs".to_string());
    println!("--- RUST DEBUG: target directory is {} ---", log_dir);

    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        println!(
            "--- RUST DEBUG WARNING: Failed to create log dir: {} ---",
            e
        );
    }

    let file_appender = tracing_appender::rolling::daily(&log_dir, "backend.log");

    let (non_blocking_writer, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with(
            fmt::layer()
                .with_writer(non_blocking_writer)
                .with_ansi(false)
                .with_target(true)
                .with_level(true)
                .compact(),
        )
        .with(
            fmt::layer()
                .with_writer(std::io::stdout)
                .with_ansi(true)
                .with_target(true)
                .with_level(true)
                .compact(),
        )
        .init();

    tracing::info!("Logger initialized! Logging to stdout");

    guard
}

fn init_env() {
    dotenvy::dotenv().ok();
}
