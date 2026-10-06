--: Record()

--! run (p1) : Record
SELECT name,encrypted_value FROM sandbox_blueprint_env WHERE blueprint_id=:p1 ORDER BY name;
