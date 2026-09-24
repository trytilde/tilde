--! run (p1, p2, p3, p4, p5, p6, p7)
INSERT INTO connection_setups(id,connection_id,step,action_id,connection_setup_token,connection_setup_token_hash,callback_token,callback_hash,expires_at) VALUES(:p1,:p2,'fields',:p3,:p4,:p5,:p6,:p7,now()+interval '10 minutes');
