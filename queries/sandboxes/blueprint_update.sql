--! run (p1, p2?, p3?, p4?, p5?, p6?, p7?, p8?)
-- Absent fields keep their value; a name another blueprint has updates nothing.
UPDATE sandbox_blueprints SET name=COALESCE(:p2,name),connection_id=COALESCE(:p3,connection_id),
  template=COALESCE(:p4,template),reuse=COALESCE(:p5,reuse),sleep_after_secs=COALESCE(:p6,sleep_after_secs),
  terminate_after_secs=COALESCE(:p7,terminate_after_secs),connect_timeout_secs=COALESCE(:p8,connect_timeout_secs)
WHERE id=:p1 AND NOT EXISTS (SELECT 1 FROM sandbox_blueprints o WHERE o.name=:p2 AND o.id<>:p1);
