CREATE TABLE saved_views (
    id               UUID PRIMARY KEY,
    organization_id  UUID NOT NULL,
    user_id          UUID NOT NULL,
    name             TEXT NOT NULL CHECK (length(name) BETWEEN 1 AND 60),
    filter           TEXT NOT NULL CHECK (length(filter) BETWEEN 1 AND 500),
    shared           BOOLEAN NOT NULL DEFAULT FALSE,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT saved_views_name_key UNIQUE (organization_id, user_id, name),
    CONSTRAINT saved_views_user_fkey FOREIGN KEY (organization_id, user_id)
        REFERENCES users (organization_id, id) ON DELETE CASCADE
);
