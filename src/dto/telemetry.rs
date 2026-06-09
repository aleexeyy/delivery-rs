use serde::Deserialize;

use crate::models::{delivery::Coordinates, ids::VehicleId};

#[derive(Debug, Deserialize)]
pub struct IngestTelemetryPayload {
    pub vehicle_id: VehicleId,
    #[serde(flatten)]
    pub position: Coordinates,
}
