use crate::models::delivery::{Coordinates, Delivery, DeliveryAssignment, DeliveryStatus};
use crate::models::ids::{DeliveryAssignmentId, DeliveryId, VehicleId};
use chrono::{DateTime, Utc};
use sqlx::PgPool;

pub struct PostgresVehicleRepo {
    pool: PgPool,
}

impl PostgresVehicleRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct ActiveDeliveryRow {
    // Assignment fields
    id: i32,
    vehicle_id: i32,
    delivery_id: i32,
    assigned_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
    // Delivery fields
    latitude: f64,
    longitude: f64,
    status: String,
}

impl PostgresVehicleRepo {
    pub async fn get_active_deliveries(
        &self,
        vehicle_id: VehicleId,
    ) -> Result<Vec<(DeliveryAssignment, Delivery)>, String> {
        let rows: Vec<ActiveDeliveryRow> = sqlx::query_as::<_, ActiveDeliveryRow>(
            r#"
            SELECT
                da.id,
                da.delivery_id,
                da.vehicle_id,
                da.assigned_at,
                da.completed_at,
                d.lat,
                d.lng,
                d.status
            FROM delivery_assignments da
            INNER JOIN deliveries d
                ON da.delivery_id = d.id
            WHERE da.vehicle_id = $1
                AND da.completed_at IS NULL
            "#,
        )
        .bind(vehicle_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Failed to fetch active deliveries: {e}"))?;

        let result = rows
            .into_iter()
            .map(|row| {
                let assignment = DeliveryAssignment {
                    id: DeliveryAssignmentId(row.id),
                    vehicle_id: VehicleId(row.vehicle_id),
                    delivery_id: DeliveryId(row.delivery_id),
                    assigned_at: row.assigned_at,
                    completed_at: row.completed_at,
                };
                let delivery = Delivery {
                    id: DeliveryId(row.delivery_id),
                    destination: Coordinates {
                        latitude: row.latitude,
                        longitude: row.longitude,
                    },
                    status: row.status.try_into().unwrap_or(DeliveryStatus::Pending),
                };
                (assignment, delivery)
            })
            .collect();
        Ok(result)
    }

    pub async fn get_active_delivery(
        &self,
        vehicle_id: VehicleId,
    ) -> Result<Vec<Delivery>, String> {
        sqlx::query_as::<_, Delivery>(
            r#"
            SELECT
                d.id,
                d.lat,
                d.lng,
                d.status
            FROM deliveries d
            INNER JOIN delivery_assignments da
                ON d.id = da.delivery_id
            WHERE da.vehicle_id = $1
                AND da.completed_at IS NULL
            "#,
        )
        .bind(vehicle_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Failed to fetch active deliveries: {e}"))
    }

    pub async fn log_proximity_event(
        &self,
        delivery_assignment_id: DeliveryAssignmentId,
        distance_meters: f64,
        detected_at: DateTime<Utc>,
    ) -> Result<(), String> {
        sqlx::query(
            r#"
            INSERT INTO proximity_events (delivery_assignment_id, distance, detected_at)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(delivery_assignment_id)
        .bind(distance_meters)
        .bind(detected_at)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to log proximity event: {e}"))?;
        Ok(())
    }

    pub async fn assign_delivery(
        &self,
        delivery_id: DeliveryId,
        vehicle_id: VehicleId,
        now: DateTime<Utc>,
    ) -> Result<DeliveryAssignment, String> {
        sqlx::query_as::<_, DeliveryAssignment>(
            r#"
            INSERT INTO delivery_assignments (
                vehicle_id,
                delivery_id,
                assigned_at
            )
            VALUES ($1, $2, $3)
            RETURNING
                id,
                vehicle_id,
                delivery_id,
                assigned_at,
                completed_at
            "#,
        )
        .bind(vehicle_id)
        .bind(delivery_id)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("Failed to assign delivery: {e}"))
    }
}
