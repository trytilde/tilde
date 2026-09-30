--: Record(tool_host_id?)

--! run (p1, p2?) : Record
-- Ready tool connections owned by this user or by another identity under the same root.
SELECT c.id,c.name,c.provider_id,c.type_id,p.tool_host_id,t.mcp_credential IS NOT NULL AS mcp FROM connections c
JOIN connection_types t ON(t.provider_id=c.provider_id AND t.type_id=c.type_id)
JOIN connection_providers p ON p.provider_id=c.provider_id
JOIN chat_users o ON o.id=c.owner_user_id
WHERE t.tool_capable AND c.status='ready'
  AND (o.id=:p1 OR (o.root_identity_id IS NOT NULL AND o.root_identity_id=:p2))
ORDER BY c.provider_id,c.name,c.id;
