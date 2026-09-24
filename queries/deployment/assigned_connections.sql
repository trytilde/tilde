--: Record(alias?)

--! run (p1) : Record
SELECT c.id,a.capability,a.access_mode,a.alias FROM connections c JOIN connection_agents a ON a.connection_id=c.id WHERE a.agent_id=:p1 ORDER BY a.capability,c.id;
