use serde::Serialize;

use crate::models::ids::VehicleId;

#[derive(Debug, Clone, Serialize)]
pub struct DestinationInfo {
    pub lat: f64,
    pub lng: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct VehiclePosition {
    pub id: VehicleId,
    pub lat: f64,
    pub lng: f64,
    pub destination: Option<DestinationInfo>,
    pub distance_meters: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FleetSnapshot {
    pub vehicles: Vec<VehiclePosition>,
}
