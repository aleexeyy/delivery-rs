use std::cmp::Ordering;

use chrono::Utc;
use redis::aio::MultiplexedConnection;
use sqlx::PgPool;

use crate::db::delivery::PostgresDeliveryRepo;
use crate::db::telemetry::RedisTelemetryRepo;
use crate::models::delivery::{Coordinates, Delivery};
use crate::models::ids::DeliveryId;

pub struct DeliveryService {
    repo: PostgresDeliveryRepo,
    telemetry_repo: RedisTelemetryRepo,
}

impl DeliveryService {
    pub fn new(pool: PgPool, redis: MultiplexedConnection) -> Self {
        Self {
            repo: PostgresDeliveryRepo::new(pool),
            telemetry_repo: RedisTelemetryRepo::new(redis),
        }
    }

    pub async fn create_delivery(&self, lat: f64, lng: f64) -> Result<Delivery, String> {
        let dest = Coordinates {
            latitude: lat,
            longitude: lng,
        };
        let now = Utc::now();

        let delivery = self.repo.create_delivery(lat, lng).await?;

        if let Err(e) = self.assign_best_vehicle(delivery.id, &dest, now).await {
            tracing::warn!(delivery_id = delivery.id.0, error = %e, "Could not assign vehicle to delivery");
        }

        Ok(delivery)
    }

    async fn assign_best_vehicle(
        &self,
        delivery_id: DeliveryId,
        dest: &Coordinates,
        now: chrono::DateTime<Utc>,
    ) -> Result<(), String> {
        let positions = self
            .telemetry_repo
            .get_all_locations()
            .await
            .unwrap_or_default();

        // Cold start: no vehicles have reported positions yet — fall back to any free vehicle
        if positions.is_empty() {
            return self.repo.assign_any_free_vehicle(delivery_id, now).await;
        }

        let counts = self.repo.get_vehicle_assignment_counts().await?;

        // Pick the vehicle with the fewest active assignments; break ties by distance to destination
        let best = positions
            .iter()
            .min_by(|(id_a, pos_a), (id_b, pos_b)| {
                let ca = counts.get(id_a).copied().unwrap_or(0);
                let cb = counts.get(id_b).copied().unwrap_or(0);
                ca.cmp(&cb).then_with(|| {
                    pos_a
                        .distance_to(dest)
                        .partial_cmp(&pos_b.distance_to(dest))
                        .unwrap_or(Ordering::Equal)
                })
            })
            .map(|(id, _)| *id);

        if let Some(vehicle_id) = best {
            self.repo
                .assign_vehicle(vehicle_id, delivery_id, now)
                .await?;
        }

        Ok(())
    }
}
