-- Which of the kind's actions the caller holds on one resource.
--! run (kind, resource_id, actions, is_admin, caller_user?, caller_key?, caller_groups)
SELECT a.action FROM unnest(:actions::TEXT[]) AS a(action)
WHERE iam_held(:kind,:resource_id,ARRAY[a.action]::TEXT[],:is_admin,:caller_user,:caller_key,:caller_groups::TEXT[]);
