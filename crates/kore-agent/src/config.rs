//! Configuration from environment
//!
//! All config comes from env vars. Explicit. No defaults for required values.

use std::path::PathBuf;

/// Agent configuration
#[derive(Debug)]
pub struct Config {
    /// Workspace directory - agent reads/writes here only
    pub workspace: PathBuf,
    
    /// Path to prompt file (master prompt)
    pub prompt: PathBuf,
    
    /// Goal for the agent
    pub goal: String,
    
    /// Maximum iterations before terminating (0 = unlimited)
    pub max_iterations: u32,
}

impl Config {
    /// Load config from environment variables
    pub fn from_env() -> Result<Self, String> {
        // Required
        let prompt = std::env::var("KORE_PROMPT")
            .map_err(|_| "KORE_PROMPT is required")?;
        
        let goal = std::env::var("KORE_GOAL")
            .map_err(|_| "KORE_GOAL is required")?;
        
        // Check LLM config - one key only
        let has_key = std::env::var("OPENAI_API_KEY")
            .map(|k| !k.is_empty())
            .unwrap_or(false);
        
        if !has_key {
            return Err("OPENAI_API_KEY is required (your one LLM key)".to_string());
        }
        
        // Optional with defaults
        let workspace = std::env::var("KORE_WORKSPACE")
            .unwrap_or_else(|_| "/world".to_string());
        
        let max_iterations = std::env::var("KORE_MAX_ITERATIONS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0); // 0 = unlimited
        
        Ok(Self {
            workspace: PathBuf::from(workspace),
            prompt: PathBuf::from(prompt),
            goal,
            max_iterations,        })
    }
}