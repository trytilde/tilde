INSERT INTO telemetry_delivery(id,payload) VALUES($1,$2) ON CONFLICT(id) DO NOTHING;
