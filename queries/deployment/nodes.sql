SELECT instance_id,deployment_id,public_url,runtime_url,ready,agent_ready,last_seen_at FROM agent_instances WHERE agent_id=$1 ORDER BY instance_id;
