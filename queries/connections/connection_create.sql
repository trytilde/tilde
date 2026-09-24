--! run (p1, p2, p3, p4)
INSERT INTO connections(id,name,provider_id,type_id,status) VALUES(:p1,:p2,:p3,:p4,'requires_action') ON CONFLICT(id) DO NOTHING;
