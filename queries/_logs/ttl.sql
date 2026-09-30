-- Whether a telemetry table currently carries a TTL, so indefinite retention only removes one that exists.
SELECT position(engine_full, ' TTL ') > 0 AS has_ttl FROM system.tables
WHERE database = currentDatabase() AND name = {table:String}
FORMAT JSON
