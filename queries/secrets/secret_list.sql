--: Record()

-- Secrets with the blueprints whose environment references them (IDs and names, in step).
--! run : Record
SELECT s.id,s.name,s.description,s.created_at,s.updated_at,
  COALESCE((SELECT array_agg(b.id ORDER BY b.name) FROM sandbox_blueprints b
    WHERE EXISTS (SELECT 1 FROM sandbox_blueprint_env e WHERE e.blueprint_id=b.id AND e.secret_id=s.id)),'{}')::UUID[] AS blueprint_ids,
  COALESCE((SELECT array_agg(b.name ORDER BY b.name) FROM sandbox_blueprints b
    WHERE EXISTS (SELECT 1 FROM sandbox_blueprint_env e WHERE e.blueprint_id=b.id AND e.secret_id=s.id)),'{}')::TEXT[] AS blueprints
FROM secrets s
WHERE true
ORDER BY s.name;
