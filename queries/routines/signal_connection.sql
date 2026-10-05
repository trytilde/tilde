--: Record()

-- A connection routines may trigger on: its type emits signals and it is installation-owned. A
-- personal connection belongs to one chat user and never triggers an agent's routines.
--! run (id) : Record
SELECT c.provider_id,c.type_id FROM connections c
JOIN connection_types t ON t.provider_id=c.provider_id AND t.type_id=c.type_id
WHERE c.id=:id AND t.signal_capable AND c.owner_user_id IS NULL;
