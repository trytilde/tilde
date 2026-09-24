--: Record(verified_at?, attested_at?, verification_id?, verification_status?)

--! run (p1, p2?, p3?, p4) : Record
SELECT i.id,i.connection_id,i.value,i.identity_type,i.verified_at,u.attested_at,COALESCE(a.allowed,FALSE) AS allowed,
 v.id AS verification_id, CASE WHEN v.status<>'approved' AND v.expires_at<=NOW() THEN 'expired' ELSE v.status END AS verification_status
FROM chat_channel_identities i JOIN chat_users u ON u.id=i.id
JOIN connection_agents ca ON ca.connection_id=i.connection_id AND ca.agent_id=:p1 AND ca.capability='channel'
LEFT JOIN chat_channel_identity_access a ON a.identity_id=i.id AND a.agent_id=ca.agent_id AND a.connection_id=i.connection_id
LEFT JOIN LATERAL (SELECT id,status,expires_at FROM chat_identity_verifications WHERE identity_id=i.id AND agent_id=:p1 ORDER BY created_at DESC LIMIT 1) v ON TRUE
WHERE (:p2::UUID IS NULL OR i.connection_id=:p2) AND (:p3::UUID IS NULL OR i.id>:p3) ORDER BY i.id LIMIT :p4;
