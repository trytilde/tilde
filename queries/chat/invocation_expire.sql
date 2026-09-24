--: Record()

--! run : Record
-- Gateway-executed invocations only: a sidecar invocation's liveness is its lease holder's heartbeat, reclaimed by recovery.
UPDATE chat_invocations i SET status='failed',ended_at=NOW() FROM agents a WHERE a.id=i.agent_id AND EXISTS(SELECT 1 FROM agent_deployments d WHERE d.id=i.deployment_id AND d.target IN ('gateway','lambda')) AND i.status='running' AND i.lease_expires_at<NOW() RETURNING i.id,i.run_id,i.thread_id;
