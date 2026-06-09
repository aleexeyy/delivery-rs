-- The active assignment index previously only covered vehicle_id.
-- Both hot queries sort by assigned_at:
--
--   get_active_deliveries:      WHERE vehicle_id = $1 ... ORDER BY assigned_at
--   get_all_active_destinations: DISTINCT ON (vehicle_id) ORDER BY vehicle_id, assigned_at
--
-- Without assigned_at in the index Postgres must fetch all matching rows from
-- the heap and sort them. With the composite index the sort is free — the index
-- already delivers rows in (vehicle_id, assigned_at) order.
DROP INDEX IF EXISTS idx_active_assignment;
CREATE INDEX idx_active_assignment
    ON delivery_assignments (vehicle_id, assigned_at)
    WHERE completed_at IS NULL;
