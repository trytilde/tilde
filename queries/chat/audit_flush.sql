--: Record()

--! run (p1, p2, p3, p4, p5, p6, p7, p8) : Record
-- Only the background writer touches history. A short grace waits for earlier
-- batches from other instances. After a crash/overflow, a missing sequence may
-- never arrive; skip that gap and discard any later arrival below the watermark.
WITH input AS MATERIALIZED (
    SELECT * FROM UNNEST(:p2::BIGINT[], :p3::TEXT[], :p4::UUID[], :p5::TEXT[], :p6::BYTEA[], :p7::TIMESTAMPTZ[], :p8::BOOLEAN[])
    AS v(sequence, kind, entity_id, text_delta, snapshot, created_at, skip_gap)
), watermark AS (
    SELECT COALESCE(MAX(sequence), 0) AS last FROM chat_activity WHERE thread_id = :p1
), ordered AS (
    SELECT i.*, ROW_NUMBER() OVER (ORDER BY i.sequence) AS position,
           FIRST_VALUE(i.skip_gap) OVER (ORDER BY i.sequence) AS allow_gap, w.last
    FROM input i CROSS JOIN watermark w WHERE i.sequence > w.last
), written AS (
    INSERT INTO chat_activity(thread_id, sequence, kind, entity_id, text_delta, snapshot, created_at)
    SELECT :p1, sequence, kind, entity_id, text_delta, snapshot, created_at FROM ordered
    WHERE (sequence = last + position OR allow_gap) AND EXISTS(SELECT 1 FROM chat_threads WHERE id = :p1)
    ORDER BY sequence ON CONFLICT(thread_id, sequence) DO NOTHING RETURNING sequence
)
SELECT i.sequence AS sequence FROM input i CROSS JOIN watermark w
WHERE i.sequence <= w.last OR i.sequence IN (SELECT sequence FROM written)
   OR NOT EXISTS(SELECT 1 FROM chat_threads WHERE id = :p1);
