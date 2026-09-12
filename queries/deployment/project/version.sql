INSERT INTO sidecar_projection_versions(agent_id,entity_type,entity_id,origin_instance_id,origin_sequence,created_at)
VALUES($1,$2,$3,$4,$5,$6)
ON CONFLICT(agent_id,entity_type,entity_id) DO UPDATE SET origin_instance_id=EXCLUDED.origin_instance_id,origin_sequence=EXCLUDED.origin_sequence,created_at=EXCLUDED.created_at
WHERE (sidecar_projection_versions.origin_instance_id=EXCLUDED.origin_instance_id AND sidecar_projection_versions.origin_sequence<EXCLUDED.origin_sequence)
 OR (sidecar_projection_versions.origin_instance_id<>EXCLUDED.origin_instance_id AND (sidecar_projection_versions.created_at,sidecar_projection_versions.origin_instance_id)<(EXCLUDED.created_at,EXCLUDED.origin_instance_id))
RETURNING entity_id;
