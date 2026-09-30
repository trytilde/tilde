-- Telemetry delivery is stateless: an accepted batch is written to object storage and only this
-- pointer lives in Postgres. Any gateway replica claims a pointer under a short lease, delivers
-- the object and removes the pointer.
DROP TABLE telemetry_delivery;
CREATE TABLE telemetry_objects (
 object_key TEXT PRIMARY KEY,
 -- signal and destination, for example traces/clickhouse or logs/external
 queue TEXT NOT NULL,
 agent_id TEXT NOT NULL,
 -- a retried upload of the same batch stays one pending pointer
 batch_id TEXT NOT NULL,
 bytes BIGINT NOT NULL CHECK (bytes > 0),
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 leased_until TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 UNIQUE (queue, batch_id)
);
CREATE INDEX telemetry_objects_due ON telemetry_objects(queue, leased_until);
CREATE INDEX telemetry_objects_agent ON telemetry_objects(queue, agent_id);
CREATE INDEX telemetry_objects_created ON telemetry_objects(created_at);
CREATE TRIGGER telemetry_objects_changed AFTER INSERT ON telemetry_objects
 FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_telemetry');
