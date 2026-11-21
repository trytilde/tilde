--: Record(encrypted_value?, secret_id?, secret_value?)

--! run (p1) : Record
-- A variable's own sealed value, or the secret it references with that secret's sealed value.
SELECT e.name,e.encrypted_value,e.secret_id,s.encrypted_value AS secret_value
FROM sandbox_blueprint_env e LEFT JOIN secrets s ON s.id=e.secret_id
WHERE e.blueprint_id=:p1 ORDER BY e.name;
