--: Record(history_through_message_id?, source_identity_id?)

--! run (p1) : Record
SELECT i.id,i.text,i.history_through_message_id,CASE WHEN m.id IS NULL THEN r.source_identity_id ELSE m.source_identity_id END AS source_identity_id,
 EXISTS(SELECT 1 FROM chat_channel_threads b WHERE b.thread_id=v.thread_id) AS channel_origin
FROM chat_inputs i JOIN chat_invocations v ON v.id=i.invocation_id
JOIN chat_runs r ON r.id=v.run_id
LEFT JOIN chat_messages m ON m.id=i.id AND m.thread_id=v.thread_id
WHERE i.invocation_id=:p1 AND NOT i.accepted
 AND (m.source_identity_id IS NULL OR chat_identity_allowed(v.agent_id,m.source_identity_id))
ORDER BY i.queue_order,i.sequence;
