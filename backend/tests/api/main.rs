//! End-to-end API tests. Each test gets a fresh, fully migrated database from `sqlx::test`
//! and drives the real router in-process.

mod analytics;
mod auth;
mod common;
mod deals;
mod permissions;
mod records;
mod tenancy;
