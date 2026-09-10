SELECT ca.agent_id FROM connection_agents ca JOIN connections c ON c.id=ca.connection_id WHERE ca.connection_id=$1 AND ca.capability='channel' AND c.status='ready';
