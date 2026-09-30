--: Record(tool_host_id?)

--! run (p1) : Record
-- Agents may only use installation-owned connections whose type ships tools.
SELECT c.id,c.name,c.provider_id,c.type_id,c.status,p.tool_host_id,t.mcp_credential IS NOT NULL AS mcp FROM connections c
JOIN connection_types t ON(t.provider_id=c.provider_id AND t.type_id=c.type_id)
JOIN connection_providers p ON p.provider_id=c.provider_id
WHERE c.id=:p1 AND t.tool_capable AND c.owner_user_id IS NULL;
