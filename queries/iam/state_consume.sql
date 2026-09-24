--: Record()

--! run (p1, p2) : Record
DELETE FROM iam_oidc_states WHERE state_hash=:p1 AND browser_hash=:p2 AND expires_at>NOW() RETURNING id,nonce,verifier;
