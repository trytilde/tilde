CREATE TABLE chat_message_deliveries (
 message_id UUID PRIMARY KEY REFERENCES chat_messages(id), connection_id UUID NOT NULL REFERENCES connections(id),
 destination TEXT NOT NULL, external_message_id TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'accepted'
);
CREATE TABLE chat_channel_threads (
 thread_id UUID PRIMARY KEY REFERENCES chat_threads(id), connection_id UUID NOT NULL REFERENCES connections(id),
 external_id TEXT NOT NULL, agent_id UUID NOT NULL REFERENCES agents(id), UNIQUE(connection_id,external_id,agent_id)
);
CREATE TABLE chat_channel_receipts (
 connection_id UUID NOT NULL REFERENCES connections(id), external_id TEXT NOT NULL,
 message_id UUID REFERENCES chat_messages(id), PRIMARY KEY(connection_id,external_id)
);

ALTER TABLE chat_attachments ALTER COLUMN content DROP NOT NULL;
ALTER TABLE chat_attachments ADD COLUMN connection_id UUID REFERENCES connections(id);
ALTER TABLE chat_attachments ADD COLUMN provider_attachment_id TEXT;
ALTER TABLE chat_attachments ADD COLUMN source_url BYTEA;
ALTER TABLE chat_attachments ADD CONSTRAINT attachment_source CHECK(content IS NOT NULL OR (connection_id IS NOT NULL AND (provider_attachment_id IS NOT NULL OR source_url IS NOT NULL)));

ALTER TABLE chat_messages DROP CONSTRAINT chat_messages_status_check;
ALTER TABLE chat_messages ADD CONSTRAINT chat_messages_status_check CHECK(status IN ('streaming','complete','aborted','deleted'));

ALTER TABLE chat_messages ADD COLUMN subject TEXT;

CREATE INDEX chat_completed_message_snapshot ON chat_activity(thread_id,entity_id,sequence DESC) WHERE kind='message.completed';
