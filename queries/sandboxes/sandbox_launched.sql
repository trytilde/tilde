--! run (p1, p2, p3)
-- A VM was launched or woken (p2); the provider ends or pauses it after p3 seconds unless renewed.
UPDATE sandboxes SET provider_sandbox_id=:p2,status='starting',error='',last_used_at=NOW(),
  expires_at=NOW()+make_interval(secs => :p3) WHERE id=:p1;
