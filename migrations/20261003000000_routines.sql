-- Signals are a fourth connection capability: the connection's provider turns some of its webhook
-- deliveries into typed events (`github.issue.opened`). Like tools they are not granted through
-- connection_agents; a routine naming the connection is the use.
ALTER TABLE connection_types ADD COLUMN signal_capable BOOLEAN NOT NULL DEFAULT false;

-- A routine is one trigger and a prompt for its agent: a UTC cron schedule, or one signal type of
-- one connection. Each firing starts a run in a new thread.
CREATE TABLE routines (
 id UUID PRIMARY KEY,
 agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
 name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 200),
 prompt TEXT NOT NULL CHECK (length(prompt) BETWEEN 1 AND 8000),
 enabled BOOLEAN NOT NULL,
 schedule TEXT,
 connection_id UUID REFERENCES connections(id) ON DELETE CASCADE,
 signal_type TEXT,
 -- Due time of an enabled cron routine; NULL otherwise.
 next_run_at TIMESTAMPTZ,
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 CONSTRAINT routine_trigger CHECK (
  (schedule IS NOT NULL AND connection_id IS NULL AND signal_type IS NULL)
  OR (schedule IS NULL AND connection_id IS NOT NULL AND signal_type IS NOT NULL)
 ),
 CONSTRAINT routine_next_run CHECK (next_run_at IS NULL OR (schedule IS NOT NULL AND enabled))
);
CREATE INDEX routines_by_agent ON routines(agent_id, created_at);
CREATE INDEX routines_due ON routines(next_run_at) WHERE next_run_at IS NOT NULL;
CREATE INDEX routines_by_signal ON routines(connection_id, signal_type) WHERE connection_id IS NOT NULL;

-- One row per firing. `fire_key` (the scheduled time, or the provider's delivery ID) makes a
-- redelivered webhook or a second replica's claim a no-op. The thread is the run's record.
CREATE TABLE routine_runs (
 id UUID PRIMARY KEY,
 routine_id UUID NOT NULL REFERENCES routines(id) ON DELETE CASCADE,
 fire_key TEXT NOT NULL,
 thread_id UUID REFERENCES chat_threads(id) ON DELETE SET NULL,
 error TEXT,
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 UNIQUE (routine_id, fire_key)
);
CREATE INDEX routine_runs_latest ON routine_runs(routine_id, created_at DESC);
