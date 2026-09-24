-- An agent may name an inference connection; the alias replaces the slug in SDK calls.
ALTER TABLE connection_agents ADD COLUMN alias TEXT;
CREATE UNIQUE INDEX connection_agent_alias ON connection_agents(agent_id, alias) WHERE alias IS NOT NULL;
