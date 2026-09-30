--: Record(tool_host_id?)

--! run : Record
-- Per provider, the type a person can most easily set up comes first: signing in with a
-- dynamically registered client, then a key, then their own OAuth app, then a custom
-- setup page. Personal setup links use the first.
SELECT t.provider_id,t.type_id,p.tool_host_id,t.mcp_credential IS NOT NULL AS mcp FROM connection_types t
JOIN connection_providers p ON p.provider_id=t.provider_id
WHERE t.tool_capable
ORDER BY t.provider_id,
  CASE WHEN t.driver LIKE 'oauth%' AND t.oauth_client='dynamic' THEN 0
       WHEN t.driver='static' THEN 1 WHEN t.driver='custom' THEN 3 ELSE 2 END,
  t.type_id;
