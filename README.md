# DealFlow CRM

A multi-tenant sales CRM for small B2B sales teams: a drag-and-drop pipeline, companies and
contacts, follow-up tasks, notes, a full change history and sales reports.

The backend is written in Rust (Axum, SQLx, PostgreSQL). The web app is Next.js with
TypeScript.

[![CI](https://github.com/McQueeeeeen/dealflow-crm/actions/workflows/ci.yml/badge.svg)](https://github.com/McQueeeeeen/dealflow-crm/actions/workflows/ci.yml)

## What it does

- **Today**: your overdue and upcoming follow-ups, and the deals you expect to close this month.
- **Pipeline**: a kanban board of deals by stage. Drag with a mouse, press and hold on touch
  screens, or use the keyboard (space to pick up, arrows to move, space to drop).
- **Deals, companies, contacts**: linked records with notes, follow-ups and a readable history
  of every change.
- **Reports**: open and weighted pipeline, a stage-by-stage conversion funnel, a six-month
  forecast by expected close date, and results per salesperson for managers.
- **Settings**: invite teammates with a role, rename, reorder and re-weight pipeline stages.

## Architecture

```
Browser ──► Next.js (React, TanStack Query)
              │  /api/auth/*   sets httpOnly session cookies
              │  /api/v1/*     same-origin proxy, adds the bearer token
              ▼
           Rust API (Axum) ──► PostgreSQL 16
```

```
backend/
  migrations/        schema, constraints, indexes, append-only audit trigger
  src/api/           one module per resource: handlers, DTOs and SQL side by side
  src/auth/          JWT access tokens, rotating refresh tokens, Argon2id hashing
  src/authz.rs       every role rule in one place
  src/audit.rs       field-level diffs written in the same transaction as the change
  src/bin/seed.rs    deterministic demo workspace with realistic stage history
  tests/api/         end-to-end tests against a real database
frontend/
  src/app/           routes: today, pipeline, deals, companies, contacts, reports, settings
  src/app/api/       session endpoints and the API proxy
  src/components/    UI primitives, forms, the pipeline board, charts
```

## Engineering decisions

**Tenant isolation is enforced by the database, not only by handlers.** Every table carries
`organization_id`, and every reference between tables is a composite foreign key on
`(organization_id, id)`. A deal cannot point at another workspace's company, stage or user
even if a handler forgot a check; the insert fails and the API answers with a field error.
Every query is also scoped by the caller's organization, so another workspace's IDs behave
exactly like IDs that do not exist (404, not 403).

**Concurrent edits do not overwrite each other.** Deals carry a `version`. Updates and stage
moves lock the row, compare versions and return `409 Conflict` if someone else saved first.
The UI tells the person what happened and reloads the current state.

**A stolen refresh token is caught on reuse.** Access tokens live for 15 minutes. Refresh
tokens are random, stored only as SHA-256 hashes, and rotate on every use. Presenting a spent
refresh token outside a 10-second grace window (two tabs refreshing at once) is treated as
theft and revokes the whole session family. Deactivating a user revokes their sessions, and
role changes apply to the very next request because the role is read from the database, not
from the token.

**Tokens never reach browser JavaScript.** The Next.js server keeps them in httpOnly cookies
and forwards API calls with the bearer token attached. The browser only talks to its own
origin, so there is no CORS surface.

**The audit log is append-only.** Changes are recorded as field-level diffs inside the same
transaction as the change, and a database trigger rejects any `UPDATE` on audit rows.

**Reports come from history, not snapshots.** Every stage move is stored. The funnel counts a
deal as having passed a stage if it ever reached that stage or a later one, so skipping a
column on the board does not distort conversion. Money is stored as integer minor units in a
single workspace currency, so sums are exact.

**Errors have one shape.** Every failure is `{"error": {"code", "message", "fields"}}`.
Constraint violations are mapped to field-level validation errors by constraint name.

## Running it

Everything in containers:

```bash
docker compose up --build
docker compose run --rm seed
```

Open http://localhost:3000 and sign in as `demo@dealflow.dev` with the password
`dealflow-demo`. These credentials only exist in the seeded demo workspace.

Without Docker you need Rust (stable), Node.js 22 and PostgreSQL 16:

```bash
cd backend
export DATABASE_URL=postgres://localhost/dealflow JWT_SECRET=$(openssl rand -hex 32)
cargo run --bin seed      # migrates and loads demo data
cargo run                 # API on :8080, docs at http://localhost:8080/docs

cd ../frontend
cp .env.example .env.local
npm install && npm run dev   # web app on :3000
```

## API

Interactive reference at `/docs` (Scalar) and the OpenAPI document at `/openapi.json`, both
generated from the handler annotations, so they cannot drift from the code.

## Tests

```bash
cd backend && cargo test
```

Unit tests cover password hashing, token signing, validation, pagination, audit diffs and the
role rules. The integration suite starts a fresh, migrated PostgreSQL database for each test
and drives the real router: registration and login, refresh-token rotation and reuse
detection, deactivation, tenant isolation, cross-tenant references, role permissions,
optimistic concurrency, stage moves, the audit trail and its immutability, search and
pagination, tasks, notes, pipeline reordering, and the numbers behind every report.

CI runs `rustfmt`, `clippy -D warnings`, the full test suite and the seed script for the
backend, and Prettier, ESLint, a production build and `tsc` for the frontend.

## Design

The visual direction, palette with contrast reasoning, type scale and motion rules are in
[DESIGN.md](DESIGN.md). Both themes meet WCAG AA contrast, every control is reachable by
keyboard, and the board can be operated entirely from the keyboard.
