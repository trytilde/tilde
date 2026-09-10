INSERT INTO connections(id,name,provider_id,type_id,status) VALUES($1,$2,$3,$4,'requires_action') ON CONFLICT(id) DO NOTHING;
