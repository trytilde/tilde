--: Record()

--! run (p1, p2) : Record
-- Lock the thread before the mutation's fresh snapshot. Attachment metadata is
-- needed to serialize the canonical tool result in Rust; lock only those rows
-- so a concurrent download cannot change their size/hash before we commit.
WITH locked AS MATERIALIZED (
    SELECT NOW() AS at FROM chat_threads WHERE id = :p1 FOR NO KEY UPDATE
), attachments AS MATERIALIZED (
    SELECT a.id, a.thread_id, a.filename, a.media_type, a.size_bytes, a.sha256
    FROM chat_attachments a CROSS JOIN locked
    WHERE a.thread_id = :p1 AND a.id = ANY(:p2::UUID[])
    ORDER BY a.id FOR SHARE OF a
)
SELECT locked.at AS at, COALESCE((
    SELECT jsonb_agg(to_jsonb(a) ORDER BY a.id) FROM attachments a
), '[]'::JSONB) AS attachments
FROM locked;
