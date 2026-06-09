-- Seed 500 vehicles for simulation (IDs 1–500)
INSERT INTO vehicles (id)
SELECT generate_series(1, 500)
ON CONFLICT (id) DO NOTHING;

-- Advance the sequence so future auto-inserts start from 501
SELECT setval('vehicles_id_seq', 500, true);
