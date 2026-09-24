--: Record()

--! run (p1, p2) : Record
SELECT phase,name,value FROM connection_oauth_parameters WHERE provider_id=:p1 AND type_id=:p2 ORDER BY phase,name;
