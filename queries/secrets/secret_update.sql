--! run (p1, p2?, p3)
-- Absent value keeps the sealed value.
UPDATE secrets SET encrypted_value=COALESCE(:p2,encrypted_value),description=:p3,updated_at=NOW() WHERE id=:p1;
