use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CreateDeliveryPayload {
    pub lat: f64,
    pub lng: f64,
}
