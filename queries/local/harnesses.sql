--: Record()

--! run (agents) : Record
SELECT agent_id, harness FROM local_agents WHERE agent_id = ANY(:agents);
