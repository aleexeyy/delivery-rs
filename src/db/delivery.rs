use crate::models::delivery::Delivery;
use crate::models::ids::{DeliveryId, VehicleId};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::collections::HashMap;

pub struct PostgresDeliveryRepo {
    pool: PgPool,
}

impl PostgresDeliveryRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Inserts a new delivery row with status 'pending'. Does NOT assign a vehicle —
    /// the service layer handles vehicle selection after querying Redis for positions.
    pub async fn create_delivery(&self, lat: f64, lng: f64) -> Result<Delivery, String> {
        sqlx::query_as(
            r#"
            INSERT INTO deliveries (lat, lng, status)
            VALUES ($1, $2, 'pending')
            RETURNING id, lat, lng, status
            "#,
        )
        .bind(lat)
        .bind(lng)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| format!("Failed to create delivery: {e}"))
    }

    /// Returns the number of active (non-completed) assignments per vehicle for
    /// every vehicle in the fleet. Vehicles with no assignments appear with count 0.
    pub async fn get_vehicle_assignment_counts(&self) -> Result<HashMap<VehicleId, i64>, String> {
        #[derive(sqlx::FromRow)]
        struct Row {
            vehicle_id: VehicleId,
            assignment_count: i64,
        }

        let rows: Vec<Row> = sqlx::query_as(
            r#"
            SELECT v.id AS vehicle_id,
                   COUNT(da.id)::bigint AS assignment_count
            FROM vehicles v
            LEFT JOIN delivery_assignments da
                ON v.id = da.vehicle_id AND da.completed_at IS NULL
            GROUP BY v.id
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("Failed to fetch vehicle assignment counts: {e}"))?;

        Ok(rows
            .into_iter()
            .map(|r| (r.vehicle_id, r.assignment_count))
            .collect())
    }

    /// Atomically assigns a specific vehicle to a delivery and marks it 'delivering'.
    /// Single CTE — one round-trip, no explicit transaction required.
    pub async fn assign_vehicle(
        &self,
        vehicle_id: VehicleId,
        delivery_id: DeliveryId,
        now: DateTime<Utc>,
    ) -> Result<(), String> {
        sqlx::query(
            r#"
            WITH new_assignment AS (
                INSERT INTO delivery_assignments (vehicle_id, delivery_id, assigned_at)
                VALUES ($1, $2, $3)
                RETURNING delivery_id
            )
            UPDATE deliveries
            SET status = 'delivering'
            WHERE id = (SELECT delivery_id FROM new_assignment)
            "#,
        )
        .bind(vehicle_id)
        .bind(delivery_id)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to assign vehicle: {e}"))?;

        Ok(())
    }

    /// Fallback: assigns whichever free (0 active assignments) vehicle Postgres
    /// locks first. Used when no vehicle positions are available in Redis yet.
    /// Single CTE — one round-trip, no explicit transaction required.
    pub async fn assign_any_free_vehicle(
        &self,
        delivery_id: DeliveryId,
        now: DateTime<Utc>,
    ) -> Result<(), String> {
        sqlx::query(
            r#"
            WITH free_vehicle AS (
                SELECT v.id AS vehicle_id
                FROM vehicles v
                LEFT JOIN delivery_assignments da
                    ON v.id = da.vehicle_id AND da.completed_at IS NULL
                WHERE da.id IS NULL
                LIMIT 1
                FOR UPDATE OF v SKIP LOCKED
            ),
            new_assignment AS (
                INSERT INTO delivery_assignments (vehicle_id, delivery_id, assigned_at)
                SELECT vehicle_id, $1, $2 FROM free_vehicle
                RETURNING delivery_id
            )
            UPDATE deliveries
            SET status = 'delivering'
            WHERE id = (SELECT delivery_id FROM new_assignment)
            "#,
        )
        .bind(delivery_id)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to assign any free vehicle: {e}"))?;

        Ok(())
    }

    /// Called when a vehicle finishes a delivery. Finds the oldest pending delivery
    /// and assigns this vehicle to it atomically. No-op if nothing is pending.
    pub async fn assign_next_pending_to_vehicle(
        &self,
        vehicle_id: VehicleId,
        now: DateTime<Utc>,
    ) -> Result<(), String> {
        sqlx::query(
            r#"
            WITH next_delivery AS (
                SELECT id FROM deliveries
                WHERE status = 'pending'
                ORDER BY id
                LIMIT 1
                FOR UPDATE SKIP LOCKED
            ),
            new_assignment AS (
                INSERT INTO delivery_assignments (vehicle_id, delivery_id, assigned_at)
                SELECT $1, id, $2 FROM next_delivery
                RETURNING delivery_id
            )
            UPDATE deliveries
            SET status = 'delivering'
            WHERE id = (SELECT delivery_id FROM new_assignment)
            "#,
        )
        .bind(vehicle_id)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("Failed to assign next pending delivery: {e}"))?;

        Ok(())
    }
}
