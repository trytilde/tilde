--: Record()

--! run (id, slug, name, kind, catalog_group?, repository_url?, git_ref?, git_path, agent_id?) : Record
INSERT INTO skill_sources(id,slug,name,kind,catalog_group,repository_url,git_ref,git_path,agent_id) VALUES(:id,:slug,:name,:kind,:catalog_group,:repository_url,:git_ref,:git_path,:agent_id)
ON CONFLICT DO NOTHING RETURNING id;
