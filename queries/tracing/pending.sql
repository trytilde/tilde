SELECT trace_id,span_id,otlp,attempts FROM tracing_spans
WHERE delivery='pending' AND retry_at<=NOW()
ORDER BY retry_at LIMIT 256 FOR UPDATE SKIP LOCKED;
