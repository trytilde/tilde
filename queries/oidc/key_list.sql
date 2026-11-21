--: Record()

-- Keys still published, newest (the signing key) first.
--! run (published_days) : Record
SELECT id,sealed,created_at FROM oidc_signing_keys
WHERE created_at > NOW()-make_interval(days=>:published_days::INTEGER) ORDER BY created_at DESC,id;
