-- The relationship is the chat provider instance; credentials remain owned by Connections.
CREATE TABLE connection_agents (
 connection_id UUID NOT NULL REFERENCES connections(id) ON DELETE CASCADE,
 capability TEXT NOT NULL CHECK(capability IN ('channel')),
 agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
 PRIMARY KEY(connection_id, capability, agent_id)
);
-- Deliberately partial: do not impose chat's cardinality on future tool capabilities.
CREATE UNIQUE INDEX connection_one_chat_agent ON connection_agents(connection_id) WHERE capability='channel';
CREATE INDEX connection_agents_by_agent ON connection_agents(agent_id,connection_id);
