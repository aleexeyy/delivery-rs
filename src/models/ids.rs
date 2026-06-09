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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn vehicle_id_equality() {
        assert_eq!(VehicleId(1), VehicleId(1));
        assert_ne!(VehicleId(1), VehicleId(2));
    }

    #[test]
    fn delivery_id_equality() {
        assert_eq!(DeliveryId(42), DeliveryId(42));
        assert_ne!(DeliveryId(1), DeliveryId(2));
    }

    #[test]
    fn assignment_id_equality() {
        assert_eq!(DeliveryAssignmentId(7), DeliveryAssignmentId(7));
        assert_ne!(DeliveryAssignmentId(1), DeliveryAssignmentId(2));
    }

    #[test]
    fn ids_are_copy() {
        let id = VehicleId(5);
        let id2 = id; // copy, not move
        assert_eq!(id, id2);
    }

    #[test]
    fn ids_can_be_used_as_hash_map_keys() {
        let mut map: HashMap<VehicleId, &str> = HashMap::new();
        map.insert(VehicleId(1), "truck");
        map.insert(VehicleId(2), "van");
        assert_eq!(map[&VehicleId(1)], "truck");
        assert_eq!(map[&VehicleId(2)], "van");
        assert!(!map.contains_key(&VehicleId(99)));
    }

    #[test]
    fn ids_serialize_as_plain_numbers() {
        assert_eq!(serde_json::to_string(&VehicleId(42)).unwrap(), "42");
        assert_eq!(serde_json::to_string(&DeliveryId(7)).unwrap(), "7");
        assert_eq!(serde_json::to_string(&DeliveryAssignmentId(99)).unwrap(), "99");
    }

    #[test]
    fn ids_deserialize_from_plain_numbers() {
        let v: VehicleId = serde_json::from_str("42").unwrap();
        assert_eq!(v, VehicleId(42));

        let d: DeliveryId = serde_json::from_str("7").unwrap();
        assert_eq!(d, DeliveryId(7));

        let a: DeliveryAssignmentId = serde_json::from_str("99").unwrap();
        assert_eq!(a, DeliveryAssignmentId(99));
    }

    #[test]
    fn delivery_assignment_id_can_be_used_as_hash_map_key() {
        let mut map: HashMap<DeliveryAssignmentId, f64> = HashMap::new();
        map.insert(DeliveryAssignmentId(10), 55.3);
        assert_eq!(map[&DeliveryAssignmentId(10)], 55.3);
    }
}
