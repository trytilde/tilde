-- Preserve the complete OTLP resource/scope/span envelope as protobuf bytes.
-- Query columns intentionally exclude prompt, tool, event, and attribute content.
CREATE TABLE tracing_spans (
 trace_id BYTEA NOT NULL CHECK(octet_length(trace_id)=16),
 span_id BYTEA NOT NULL CHECK(octet_length(span_id)=8),
 parent_span_id BYTEA NOT NULL CHECK(octet_length(parent_span_id) IN (0,8)),
 invocation_id UUID REFERENCES chat_invocations(id) ON DELETE SET NULL,
 start_unix_nano BIGINT NOT NULL,
 end_unix_nano BIGINT NOT NULL,
 otlp BYTEA NOT NULL,
 received_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 delivery TEXT NOT NULL CHECK(delivery IN ('disabled','pending','sent','rejected')),
 attempts INTEGER NOT NULL DEFAULT 0,
 retry_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 PRIMARY KEY(trace_id,span_id)
);
CREATE INDEX tracing_spans_invocation ON tracing_spans(invocation_id,start_unix_nano);
CREATE INDEX tracing_spans_pending ON tracing_spans(retry_at) WHERE delivery='pending';
CREATE INDEX tracing_spans_retention ON tracing_spans(received_at);

CREATE TRIGGER tracing_insert_notify AFTER INSERT ON tracing_spans
FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_tracing');
CREATE TRIGGER tracing_delivery_notify AFTER UPDATE ON tracing_spans
FOR EACH STATEMENT EXECUTE FUNCTION tilde_notify_change('tilde_tracing');
