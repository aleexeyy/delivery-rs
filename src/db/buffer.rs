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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ids::DeliveryAssignmentId;
    use chrono::Utc;
    use std::sync::Arc;

    #[test]
    fn new_buffer_is_empty() {
        let buffer = ProximityEventBuffer::new();
        let inner = buffer.inner.lock().unwrap();
        assert!(inner.assignment_ids.is_empty());
        assert!(inner.distances.is_empty());
        assert!(inner.detected_ats.is_empty());
    }

    #[test]
    fn default_creates_empty_buffer() {
        let buffer = ProximityEventBuffer::default();
        let inner = buffer.inner.lock().unwrap();
        assert!(inner.assignment_ids.is_empty());
    }

    #[test]
    fn push_appends_to_all_three_columns() {
        let buffer = ProximityEventBuffer::new();
        let now = Utc::now();

        buffer.push(ProximityEventRecord {
            delivery_assignment_id: DeliveryAssignmentId(10),
            distance_meters: 42.5,
            detected_at: now,
        });

        let inner = buffer.inner.lock().unwrap();
        assert_eq!(inner.assignment_ids, vec![10]);
        assert_eq!(inner.distances, vec![42.5]);
        assert_eq!(inner.detected_ats, vec![now]);
    }

    #[test]
    fn push_multiple_records_maintains_insertion_order() {
        let buffer = ProximityEventBuffer::new();

        buffer.push(ProximityEventRecord {
            delivery_assignment_id: DeliveryAssignmentId(1),
            distance_meters: 10.0,
            detected_at: Utc::now(),
        });
        buffer.push(ProximityEventRecord {
            delivery_assignment_id: DeliveryAssignmentId(2),
            distance_meters: 20.0,
            detected_at: Utc::now(),
        });
        buffer.push(ProximityEventRecord {
            delivery_assignment_id: DeliveryAssignmentId(3),
            distance_meters: 30.0,
            detected_at: Utc::now(),
        });

        let inner = buffer.inner.lock().unwrap();
        assert_eq!(inner.assignment_ids, vec![1, 2, 3]);
        assert_eq!(inner.distances, vec![10.0, 20.0, 30.0]);
        assert_eq!(inner.detected_ats.len(), 3);
    }

    #[test]
    fn clone_shares_inner_arc() {
        let buffer = ProximityEventBuffer::new();
        let clone = buffer.clone();
        assert!(Arc::ptr_eq(&buffer.inner, &clone.inner));
    }

    #[test]
    fn push_to_original_is_visible_via_clone() {
        let buffer = ProximityEventBuffer::new();
        let clone = buffer.clone();

        buffer.push(ProximityEventRecord {
            delivery_assignment_id: DeliveryAssignmentId(5),
            distance_meters: 99.9,
            detected_at: Utc::now(),
        });

        let inner = clone.inner.lock().unwrap();
        assert_eq!(inner.assignment_ids.len(), 1);
        assert_eq!(inner.assignment_ids[0], 5);
        assert_eq!(inner.distances[0], 99.9);
    }

    #[test]
    fn push_via_clone_is_visible_on_original() {
        let buffer = ProximityEventBuffer::new();
        let clone = buffer.clone();

        clone.push(ProximityEventRecord {
            delivery_assignment_id: DeliveryAssignmentId(7),
            distance_meters: 15.0,
            detected_at: Utc::now(),
        });

        let inner = buffer.inner.lock().unwrap();
        assert_eq!(inner.assignment_ids.len(), 1);
        assert_eq!(inner.assignment_ids[0], 7);
    }
}
