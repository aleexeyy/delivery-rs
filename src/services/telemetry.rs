use crate::db::buffer::{ProximityEventBuffer, ProximityEventRecord};
use crate::db::delivery::PostgresDeliveryRepo;
use crate::db::telemetry::RedisTelemetryRepo;
use crate::db::vehicle::PostgresVehicleRepo;
use crate::models::delivery::{Coordinates, GEOFENCE_RADIUS_METERS};
use crate::models::ids::VehicleId;
use redis::aio::MultiplexedConnection;
use sqlx::PgPool;

pub struct FleetService {
    vehicle_repo: PostgresVehicleRepo,
    delivery_repo: PostgresDeliveryRepo,
    telemetry_repo: RedisTelemetryRepo,
    proximity_buffer: ProximityEventBuffer,
}

impl FleetService {
    pub fn new(
        pg_pool: PgPool,
        redis_conn: MultiplexedConnection,
        proximity_buffer: ProximityEventBuffer,
    ) -> Self {
        Self {
            vehicle_repo: PostgresVehicleRepo::new(pg_pool.clone()),
            delivery_repo: PostgresDeliveryRepo::new(pg_pool),
            telemetry_repo: RedisTelemetryRepo::new(redis_conn),
            proximity_buffer,
        }
    }

    pub async fn process_telemetry(
        &self,
        vehicle_id: VehicleId,
        position: Coordinates,
    ) -> Result<(), String> {
        let now = chrono::Utc::now();

        // Update live position in Redis.
        self.telemetry_repo
            .update_location(vehicle_id, &position)
            .await?;

        // Serve active deliveries from Redis cache; fall back to Postgres on miss.
        let active_deliveries = match self
            .telemetry_repo
            .get_active_deliveries_cached(vehicle_id)
            .await
        {
            Some(cached) => cached,
            None => {
                let fresh = self.vehicle_repo.get_active_deliveries(vehicle_id).await?;
                self.telemetry_repo
                    .set_active_deliveries_cached(vehicle_id, &fresh)
                    .await;
                fresh
            }
        };

        for (assignment, delivery) in active_deliveries {
            let distance = position.distance_to(&delivery.destination);

            // Buffer the proximity event — flushed to DB in bulk every 500 ms.
            self.proximity_buffer.push(ProximityEventRecord {
                delivery_assignment_id: assignment.id,
                distance_meters: distance,
                detected_at: now,
            });

            if distance <= GEOFENCE_RADIUS_METERS {
                // complete_delivery is idempotent: returns false if already completed
                // by a concurrent request for the same vehicle. Only assign the next
                // delivery when we are the request that actually completed this one.
                let actually_completed = self
                    .vehicle_repo
                    .complete_delivery(assignment.id, now)
                    .await?;

                if actually_completed {
                    // Invalidate before assigning so the next cache read sees the
                    // updated state from Postgres, not stale data.
                    self.telemetry_repo
                        .invalidate_active_deliveries(vehicle_id)
                        .await;

                    // Pick up the next pending delivery. Failure here must NOT
                    // propagate — the delivery is already committed as complete.
                    // A failed assignment leaves the vehicle idle for one tick;
                    // the next cache miss will hit DB and return empty, which is
                    // the correct (idle) state until a new delivery is created.
                    if let Err(e) = self
                        .delivery_repo
                        .assign_next_pending_to_vehicle(vehicle_id, now)
                        .await
                    {
                        crate::throttled_warn!(
                            30,
                            vehicle_id = vehicle_id.0,
                            error = %e,
                            "assign_next_pending failed after delivery completion"
                        );
                    }

                    self.telemetry_repo
                        .invalidate_active_deliveries(vehicle_id)
                        .await;

                    // Stop processing the stale snapshot. The cache was just
                    // invalidated; the next tick will load the fresh active set.
                    break;
                }
            }
        }

        Ok(())
    }
}
