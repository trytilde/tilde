--: Record(sequence?, kind?, entity_id?, text_delta?, snapshot?, created_at?)

--! run (p1, p2, p3) : Record
-- Retain a row for an existing thread with an empty page.
SELECT t.primary_agent_id,
       e.sequence AS sequence, e.kind AS kind, e.entity_id AS entity_id,
       e.text_delta AS text_delta, e.snapshot, e.created_at AS created_at
FROM chat_threads t
LEFT JOIN LATERAL (
    SELECT sequence, kind, entity_id, text_delta, snapshot, created_at
    FROM chat_activity WHERE thread_id = t.id AND sequence > :p2
    ORDER BY sequence LIMIT :p3
) e ON TRUE
WHERE t.id = :p1
ORDER BY e.sequence;
