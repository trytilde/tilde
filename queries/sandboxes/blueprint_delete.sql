--! run (p1)
DELETE FROM sandbox_blueprints b WHERE id=:p1 AND NOT EXISTS (SELECT 1 FROM agent_sandboxes a WHERE a.blueprint_id=b.id);
