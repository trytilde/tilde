ALTER TABLE agents ADD COLUMN capabilities JSONB NOT NULL DEFAULT '{}' CHECK (jsonb_typeof(capabilities) = 'object');
CREATE TABLE iam_signing_key (id BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK(id), sealed BYTEA NOT NULL);
ALTER TABLE chat_invocations DROP COLUMN capability_hash;
