use serde::Serialize;

use crate::models::delivery::Coordinates;
use crate::models::ids::VehicleId;

#[derive(Serialize)]
pub struct VehicleDestination {
    pub vehicle_id: VehicleId,
    #[serde(flatten)]
    pub coords: Coordinates,
}
