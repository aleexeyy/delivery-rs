use std::collections::HashMap;
use std::time::Duration;

use redis::aio::MultiplexedConnection;
use sqlx::PgPool;
use tokio::sync::broadcast;

use crate::db::telemetry::RedisTelemetryRepo;
use crate::db::vehicle::PostgresVehicleRepo;
use crate::dto::websocket::{DestinationInfo, FleetSnapshot, VehiclePosition};
use crate::models::delivery::Coordinates;
use crate::models::ids::VehicleId;

pub async fn run_fleet_broadcast(
    pool: PgPool,
    redis: MultiplexedConnection,
    tx: broadcast::Sender<Vec<u8>>,
) {
    // Create repos once — they hold cheap Arc clones of the pool/connection.
    let telemetry_repo = RedisTelemetryRepo::new(redis);
    let vehicle_repo = PostgresVehicleRepo::new(pool);

    let mut interval = tokio::time::interval(Duration::from_millis(1000));

    loop {
        interval.tick().await;

        // Skip the Redis + Postgres reads entirely when nobody is listening.
        if tx.receiver_count() == 0 {
            continue;
        }

        let locations = match telemetry_repo.get_all_locations().await {
            Ok(locs) => locs,
            Err(e) => {
                crate::throttled_warn!(30, error = %e, "Fleet broadcast: Redis read failed");
                continue;
            }
        };

        let destinations: HashMap<VehicleId, Coordinates> =
            match vehicle_repo.get_all_active_destinations().await {
                Ok(d) => d,
                Err(e) => {
                    crate::throttled_warn!(30, error = %e, "Fleet broadcast: Postgres read failed");
                    HashMap::new()
                }
            };

        let vehicles: Vec<VehiclePosition> = locations
            .into_iter()
            .map(|(id, coords)| {
                let dest_opt = destinations.get(&id);
                let distance = dest_opt.map(|dest| coords.distance_to(dest));
                let destination = dest_opt.map(|dest| DestinationInfo {
                    lat: dest.latitude,
                    lng: dest.longitude,
                });

                VehiclePosition {
                    id,
                    lat: coords.latitude,
                    lng: coords.longitude,
                    destination,
                    distance_meters: distance,
                }
            })
            .collect();

        let snapshot = FleetSnapshot { vehicles };

        match rmp_serde::to_vec_named(&snapshot) {
            Ok(bin) => {
                // Errors only when there are no receivers — fine to ignore.
                let _ = tx.send(bin);
            }
            Err(e) => {
                crate::throttled_warn!(30, error = %e, "Fleet broadcast: snapshot serialization failed");
            }
        }
    }
}
