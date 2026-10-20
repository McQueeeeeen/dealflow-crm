CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE TYPE user_role AS ENUM ('admin', 'manager', 'sales_rep');
CREATE TYPE stage_kind AS ENUM ('open', 'won', 'lost');
CREATE TYPE audit_action AS ENUM ('created', 'updated', 'deleted');

CREATE TABLE organizations (
    id          UUID PRIMARY KEY,
    name        TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 120),
    currency    TEXT NOT NULL DEFAULT 'USD' CHECK (currency ~ '^[A-Z]{3}$'),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE users (
    id               UUID PRIMARY KEY,
    organization_id  UUID NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    email            TEXT NOT NULL CHECK (email = lower(email)),
    full_name        TEXT NOT NULL CHECK (length(full_name) BETWEEN 1 AND 120),
    password_hash    TEXT NOT NULL,
    role             user_role NOT NULL,
    is_active        BOOLEAN NOT NULL DEFAULT TRUE,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT users_email_key UNIQUE (email),
    CONSTRAINT users_org_id_key UNIQUE (organization_id, id)
);

CREATE TABLE refresh_tokens (
    id          UUID PRIMARY KEY,
    user_id     UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    family_id   UUID NOT NULL,
    token_hash  TEXT NOT NULL UNIQUE,
    expires_at  TIMESTAMPTZ NOT NULL,
    used_at     TIMESTAMPTZ,
    revoked_at  TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX refresh_tokens_family_idx ON refresh_tokens (family_id);

CREATE TABLE pipeline_stages (
    id               UUID PRIMARY KEY,
    organization_id  UUID NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    name             TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 60),
    position         INT NOT NULL CHECK (position >= 0),
    probability      SMALLINT NOT NULL CHECK (probability BETWEEN 0 AND 100),
    kind             stage_kind NOT NULL DEFAULT 'open',
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT pipeline_stages_org_id_key UNIQUE (organization_id, id),
    CONSTRAINT pipeline_stages_name_key UNIQUE (organization_id, name),
    CONSTRAINT pipeline_stages_position_key UNIQUE (organization_id, position) DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE companies (
    id               UUID PRIMARY KEY,
    organization_id  UUID NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    name             TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 200),
    domain           TEXT,
    industry         TEXT,
    owner_id         UUID NOT NULL,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT companies_org_id_key UNIQUE (organization_id, id),
    CONSTRAINT companies_owner_fkey FOREIGN KEY (organization_id, owner_id)
        REFERENCES users (organization_id, id)
);

CREATE INDEX companies_name_trgm_idx ON companies USING gin (name gin_trgm_ops);
CREATE INDEX companies_org_updated_idx ON companies (organization_id, updated_at DESC);

CREATE TABLE contacts (
    id               UUID PRIMARY KEY,
    organization_id  UUID NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    first_name       TEXT NOT NULL CHECK (length(first_name) BETWEEN 1 AND 100),
    last_name        TEXT NOT NULL DEFAULT '' CHECK (length(last_name) <= 100),
    email            TEXT,
    phone            TEXT,
    job_title        TEXT,
    company_id       UUID,
    owner_id         UUID NOT NULL,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT contacts_org_id_key UNIQUE (organization_id, id),
    CONSTRAINT contacts_company_fkey FOREIGN KEY (organization_id, company_id)
        REFERENCES companies (organization_id, id) ON DELETE SET NULL (company_id),
    CONSTRAINT contacts_owner_fkey FOREIGN KEY (organization_id, owner_id)
        REFERENCES users (organization_id, id)
);

CREATE INDEX contacts_search_trgm_idx ON contacts
    USING gin ((first_name || ' ' || last_name || ' ' || coalesce(email, '')) gin_trgm_ops);
CREATE INDEX contacts_org_company_idx ON contacts (organization_id, company_id);
CREATE INDEX contacts_org_updated_idx ON contacts (organization_id, updated_at DESC);

CREATE TABLE deals (
    id                   UUID PRIMARY KEY,
    organization_id      UUID NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    title                TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 200),
    value_cents          BIGINT NOT NULL DEFAULT 0 CHECK (value_cents >= 0),
    stage_id             UUID NOT NULL,
    board_position       DOUBLE PRECISION NOT NULL DEFAULT 0,
    company_id           UUID,
    contact_id           UUID,
    owner_id             UUID NOT NULL,
    expected_close_date  DATE,
    closed_at            TIMESTAMPTZ,
    version              INT NOT NULL DEFAULT 1,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT deals_org_id_key UNIQUE (organization_id, id),
    CONSTRAINT deals_stage_fkey FOREIGN KEY (organization_id, stage_id)
        REFERENCES pipeline_stages (organization_id, id),
    CONSTRAINT deals_company_fkey FOREIGN KEY (organization_id, company_id)
        REFERENCES companies (organization_id, id) ON DELETE SET NULL (company_id),
    CONSTRAINT deals_contact_fkey FOREIGN KEY (organization_id, contact_id)
        REFERENCES contacts (organization_id, id) ON DELETE SET NULL (contact_id),
    CONSTRAINT deals_owner_fkey FOREIGN KEY (organization_id, owner_id)
        REFERENCES users (organization_id, id)
);

CREATE INDEX deals_board_idx ON deals (organization_id, stage_id, board_position);
CREATE INDEX deals_org_owner_idx ON deals (organization_id, owner_id);
CREATE INDEX deals_org_company_idx ON deals (organization_id, company_id);
CREATE INDEX deals_title_trgm_idx ON deals USING gin (title gin_trgm_ops);
CREATE TABLE deal_stage_changes (
    id               BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    organization_id  UUID NOT NULL,
    deal_id          UUID NOT NULL,
    from_stage_id    UUID,
    to_stage_id      UUID NOT NULL,
    changed_by       UUID REFERENCES users (id),
    changed_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT deal_stage_changes_deal_fkey FOREIGN KEY (organization_id, deal_id)
        REFERENCES deals (organization_id, id) ON DELETE CASCADE,
    CONSTRAINT deal_stage_changes_to_stage_fkey FOREIGN KEY (organization_id, to_stage_id)
        REFERENCES pipeline_stages (organization_id, id),
    CONSTRAINT deal_stage_changes_from_stage_fkey FOREIGN KEY (organization_id, from_stage_id)
        REFERENCES pipeline_stages (organization_id, id)
);

CREATE INDEX deal_stage_changes_deal_idx ON deal_stage_changes (organization_id, deal_id);

CREATE TABLE tasks (
    id               UUID PRIMARY KEY,
    organization_id  UUID NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    title            TEXT NOT NULL CHECK (length(title) BETWEEN 1 AND 200),
    details          TEXT,
    due_at           TIMESTAMPTZ,
    completed_at     TIMESTAMPTZ,
    assignee_id      UUID NOT NULL,
    created_by       UUID NOT NULL,
    deal_id          UUID,
    contact_id       UUID,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT tasks_org_id_key UNIQUE (organization_id, id),
    CONSTRAINT tasks_assignee_fkey FOREIGN KEY (organization_id, assignee_id)
        REFERENCES users (organization_id, id),
    CONSTRAINT tasks_created_by_fkey FOREIGN KEY (organization_id, created_by)
        REFERENCES users (organization_id, id),
    CONSTRAINT tasks_deal_fkey FOREIGN KEY (organization_id, deal_id)
        REFERENCES deals (organization_id, id) ON DELETE CASCADE,
    CONSTRAINT tasks_contact_fkey FOREIGN KEY (organization_id, contact_id)
        REFERENCES contacts (organization_id, id) ON DELETE CASCADE
);

CREATE INDEX tasks_open_by_assignee_idx ON tasks (organization_id, assignee_id, due_at)
    WHERE completed_at IS NULL;
CREATE INDEX tasks_deal_idx ON tasks (organization_id, deal_id);

CREATE TABLE notes (
    id               UUID PRIMARY KEY,
    organization_id  UUID NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    author_id        UUID NOT NULL,
    body             TEXT NOT NULL CHECK (length(body) BETWEEN 1 AND 10000),
    deal_id          UUID,
    contact_id       UUID,
    company_id       UUID,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT notes_single_parent CHECK (num_nonnulls(deal_id, contact_id, company_id) = 1),
    CONSTRAINT notes_author_fkey FOREIGN KEY (organization_id, author_id)
        REFERENCES users (organization_id, id),
    CONSTRAINT notes_deal_fkey FOREIGN KEY (organization_id, deal_id)
        REFERENCES deals (organization_id, id) ON DELETE CASCADE,
    CONSTRAINT notes_contact_fkey FOREIGN KEY (organization_id, contact_id)
        REFERENCES contacts (organization_id, id) ON DELETE CASCADE,
    CONSTRAINT notes_company_fkey FOREIGN KEY (organization_id, company_id)
        REFERENCES companies (organization_id, id) ON DELETE CASCADE
);

CREATE INDEX notes_deal_idx ON notes (organization_id, deal_id, created_at DESC);
CREATE INDEX notes_contact_idx ON notes (organization_id, contact_id, created_at DESC);
CREATE INDEX notes_company_idx ON notes (organization_id, company_id, created_at DESC);

CREATE TABLE audit_log (
    id               BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    organization_id  UUID NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    actor_id         UUID REFERENCES users (id),
    entity_type      TEXT NOT NULL,
    entity_id        UUID NOT NULL,
    action           audit_action NOT NULL,
    changes          JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX audit_log_entity_idx ON audit_log (organization_id, entity_type, entity_id, created_at DESC);
CREATE INDEX audit_log_org_recent_idx ON audit_log (organization_id, created_at DESC);

CREATE FUNCTION audit_log_is_append_only() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'audit_log is append-only';
END;
$$;

CREATE TRIGGER audit_log_no_update
    BEFORE UPDATE ON audit_log
    FOR EACH ROW EXECUTE FUNCTION audit_log_is_append_only();
