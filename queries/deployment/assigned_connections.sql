SELECT c.id,a.access_mode FROM connections c JOIN connection_agents a ON a.connection_id=c.id WHERE a.agent_id=$1 AND a.capability='channel';
