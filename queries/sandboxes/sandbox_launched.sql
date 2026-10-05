--! run (p1, p2)
-- A VM was launched or woken (p2).
UPDATE sandboxes SET provider_sandbox_id=:p2,status='starting',error='',last_used_at=NOW() WHERE id=:p1;
