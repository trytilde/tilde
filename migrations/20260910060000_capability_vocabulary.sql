-- Preserve existing grants while adopting the public permission vocabulary.
-- Invalid modes stay invalid; this migration must never broaden authority.
UPDATE agents
SET capabilities = (
    SELECT COALESCE(jsonb_object_agg(grant_entry.key,
        grant_entry.value || jsonb_build_object('mode', CASE
            WHEN grant_entry.key IN ('agents.create', 'thread.read', 'work.read', 'work.write', 'run.update')
                 AND grant_entry.value->>'mode' = 'none' THEN 'no'
            WHEN grant_entry.key IN ('agents.create', 'thread.read', 'work.read', 'work.write', 'run.update')
                 AND grant_entry.value->>'mode' = 'any' THEN 'yes'
            WHEN grant_entry.value->>'mode' = 'any' THEN 'all'
            WHEN grant_entry.value->>'mode' = 'only' THEN 'selected'
            ELSE grant_entry.value->>'mode'
        END)
    ), '{}'::jsonb)
    FROM jsonb_each(agents.capabilities) AS grant_entry
)
WHERE EXISTS (
    SELECT 1 FROM jsonb_each(agents.capabilities) AS grant_entry
    WHERE grant_entry.value->>'mode' IN ('any', 'only')
       OR (grant_entry.key IN ('agents.create', 'thread.read', 'work.read', 'work.write', 'run.update')
           AND grant_entry.value->>'mode' = 'none')
);
