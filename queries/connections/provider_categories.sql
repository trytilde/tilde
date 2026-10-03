--: Record()

-- The categories a provider list's category menu offers: provider_list.sql's p4 and p6 filters.
--! run (p4?, p6?) : Record
SELECT DISTINCT c AS category FROM connection_providers p, unnest(p.categories) c WHERE
 (:p4::TEXT IS NULL OR EXISTS(SELECT 1 FROM connection_types t WHERE t.provider_id=p.provider_id
  AND CASE :p4 WHEN 'channel' THEN t.channel_capable WHEN 'inference' THEN t.inference_capable WHEN 'tool' THEN t.tool_capable WHEN 'signal' THEN t.signal_capable END))
 AND (:p6::TEXT IS NULL OR provider_source(p.provider_id)=:p6)
 ORDER BY category;
