--! run (p1, p2)
-- The provider keeps the VM for another p2 seconds.
UPDATE sandboxes SET expires_at=NOW()+make_interval(secs => :p2) WHERE id=:p1;
