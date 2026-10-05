--! run (p1, p2, p3, p4, p5, p6, p7, p8)
-- Inserts nothing when the name is taken.
INSERT INTO sandbox_blueprints(id,name,connection_id,template,reuse,sleep_after_secs,terminate_after_secs,connect_timeout_secs)
VALUES(:p1,:p2,:p3,:p4,:p5,:p6,:p7,:p8) ON CONFLICT(name) DO NOTHING;
