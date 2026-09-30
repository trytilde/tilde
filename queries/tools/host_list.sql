--: Record(function_arn?, provider_id?)

-- p3 searches the name; p4 keeps hosts that publish a provider (true) or that do not (false).
--! run (p1?, p2, p3?, p4?) : Record
-- auth_methods: the names of the host's provider methods, empty when it needs no credentials.
SELECT h.id,h.name,h.execution_type,h.function_arn,p.provider_id,
  (h.execution_type='lambda' OR COALESCE(h.connected_at > NOW() - make_interval(secs => :p2), false)) AS available,
  COALESCE((SELECT array_agg(t.name ORDER BY t.name) FROM connection_types t WHERE t.provider_id=p.provider_id), '{}')::TEXT[] AS auth_methods
FROM tool_hosts h LEFT JOIN connection_providers p ON p.tool_host_id=h.id
WHERE (:p1::UUID IS NULL OR h.id=:p1) AND (:p3::TEXT IS NULL OR h.name ILIKE '%'||:p3||'%')
 AND (:p4::BOOLEAN IS NULL OR (p.provider_id IS NOT NULL)=:p4) ORDER BY h.name;
