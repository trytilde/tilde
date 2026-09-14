SELECT i.id,i.connection_id,i.value,i.identity_type,i.verified_at,COALESCE(a.allowed,FALSE) AS "allowed!",
 v.id AS "verification_id?", CASE WHEN v.status<>'approved' AND v.expires_at<=NOW() THEN 'expired' ELSE v.status END AS verification_status
FROM chat_channel_identities i
JOIN connection_agents ca ON ca.connection_id=i.connection_id AND ca.agent_id=$1 AND ca.capability='channel'
LEFT JOIN chat_channel_identity_access a ON a.identity_id=i.id AND a.agent_id=ca.agent_id AND a.connection_id=i.connection_id
LEFT JOIN LATERAL (SELECT id,status,expires_at FROM chat_identity_verifications WHERE identity_id=i.id AND agent_id=$1 ORDER BY created_at DESC LIMIT 1) v ON TRUE
WHERE ($2::UUID IS NULL OR i.connection_id=$2) AND ($3::UUID IS NULL OR i.id>$3) ORDER BY i.id LIMIT $4;
