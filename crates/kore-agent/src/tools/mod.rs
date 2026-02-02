//! Agent tools module
//!
//! All tools are STATELESS. Each tool does ONE thing.

pub mod io;
pub mod llm;
pub mod network;
pub mod math;
pub mod fs;
pub mod shell;
pub mod checkpoint;
pub mod introspection;
pub mod helpers;

use kore::{Context, Tool};

/// Get all agent tools
pub fn all_tools() -> Vec<Tool> {
    let mut tools = Vec::new();
    
    // I/O
    tools.push(io::env_get_tool());
    tools.push(io::print_tool());
    tools.push(io::read_line_tool());
    tools.push(io::done_tool());
    
    // LLM
    tools.push(llm::llm_tool());
    tools.push(llm::llm_system_tool());
    
    // Network
    tools.push(network::http_get_tool());
    tools.push(network::http_post_tool());
    
    // Math
    tools.push(math::add_tool());
    tools.push(math::sub_tool());
    tools.push(math::mul_tool());
    tools.push(math::div_tool());
    tools.push(math::eq_tool());
    tools.push(math::lt_tool());
    tools.push(math::gt_tool());
    tools.push(math::concat_tool());
    
    // File system
    tools.push(fs::file_read_tool());
    tools.push(fs::file_write_tool());
    tools.push(fs::file_append_tool());
    tools.push(fs::file_exists_tool());
    tools.push(fs::file_delete_tool());
    tools.push(fs::dir_list_tool());
    tools.push(fs::dir_create_tool());
    tools.push(fs::dir_delete_tool());
    
    // Shell
    tools.push(shell::shell_tool());
    tools.push(shell::shell_dir_tool());
    
    // Checkpointing
    tools.push(checkpoint::checkpoint_tool());
    tools.push(checkpoint::restore_tool());
    tools.push(checkpoint::list_checkpoints_tool());
    
    // Introspection
    tools.push(introspection::list_tools_tool());
    tools.push(introspection::tool_help_tool());
    
    // Helpers
    tools.push(helpers::now_tool());
    tools.push(helpers::uuid_tool());
    tools.push(helpers::json_parse_tool());
    tools.push(helpers::json_format_tool());
    
    tools
}

/// Register all agent tools into a context
pub async fn register_all(ctx: &mut Context) {
    let mut dict = ctx.dict.write().await;
    for tool in all_tools() {
        dict.register(tool);
    }
}

/// Get workspace path (sandbox root)
pub fn workspace_path(relative: &str) -> std::path::PathBuf {
    let base = std::env::var("KORE_WORKSPACE").unwrap_or_else(|_| "./workspace".to_string());
    std::path::PathBuf::from(base).join(relative.trim_start_matches('/'))
}

/// Get checkpoint path
pub fn checkpoint_path(name: &str) -> std::path::PathBuf {
    let base = std::env::var("KORE_CHECKPOINTS").unwrap_or_else(|_| "./checkpoints".to_string());
    std::path::PathBuf::from(base).join(name)
}
