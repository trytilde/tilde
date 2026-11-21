-- Local agents: agents that run in a harness on a person's machine (Claude Code, Codex, the
-- Claude and ChatGPT desktop apps). Tilde never runs one, so it has no deployments: the
-- harness reports its sessions and is given the agent's tools and skills. An agent with a row
-- here is local; the harness is fixed when it is registered.
CREATE TABLE local_agents (
    agent_id UUID PRIMARY KEY REFERENCES agents(id) ON DELETE CASCADE,
    harness TEXT NOT NULL CHECK (harness IN ('claude_code', 'codex', 'claude_desktop', 'chatgpt_desktop'))
);
