--: Record()

--! run (p1, p2) : Record
-- Take the sandbox's lease for p2 seconds unless another process holds it; `now` is the
-- database's clock, which connected_at is compared against.
UPDATE sandboxes SET lease_until=NOW()+make_interval(secs => :p2)
WHERE id=:p1 AND (lease_until IS NULL OR lease_until <= NOW()) RETURNING NOW() AS now;
