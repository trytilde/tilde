--! run (published_days)
DELETE FROM oidc_signing_keys WHERE created_at <= NOW()-make_interval(days=>:published_days::INTEGER)
AND id <> (SELECT id FROM oidc_signing_keys ORDER BY created_at DESC,id LIMIT 1);
