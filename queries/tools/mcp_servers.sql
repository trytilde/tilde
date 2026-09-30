--: Record()

--! run : Record
-- MCP servers added by URL (registered providers served over MCP), with each server URL.
SELECT DISTINCT p.provider_id, t.mcp_url AS url FROM connection_providers p
JOIN connection_types t ON t.provider_id=p.provider_id
WHERE p.kind<>'built_in' AND p.tool_host_id IS NULL AND t.mcp_url IS NOT NULL
ORDER BY p.provider_id, url;
