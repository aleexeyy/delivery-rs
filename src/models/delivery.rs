use crate::models::ids::{DeliveryAssignmentId, DeliveryId, VehicleId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row, postgres::PgRow};

pub const GEOFENCE_RADIUS_METERS: f64 = 100.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delivery {
    pub id: DeliveryId,
    pub destination: Coordinates,
    pub status: DeliveryStatus,
}

impl<'r> FromRow<'r, PgRow> for Delivery {
    fn from_row(row: &'r PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            destination: Coordinates {
                latitude: row.try_get("lat")?,
                longitude: row.try_get("lng")?,
            },
            status: row.try_get("status")?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DeliveryAssignment {
    pub id: DeliveryAssignmentId,
    pub vehicle_id: VehicleId,
    pub delivery_id: DeliveryId,
    pub assigned_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "VARCHAR", rename_all = "lowercase")]
pub enum DeliveryStatus {
    Pending,
    Delivering,
    Delivered,
    Failed,
}

impl TryFrom<String> for DeliveryStatus {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "pending" => Ok(Self::Pending),
            "delivering" => Ok(Self::Delivering),
            "delivered" => Ok(Self::Delivered),
            "failed" => Ok(Self::Failed),
            _ => Err(value),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coordinates {
    #[serde(rename = "lat")]
    pub latitude: f64,
    #[serde(rename = "lng")]
    pub longitude: f64,
}

impl Coordinates {
    pub fn new(latitude: f64, longitude: f64) -> Self {
        Self {
            latitude,
            longitude,
        }
    }

    pub fn distance_to(&self, other: &Coordinates) -> f64 {
        calculate_haversine(self, other)
    }
}

fn calculate_haversine(c1: &Coordinates, c2: &Coordinates) -> f64 {
    let r = 6371e3; // Earth radius in meters
    let phi1 = c1.latitude.to_radians();
    let phi2 = c2.latitude.to_radians();
    let delta_phi = (c2.latitude - c1.latitude).to_radians();
    let delta_lambda = (c2.longitude - c1.longitude).to_radians();

    let a = (delta_phi / 2.0).sin().powi(2)
        + phi1.cos() * phi2.cos() * (delta_lambda / 2.0).sin().powi(2);

    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    r * c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinates_new_stores_values() {
        let c = Coordinates::new(48.8566, 2.3508);
        assert_eq!(c.latitude, 48.8566);
        assert_eq!(c.longitude, 2.3508);
    }

    #[test]
    fn distance_to_same_point_is_zero() {
        let c = Coordinates::new(0.0, 0.0);
        assert_eq!(c.distance_to(&c), 0.0);
    }

    #[test]
    fn distance_to_known_pair_nyc_to_london() {
        // NYC (40.7128, -74.0060) to London (51.5074, -0.1278) ≈ 5,570 km
        let nyc = Coordinates::new(40.7128, -74.0060);
        let london = Coordinates::new(51.5074, -0.1278);
        let dist = nyc.distance_to(&london);
        assert!(
            (5_500_000.0..=5_650_000.0).contains(&dist),
            "Expected ~5570 km, got {dist:.0} m"
        );
    }

    #[test]
    fn distance_is_symmetric() {
        let berlin = Coordinates::new(52.5200, 13.4050);
        let paris = Coordinates::new(48.8566, 2.3508);
        let diff = (berlin.distance_to(&paris) - paris.distance_to(&berlin)).abs();
        assert!(diff < 1e-6, "Distance must be symmetric, diff was {diff}");
    }

    #[test]
    fn points_fifty_meters_apart_are_within_geofence() {
        let base = Coordinates::new(52.5200, 13.4050);
        let nearby = Coordinates::new(52.5204, 13.4050); // ~45 m north
        assert!(
            base.distance_to(&nearby) < GEOFENCE_RADIUS_METERS,
            "Points ~45 m apart must be within {GEOFENCE_RADIUS_METERS} m geofence"
        );
    }

    #[test]
    fn points_two_hundred_meters_apart_are_outside_geofence() {
        let base = Coordinates::new(52.5200, 13.4050);
        let far = Coordinates::new(52.5218, 13.4050); // ~200 m north
        assert!(
            base.distance_to(&far) > GEOFENCE_RADIUS_METERS,
            "Points ~200 m apart must be outside {GEOFENCE_RADIUS_METERS} m geofence"
        );
    }

    #[test]
    fn delivery_status_parses_all_valid_variants() {
        assert!(matches!(
            DeliveryStatus::try_from("pending".to_string()),
            Ok(DeliveryStatus::Pending)
        ));
        assert!(matches!(
            DeliveryStatus::try_from("delivering".to_string()),
            Ok(DeliveryStatus::Delivering)
        ));
        assert!(matches!(
            DeliveryStatus::try_from("delivered".to_string()),
            Ok(DeliveryStatus::Delivered)
        ));
        assert!(matches!(
            DeliveryStatus::try_from("failed".to_string()),
            Ok(DeliveryStatus::Failed)
        ));
    }

    #[test]
    fn delivery_status_rejects_unknown_string() {
        let result = DeliveryStatus::try_from("cancelled".to_string());
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "cancelled");
    }

    #[test]
    fn delivery_status_is_case_sensitive() {
        assert!(DeliveryStatus::try_from("Pending".to_string()).is_err());
        assert!(DeliveryStatus::try_from("DELIVERING".to_string()).is_err());
    }
}
