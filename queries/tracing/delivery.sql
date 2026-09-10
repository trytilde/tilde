UPDATE tracing_spans SET delivery=$3, attempts=LEAST(attempts+1,20),
 retry_at=NOW()+make_interval(secs=>GREATEST(COALESCE($4,0),LEAST(300,POWER(2,LEAST(attempts+1,9)))::integer))
WHERE (trace_id,span_id) IN (SELECT * FROM UNNEST($1::bytea[],$2::bytea[]));
