--: Record()

--! run (p1, p2?, p3) : Record
SELECT provider_id,name,kind,categories FROM connection_providers WHERE provider_id>:p1 AND (:p2::TEXT IS NULL OR name ILIKE '%'||:p2||'%' OR provider_id ILIKE '%'||:p2||'%') ORDER BY provider_id LIMIT :p3;
