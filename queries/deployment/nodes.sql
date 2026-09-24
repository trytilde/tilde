--: Record(connection_id?)

--! run (p1) : Record
SELECT instance_id,deployment_id,public_url,runtime_url,ready,agent_ready,agent_connected,connection_id,last_seen_at FROM agent_instances WHERE agent_id=:p1 ORDER BY instance_id;
