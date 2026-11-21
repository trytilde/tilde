-- Personal tool federation: the agents an owner lets use their personal connection. An agent
-- uses one only when its `tools.personal` grant covers the provider and the owner allows it:
-- every agent so permitted (`all_permitted_agents`, the default, which keeps every connection
-- made before this as it was) or only the agents listed here. Installation connections ignore both.
ALTER TABLE connections ADD COLUMN all_permitted_agents BOOLEAN NOT NULL DEFAULT TRUE;
CREATE TABLE personal_connection_agents (
    connection_id UUID NOT NULL REFERENCES connections(id) ON DELETE CASCADE,
    agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    PRIMARY KEY (connection_id, agent_id)
);
CREATE INDEX personal_connection_agents_agent ON personal_connection_agents(agent_id);
-- The personal connection a tool call ran on (its owner is the connection's, fixed at setup).
-- No foreign key: the audit record outlives the connection.
ALTER TABLE chat_tool_calls ADD COLUMN personal_connection_id UUID;
