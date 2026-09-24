--: Record(connection_id?, external_message_id?, destination?, delivery_status?, id?, participant_id?, text?, status?, in_reply_to_message_id?, created_at?, format?, subject?, cached_representation?)

--! run (p1, p2, p3?, p4?) : Record
-- A thread with no matching messages still produces one row, so existence and
-- cursor validation do not require separate reads. The caller drops that row.
SELECT t.activity_sequence AS activity_sequence, (:p3::UUID IS NULL OR cursor.id IS NOT NULL) AS cursor_valid,
       d.connection_id AS connection_id, d.external_message_id AS external_message_id,
       d.destination AS destination, d.status AS delivery_status,
       m.id AS id, t.id AS thread_id, m.participant_id AS participant_id,
       m.text AS text, m.status AS status, m.in_reply_to_message_id,
       m.created_at AS created_at, m.format AS format, m.subject,
       cached.representation AS cached_representation,
       COALESCE((
           SELECT jsonb_agg(jsonb_build_object(
               'id', a.id, 'thread_id', a.thread_id, 'filename', a.filename,
               'media_type', a.media_type, 'size_bytes', a.size_bytes, 'sha256', a.sha256
           ) ORDER BY a.id)
           FROM chat_message_attachments ma
           JOIN chat_attachments a ON a.id = ma.attachment_id
           WHERE ma.message_id = m.id
       ), '[]'::jsonb) AS attachments,
       ARRAY(SELECT participant_id FROM chat_message_targets
             WHERE message_id = m.id ORDER BY participant_id) AS targets
FROM chat_threads t
LEFT JOIN chat_messages cursor ON cursor.id = :p3 AND cursor.thread_id = t.id
LEFT JOIN chat_invocations invocation ON invocation.id = :p4 AND invocation.thread_id = t.id
LEFT JOIN chat_messages anchor ON anchor.id = invocation.history_through_message_id
LEFT JOIN LATERAL (
    SELECT m.*
    FROM chat_messages m
    WHERE m.thread_id = t.id
      AND (:p4::UUID IS NULL OR m.invocation_id = :p4
           OR (m.created_at, m.id) <= (anchor.created_at, anchor.id))
      AND (:p4::UUID IS NULL OR (
        NOT (m.status='aborted' AND EXISTS(SELECT 1 FROM chat_participants p WHERE p.id=m.participant_id AND p.user_id IS NOT NULL))
        AND NOT EXISTS(SELECT 1 FROM chat_inputs i JOIN chat_invocations v ON v.id=i.invocation_id WHERE i.id=m.id AND v.agent_id=invocation.agent_id AND NOT i.accepted)
      ))
      AND (:p3::UUID IS NULL OR (m.created_at, m.id) < (cursor.created_at, cursor.id))
    ORDER BY m.created_at DESC, m.id DESC
    LIMIT :p2
) m ON TRUE
LEFT JOIN chat_message_deliveries d ON d.message_id = m.id
LEFT JOIN chat_converted_messages cached
    ON cached.message_id = m.id AND cached.agent_id = invocation.agent_id
WHERE t.id = :p1
ORDER BY m.created_at DESC, m.id DESC;
