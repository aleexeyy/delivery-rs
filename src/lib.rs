pub mod db;
pub mod dto;
pub mod error;
pub mod models;
pub mod routes;
pub mod services;
pub mod state;

/// Emit a `warn!` event at most once every `$secs` seconds at this call site.
/// Each expansion gets its own independent `static` cooldown counter.
#[macro_export]
macro_rules! throttled_warn {
    ($secs:expr, $($tt:tt)*) => {{
        use std::sync::atomic::{AtomicU64, Ordering};
        static LAST: AtomicU64 = AtomicU64::new(0);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if now.saturating_sub(LAST.load(Ordering::Relaxed)) >= $secs {
            LAST.store(now, Ordering::Relaxed);
            tracing::warn!($($tt)*);
        }
    }};
}

/// Emit an `error!` event at most once every `$secs` seconds at this call site.
#[macro_export]
macro_rules! throttled_error {
    ($secs:expr, $($tt:tt)*) => {{
        use std::sync::atomic::{AtomicU64, Ordering};
        static LAST: AtomicU64 = AtomicU64::new(0);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if now.saturating_sub(LAST.load(Ordering::Relaxed)) >= $secs {
            LAST.store(now, Ordering::Relaxed);
            tracing::error!($($tt)*);
        }
    }};
}
