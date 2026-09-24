--: Record()

-- With no users yet, the one signing in administers the installation; later admins are
-- appointed or mapped from the provider, and emptying the group never re-arms this.
--! run (id, issuer, subject, email?, display_name?) : Record
WITH u AS (
 INSERT INTO iam_users(id,issuer,subject,email,display_name) VALUES(:id,:issuer,:subject,:email,:display_name)
 ON CONFLICT(issuer,subject) DO UPDATE SET email=COALESCE(EXCLUDED.email,iam_users.email), display_name=COALESCE(EXCLUDED.display_name,iam_users.display_name)
 RETURNING id
), first AS (
 INSERT INTO iam_group_members(group_id,user_id) SELECT 'tilde_system:admin',u.id FROM u
 WHERE NOT EXISTS(SELECT 1 FROM iam_users x WHERE x.id<>u.id)
 ON CONFLICT DO NOTHING
)
SELECT id FROM u;
