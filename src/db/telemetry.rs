use crate::db::traits::TelemetryRepository;
use crate::models::delivery::Coordinates;
use crate::models::ids::VehicleId;
use async_trait::async_trait;
use redis::AsyncCommands;
use redis::aio::MultiplexedConnection;

pub struct RedisTelemetryRepo {
    conn: MultiplexedConnection,
}

impl RedisTelemetryRepo {
    pub fn new(conn: MultiplexedConnection) -> Self {
        Self { conn }
    }
}

#[async_trait]
impl TelemetryRepository for RedisTelemetryRepo {
    async fn update_location(&self, v_id: VehicleId, c: &Coordinates) -> Result<(), String> {
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
}
