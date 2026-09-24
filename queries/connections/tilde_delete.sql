--! run (p1)
DELETE FROM connections WHERE id=:p1 AND provider_id='tilde';
