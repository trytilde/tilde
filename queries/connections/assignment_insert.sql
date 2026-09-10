INSERT INTO connection_agents(connection_id,capability,agent_id) VALUES($1,$2,$3) ON CONFLICT DO NOTHING;
