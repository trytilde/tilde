--: Record()

-- Installation-owned connections named by a slug (`provider/name`), ready ones first.
--! run (p1, p2) : Record
SELECT c.id,c.name,c.provider_id,c.type_id,c.status FROM connections c
WHERE c.provider_id=:p1 AND c.name=:p2 AND c.owner_user_id IS NULL
ORDER BY c.status='ready' DESC,c.created_at DESC;
