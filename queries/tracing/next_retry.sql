SELECT MIN(retry_at) AS retry_at FROM tracing_spans WHERE delivery='pending';
