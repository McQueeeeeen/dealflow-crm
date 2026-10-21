CREATE TABLE automation_rules (
    id               UUID PRIMARY KEY,
    organization_id  UUID NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    name             TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 80),
    enabled          BOOLEAN NOT NULL DEFAULT TRUE,
    trigger          JSONB NOT NULL,
    condition        TEXT NOT NULL DEFAULT '' CHECK (length(condition) <= 500),
    actions          JSONB NOT NULL CHECK (jsonb_typeof(actions) = 'array'),
    created_by       UUID NOT NULL REFERENCES users (id),
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT automation_rules_org_id_key UNIQUE (organization_id, id)
);

CREATE INDEX automation_rules_trigger_idx ON automation_rules (organization_id, trigger) WHERE enabled;

CREATE TABLE automation_runs (
    id               BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    organization_id  UUID NOT NULL,
    rule_id          UUID NOT NULL,
    deal_id          UUID NOT NULL,
    succeeded        BOOLEAN NOT NULL,
    detail           TEXT NOT NULL,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT automation_runs_rule_fkey FOREIGN KEY (organization_id, rule_id)
        REFERENCES automation_rules (organization_id, id) ON DELETE CASCADE,
    CONSTRAINT automation_runs_deal_fkey FOREIGN KEY (organization_id, deal_id)
        REFERENCES deals (organization_id, id) ON DELETE CASCADE
);

CREATE INDEX automation_runs_rule_idx ON automation_runs (organization_id, rule_id, created_at DESC);
