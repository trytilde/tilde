-- Durable propagation across the request -> pending work -> execution boundary.
ALTER TABLE chat_messages ADD COLUMN traceparent TEXT NOT NULL DEFAULT '';
ALTER TABLE chat_messages ADD COLUMN tracestate TEXT NOT NULL DEFAULT '';
ALTER TABLE chat_invocations ADD COLUMN traceparent TEXT NOT NULL DEFAULT '';
ALTER TABLE chat_invocations ADD COLUMN tracestate TEXT NOT NULL DEFAULT '';
ALTER TABLE chat_invocations ADD COLUMN execution_traceparent TEXT NOT NULL DEFAULT '';
