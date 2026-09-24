--! run (p1, p2, p3)
UPDATE connection_agents SET access_mode=:p3 WHERE connection_id=:p1 AND agent_id=:p2 AND capability='channel';
