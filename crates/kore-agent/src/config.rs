//! Configuration from environment
//!
//! All config comes from env vars. Explicit. No defaults for required values.

use std::path::PathBuf;

/// Agent configuration
#[derive(Debug)]
pub struct Config {
    /// Workspace directory - agent reads/writes here only
    pub workspace: PathBuf,
    
    /// Logs directory - trace and stdout logs
    pub logs: PathBuf,
    
    /// Path to prompt file (master prompt)
    pub prompt: PathBuf,
    
    /// Goal for the agent
    pub goal: String,
}

impl Config {
    /// Load config from environment variables
    pub fn from_env() -> Result<Self, String> {
        // Required
        let prompt = std::env::var("KORE_PROMPT")
            .map_err(|_| "KORE_PROMPT is required")?;
        
        let goal = std::env::var("KORE_GOAL")
            .map_err(|_| "KORE_GOAL is required")?;
        
        // Check LLM config
        if std::env::var("OPENAI_API_KEY").map(|k| k.is_empty()).unwrap_or(true) {
            return Err("OPENAI_API_KEY is required".to_string());
        }
        
        // Optional with defaults (for local dev)
        let workspace = std::env::var("KORE_WORKSPACE")
            .unwrap_or_else(|_| "./workspace".to_string());
        
        let logs = std::env::var("KORE_LOGS")
            .unwrap_or_else(|_| "./logs".to_string());
        
        Ok(Self {
            workspace: PathBuf::from(workspace),
            logs: PathBuf::from(logs),
            prompt: PathBuf::from(prompt),
            goal,
        })
    }
}
