--: Record()

--! run (p1, p2, p3) : Record
SELECT c.id FROM connections c
JOIN connection_agents other ON other.connection_id=c.id
JOIN connection_agents mine ON mine.agent_id=other.agent_id AND mine.capability=other.capability
WHERE c.provider_id=:p1 AND c.name=:p2 AND c.id<>:p3 AND mine.connection_id=:p3 AND c.status='ready' LIMIT 1;
