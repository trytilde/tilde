--! run (p1, p2, p3, p4, p5)
INSERT INTO agent_instances(agent_id,instance_id,deployment_id,public_url,runtime_url) VALUES(:p1,:p2,:p3,:p4,:p5) ON CONFLICT(agent_id,instance_id) DO UPDATE SET deployment_id=EXCLUDED.deployment_id,public_url=EXCLUDED.public_url,runtime_url=EXCLUDED.runtime_url,last_seen_at=NOW(),connection_id=NULL,ready=false,agent_ready=false,agent_connected=false;
