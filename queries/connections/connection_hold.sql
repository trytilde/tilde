--! run (p1)
-- The connection's row, held until commit by a disconnect while it looks for uses: whatever would
-- put the connection to use meanwhile waits, then sees it disconnected (`connection_usable`).
SELECT id FROM connections WHERE id=:p1 FOR UPDATE;
