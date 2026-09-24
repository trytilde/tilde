-- The agent's sending identity is distinct from recipient identities and connection labels.
-- It belongs to the assigned channel; reassignment removes the old agent's identity.
CREATE TABLE agent_channel_identities (
 connection_id UUID NOT NULL,
 agent_id UUID NOT NULL,
 capability TEXT NOT NULL DEFAULT 'channel' CHECK (capability = 'channel'),
 identity_type TEXT NOT NULL CHECK (identity_type IN ('email','phone_number','username')),
 value TEXT NOT NULL CHECK (length(value) BETWEEN 1 AND 320),
 PRIMARY KEY (connection_id, agent_id),
 FOREIGN KEY (connection_id, capability, agent_id)
  REFERENCES connection_agents(connection_id, capability, agent_id) ON DELETE CASCADE
);
