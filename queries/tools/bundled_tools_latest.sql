--: Record(deployment_id?)

--! run (p1) : Record
SELECT i.id,i.started_at,i.deployment_id FROM chat_invocations i
WHERE i.agent_id=:p1 AND i.started_at IS NOT NULL AND EXISTS(SELECT 1 FROM invocation_bundled_tools b WHERE b.invocation_id=i.id)
ORDER BY i.started_at DESC,i.id DESC LIMIT 1;
