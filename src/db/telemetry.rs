use crate::models::delivery::{Coordinates, Delivery, DeliveryAssignment};
use crate::models::ids::VehicleId;
use redis::AsyncCommands;
use redis::aio::MultiplexedConnection;

pub struct RedisTelemetryRepo {
    conn: MultiplexedConnection,
}

impl RedisTelemetryRepo {
    pub fn new(conn: MultiplexedConnection) -> Self {
        Self { conn }
    }

    pub async fn update_location(&self, v_id: VehicleId, c: &Coordinates) -> Result<(), String> {
        let mut conn = self.conn.clone();
        let _: i64 = conn
            .geo_add(
                "fleet:locations",
                (c.longitude, c.latitude, v_id.0.to_string()),
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn get_all_locations(&self) -> Result<Vec<(VehicleId, Coordinates)>, String> {
        let mut conn = self.conn.clone();

        let members: Vec<String> = conn
            .zrange("fleet:locations", 0isize, -1isize)
            .await
            .map_err(|e| format!("Failed to fetch fleet members: {e}"))?;

        if members.is_empty() {
            return Ok(vec![]);
        }

        // GEOPOS returns [[longitude, latitude], ...] — note lng before lat
        let mut cmd = redis::cmd("GEOPOS");
        cmd.arg("fleet:locations");
        for m in &members {
            cmd.arg(m);
        }

        let positions: Vec<redis::Value> = cmd
            .query_async(&mut conn)
            .await
            .map_err(|e| format!("Failed to fetch fleet positions: {e}"))?;

        let result = members
            .iter()
            .zip(positions.into_iter())
            .filter_map(|(id_str, val)| {
                let (lng, lat) = parse_geopos_value(val)?;
                Some((
                    VehicleId(id_str.parse().ok()?),
                    Coordinates {
                        latitude: lat,
                        longitude: lng,
                    },
                ))
            })
            .collect();

        Ok(result)
    }

    /// Returns cached active deliveries for a vehicle, or None on miss/error.
    pub async fn get_active_deliveries_cached(
        &self,
        vehicle_id: VehicleId,
    ) -> Option<Vec<(DeliveryAssignment, Delivery)>> {
        let mut conn = self.conn.clone();
        let key = format!("vehicle:active:{}", vehicle_id.0);
        let cached: Option<String> = conn.get(&key).await.ok().flatten();
        cached.and_then(|s| serde_json::from_str(&s).ok())
    }

    /// Writes active deliveries for a vehicle to Redis with a 30-second TTL.
    pub async fn set_active_deliveries_cached(
        &self,
        vehicle_id: VehicleId,
        deliveries: &Vec<(DeliveryAssignment, Delivery)>,
    ) {
        let mut conn = self.conn.clone();
        let key = format!("vehicle:active:{}", vehicle_id.0);
        if let Ok(json) = serde_json::to_string(deliveries) {
            let _: Result<(), _> = conn.set_ex(&key, json, 30u64).await;
        }
    }

    /// Removes the active-deliveries cache entry so the next read hits Postgres.
    pub async fn invalidate_active_deliveries(&self, vehicle_id: VehicleId) {
        let mut conn = self.conn.clone();
        let key = format!("vehicle:active:{}", vehicle_id.0);
        let _: Result<(), _> = conn.del(&key).await;
    }
}

fn parse_geopos_value(val: redis::Value) -> Option<(f64, f64)> {
    // GEOPOS returns [longitude, latitude] — consume in declared order.
    let redis::Value::Array(arr) = val else {
        return None;
    };
    if arr.len() != 2 {
        return None;
    }
    let mut iter = arr.into_iter();
    let lng = bulk_bytes_to_f64(iter.next()?)?;
    let lat = bulk_bytes_to_f64(iter.next()?)?;
    Some((lng, lat))
}

fn bulk_bytes_to_f64(val: redis::Value) -> Option<f64> {
    match val {
        redis::Value::BulkString(bytes) => std::str::from_utf8(&bytes).ok()?.parse().ok(),
        redis::Value::SimpleString(s) => s.parse().ok(),
        _ => None,
    }
}
