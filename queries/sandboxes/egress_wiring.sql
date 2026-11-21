--: Record()

--! run (p1) : Record
-- The enabled grants of blueprint p1 with their connections' providers, for a connecting sandbox.
SELECT g.slug,c.provider_id FROM sandbox_blueprint_egress g
JOIN connections c ON c.id=g.connection_id
WHERE g.blueprint_id=:p1 AND g.enabled ORDER BY g.slug;
