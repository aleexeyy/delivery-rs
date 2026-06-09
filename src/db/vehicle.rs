use crate::models::delivery::{Coordinates, Delivery, DeliveryAssignment, DeliveryStatus};
use crate::models::ids::{DeliveryAssignmentId, DeliveryId, VehicleId};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::collections::HashMap;

pub struct PostgresVehicleRepo {
    pool: PgPool,
}

impl PostgresVehicleRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct ActiveDestinationRow {
    vehicle_id: VehicleId,
    lat: f64,
    lng: f64,
}

#[derive(sqlx::FromRow)]
struct ActiveDeliveryRow {
    id: DeliveryAssignmentId,
    vehicle_id: VehicleId,
    delivery_id: DeliveryId,
    assigned_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
    lat: f64,
    lng: f64,
    status: String,
}

impl PostgresVehicleRepo {
    pub async fn get_active_deliveries(
        &self,
        vehicle_id: VehicleId,
    ) -> Result<Vec<(DeliveryAssignment, Delivery)>, String> {
        let rows: Vec<ActiveDeliveryRow> = sqlx::query_as(
            r#"
            SELECT
                da.id,
                da.vehicle_id,
                da.delivery_id,
                da.assigned_at,
                da.completed_at,
                d.lat,
                d.lng,
                d.status
            FROM delivery_assignments da
            INNER JOIN deliveries d ON da.delivery_id = d.id
            WHERE da.vehicle_id = $1
              AND da.completed_at IS NULL
            ORDER BY da.assigned_at
            "#,
        )
        .bind(vehicle_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Failed to fetch active deliveries: {e}"))?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let assignment = DeliveryAssignment {
                    id: row.id,
                    vehicle_id: row.vehicle_id,
                    delivery_id: row.delivery_id,
                    assigned_at: row.assigned_at,
                    completed_at: row.completed_at,
                };
                let delivery = Delivery {
                    id: row.delivery_id,
                    destination: Coordinates {
                        latitude: row.lat,
                        longitude: row.lng,
                    },
                    status: row.status.try_into().unwrap_or(DeliveryStatus::Pending),
                };
                (assignment, delivery)
            })
            .collect())
    }

    /// Returns one active destination per vehicle — the oldest active assignment.
    /// DISTINCT ON guarantees one row per vehicle_id even if multiple active
    /// assignments exist, preventing silent HashMap key collision in callers.
    pub async fn get_all_active_destinations(
        &self,
    ) -> Result<HashMap<VehicleId, Coordinates>, String> {
        let rows: Vec<ActiveDestinationRow> = sqlx::query_as(
            r#"
            SELECT DISTINCT ON (da.vehicle_id)
                da.vehicle_id, d.lat, d.lng
            FROM delivery_assignments da
            INNER JOIN deliveries d ON da.delivery_id = d.id
            WHERE da.completed_at IS NULL
            ORDER BY da.vehicle_id, da.assigned_at
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Failed to fetch active destinations: {e}"))?;

        Ok(rows
            .into_iter()
            .map(|row| {
                (
                    row.vehicle_id,
                    Coordinates {
                        latitude: row.lat,
                        longitude: row.lng,
                    },
                )
            })
            .collect())
    }

    /// Atomically closes the assignment and marks the delivery as delivered.
    /// Returns true if this call actually completed the delivery, false if it was
    /// already completed by a concurrent request (idempotent no-op).
    pub async fn complete_delivery(
        &self,
        assignment_id: DeliveryAssignmentId,
        completed_at: DateTime<Utc>,
    ) -> Result<bool, String> {
        let result = sqlx::query(
            r#"
            WITH closed AS (
                UPDATE delivery_assignments
                SET completed_at = $1
                WHERE id = $2 AND completed_at IS NULL
                RETURNING delivery_id
            )
            UPDATE deliveries
            SET status = 'delivered'
            WHERE id = (SELECT delivery_id FROM closed)
            "#,
        )
        .bind(completed_at)
        .bind(assignment_id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to complete delivery: {e}"))?;

        Ok(result.rows_affected() > 0)
    }
}
