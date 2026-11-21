--! run (p1, p2)
DELETE FROM sandbox_blueprint_egress WHERE blueprint_id=:p1 AND slug=:p2;
