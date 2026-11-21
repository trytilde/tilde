-- Adds a signing key unless one was added within `fresh_days` (0 always adds). Two replicas
-- rotating at once may both add one; the newer signs and both stay published.
--! run (id, sealed, fresh_days)
INSERT INTO oidc_signing_keys(id,sealed)
SELECT :id,:sealed
WHERE NOT EXISTS (SELECT 1 FROM oidc_signing_keys WHERE created_at > NOW()-make_interval(days=>:fresh_days::INTEGER));
