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

#[cfg(test)]
mod tests {
    use super::*;
    use redis::Value;

    // --- bulk_bytes_to_f64 ---

    #[test]
    fn bulk_bytes_to_f64_parses_valid_bulk_string() {
        let val = Value::BulkString(b"3.14159".to_vec());
        let result = bulk_bytes_to_f64(val).unwrap();
        assert!((result - 3.14159).abs() < 1e-5);
    }

    #[test]
    fn bulk_bytes_to_f64_parses_negative_bulk_string() {
        let val = Value::BulkString(b"-74.0060".to_vec());
        let result = bulk_bytes_to_f64(val).unwrap();
        assert!((result - (-74.0060)).abs() < 1e-4);
    }

    #[test]
    fn bulk_bytes_to_f64_parses_simple_string() {
        let val = Value::SimpleString("2.71828".to_string());
        let result = bulk_bytes_to_f64(val).unwrap();
        assert!((result - 2.71828).abs() < 1e-5);
    }

    #[test]
    fn bulk_bytes_to_f64_returns_none_for_invalid_utf8() {
        let val = Value::BulkString(vec![0xFF, 0xFE, 0xFD]);
        assert_eq!(bulk_bytes_to_f64(val), None);
    }

    #[test]
    fn bulk_bytes_to_f64_returns_none_for_non_numeric_string() {
        let val = Value::BulkString(b"not-a-number".to_vec());
        assert_eq!(bulk_bytes_to_f64(val), None);
    }

    #[test]
    fn bulk_bytes_to_f64_returns_none_for_array_value() {
        let val = Value::Array(vec![]);
        assert_eq!(bulk_bytes_to_f64(val), None);
    }

    // --- parse_geopos_value ---

    #[test]
    fn parse_geopos_value_returns_lng_lat_for_valid_pair() {
        // GEOPOS returns [longitude, latitude]
        let val = Value::Array(vec![
            Value::BulkString(b"2.3508".to_vec()),  // longitude
            Value::BulkString(b"48.8566".to_vec()), // latitude
        ]);
        let (lng, lat) = parse_geopos_value(val).unwrap();
        assert!((lng - 2.3508).abs() < 1e-4, "lng mismatch: {lng}");
        assert!((lat - 48.8566).abs() < 1e-4, "lat mismatch: {lat}");
    }

    #[test]
    fn parse_geopos_value_returns_none_for_non_array() {
        let val = Value::BulkString(b"2.3508".to_vec());
        assert_eq!(parse_geopos_value(val), None);
    }

    #[test]
    fn parse_geopos_value_returns_none_for_empty_array() {
        let val = Value::Array(vec![]);
        assert_eq!(parse_geopos_value(val), None);
    }

    #[test]
    fn parse_geopos_value_returns_none_for_single_element_array() {
        let val = Value::Array(vec![Value::BulkString(b"2.3508".to_vec())]);
        assert_eq!(parse_geopos_value(val), None);
    }

    #[test]
    fn parse_geopos_value_returns_none_for_three_element_array() {
        let val = Value::Array(vec![
            Value::BulkString(b"2.3508".to_vec()),
            Value::BulkString(b"48.8566".to_vec()),
            Value::BulkString(b"extra".to_vec()),
        ]);
        assert_eq!(parse_geopos_value(val), None);
    }

    #[test]
    fn parse_geopos_value_returns_none_for_non_numeric_longitude() {
        let val = Value::Array(vec![
            Value::BulkString(b"not-a-number".to_vec()),
            Value::BulkString(b"48.8566".to_vec()),
        ]);
        assert_eq!(parse_geopos_value(val), None);
    }

    #[test]
    fn parse_geopos_value_returns_none_for_non_numeric_latitude() {
        let val = Value::Array(vec![
            Value::BulkString(b"2.3508".to_vec()),
            Value::BulkString(b"not-a-number".to_vec()),
        ]);
        assert_eq!(parse_geopos_value(val), None);
    }
}
