//! Kore Agent - Autonomous LLM-driven agent
//!
//! Simple loop: prompt → LLM → parse → execute → trace
//! No magic. No preprocessing. Errors go to trace, LLM learns.

mod tools;
mod trace;
mod config;

use kore::{Context, Stack, Op, execute, register_builtins};
use trace::Trace;
use config::Config;

#[tokio::main]
async fn main() {
    // Load config from environment
    let config = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Configuration error: {}", e);
            eprintln!("\nRequired environment variables:");
            eprintln!("  KORE_PROMPT  - Path to prompt file");
            eprintln!("  KORE_GOAL    - Goal for the agent");
            eprintln!("  OPENAI_API_KEY - LLM API key");
            std::process::exit(1);
        }
    };
    
    // Load prompt (master prompt - attached to every LLM call)
    let prompt = match std::fs::read_to_string(&config.prompt) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to read prompt file '{}': {}", config.prompt.display(), e);
            std::process::exit(1);
        }
    };
    
    // Setup workspace and logs
    std::fs::create_dir_all(&config.workspace).ok();
    std::fs::create_dir_all(&config.logs).ok();
    
    // Initialize trace (logs to stdout AND file)
    let trace_file = config.logs.join("trace.jsonl");
    let mut trace = Trace::new(&trace_file);
    
    // Initialize kore runtime
    let mut ctx = Context::new();
    register_builtins(&mut ctx).await;
    tools::register_all(&mut ctx).await;
    
    let mut stack = Stack::new();
    
    // Log start
    trace.start(&config.goal);
    
    // The loop: simple, no magic
    let mut iteration = 0;
    loop {
        iteration += 1;
        trace.iteration(iteration);
        
        // Build context for LLM: goal + recent trace
        // Master prompt is ALWAYS prepended - agent remembers principles
        let context = build_context(&config.goal, &trace, iteration);
        
        // Call LLM (prompt is system message, context is user message)
        trace.thinking();
        let response = match tools::llm::call_llm(&prompt, &context).await {
            Ok(r) => r,
            Err(e) => {
                trace.error(&format!("LLM call failed: {}", e));
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                continue;
            }
        };
        
        trace.response(&response);
        
        // Parse response as kore code - NO extraction, NO cleaning
        // If LLM outputs bad format, parser error goes to trace, LLM learns
        let code = response.trim();
        
        match Op::parse(code) {
            Ok(ops) => {
                trace.code(code);
                
                // Execute
                match execute(&ops, stack.clone(), ctx.clone()).await {
                    Ok((new_stack, new_ctx)) => {
                        let result = format_stack(&new_stack);
                        trace.success(&result);
                        stack = new_stack;
                        ctx = new_ctx;
                    }
                    Err(e) => {
                        trace.error(&format!("Execution error: {}", e));
                    }
                }
            }
            Err(e) => {
                // Parse error - this goes to trace, LLM sees it next iteration
                trace.error(&format!("Parse error: {} | Input was: {}", e, truncate(code, 100)));
            }
        }
        
        // Small delay between iterations
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }
}

/// Build context for LLM - just goal and recent trace
fn build_context(goal: &str, trace: &Trace, iteration: u32) -> String {
    let mut ctx = String::new();
    
    ctx.push_str(&format!("GOAL: {}\n", goal));
    ctx.push_str(&format!("ITERATION: {}\n\n", iteration));
    
    // Recent trace - LLM sees what happened
    let recent = trace.recent(10);
    if !recent.is_empty() {
        ctx.push_str("RECENT TRACE:\n");
        ctx.push_str(&recent);
        ctx.push_str("\n");
    }
    
    ctx.push_str("YOUR TURN: Output kore code to execute.\n");
    
    ctx
}

/// Format stack for display
fn format_stack(stack: &Stack) -> String {
    if stack.is_empty() {
        "(empty)".to_string()
    } else {
        format!("{:?}", stack.values())
    }
}

/// Truncate string for display
fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}...", &s[..max])
    } else {
        s.to_string()
    }
}
