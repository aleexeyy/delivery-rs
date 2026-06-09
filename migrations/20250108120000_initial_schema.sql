-- Up Migration

-- 1. Deliveries
CREATE TABLE IF NOT EXISTS deliveries (
    id SERIAL PRIMARY KEY,
    lat DOUBLE PRECISION NOT NULL,
    lng DOUBLE PRECISION NOT NULL,
    status VARCHAR(20) DEFAULT 'pending' NOT NULL,
    CONSTRAINT chk_delivery_status CHECK (status IN ('pending', 'delivering', 'delivered', 'failed'))
);

-- 2. Vehicles (Static Asset Registry)
CREATE TABLE IF NOT EXISTS vehicles (
    id SERIAL PRIMARY KEY
);


-- 3. The Assignment Registry
CREATE TABLE IF NOT EXISTS delivery_assignments (
    id SERIAL PRIMARY KEY,
    vehicle_id INTEGER NOT NULL REFERENCES vehicles(id) ON DELETE CASCADE,
    delivery_id INTEGER NOT NULL REFERENCES deliveries(id) ON DELETE CASCADE,
    assigned_at TIMESTAMPTZ DEFAULT NOW() NOT NULL,
    completed_at TIMESTAMPTZ,

    CONSTRAINT unique_active_assignment UNIQUE (vehicle_id, completed_at)
);

-- 4. Proximity Events (Append-Only Historical Ledger)
CREATE TABLE IF NOT EXISTS proximity_events (
    id BIGSERIAL PRIMARY KEY,
    delivery_assignment_id INTEGER NOT NULL REFERENCES delivery_assignments(id) ON DELETE CASCADE,
    distance DOUBLE PRECISION NOT NULL,
    detected_at TIMESTAMPTZ DEFAULT NOW() NOT NULL
);


-- Indexes optimized for append-only time-series querying
CREATE INDEX IF NOT EXISTS idx_proximity_events_vehicle_timeline ON proximity_events(delivery_assignment_id, detected_at DESC);
CREATE INDEX idx_active_assignment ON delivery_assignments(vehicle_id) WHERE completed_at IS NULL;
