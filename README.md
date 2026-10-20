# DealFlow CRM

A multi-tenant sales CRM for small B2B sales teams: a drag-and-drop pipeline that updates
live for the whole team, Cmd+K search, deals that flag themselves when they go cold, monthly
targets, CSV import and export, follow-ups, notes, a full change history and sales reports.

The backend is written in Rust (Axum, SQLx, PostgreSQL). The web app is Next.js with
TypeScript.

[![CI](https://github.com/McQueeeeeen/dealflow-crm/actions/workflows/ci.yml/badge.svg)](https://github.com/McQueeeeeen/dealflow-crm/actions/workflows/ci.yml)
[![E2E](https://github.com/McQueeeeeen/dealflow-crm/actions/workflows/e2e.yml/badge.svg)](https://github.com/McQueeeeeen/dealflow-crm/actions/workflows/e2e.yml)

**Live demo: https://dealflow-crm-demo.vercel.app**. Use "Fill in the demo account" on the
sign-in page. The demo workspace is reset every night.

![Pipeline board](docs/screenshots/pipeline.png)

## What it does

- **Live for the whole team**: when a teammate moves a deal, adds a note or completes a
  follow-up, every open screen updates within a second, and the header shows who else is
  looking at the same record.
- **Cmd+K**: one box searches deals, companies and people, tolerates typos ("copperleef"
  finds Copperleaf), and runs actions like "New deal" or "Switch theme".
- **Today**: your follow-ups, progress toward this month's target, your pipeline by stage,
  deals closing this month, and deals going cold.
- **Going cold**: an open deal with no notes, completed follow-ups or changes for 14 days
  (adjustable per workspace) gets a snowflake on the board and a nudge on its page.
- **Pipeline**: a kanban board of deals by stage. Drag with a mouse, press and hold on touch
  screens, or focus a deal's handle and press the left or right arrow key.
- **Deals, companies, contacts**: linked records with notes, follow-ups and a readable history
  of every change.
- **Reports**: open and weighted pipeline, a stage-by-stage conversion funnel, a six-month
  forecast by expected close date, monthly targets per rep (managers set them inline), and
  results per salesperson for managers.
- **CSV import and export**: map columns, check the file first (nothing is saved until you
  confirm), then import; existing contacts match by email and companies by domain or name.
  Every list exports to a spreadsheet-safe CSV.
- **Settings**: invite teammates with a role, rename, reorder and re-weight pipeline stages.

| Today | Cmd+K search |
|---|---|
| ![Today](docs/screenshots/today.png) | ![Search](docs/screenshots/palette.png) |
| **Reports, light theme** | **Deal** |
| ![Reports](docs/screenshots/reports-light.png) | ![Deal](docs/screenshots/deal.png) |

<img src="docs/screenshots/mobile-today.png" alt="Today on a phone" width="320">

Screenshots are produced by the end-to-end suite against the seeded demo workspace.

## Architecture

```
Browser ──► Next.js (React, TanStack Query)
   ▲          │  /api/auth/*   sets httpOnly session cookies
   │          │  /api/v1/*     same-origin streaming proxy, adds the bearer token
   │          ▼
   └─ SSE ── Rust API (Axum) ──► PostgreSQL 16
                  ▲                    │
                  └── LISTEN ◄─ NOTIFY ┘  (trigger on every audit row)
```

```
backend/
  migrations/        schema, constraints, indexes, append-only audit trigger
  src/api/           one module per resource: handlers, DTOs and SQL side by side
  src/auth/          JWT access tokens, rotating refresh tokens, Argon2id hashing
  src/authz.rs       every role rule in one place
  src/audit.rs       field-level diffs written in the same transaction as the change
  src/live.rs        LISTEN/NOTIFY fan-out to server-sent events, presence
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

**Live updates ride on the audit log.** Every change already writes an audit row inside its
transaction, so a trigger on that table calls `pg_notify`. PostgreSQL delivers the
notification only if the transaction commits, which means a browser never hears about a
change that rolled back. Each API instance holds one `LISTEN` connection and fans
notifications out to server-sent event streams, filtered to the caller's workspace. Browsers
batch the events and refetch only the affected queries; a stream that falls behind gets a
single "resync" instead of a flood. Streams close after ten minutes so the access token is
re-checked, and the client reconnects with backoff.

**Search is one trigram query per kind.** Deals, companies and contacts are matched with
`ILIKE` for exact substrings and `pg_trgm` word similarity for typos, both served by the same
GIN indexes, inside a transaction that sets the similarity threshold locally.

**Imports preview by rolling back.** A dry run executes the real import, every validation,
dedupe and insert, inside a transaction and then rolls it back. The preview cannot disagree
with what the import will do, because it is the import. Exported cells that start with `=`,
`+`, `-` or `@` are prefixed so a spreadsheet will not run them as formulas.

**Errors have one shape.** Every failure is `{"error": {"code", "message", "fields"}}`.
Constraint violations are mapped to field-level validation errors by constraint name.

## Deployment

One Vercel project runs two services (see `vercel.json`): the Next.js app is public, and the
Rust API is a private container that only the web service can reach, through a service binding
that sets `API_URL`. PostgreSQL is Neon from the Vercel Marketplace. A scheduled workflow
re-seeds the demo workspace every night.

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
cargo run                 # API on :8080

cd ../frontend
cp .env.example .env.local
npm install && npm run dev   # web app on :3000
```

## Tests

```bash
cd backend && cargo test
```

Unit tests cover password hashing, token signing, validation, pagination, audit diffs and the
role rules. The integration suite starts a fresh, migrated PostgreSQL database for each test
and drives the real router: registration and login, refresh-token rotation and reuse
detection, deactivation, tenant isolation, cross-tenant references, role permissions,
optimistic concurrency, stage moves, the audit trail and its immutability, search and
pagination, tasks, notes, pipeline reordering, the numbers behind every report, live events
that never cross workspaces, presence, typo-tolerant search, targets, CSV dry runs and
dedupe, formula-safe export, and last-activity tracking.

The end-to-end suite boots the whole stack in CI (PostgreSQL, the API with demo data, the
production Next.js build) and drives it with Playwright on desktop and phone viewports. It
visits every screen in both themes and fails on any console error, any sideways scrolling on
a phone, or any serious WCAG 2.1 AA violation reported by axe. It also moves a deal across the
board with the arrow keys, completes a follow-up, adds a note, checks form validation, the theme
toggle and signing out, finds a company through Cmd+K with a typo, watches a deal created by
another user appear on the open board, imports a CSV through its preview and downloads an
export.

CI runs `rustfmt`, `clippy -D warnings`, the full test suite and the seed script for the
backend, and Prettier, ESLint, a production build and `tsc` for the frontend.

## Design

The visual direction, palette with contrast reasoning, type scale and motion rules are in
[DESIGN.md](DESIGN.md). Both themes meet WCAG AA contrast, every control is reachable by
keyboard, including moving deals across the board.
