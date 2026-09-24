-- Message scheduling belongs to the API, scoped by thread and agent.
ALTER TABLE agents ADD COLUMN concurrency_policy TEXT NOT NULL DEFAULT 'queue'
 CHECK (concurrency_policy IN ('queue','interrupt','queue_and_batch'));
-- Retain durable queued inputs, but they are now dispatched by the API, not consumed by hosts.
ALTER TABLE chat_inputs ADD COLUMN sequence BIGINT GENERATED ALWAYS AS IDENTITY;
ALTER TABLE chat_inputs ADD COLUMN origin_invocation_id UUID REFERENCES chat_invocations(id);
UPDATE chat_inputs SET origin_invocation_id=invocation_id;
ALTER TABLE chat_inputs ALTER COLUMN origin_invocation_id SET NOT NULL;
ALTER TABLE chat_inputs DROP CONSTRAINT chat_inputs_pkey;
ALTER TABLE chat_inputs ADD PRIMARY KEY(origin_invocation_id,id);
ALTER TABLE chat_inputs ADD COLUMN history_through_message_id UUID REFERENCES chat_messages(id);
UPDATE chat_inputs i SET history_through_message_id=m.id FROM chat_messages m WHERE m.id=i.id;
ALTER TABLE chat_invocations ADD COLUMN history_through_message_id UUID REFERENCES chat_messages(id);
