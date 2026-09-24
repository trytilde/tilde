--: Record()

-- Live agents created before every agent owned a Tilde connection and its system roles.
--! run : Record
SELECT a.id,a.name FROM agents a WHERE a.deleted_at IS NULL AND (NOT EXISTS(
 SELECT 1 FROM connection_agents ca JOIN connections c ON c.id=ca.connection_id
 WHERE ca.agent_id=a.id AND ca.capability='channel' AND c.provider_id='tilde')
 OR NOT EXISTS(SELECT 1 FROM iam_role_statements s WHERE s.resource_kind='agent' AND s.resource_id=a.id))
ORDER BY a.created_at,a.id;
