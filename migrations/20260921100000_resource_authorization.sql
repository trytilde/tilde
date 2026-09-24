-- Management authorization: a role is a named set of statements (an action on a resource, or
-- on every resource of a kind), and a principal (user, group or API key) holds roles. Each
-- kind declares its actions in the engine (iam/authz.rs); actions do not imply each other.
ALTER TABLE iam_users ADD COLUMN email TEXT, ADD COLUMN display_name TEXT;

-- The id prefix names the owner of the membership: system groups are managed by admins,
-- external groups are reconciled from the identity provider at login, local groups never are.
CREATE TABLE iam_groups (
 id TEXT PRIMARY KEY CHECK (length(id) BETWEEN 1 AND 200),
 name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
 source TEXT NOT NULL CHECK (source IN ('system','external','local')),
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 CHECK (starts_with(id, CASE source WHEN 'system' THEN 'tilde_system:' WHEN 'external' THEN 'external:' ELSE 'local:' END))
);
CREATE TABLE iam_group_members (
 group_id TEXT NOT NULL REFERENCES iam_groups(id) ON DELETE CASCADE,
 user_id UUID NOT NULL REFERENCES iam_users(id) ON DELETE CASCADE,
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 PRIMARY KEY (group_id, user_id)
);
CREATE INDEX iam_group_members_user ON iam_group_members(user_id);

-- Roles. Every agent gets its three system roles when it is created, identified as
-- agent/<id>/<name>; the installation-wide agents/<name> roles below cover every agent, present
-- and future, through the nil resource id, which iam_held matches for any target.
CREATE TABLE iam_roles (
 id TEXT PRIMARY KEY CHECK (length(id) BETWEEN 1 AND 200),
 name TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
 description TEXT NOT NULL CHECK (length(description) <= 500),
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE TABLE iam_role_statements (
 role_id TEXT NOT NULL REFERENCES iam_roles(id) ON DELETE CASCADE,
 resource_kind TEXT NOT NULL CHECK (resource_kind IN ('agent')),
 resource_id UUID NOT NULL,
 action TEXT NOT NULL CHECK (length(action) BETWEEN 1 AND 40),
 PRIMARY KEY (role_id, resource_kind, resource_id, action)
);
CREATE INDEX iam_role_statements_resource ON iam_role_statements(resource_kind, resource_id);
CREATE TABLE iam_role_members (
 role_id TEXT NOT NULL REFERENCES iam_roles(id) ON DELETE CASCADE,
 group_id TEXT REFERENCES iam_groups(id) ON DELETE CASCADE,
 user_id UUID REFERENCES iam_users(id) ON DELETE CASCADE,
 api_key_id UUID REFERENCES iam_api_keys(id) ON DELETE CASCADE,
 principal TEXT GENERATED ALWAYS AS (COALESCE('group:'||group_id,'user:'||user_id::text,'api_key:'||api_key_id::text)) STORED,
 granted_by_user_id UUID REFERENCES iam_users(id) ON DELETE SET NULL,
 created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
 PRIMARY KEY (role_id, principal),
 CHECK (num_nonnulls(group_id,user_id,api_key_id)=1)
);
CREATE INDEX iam_role_members_group ON iam_role_members(group_id) WHERE group_id IS NOT NULL;
CREATE INDEX iam_role_members_user ON iam_role_members(user_id) WHERE user_id IS NOT NULL;
CREATE INDEX iam_role_members_api_key ON iam_role_members(api_key_id) WHERE api_key_id IS NOT NULL;

-- Whether the caller holds any of the actions on the resource through some role; NULL
-- actions means any action, which no caller path uses for reads (visibility is 'view').
-- Point checks and list filters both call it.
CREATE FUNCTION iam_held(target_kind TEXT, target_id UUID, actions TEXT[], is_admin BOOLEAN, caller_user UUID, caller_key UUID, caller_groups TEXT[])
RETURNS BOOLEAN LANGUAGE SQL STABLE AS $$
 SELECT is_admin OR EXISTS(SELECT 1 FROM iam_role_statements s JOIN iam_role_members m ON m.role_id=s.role_id
  WHERE s.resource_kind=target_kind AND s.resource_id IN (target_id,'00000000-0000-0000-0000-000000000000')
   AND (actions IS NULL OR s.action=ANY(actions))
   AND (m.user_id=caller_user OR m.api_key_id=caller_key OR m.group_id=ANY(caller_groups)));
$$;

INSERT INTO iam_roles(id,name,description) VALUES
 ('agents/reader','Reader of all agents','Can view every agent, including ones created later.'),
 ('agents/editor','Editor of all agents','Can view, change and share every agent, including ones created later, and create agents.'),
 ('agents/deployer','Deployer of all agents','Can view and deploy every agent, including ones created later.');
INSERT INTO iam_role_statements(role_id,resource_kind,resource_id,action) VALUES
 ('agents/reader','agent','00000000-0000-0000-0000-000000000000','view'),
 ('agents/editor','agent','00000000-0000-0000-0000-000000000000','view'),
 ('agents/editor','agent','00000000-0000-0000-0000-000000000000','edit'),
 ('agents/editor','agent','00000000-0000-0000-0000-000000000000','share'),
 ('agents/deployer','agent','00000000-0000-0000-0000-000000000000','view'),
 ('agents/deployer','agent','00000000-0000-0000-0000-000000000000','deploy'),
 ('agents/deployer','agent','00000000-0000-0000-0000-000000000000','share');

INSERT INTO iam_groups(id,name,source) VALUES
 ('tilde_system:admin','Administrators','system'),
 ('tilde_system:user','All users','system');

-- Before this migration every user had full authority; keep it that way for them.
INSERT INTO iam_group_members(group_id,user_id) SELECT 'tilde_system:admin',id FROM iam_users;
