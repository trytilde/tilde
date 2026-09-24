-- Partial weighted capacity remains routable but is distinct from full health.
ALTER TABLE agent_health ADD COLUMN degraded BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE agent_health ADD CONSTRAINT agent_health_degraded_available CHECK(NOT degraded OR healthy);
