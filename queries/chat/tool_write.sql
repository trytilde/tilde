--: Record()

--! run (p1, p2, p3, p4, p5, p6, p7, p8, p9, p10, p11, p12, p13, p14, p15) : Record
-- The caller holds the thread lock. A tool start must win the unique call ID
-- before its audit is committed and before the provider can execute.
WITH existing AS MATERIALIZED (
    SELECT * FROM chat_tool_calls WHERE id = :p1
), matched AS MATERIALIZED (
    SELECT * FROM existing
    WHERE thread_id = :p2 AND invocation_id = :p3 AND participant_id = :p4
      AND name = :p5 AND provider_id = :p6 AND (:p8 = '' OR input_json = :p8)
), started AS (
    INSERT INTO chat_tool_calls(id, thread_id, invocation_id, participant_id, name, provider_id, status, input_json, summary, detached, display)
    SELECT :p1, :p2, :p3, :p4, :p5, :p6, 'running', :p8, :p13, :p14, :p15
    WHERE :p11 = 'tool.started' AND :p8 <> '' AND :p9 = ''
    ON CONFLICT(id) DO NOTHING RETURNING id
), finished AS (
    UPDATE chat_tool_calls SET status = :p7, output_json = :p9, error = :p10, updated_at = NOW()
    WHERE id IN (SELECT id FROM matched WHERE status = 'running')
      AND :p11 IN ('tool.completed', 'tool.failed', 'tool.aborted')
    RETURNING id
), accepted AS (
    SELECT 1 WHERE EXISTS(SELECT 1 FROM started) OR EXISTS(SELECT 1 FROM finished)
       OR (:p11 = 'tool.input.delta' AND EXISTS(SELECT 1 FROM matched WHERE status = 'running'))
), advanced AS (
    UPDATE chat_threads SET activity_sequence = :p12::BIGINT
    WHERE id = :p2 AND activity_sequence = :p12::BIGINT - 1 AND EXISTS(SELECT 1 FROM accepted)
    RETURNING id
)
SELECT EXISTS(SELECT 1 FROM existing) AS existed,
       EXISTS(SELECT 1 FROM advanced) AS applied,
       EXISTS(SELECT 1 FROM matched WHERE status <> 'running'
              AND status = :p7 AND output_json = :p9 AND error = :p10) AS replayed, txid_current() AS transaction_id,
       COALESCE((SELECT display FROM existing), :p15) AS display;
