//! Kore Agent - Autonomous LLM-driven agent
//!
//! Simple loop: check inbox → prompt → LLM → parse → execute → trace
//! 
//! Control channels:
//!   /mnt/inbox.txt   - Human writes commands here (agent reads & clears)
//!   /mnt/outbox.txt  - Agent writes messages here (human reads)
//!   /mnt/trace.jsonl - Full execution trace (append-only)
//!
//! Workspace:
//!   /world/          - Agent's persistent workspace

mod tools;
mod trace;
mod config;

use kore::{Context, Stack, Op, execute, register_builtins};
use trace::Trace;
use config::Config;

const INBOX_PATH: &str = "/mnt/inbox.txt";
const TRACE_PATH: &str = "/mnt/trace.jsonl";

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
            eprintln!("  OPENAI_API_KEY or ANTHROPIC_API_KEY");
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
    
    // Setup workspace
    std::fs::create_dir_all(&config.workspace).ok();
    std::fs::create_dir_all("/mnt").ok();
    
    // Initialize trace (writes to /mnt/trace.jsonl for host observation)
    let mut trace = Trace::new(std::path::Path::new(TRACE_PATH));
    
    // Initialize kore runtime
    let mut ctx = Context::new();
    register_builtins(&mut ctx).await;
    tools::register_all(&mut ctx).await;
    
    let mut stack = Stack::new();
    
    // Log start
    trace.start(&config.goal);
    println!("🌍 Kore World starting...");
    println!("   Goal: {}", config.goal);
    println!("   Inbox: {}", INBOX_PATH);
    println!("   Trace: {}", TRACE_PATH);
    if config.max_iterations > 0 {
        println!("   Max iterations: {}", config.max_iterations);
    }
    println!();
    
    // The loop: simple, no magic
    let mut iteration = 0;
    loop {
        iteration += 1;
        
        // Check max iterations limit
        if config.max_iterations > 0 && iteration > config.max_iterations {
            println!("🏁 Max iterations ({}) reached. Terminating.", config.max_iterations);
            trace.done("Max iterations reached");
            break;
        }
        
        trace.iteration(iteration);
        
        // Check inbox for human guidance
        let inbox = read_and_clear_inbox();
        if !inbox.is_empty() {
            trace.inbox(&inbox);
            println!("📬 Inbox: {}", inbox.trim());
        }
        
        // Build context for LLM
        let context = build_context(&config.goal, &inbox, &trace, iteration);
        
        // Call LLM
        trace.thinking();
        let response = match tools::llm::call_llm(&prompt, &context).await {
            Ok(r) => r,
            Err(e) => {
                trace.error(&format!("LLM call failed: {}", e));
                eprintln!("❌ LLM error: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                continue;
            }
        };
        
        trace.response(&response);
        
        // Extract code from response (```kore ... ``` or raw)
        let code = extract_code(&response);
        
        if code.is_empty() {
            trace.error("No code found in response");
            continue;
        }
        
        println!("▶ [{}] {}", iteration, truncate(&code.replace('\n', " "), 60));
        
        match Op::parse(&code) {
            Ok(ops) => {
                trace.code(&code);
                
                // Execute
                match execute(&ops, stack.clone(), ctx.clone()).await {
                    Ok((new_stack, new_ctx)) => {
                        let result = format_stack(&new_stack);
                        trace.success(&result);
                        println!("  ✓ {}", truncate(&result, 60));
                        stack = new_stack;
                        ctx = new_ctx;
                    }
                    Err(e) => {
                        let err = format!("Execution error: {}", e);
                        trace.error(&err);
                        eprintln!("  ✗ {}", err);
                    }
                }
            }
            Err(e) => {
                let err = format!("Parse error: {}", e);
                trace.error(&err);
                eprintln!("  ✗ {}", err);
            }
        }
        
        // Small delay between iterations
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
}

/// Read inbox and clear it (atomic read-then-clear)
fn read_and_clear_inbox() -> String {
    match std::fs::read_to_string(INBOX_PATH) {
        Ok(content) if !content.trim().is_empty() => {
            // Clear the inbox after reading
            let _ = std::fs::write(INBOX_PATH, "");
            content
        }
        _ => String::new(),
    }
}

/// Build context for LLM
fn build_context(goal: &str, inbox: &str, trace: &Trace, iteration: u32) -> String {
    let mut ctx = String::new();
    
    ctx.push_str(&format!("**Goal:** {}\n", goal));
    ctx.push_str(&format!("**Iteration:** {}\n\n", iteration));
    
    // Inbox from human
    if !inbox.is_empty() {
        ctx.push_str("**Inbox (from human):**\n");
        ctx.push_str(inbox);
        ctx.push_str("\n\n");
    }
    
    // Recent trace
    let recent = trace.recent(10);
    if !recent.is_empty() {
        ctx.push_str("**Recent:**\n");
        ctx.push_str(&recent);
        ctx.push_str("\n");
    }
    
    ctx
}

/// Extract code from response - looks for ```kore blocks or raw code
fn extract_code(response: &str) -> String {
    // Try ```kore ... ```
    if let Some(start) = response.find("```kore") {
        if let Some(end) = response[start + 7..].find("```") {
            return response[start + 7..start + 7 + end].trim().to_string();
        }
    }
    
    // Try ``` ... ``` (generic code block)
    if let Some(start) = response.find("```") {
        let after_start = start + 3;
        // Skip language identifier if present
        let code_start = response[after_start..]
            .find('\n')
            .map(|n| after_start + n + 1)
            .unwrap_or(after_start);
        if let Some(end) = response[code_start..].find("```") {
            return response[code_start..code_start + end].trim().to_string();
        }
    }
    
    // No code block found - try the whole response
    response.trim().to_string()
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
