UPDATE agents SET paused=$2, generation=generation + CASE WHEN paused <> $2 THEN 1 ELSE 0 END,
updated_at=NOW() WHERE id=$1 AND deleted_at IS NULL
RETURNING generation, endpoint_url, webhook_signing_key;
