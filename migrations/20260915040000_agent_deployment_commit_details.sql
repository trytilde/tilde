-- CI-registered deployments carry what the git provider knows about the commit so the
-- Deployments tab can show the commit message, branch and author like a hosting console.
ALTER TABLE agent_deployments ADD COLUMN commit_message TEXT, ADD COLUMN branch TEXT, ADD COLUMN commit_author TEXT;
