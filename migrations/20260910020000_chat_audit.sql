ALTER TABLE chat_participants ADD COLUMN active BOOLEAN NOT NULL DEFAULT TRUE;
ALTER TABLE chat_activity ADD COLUMN created_at TIMESTAMPTZ NOT NULL DEFAULT NOW();
-- Canonical, versioned protobuf event snapshot. Routing and lifecycle remain in typed tables.
ALTER TABLE chat_activity ADD COLUMN snapshot BYTEA;
ALTER TABLE chat_messages ADD COLUMN format TEXT NOT NULL DEFAULT 'text';
CREATE TABLE chat_attachments (
 id UUID PRIMARY KEY, thread_id UUID NOT NULL REFERENCES chat_threads(id),
 filename TEXT NOT NULL, media_type TEXT NOT NULL, size_bytes BIGINT NOT NULL CHECK(size_bytes >= 0),
 sha256 TEXT NOT NULL, content BYTEA NOT NULL,
 UNIQUE(thread_id,id)
);
CREATE TABLE chat_message_attachments (
 thread_id UUID NOT NULL, message_id UUID NOT NULL, attachment_id UUID NOT NULL UNIQUE,
 PRIMARY KEY(message_id,attachment_id),
 FOREIGN KEY(thread_id,message_id) REFERENCES chat_messages(thread_id,id),
 FOREIGN KEY(thread_id,attachment_id) REFERENCES chat_attachments(thread_id,id)
);
CREATE TABLE chat_converted_messages (
 agent_id UUID NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
 message_id UUID NOT NULL REFERENCES chat_messages(id) ON DELETE CASCADE,
 -- Explicitly client-opaque; Chat validates JSON but never interprets its fields.
 representation JSONB NOT NULL, PRIMARY KEY(agent_id,message_id)
);
CREATE TABLE chat_typing (
 thread_id UUID NOT NULL, participant_id UUID NOT NULL, expires_at TIMESTAMPTZ NOT NULL,
 PRIMARY KEY(thread_id,participant_id),
 FOREIGN KEY(thread_id,participant_id) REFERENCES chat_participants(thread_id,id)
);
CREATE TABLE chat_tool_calls (
 id UUID PRIMARY KEY, thread_id UUID NOT NULL REFERENCES chat_threads(id),
 invocation_id UUID NOT NULL REFERENCES chat_invocations(id), participant_id UUID NOT NULL REFERENCES chat_participants(id),
 name TEXT NOT NULL, provider_id TEXT NOT NULL, status TEXT NOT NULL CHECK(status IN ('running','completed','failed','aborted')),
 input_json TEXT NOT NULL, output_json TEXT NOT NULL DEFAULT '', error TEXT NOT NULL DEFAULT '',
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
