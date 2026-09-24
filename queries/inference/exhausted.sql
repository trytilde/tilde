--: Record(connection_id?)

--! run : Record
-- Identity budgets expand to every identity under the root, so sidecars can match a run's sender directly.
SELECT b.scope,COALESCE(u.id,b.scope_id) AS scope_id,b.connection_id FROM inference_budgets b
LEFT JOIN chat_users u ON b.scope='identity' AND u.root_identity_id=b.scope_id
WHERE b.action='block' AND b.exhausted_until>NOW() ORDER BY b.scope,scope_id,b.connection_id;
