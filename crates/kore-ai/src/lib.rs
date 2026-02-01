//! # Kore AI
//!
//! AI and LLM tools for Kore programs.
//!
//! ## Purpose
//!
//! Agents need AI capabilities:
//! - LLM calls for reasoning
//! - Embeddings for semantic operations
//! - Classification for categorization
//!
//! ## Modules
//!
//! | Module | Purpose | Key Tools |
//! |--------|---------|-----------|
//! | [`llm`] | LLM calls | `llm-chat`, `llm-complete` |
//! | [`embeddings`] | Vector embeddings | `embed`, `embed-batch` |
//! | [`classify`] | Classification | `classify`, `classify-multi` |
//!
//! ## Configuration
//!
//! AI tools require configuration for the provider (OpenAI, Anthropic, etc.).
//! Use `ai-config` to set up the provider.

pub mod classify;
pub mod embeddings;
pub mod llm;

pub use classify::register_classify_tools;
pub use embeddings::register_embedding_tools;
pub use llm::register_llm_tools;

use kore::Context;

/// Register all AI tools into a context.
pub async fn register_all(ctx: &mut Context) {
    register_llm_tools(ctx).await;
    register_embedding_tools(ctx).await;
    register_classify_tools(ctx).await;
}
