--! run (p1, p2)
DELETE FROM connection_types WHERE provider_id=:p1 AND NOT(type_id=ANY(:p2));
