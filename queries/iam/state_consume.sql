DELETE FROM iam_oidc_states WHERE state_hash=$1 AND browser_hash=$2 AND expires_at>NOW() RETURNING id,nonce,verifier;
