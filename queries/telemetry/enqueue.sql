--! run (p1, p2)
INSERT INTO telemetry_delivery(id,payload) VALUES(:p1,:p2) ON CONFLICT(id) DO NOTHING;
