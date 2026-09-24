--: Record()

--! run (p1, p2) : Record
-- Identity budgets target the sender's root identity.
SELECT a.connection_id FROM connection_agents a JOIN connections c ON c.id=a.connection_id
WHERE a.agent_id=:p1 AND a.capability='inference' AND c.status='ready'
 AND NOT EXISTS(SELECT 1 FROM inference_budgets b WHERE b.action='block' AND b.exhausted_until>NOW()
  AND ((b.scope='agent' AND b.scope_id=:p1 AND (b.connection_id IS NULL OR b.connection_id=a.connection_id))
    OR (b.scope='identity' AND b.scope_id IN (SELECT u.root_identity_id FROM chat_runs cr JOIN chat_users u ON u.id=cr.source_identity_id WHERE cr.id=:p2 AND u.root_identity_id IS NOT NULL))))
ORDER BY a.connection_id;
