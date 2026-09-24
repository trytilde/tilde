-- Login reconciles only provider-owned memberships; local and system groups survive.
--! run (user_id, ids)
DELETE FROM iam_group_members WHERE user_id=:user_id AND starts_with(group_id,'external:') AND NOT group_id=ANY(:ids::TEXT[]);
