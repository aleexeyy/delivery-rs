use crate::models::ids::DeliveryAssignmentId;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::{Arc, Mutex};

/// Public record type used at call sites to group the three fields by name.
pub struct ProximityEventRecord {
    pub delivery_assignment_id: DeliveryAssignmentId,
    pub distance_meters: f64,
    pub detected_at: DateTime<Utc>,
}

/// Internal SoA (Structure-of-Arrays) storage.
///
/// Keeping each column in its own Vec means:
///  - push: three scalar appends, no struct-copy overhead
///  - flush: column slices bind directly to the UNNEST query — zero
///    intermediate allocation, zero extraction loop
struct BufferInner {
    assignment_ids: Vec<i32>,
    distances: Vec<f64>,
    detected_ats: Vec<DateTime<Utc>>,
}

impl BufferInner {
    fn new() -> Self {
        Self {
            assignment_ids: Vec::new(),
            distances: Vec::new(),
            detected_ats: Vec::new(),
        }
    }
}

/// Accumulates proximity events in memory and flushes them to Postgres in bulk.
/// Holds an Arc internally so Clone is cheap.
#[derive(Clone)]
pub struct ProximityEventBuffer {
    inner: Arc<Mutex<BufferInner>>,
}

impl Default for ProximityEventBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl ProximityEventBuffer {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(BufferInner::new())),
        }
    }

    /// Append one event. Splits the record into its three columns immediately
    /// so the flush path binds column slices with no extraction work.
    pub fn push(&self, record: ProximityEventRecord) {
        if let Ok(mut buf) = self.inner.lock() {
            buf.assignment_ids.push(record.delivery_assignment_id.0);
            buf.distances.push(record.distance_meters);
            buf.detected_ats.push(record.detected_at);
        }
    }

    /// Drain the buffer and bulk-insert all accumulated events via UNNEST.
    /// Returns the number of events flushed.
    pub async fn flush(&self, pool: &PgPool) -> Result<usize, String> {
        // Swap the buffer out under the lock; release the lock before any I/O.
        let drained = {
            let mut buf = self
                .inner
                .lock()
                .map_err(|e| format!("Lock poisoned: {e}"))?;
            std::mem::replace(&mut *buf, BufferInner::new())
        };

        let count = drained.assignment_ids.len();
        if count == 0 {
            return Ok(0);
        }

        // Column slices bind directly — no intermediate Vec allocation.
        sqlx::query(
            r#"
            INSERT INTO proximity_events (delivery_assignment_id, distance, detected_at)
            SELECT * FROM UNNEST($1::integer[], $2::double precision[], $3::timestamptz[])
            "#,
        )
        .bind(&drained.assignment_ids[..])
        .bind(&drained.distances[..])
        .bind(&drained.detected_ats[..])
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to flush proximity events: {e}"))?;

        Ok(count)
    }
}
