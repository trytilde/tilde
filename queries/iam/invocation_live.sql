--: Record()

--! run (p1, p2, p3, p4) : Record
SELECT p.id AS participant_id, a.capabilities AS capabilities
FROM chat_invocations i
JOIN agents a ON a.id = i.agent_id
JOIN chat_runs r ON r.id = i.run_id
JOIN chat_participants p ON p.thread_id = i.thread_id AND p.agent_id = i.agent_id
WHERE i.id = :p1
  AND i.agent_id = :p2
  AND i.thread_id = :p3
  AND i.run_id = :p4
  AND i.status = 'running'
  AND r.status IN ('active', 'suspending')
  AND i.lease_expires_at > NOW()
  AND chat_channel_run_allowed(i.agent_id, i.thread_id, r.source_identity_id, r.channel_origin)
  AND NOT a.paused
  AND a.deleted_at IS NULL
  AND p.active;
