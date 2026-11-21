--: Record()

--! run (p1, p2, p3) : Record
-- Exchange an enrollment token (digest p1) for a session (digest p2) and an egress token (digest
-- p3), once.
UPDATE sandboxes SET enrollment_hash=NULL,session_hash=:p2,egress_hash=:p3,connected_at=NOW() WHERE enrollment_hash=:p1 RETURNING id,blueprint_id;
