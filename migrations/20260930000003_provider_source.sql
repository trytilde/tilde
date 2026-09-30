-- Where a provider comes from, as the Tools pages group it: 'tool_host' when a tool host
-- published it, 'mcp_server' for a registered provider whose every method is served over MCP
-- (an MCP server added by URL), otherwise 'catalog' (built in, curated MCP servers included).
CREATE FUNCTION provider_source(provider TEXT) RETURNS TEXT LANGUAGE sql STABLE AS $$
 SELECT CASE
  WHEN p.tool_host_id IS NOT NULL THEN 'tool_host'
  WHEN p.kind<>'built_in'
   AND EXISTS(SELECT 1 FROM connection_types t WHERE t.provider_id=p.provider_id)
   AND NOT EXISTS(SELECT 1 FROM connection_types t WHERE t.provider_id=p.provider_id AND t.mcp_url IS NULL)
  THEN 'mcp_server'
  ELSE 'catalog' END
 FROM connection_providers p WHERE p.provider_id=provider
$$;
