--: Record()

--! run (p1) : Record
-- The egress grants of the blueprints p1.
SELECT blueprint_id,slug,connection_id,config,config_version,enabled FROM sandbox_blueprint_egress
WHERE blueprint_id=ANY(:p1) ORDER BY blueprint_id,slug;
