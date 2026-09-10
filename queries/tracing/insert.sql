INSERT INTO tracing_spans(trace_id,span_id,parent_span_id,invocation_id,start_unix_nano,end_unix_nano,otlp,delivery)
SELECT * FROM UNNEST($1::bytea[],$2::bytea[],$3::bytea[],$4::uuid[],$5::bigint[],$6::bigint[],$7::bytea[],$8::text[])
ON CONFLICT(trace_id,span_id) DO NOTHING;
