--! run (p1, p2, p3)
-- End a transition: the new status (p2) and error (p3), releasing the lease.
UPDATE sandboxes SET status=:p2,error=:p3,lease_until=NULL,last_used_at=CASE WHEN :p2='running' THEN NOW() ELSE last_used_at END WHERE id=:p1;
