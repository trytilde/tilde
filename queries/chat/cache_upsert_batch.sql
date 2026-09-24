--: Record()

--! run (p1, p2, p3, p4) : Record
-- The caller holds the thread lock. Validate the complete batch before writing any row.
WITH input AS MATERIALIZED (
    SELECT * FROM UNNEST(:p3::UUID[], :p4::TEXT[]) AS v(message_id, representation)
), checked AS MATERIALIZED (
    SELECT i.*, m.id IS NOT NULL AS present, m.status = 'complete' AS complete
    FROM input i LEFT JOIN chat_messages m ON m.id = i.message_id AND m.thread_id = :p2
), validation AS (
    SELECT COUNT(*) FILTER (WHERE NOT present) AS missing,
           COUNT(*) FILTER (WHERE present AND NOT complete) AS incomplete
    FROM checked
), written AS (
    INSERT INTO chat_converted_messages(agent_id, message_id, representation)
    SELECT :p1, message_id, representation::JSONB FROM checked
    WHERE (SELECT missing = 0 AND incomplete = 0 FROM validation)
    ON CONFLICT(agent_id, message_id) DO UPDATE SET representation = EXCLUDED.representation
    RETURNING message_id
)
SELECT missing AS missing, incomplete AS incomplete,
       (SELECT COUNT(*) FROM written) AS written FROM validation;
