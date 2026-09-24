CREATE TABLE chat_users (id UUID PRIMARY KEY, name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 200));
CREATE TABLE chat_threads (id UUID PRIMARY KEY, title TEXT NOT NULL, primary_agent_id UUID NOT NULL REFERENCES agents(id));
CREATE TABLE chat_participants (
 id UUID PRIMARY KEY, thread_id UUID NOT NULL REFERENCES chat_threads(id) ON DELETE CASCADE,
 user_id UUID REFERENCES chat_users(id), agent_id UUID REFERENCES agents(id),
 CHECK ((user_id IS NULL) <> (agent_id IS NULL)), UNIQUE(thread_id,user_id), UNIQUE(thread_id,agent_id), UNIQUE(thread_id,id)
);
CREATE TABLE chat_messages (
 id UUID PRIMARY KEY, thread_id UUID NOT NULL REFERENCES chat_threads(id), participant_id UUID NOT NULL,
 text TEXT NOT NULL DEFAULT '', status TEXT NOT NULL CHECK(status IN ('streaming','complete','aborted')),
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), FOREIGN KEY(thread_id,participant_id) REFERENCES chat_participants(thread_id,id)
);
CREATE TABLE chat_message_targets (message_id UUID REFERENCES chat_messages(id) ON DELETE CASCADE, participant_id UUID REFERENCES chat_participants(id), PRIMARY KEY(message_id,participant_id));
CREATE TABLE chat_goals (
 id UUID PRIMARY KEY, thread_id UUID NOT NULL REFERENCES chat_threads(id), agent_id UUID NOT NULL REFERENCES agents(id),
 objective TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active','completed','failed','canceled')),
 UNIQUE(thread_id,agent_id,id)
);
CREATE TABLE chat_tasks (
 id UUID PRIMARY KEY, thread_id UUID NOT NULL REFERENCES chat_threads(id), agent_id UUID NOT NULL REFERENCES agents(id),
 goal_id UUID, title TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','working','blocked','completed','failed','canceled')),
 blocked_reason TEXT NOT NULL DEFAULT '', UNIQUE(thread_id,agent_id,id),
 FOREIGN KEY(thread_id,agent_id,goal_id) REFERENCES chat_goals(thread_id,agent_id,id)
);
CREATE TABLE chat_task_dependencies (
 thread_id UUID NOT NULL, agent_id UUID NOT NULL, task_id UUID NOT NULL, dependency_id UUID NOT NULL,
 PRIMARY KEY(task_id,dependency_id), CHECK(task_id <> dependency_id),
 FOREIGN KEY(thread_id,agent_id,task_id) REFERENCES chat_tasks(thread_id,agent_id,id),
 FOREIGN KEY(thread_id,agent_id,dependency_id) REFERENCES chat_tasks(thread_id,agent_id,id)
);
CREATE TABLE chat_runs (
 id UUID PRIMARY KEY, thread_id UUID NOT NULL REFERENCES chat_threads(id), agent_id UUID NOT NULL REFERENCES agents(id),
 goal_id UUID, objective TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active','waiting','completed','failed','canceled')),
 idempotency_key TEXT NOT NULL, UNIQUE(thread_id,agent_id,idempotency_key),
 FOREIGN KEY(thread_id,agent_id,goal_id) REFERENCES chat_goals(thread_id,agent_id,id)
);
CREATE TABLE chat_invocations (
 id UUID PRIMARY KEY, run_id UUID NOT NULL REFERENCES chat_runs(id), thread_id UUID NOT NULL REFERENCES chat_threads(id), agent_id UUID NOT NULL REFERENCES agents(id),
 status TEXT NOT NULL CHECK(status IN ('pending','running','stopped','failed','canceled')),
 capability_hash BYTEA, started_at TIMESTAMPTZ, ended_at TIMESTAMPTZ,
 lease_expires_at TIMESTAMPTZ,
 FOREIGN KEY(thread_id,agent_id) REFERENCES chat_participants(thread_id,agent_id)
);
CREATE UNIQUE INDEX chat_one_active_invocation ON chat_invocations(thread_id,agent_id) WHERE status IN ('pending','running');
CREATE TABLE chat_inputs (invocation_id UUID REFERENCES chat_invocations(id), id UUID NOT NULL, text TEXT NOT NULL, accepted BOOLEAN NOT NULL DEFAULT FALSE, PRIMARY KEY(invocation_id,id));
CREATE TABLE chat_activity (
 thread_id UUID REFERENCES chat_threads(id), sequence BIGINT NOT NULL, kind TEXT NOT NULL, entity_id UUID NOT NULL,
 text_delta TEXT NOT NULL DEFAULT '', PRIMARY KEY(thread_id,sequence)
);
-- Increment under the thread lock: cursor order follows commit order within each thread.
ALTER TABLE chat_threads ADD COLUMN activity_sequence BIGINT NOT NULL DEFAULT 0;

ALTER TABLE chat_messages ADD COLUMN invocation_id UUID REFERENCES chat_invocations(id);

CREATE TABLE chat_message_dispatch (message_id UUID REFERENCES chat_messages(id), agent_id UUID REFERENCES agents(id), PRIMARY KEY(message_id,agent_id));

ALTER TABLE chat_messages ADD COLUMN stream_expires_at TIMESTAMPTZ;

ALTER TABLE chat_messages ADD CONSTRAINT chat_message_thread_id UNIQUE(thread_id,id);
ALTER TABLE chat_messages ADD COLUMN in_reply_to_message_id UUID;
ALTER TABLE chat_messages ADD CONSTRAINT chat_message_reply FOREIGN KEY(thread_id,in_reply_to_message_id) REFERENCES chat_messages(thread_id,id);

CREATE UNIQUE INDEX chat_invocation_capability ON chat_invocations(capability_hash) WHERE capability_hash IS NOT NULL;
CREATE INDEX chat_pending_invocations ON chat_invocations(id) WHERE status='pending';
CREATE INDEX chat_expired_invocations ON chat_invocations(lease_expires_at) WHERE status='running';
CREATE INDEX chat_thread_messages ON chat_messages(thread_id,created_at,id);
