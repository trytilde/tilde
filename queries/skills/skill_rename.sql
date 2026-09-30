-- Editor skills live at their name, so the directory follows. Zero rows when the name is taken.
--! run (id, name)
UPDATE skills SET name=:name, source_path=:name, updated_at=NOW()
WHERE id=:id AND NOT EXISTS(SELECT 1 FROM skills o WHERE o.source_id=skills.source_id AND o.name=:name AND o.id<>:id);
