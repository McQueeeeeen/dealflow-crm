ALTER TABLE organizations
    ADD COLUMN stale_after_days SMALLINT NOT NULL DEFAULT 14 CHECK (stale_after_days BETWEEN 1 AND 365);

CREATE TABLE sales_targets (
    organization_id  UUID NOT NULL,
    user_id          UUID NOT NULL,
    month            DATE NOT NULL CHECK (extract(day FROM month) = 1),
    amount_cents     BIGINT NOT NULL CHECK (amount_cents >= 0),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (organization_id, user_id, month),
    CONSTRAINT sales_targets_user_fkey FOREIGN KEY (organization_id, user_id)
        REFERENCES users (organization_id, id) ON DELETE CASCADE
);

CREATE INDEX deals_won_by_owner_idx ON deals (organization_id, owner_id, closed_at)
    WHERE closed_at IS NOT NULL;
CREATE INDEX tasks_deal_completed_idx ON tasks (organization_id, deal_id, completed_at)
    WHERE deal_id IS NOT NULL;
CREATE INDEX companies_domain_idx ON companies (organization_id, domain) WHERE domain IS NOT NULL;
CREATE INDEX companies_lower_name_idx ON companies (organization_id, lower(name));
CREATE INDEX contacts_email_idx ON contacts (organization_id, email) WHERE email IS NOT NULL;

CREATE FUNCTION audit_log_broadcast() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    PERFORM pg_notify('dealflow_changes', json_build_object(
        'org', NEW.organization_id,
        'entity', NEW.entity_type,
        'id', NEW.entity_id,
        'action', NEW.action,
        'actor', NEW.actor_id
    )::text);
    RETURN NULL;
END;
$$;

CREATE TRIGGER audit_log_broadcast
    AFTER INSERT ON audit_log
    FOR EACH ROW EXECUTE FUNCTION audit_log_broadcast();
