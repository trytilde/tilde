--: Record(tool_host_id?)

--! run (p1, p2?, p3) : Record
-- Ready tool connections owned by this user or by another identity under the same root, that
-- their owner lets the agent `p3` use: every agent `tools.personal` permits, or the listed ones.
SELECT c.id,c.name,c.provider_id,c.type_id,p.tool_host_id,t.mcp_credential IS NOT NULL AS mcp FROM connections c
JOIN connection_types t ON(t.provider_id=c.provider_id AND t.type_id=c.type_id)
JOIN connection_providers p ON p.provider_id=c.provider_id
JOIN chat_users o ON o.id=c.owner_user_id
WHERE t.tool_capable AND c.status='ready'
  AND (o.id=:p1 OR (o.root_identity_id IS NOT NULL AND o.root_identity_id=:p2)
  )
  AND (c.all_permitted_agents OR EXISTS(
    SELECT 1 FROM personal_connection_agents d WHERE d.connection_id=c.id AND d.agent_id=:p3))
ORDER BY c.provider_id,c.name,c.id;
