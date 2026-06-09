use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[repr(transparent)]
#[sqlx(transparent)]
pub struct VehicleId(pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[repr(transparent)]
#[sqlx(transparent)]
pub struct DeliveryId(pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[repr(transparent)]
#[sqlx(transparent)]
pub struct DeliveryAssignmentId(pub i32);
