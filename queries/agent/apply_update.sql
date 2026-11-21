--! run (key, name, description, concurrency_policy, tool_mode, capabilities, id)
UPDATE agents SET key = :key, name = :name, description = :description, concurrency_policy = :concurrency_policy,
  tool_mode = :tool_mode, capabilities = :capabilities, updated_at = NOW()
WHERE deleted_at IS NULL AND id = :id;
