//! # Kore Database
//!
//! Storage tools for Kore programs.
//!
//! ## Purpose
//!
//! Agents need persistent and queryable storage:
//! - Key-value for simple lookups
//! - SQL for structured queries
//! - Vector storage for semantic search
//!
//! ## Modules
//!
//! | Module | Purpose | Key Tools |
//! |--------|---------|-----------|
//! | [`kv`] | Key-value store | `kv-get`, `kv-set`, `kv-del`, `kv-list` |
//! | [`sql`] | SQL queries | `sql-query`, `sql-exec` |
//! | [`vector`] | Vector storage | `vec-store`, `vec-search` |
//!
//! ## Note
//!
//! This module provides in-memory implementations. For production,
//! connect to actual databases using kore-net or dedicated drivers.

pub mod kv;
pub mod sql;
pub mod vector;

pub use kv::register_kv_tools;
pub use sql::register_sql_tools;
pub use vector::register_vector_tools;

use kore::Context;

/// Register all database tools into a context.
pub async fn register_all(ctx: &mut Context) {
    register_kv_tools(ctx).await;
    register_sql_tools(ctx).await;
    register_vector_tools(ctx).await;
}
