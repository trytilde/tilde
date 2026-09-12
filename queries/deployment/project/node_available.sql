SELECT agent_id,instance_id,agent_ingress_url,public_ingress_url,runtime_url FROM sidecar_nodes WHERE agent_id=$1 AND last_seen_at>NOW()-INTERVAL '30 seconds' ORDER BY ready DESC,last_seen_at DESC;
