--: Record()

-- Filters apply before the keyset page; provider_categories.sql repeats the p4 and p6 filters.
-- p4: a capability one of the provider's methods offers. p6: its provider_source ('catalog',
-- 'mcp_server' or 'tool_host').
--! run (p1, p2?, p3, p4?, p5?, p6?) : Record
SELECT provider_id,name,kind,categories FROM connection_providers p WHERE provider_id>:p1
 AND (:p2::TEXT IS NULL OR name ILIKE '%'||:p2||'%' OR provider_id ILIKE '%'||:p2||'%' OR instructions ILIKE '%'||:p2||'%')
 AND (:p5::TEXT IS NULL OR :p5=ANY(categories))
 AND (:p4::TEXT IS NULL OR EXISTS(SELECT 1 FROM connection_types t WHERE t.provider_id=p.provider_id
  AND CASE :p4 WHEN 'channel' THEN t.channel_capable WHEN 'inference' THEN t.inference_capable WHEN 'tool' THEN t.tool_capable WHEN 'signal' THEN t.signal_capable END))
 AND (:p6::TEXT IS NULL OR provider_source(p.provider_id)=:p6)
 ORDER BY provider_id LIMIT :p3;
