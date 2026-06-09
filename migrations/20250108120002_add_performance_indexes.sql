-- Partial index so assign_next_pending_to_vehicle only scans pending rows,
-- not the full (and growing) deliveries table.
CREATE INDEX idx_deliveries_pending ON deliveries(id) WHERE status = 'pending';

-- This constraint was silently ineffective: Postgres does not treat NULL as
-- equal to NULL in unique constraints, so two rows with completed_at = NULL
-- for the same vehicle_id were never blocked. Multiple active assignments per
-- vehicle are intentional; drop the misleading constraint.
ALTER TABLE delivery_assignments DROP CONSTRAINT unique_active_assignment;
