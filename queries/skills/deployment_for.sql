--: Record(deployment_id?)

-- The deployment whose bundled skills an invocation sees. Bundled skills belong to their agent: other
-- agents' listings never include them.
--! run (invocation_id) : Record
SELECT deployment_id FROM chat_invocations WHERE id=:invocation_id;
