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
