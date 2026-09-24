--! run (p1, p2)
-- Projected membership inherits its existing lease owner's deployment, never a new routing choice.
UPDATE chat_participants p SET deployment_id=n.deployment_id FROM thread_leases l
JOIN agent_instances n ON n.agent_id=l.agent_id AND n.instance_id=l.instance_id
WHERE p.thread_id=:p1 AND p.agent_id=:p2 AND p.active AND p.deployment_id IS NULL
AND l.thread_id=p.thread_id AND l.agent_id=p.agent_id;
