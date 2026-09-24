--! run (id)
DELETE FROM iam_groups WHERE id=:id AND source<>'system';
