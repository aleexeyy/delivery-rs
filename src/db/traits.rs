use crate::models::delivery::{Coordinates, Delivery, DeliveryAssignment};
use crate::models::ids::{DeliveryAssignmentId, DeliveryId, VehicleId};

use async_trait::async_trait;
use chrono::DateTime;
use chrono::Utc;

#[async_trait]
pub trait VehicleRepository: Send + Sync {
    /// Get the currently active delivery assignment for a vehicle.
    /// Returns `None` if the vehicle has no active assignment.
    async fn get_active_delivery(&self, vehicle_id: VehicleId) -> Result<Vec<Delivery>, String>;

    /// Log a proximity event for a specific delivery assignment.
    /// `delivery_assignment_id` is obtained from an active assignment.
    async fn log_proximity_event(
        &self,
        delivery_assignment_id: DeliveryAssignmentId,
        distance_meters: f64,
        detected_at: DateTime<Utc>,
    ) -> Result<(), String>;

    /// Assign a vehicle to a delivery. Creates a new assignment row.
    /// Returns the created assignment.
    async fn assign_delivery(
        &self,
        delivery_id: DeliveryId,
        vehicle_id: VehicleId,
        now: DateTime<Utc>,
    ) -> Result<DeliveryAssignment, String>;
}
#[async_trait]
pub trait TelemetryRepository {
    async fn update_location(
        &self,
        vehicle_id: VehicleId,
        coords: &Coordinates,
    ) -> Result<(), String>;
}
