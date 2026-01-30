-- ═══════════════════════════════════════════════════════════════════════════════
-- Project Apex - RBAC & Multi-Tenancy Schema
-- Migration: 20240101000004_rbac_tables.sql
-- Description: Creates tables for role-based access control and tenant isolation
-- ═══════════════════════════════════════════════════════════════════════════════

-- ═══════════════════════════════════════════════════════════════════════════════
-- CUSTOM TYPES
-- ═══════════════════════════════════════════════════════════════════════════════

CREATE TYPE organization_status AS ENUM ('active', 'suspended', 'deactivated');
CREATE TYPE member_role AS ENUM ('owner', 'admin', 'member');

-- ═══════════════════════════════════════════════════════════════════════════════
-- ORGANIZATIONS (tenants)
-- ═══════════════════════════════════════════════════════════════════════════════

CREATE TABLE organizations (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name        TEXT        NOT NULL,
    slug        TEXT        NOT NULL UNIQUE,
    status      organization_status NOT NULL DEFAULT 'active',
    owner_id    UUID        NOT NULL,           -- references users.id once created
    settings    JSONB       NOT NULL DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

COMMENT ON TABLE organizations IS 'Tenant organizations; every resource is scoped to an organization';

CREATE INDEX idx_organizations_slug   ON organizations (slug);
CREATE INDEX idx_organizations_owner  ON organizations (owner_id);
CREATE INDEX idx_organizations_status ON organizations (status);

-- ═══════════════════════════════════════════════════════════════════════════════
-- USERS
-- ═══════════════════════════════════════════════════════════════════════════════

CREATE TABLE users (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    email           TEXT        NOT NULL UNIQUE,
    name            TEXT,
    password_hash   TEXT,                       -- NULL for SSO-only users
    is_active       BOOLEAN     NOT NULL DEFAULT TRUE,
    last_login_at   TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

COMMENT ON TABLE users IS 'Application users who interact with Apex';

CREATE INDEX idx_users_email     ON users (email);
CREATE INDEX idx_users_is_active ON users (is_active);

-- ═══════════════════════════════════════════════════════════════════════════════
-- ORGANIZATION MEMBERS
-- ═══════════════════════════════════════════════════════════════════════════════

CREATE TABLE organization_members (
    user_id         UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    organization_id UUID        NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    role            member_role NOT NULL DEFAULT 'member',
    joined_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    PRIMARY KEY (user_id, organization_id)
);

COMMENT ON TABLE organization_members IS 'Maps users to organizations with a membership role';

CREATE INDEX idx_org_members_org  ON organization_members (organization_id);
CREATE INDEX idx_org_members_user ON organization_members (user_id);

-- ═══════════════════════════════════════════════════════════════════════════════
-- ROLES
-- ═══════════════════════════════════════════════════════════════════════════════

CREATE TABLE roles (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name            TEXT        NOT NULL,
    description     TEXT        NOT NULL DEFAULT '',
    is_system       BOOLEAN     NOT NULL DEFAULT FALSE,
    organization_id UUID        REFERENCES organizations(id) ON DELETE CASCADE,  -- NULL = global role
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- A role name must be unique within its scope (global or per-org).
    UNIQUE (name, organization_id)
);

COMMENT ON TABLE roles IS 'RBAC roles that group permissions; system roles cannot be deleted';

CREATE INDEX idx_roles_org       ON roles (organization_id);
CREATE INDEX idx_roles_is_system ON roles (is_system);

-- ═══════════════════════════════════════════════════════════════════════════════
-- PERMISSIONS
-- ═══════════════════════════════════════════════════════════════════════════════

CREATE TABLE permissions (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    resource    TEXT NOT NULL,                  -- e.g., 'swarm', 'agent', 'task'
    action      TEXT NOT NULL,                  -- e.g., 'create', 'read', 'delete', 'manage'
    description TEXT NOT NULL DEFAULT '',

    UNIQUE (resource, action)
);

COMMENT ON TABLE permissions IS 'Individual permissions representing resource:action pairs';

-- Seed the defined permissions
INSERT INTO permissions (resource, action, description) VALUES
    ('swarm',    'create',  'Create new agent swarms'),
    ('swarm',    'read',    'View swarm details and status'),
    ('swarm',    'delete',  'Delete existing swarms'),
    ('agent',    'manage',  'Manage agents (create, update, remove)'),
    ('agent',    'read',    'View agent details and status'),
    ('task',     'submit',  'Submit new tasks to the system'),
    ('task',     'read',    'View task details and results'),
    ('approval', 'approve', 'Approve pending human-in-the-loop actions'),
    ('approval', 'read',    'View pending approval requests'),
    ('settings', 'manage',  'Manage organization and system settings'),
    ('settings', 'read',    'View organization and system settings');

-- ═══════════════════════════════════════════════════════════════════════════════
-- ROLE-PERMISSION MAPPING
-- ═══════════════════════════════════════════════════════════════════════════════

CREATE TABLE role_permissions (
    role_id       UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    permission_id UUID NOT NULL REFERENCES permissions(id) ON DELETE CASCADE,

    PRIMARY KEY (role_id, permission_id)
);

COMMENT ON TABLE role_permissions IS 'Maps roles to their granted permissions';

CREATE INDEX idx_role_perms_role ON role_permissions (role_id);
CREATE INDEX idx_role_perms_perm ON role_permissions (permission_id);

-- ═══════════════════════════════════════════════════════════════════════════════
-- USER-ROLE BINDINGS (per organization)
-- ═══════════════════════════════════════════════════════════════════════════════

CREATE TABLE user_roles (
    user_id         UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_id         UUID        NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    organization_id UUID        NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    granted_by      UUID        REFERENCES users(id) ON DELETE SET NULL,
    expires_at      TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    PRIMARY KEY (user_id, role_id, organization_id)
);

COMMENT ON TABLE user_roles IS 'Binds users to roles scoped to a specific organization';

CREATE INDEX idx_user_roles_user ON user_roles (user_id);
CREATE INDEX idx_user_roles_role ON user_roles (role_id);
CREATE INDEX idx_user_roles_org  ON user_roles (organization_id);
CREATE INDEX idx_user_roles_exp  ON user_roles (expires_at) WHERE expires_at IS NOT NULL;

-- ═══════════════════════════════════════════════════════════════════════════════
-- SEED SYSTEM ROLES
-- ═══════════════════════════════════════════════════════════════════════════════

-- Insert the four predefined system roles (global scope, organization_id = NULL).
INSERT INTO roles (name, description, is_system) VALUES
    ('admin',     'Full access to all resources and organization settings', TRUE),
    ('operator',  'Manage swarms, agents, and tasks; approve actions',      TRUE),
    ('developer', 'Create and manage swarms and tasks',                     TRUE),
    ('viewer',    'Read-only access to all resources',                      TRUE);

-- Wire up admin: wildcard (all permissions)
INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
CROSS JOIN permissions p
WHERE r.name = 'admin' AND r.is_system = TRUE;

-- Wire up operator permissions
INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
JOIN permissions p ON (p.resource, p.action) IN (
    ('swarm', 'create'), ('swarm', 'read'), ('swarm', 'delete'),
    ('agent', 'manage'),
    ('task', 'submit'), ('task', 'read'),
    ('approval', 'approve')
)
WHERE r.name = 'operator' AND r.is_system = TRUE;

-- Wire up developer permissions
INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
JOIN permissions p ON (p.resource, p.action) IN (
    ('swarm', 'create'), ('swarm', 'read'),
    ('agent', 'manage'),
    ('task', 'submit'), ('task', 'read')
)
WHERE r.name = 'developer' AND r.is_system = TRUE;

-- Wire up viewer permissions (read-only)
INSERT INTO role_permissions (role_id, permission_id)
SELECT r.id, p.id
FROM roles r
JOIN permissions p ON (p.resource, p.action) IN (
    ('swarm', 'read'),
    ('agent', 'read'),
    ('task', 'read'),
    ('approval', 'read'),
    ('settings', 'read')
)
WHERE r.name = 'viewer' AND r.is_system = TRUE;

-- ═══════════════════════════════════════════════════════════════════════════════
-- UPDATED_AT TRIGGER
-- ═══════════════════════════════════════════════════════════════════════════════

-- Reuse the existing trigger function if available, otherwise create it.
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_organizations_updated_at
    BEFORE UPDATE ON organizations
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER trg_users_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER trg_roles_updated_at
    BEFORE UPDATE ON roles
    FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();

-- ═══════════════════════════════════════════════════════════════════════════════
-- ROW-LEVEL SECURITY (tenant isolation foundation)
-- ═══════════════════════════════════════════════════════════════════════════════

-- Enable RLS on organization-scoped tables.
-- Policies will be activated when the application sets `current_setting('app.current_org_id')`.

ALTER TABLE organizations ENABLE ROW LEVEL SECURITY;

CREATE POLICY org_isolation_policy ON organizations
    USING (id::text = current_setting('app.current_org_id', TRUE));

-- Note: Additional RLS policies for other tenant-scoped tables (swarms, tasks, etc.)
-- should reference organization_id = current_setting('app.current_org_id', TRUE)
-- and will be added as those tables gain the organization_id column.
