SELECT instance_id,public_url,runtime_url,ready,agent_ready,last_seen_at FROM sidecar_nodes WHERE agent_id=$1 ORDER BY instance_id;
