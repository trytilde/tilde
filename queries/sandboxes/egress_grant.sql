--: Record()

--! run (p1, p2) : Record
-- The enabled grant `p2` of the blueprint of the sandbox whose egress token digest is p1, and
-- its connection.
SELECT s.id AS sandbox_id,g.connection_id,c.provider_id,c.type_id,g.config,g.config_version
FROM sandboxes s
JOIN sandbox_blueprint_egress g ON g.blueprint_id=s.blueprint_id AND g.slug=:p2 AND g.enabled
JOIN connections c ON c.id=g.connection_id AND c.status='ready' AND c.owner_user_id IS NULL
WHERE s.egress_hash=:p1;
