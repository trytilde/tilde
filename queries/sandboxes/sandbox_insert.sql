--: Record()

--! run (p1, p2, p3, p4, p5?, p6?, p7?, p8) : Record
-- Created leased by the caller, who launches it; `now` is the database's clock. A sandbox with
-- this key already existing inserts nothing.
INSERT INTO sandboxes(id,blueprint_id,reuse,connection_id,agent_id,thread_id,identity_id,lease_until)
VALUES(:p1,:p2,:p3,:p4,:p5,:p6,:p7,NOW()+make_interval(secs => :p8)) ON CONFLICT DO NOTHING RETURNING NOW() AS now;
