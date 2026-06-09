use crate::models::delivery::Coordinates;
use crate::models::ids::{DeliveryId, VehicleId};

#[derive(Debug, Clone)]
pub struct Vehicle {
    pub id: VehicleId,
    pub coordinates: Coordinates,
    // cached
    pub assigned_delivery_id: Option<DeliveryId>,
}
