--! run (p1, p2)
DELETE FROM sandbox_blueprint_env WHERE blueprint_id=:p1 AND name=:p2;
