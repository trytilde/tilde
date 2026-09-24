--: Record(bound_connection_id?, external_id?, id?, name?, provider_id?, type_id?, account_label?)

--! run (p1, p2) : Record
-- Keep a row for native threads even when the agent has no assigned connections.
SELECT b.connection_id AS bound_connection_id, b.external_id AS external_id,
       c.id AS id, c.name AS name, c.provider_id AS provider_id,
       c.type_id AS type_id, c.account_label AS account_label
FROM chat_threads t
LEFT JOIN chat_channel_threads b ON b.thread_id = t.id
LEFT JOIN LATERAL (
    SELECT c.id, c.name, c.provider_id, c.type_id, c.account_label
    FROM connection_agents ca
    JOIN connections c ON c.id = ca.connection_id
    WHERE ca.agent_id = :p1 AND ca.capability = 'channel' AND c.status = 'ready'
) c ON TRUE
WHERE t.id = :p2
ORDER BY c.id;
