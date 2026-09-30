-- Prompt histories can no longer be deleted, so the agents.edit_prompts capability that only
-- guarded deletion is retired.
UPDATE agents SET capabilities = capabilities - 'agents.edit_prompts' WHERE capabilities ? 'agents.edit_prompts';
