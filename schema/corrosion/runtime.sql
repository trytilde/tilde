-- Typed domain relations. Immutable events carry encrypted canonical Protobuf
-- snapshots, just as the Postgres audit does. No foreign/unique constraints are
-- assumed across peers: only the participant owner executes state transitions.
CREATE TABLE configuration (agent_id TEXT NOT NULL PRIMARY KEY, generation INTEGER NOT NULL DEFAULT 0, payload TEXT NOT NULL DEFAULT '');
CREATE TABLE threads (id TEXT NOT NULL PRIMARY KEY, title TEXT NOT NULL DEFAULT '', primary_agent_id TEXT NOT NULL DEFAULT '', last_activity_at INTEGER NOT NULL DEFAULT 0, payload TEXT NOT NULL DEFAULT '');
CREATE TABLE users (id TEXT NOT NULL PRIMARY KEY, name TEXT NOT NULL DEFAULT '');
CREATE TABLE participants (id TEXT NOT NULL PRIMARY KEY, thread_id TEXT NOT NULL DEFAULT '', agent_id TEXT NOT NULL DEFAULT '', user_id TEXT NOT NULL DEFAULT '', active INTEGER NOT NULL DEFAULT 1, payload TEXT NOT NULL DEFAULT '');
CREATE INDEX participants_thread ON participants(thread_id);
CREATE TABLE messages (id TEXT NOT NULL PRIMARY KEY, thread_id TEXT NOT NULL DEFAULT '', participant_id TEXT NOT NULL DEFAULT '', status TEXT NOT NULL DEFAULT 'complete', created_at INTEGER NOT NULL DEFAULT 0, payload TEXT NOT NULL DEFAULT '');
CREATE INDEX messages_thread ON messages(thread_id,created_at,id);
CREATE TABLE runs (id TEXT NOT NULL PRIMARY KEY, thread_id TEXT NOT NULL DEFAULT '', agent_id TEXT NOT NULL DEFAULT '', participant_id TEXT NOT NULL DEFAULT '', status TEXT NOT NULL DEFAULT 'active', source_identity_id TEXT NOT NULL DEFAULT '', channel_origin INTEGER NOT NULL DEFAULT 0, idempotency_key TEXT NOT NULL DEFAULT '', payload TEXT NOT NULL DEFAULT '');
CREATE INDEX runs_thread ON runs(thread_id);
CREATE TABLE invocations (id TEXT NOT NULL PRIMARY KEY, thread_id TEXT NOT NULL DEFAULT '', run_id TEXT NOT NULL DEFAULT '', agent_id TEXT NOT NULL DEFAULT '', participant_id TEXT NOT NULL DEFAULT '', owner_instance_id TEXT NOT NULL DEFAULT '', generation INTEGER NOT NULL DEFAULT 1, status TEXT NOT NULL DEFAULT 'pending', lease_expires_at INTEGER NOT NULL DEFAULT 0, payload TEXT NOT NULL DEFAULT '');
CREATE TABLE assignments (id TEXT NOT NULL PRIMARY KEY, thread_id TEXT NOT NULL DEFAULT '', participant_id TEXT NOT NULL DEFAULT '', agent_id TEXT NOT NULL DEFAULT '', owner_instance_id TEXT NOT NULL DEFAULT '', generation INTEGER NOT NULL DEFAULT 1, stopped INTEGER NOT NULL DEFAULT 0);
CREATE INDEX assignments_participant ON assignments(participant_id,generation);
CREATE TABLE commands (attempt_id TEXT NOT NULL PRIMARY KEY, id TEXT NOT NULL DEFAULT '', thread_id TEXT NOT NULL DEFAULT '', participant_id TEXT NOT NULL DEFAULT '', agent_id TEXT NOT NULL DEFAULT '', owner_instance_id TEXT NOT NULL DEFAULT '', generation INTEGER NOT NULL DEFAULT 1, kind TEXT NOT NULL DEFAULT '', created_at INTEGER NOT NULL DEFAULT 0, acked_at INTEGER, finished_at INTEGER, payload TEXT NOT NULL DEFAULT '', failure TEXT NOT NULL DEFAULT '');
CREATE INDEX commands_owner ON commands(owner_instance_id,acked_at);
CREATE TABLE events (id TEXT NOT NULL PRIMARY KEY, thread_id TEXT NOT NULL DEFAULT '', agent_id TEXT NOT NULL DEFAULT '', origin_instance_id TEXT NOT NULL DEFAULT '', origin_agent_id TEXT NOT NULL DEFAULT '', origin_sequence INTEGER NOT NULL DEFAULT 0, created_at INTEGER NOT NULL DEFAULT 0, kind TEXT NOT NULL DEFAULT '', payload TEXT NOT NULL DEFAULT '');
CREATE INDEX events_thread ON events(thread_id,origin_instance_id,origin_sequence);
CREATE TABLE goals (id TEXT NOT NULL PRIMARY KEY, thread_id TEXT NOT NULL DEFAULT '', agent_id TEXT NOT NULL DEFAULT '', status TEXT NOT NULL DEFAULT 'active', payload TEXT NOT NULL DEFAULT '');
CREATE TABLE tasks (id TEXT NOT NULL PRIMARY KEY, thread_id TEXT NOT NULL DEFAULT '', agent_id TEXT NOT NULL DEFAULT '', status TEXT NOT NULL DEFAULT 'pending', payload TEXT NOT NULL DEFAULT '');
CREATE TABLE tool_calls (id TEXT NOT NULL PRIMARY KEY, thread_id TEXT NOT NULL DEFAULT '', invocation_id TEXT NOT NULL DEFAULT '', status TEXT NOT NULL DEFAULT 'running', payload TEXT NOT NULL DEFAULT '');
CREATE TABLE attachments (id TEXT NOT NULL PRIMARY KEY, thread_id TEXT NOT NULL DEFAULT '', uploaded INTEGER NOT NULL DEFAULT 0, holder_instance_id TEXT NOT NULL DEFAULT '', source TEXT NOT NULL DEFAULT '', payload TEXT NOT NULL DEFAULT '');
CREATE TABLE converted_messages (id TEXT NOT NULL PRIMARY KEY, thread_id TEXT NOT NULL DEFAULT '', agent_id TEXT NOT NULL DEFAULT '', message_id TEXT NOT NULL DEFAULT '', representation TEXT NOT NULL DEFAULT '');
CREATE TABLE typing (id TEXT NOT NULL PRIMARY KEY, thread_id TEXT NOT NULL DEFAULT '', participant_id TEXT NOT NULL DEFAULT '', expires_at INTEGER NOT NULL DEFAULT 0);
CREATE TABLE channel_threads (id TEXT NOT NULL PRIMARY KEY, connection_id TEXT NOT NULL DEFAULT '', external_id TEXT NOT NULL DEFAULT '', thread_id TEXT NOT NULL DEFAULT '', agent_id TEXT NOT NULL DEFAULT '');
CREATE TABLE channel_receipts (id TEXT NOT NULL PRIMARY KEY, thread_id TEXT NOT NULL DEFAULT '', connection_id TEXT NOT NULL DEFAULT '', external_id TEXT NOT NULL DEFAULT '', message_id TEXT NOT NULL DEFAULT '');
CREATE TABLE traces (id TEXT NOT NULL PRIMARY KEY, thread_id TEXT NOT NULL DEFAULT '', invocation_id TEXT NOT NULL DEFAULT '', created_at INTEGER NOT NULL DEFAULT 0, payload TEXT NOT NULL DEFAULT '');
-- This relation stays empty: inserting NULL deliberately aborts a transaction
-- whose conversation has disappeared or entered retirement.
CREATE TABLE write_guards (id TEXT NOT NULL PRIMARY KEY, reason TEXT NOT NULL DEFAULT '');
CREATE TABLE retirements (epoch TEXT NOT NULL PRIMARY KEY, thread_id TEXT NOT NULL DEFAULT '', phase TEXT NOT NULL DEFAULT 'prepare');
CREATE TABLE retirement_acks (id TEXT NOT NULL PRIMARY KEY, thread_id TEXT NOT NULL DEFAULT '', epoch TEXT NOT NULL DEFAULT '', instance_id TEXT NOT NULL DEFAULT '', ready INTEGER NOT NULL DEFAULT 0, event_count INTEGER NOT NULL DEFAULT 0, max_sequence INTEGER NOT NULL DEFAULT 0);

CREATE TABLE message_dispatch (id TEXT NOT NULL PRIMARY KEY, thread_id TEXT NOT NULL DEFAULT '', message_id TEXT NOT NULL DEFAULT '', agent_id TEXT NOT NULL DEFAULT '');
CREATE TABLE health (id TEXT NOT NULL PRIMARY KEY, agent_id TEXT NOT NULL DEFAULT '', instance_id TEXT NOT NULL DEFAULT '', checked_at INTEGER NOT NULL DEFAULT 0, healthy INTEGER NOT NULL DEFAULT 0, latency_ms INTEGER NOT NULL DEFAULT 0);

CREATE TABLE bridge_message_versions (id TEXT NOT NULL PRIMARY KEY, message_id TEXT NOT NULL DEFAULT '', thread_id TEXT NOT NULL DEFAULT '', sequence INTEGER NOT NULL DEFAULT 0, payload TEXT NOT NULL DEFAULT '');
CREATE INDEX bridge_message_latest ON bridge_message_versions(message_id,sequence);

CREATE TABLE control_receipts (id TEXT NOT NULL PRIMARY KEY,thread_id TEXT NOT NULL DEFAULT '',invocation_id TEXT NOT NULL DEFAULT '',acknowledged_at INTEGER NOT NULL DEFAULT 0);

-- Immutable raw OTLP batches awaiting durable gateway log-queue acceptance.
CREATE TABLE logs (id TEXT NOT NULL PRIMARY KEY, created_at INTEGER NOT NULL DEFAULT 0, payload TEXT NOT NULL DEFAULT '');
