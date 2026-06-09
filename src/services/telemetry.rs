use crate::db::telemetry::RedisTelemetryRepo;
use crate::db::traits::*;
use crate::db::vehicle::PostgresVehicleRepo;
use crate::models::delivery::Coordinates;
use crate::models::ids::VehicleId;
use redis::aio::MultiplexedConnection;
use sqlx::PgPool;

pub struct FleetService<VR, TR> {
    vehicle_repo: VR,
    telemetry_repo: TR,
}

impl FleetService<PostgresVehicleRepo, RedisTelemetryRepo> {
    pub fn new(pg_pool: PgPool, redis_conn: MultiplexedConnection) -> Self {
        Self {
            vehicle_repo: PostgresVehicleRepo::new(pg_pool),
            telemetry_repo: RedisTelemetryRepo::new(redis_conn),
        }
    }

    pub async fn process_telemetry(
        &self,
        vehicle_id: VehicleId,
        position: Coordinates,
    ) -> Result<(), String> {
        let now = chrono::Utc::now();
        self.telemetry_repo
            .update_location(vehicle_id, &position)
            .await?;

        let active_deliveries = self.vehicle_repo.get_active_deliveries(vehicle_id).await?;
        for (delivery_assignment, delivery) in active_deliveries {
            let distance = position.distance_to(&delivery.destination);
            self.vehicle_repo
                .log_proximity_event(delivery_assignment.id, distance, now)
                .await?;
        }

        Ok(())
    }
}
