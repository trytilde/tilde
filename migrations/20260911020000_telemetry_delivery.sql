-- Temporary delivery payloads and replay receipts, never a queryable trace history.
CREATE TABLE telemetry_delivery (
 id BYTEA PRIMARY KEY CHECK(octet_length(id)=32),
 payload BYTEA,
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 completed_at TIMESTAMPTZ,
 retry_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 attempts INTEGER NOT NULL DEFAULT 0,
 CHECK ((payload IS NULL) = (completed_at IS NOT NULL))
);
CREATE INDEX telemetry_delivery_pending ON telemetry_delivery(retry_at) WHERE payload IS NOT NULL;
CREATE TRIGGER telemetry_delivery_notify AFTER INSERT OR UPDATE ON telemetry_delivery
FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_telemetry');
