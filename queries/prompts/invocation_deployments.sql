--: Record()

-- The deployments of the invocations behind a batch of inference calls.
--! run (invocation_ids) : Record
SELECT id AS invocation_id,deployment_id FROM chat_invocations WHERE id=ANY(:invocation_ids::UUID[]) AND deployment_id IS NOT NULL;
