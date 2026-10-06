--! run (p1)
-- Give up the lease without a transition.
UPDATE sandboxes SET lease_until=NULL WHERE id=:p1;
