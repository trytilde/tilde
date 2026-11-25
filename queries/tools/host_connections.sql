--! run (p1)
-- The instances of a tool host (the connections of the provider it published), held until commit
-- while its deletion looks for their uses: what would put one to use meanwhile waits.
SELECT c.id FROM connections c JOIN connection_providers p ON p.provider_id=c.provider_id
WHERE p.tool_host_id=:p1 ORDER BY c.id FOR UPDATE OF c;
