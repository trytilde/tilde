--: Record()

--! run (p1) : Record
-- Blueprints launch through installation-owned connections of a sandbox provider.
SELECT c.id,c.provider_id,c.status FROM connections c WHERE c.id=:p1 AND c.owner_user_id IS NULL;
