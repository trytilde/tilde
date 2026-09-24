-- A replaced static value invalidates cached credentials everywhere.
--! run (p1)
UPDATE connections SET credential_version=credential_version+1,updated_at=now() WHERE id=:p1;
