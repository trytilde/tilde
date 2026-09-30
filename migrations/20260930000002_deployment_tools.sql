-- The bundled tools a deployment's code declares (`tilde deploy`), fixed with the deployment
-- like its prompts and skills. Invocations still register what they actually run
-- (invocation_bundled_tools).
CREATE TABLE deployment_tools (
 deployment_id UUID NOT NULL REFERENCES agent_deployments(id) ON DELETE CASCADE,
 name TEXT NOT NULL CHECK (name <> '' AND length(name) <= 128),
 description TEXT NOT NULL CHECK (description <> '' AND length(description) <= 4096),
 summary TEXT NOT NULL CHECK (length(summary) <= 256),
 input_schema_json TEXT NOT NULL,
 output_schema_json TEXT NOT NULL,
 read_only BOOLEAN NOT NULL,
 destructive BOOLEAN NOT NULL,
 idempotent BOOLEAN NOT NULL,
 open_world BOOLEAN NOT NULL,
 display TEXT NOT NULL CHECK (display IN ('full','summary','hidden')),
 origin TEXT NOT NULL,
 PRIMARY KEY (deployment_id, name)
);
