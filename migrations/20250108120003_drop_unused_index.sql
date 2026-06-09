-- proximity_events is a write-only append ledger; no query in the codebase
-- reads from it. The index adds B-tree write overhead on every bulk UNNEST
-- flush (~2500 events/sec) with zero read benefit. Drop it.
DROP INDEX IF EXISTS idx_proximity_events_vehicle_timeline;
