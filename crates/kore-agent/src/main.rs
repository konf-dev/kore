//! Kore Agent - Autonomous LLM-driven agent
//!
//! This is the entry point for running kore as an autonomous agent.
//! It starts a genesis workflow that calls an LLM and lets it take over.

mod tools;

use kore::{Context, Stack, Op, execute, register_builtins};

const SYSTEM_PROMPT: &str = r#"You are Kore, an autonomous agent built on a stack-based runtime.

## Your Capabilities

You can execute kore code. Your responses should contain kore code blocks that will be parsed and executed.

### Available Tools

**Stack Operations:**
- `dup` (a -- a a) - duplicate top value
- `drop` (a --) - remove top value  
- `swap` (a b -- b a) - swap top two
- `over` (a b -- a b a) - copy second to top
- `rot` (a b c -- b c a) - rotate top three

**Control:**
- `call` (quote -- ...) - execute a quote
- `try` (quote -- result) - execute, capture errors
- `is-error` (value -- bool) - check if error
- `unwrap` (value -- value) - extract or fail if error

**I/O:**
- `print` (value --) - output to user
- `read-line` (-- text) - read from user
- `env-get` (key -- value) - read environment variable

**Network:**
- `http-get` (url -- {status, body}) - GET request
- `http-post` (url body headers -- {status, body}) - POST request

**AI:**
- `llm` (prompt -- response) - call Claude
- `llm-system` (system user -- response) - call Claude with system prompt

**Code:**
- `parse` (text -- quote) - parse kore code into quote

**Workflows:**
- `spawn` (quote -- handle) - start concurrent workflow
- `await` (handle -- result) - wait for completion
- `send` (handle message --) - send message
- `recv` (-- message) - receive message (blocks)
- `self` (-- handle) - get own handle
- `parent` (-- handle) - get parent's handle

## Syntax

```
42              -- push integer
3.14            -- push float
"hello"         -- push string
true false null -- push literals
(code here)     -- quote (deferred code)
tool-name       -- call a tool
```

## Example

```kore
"Hello, I am Kore!" print
"What would you like me to do?" print
read-line
llm
print
```

## Your Task

You are autonomous. When you receive a goal, break it down and accomplish it step by step.
Output kore code blocks that will be executed. The results will be shown to you.
You can spawn background workflows, make HTTP requests, and call yourself recursively for complex reasoning.

Begin by acknowledging you're ready and asking what the user wants to accomplish.
"#;

#[tokio::main]
async fn main() {
    // Initialize context with all tools
    let mut ctx = Context::new();
    register_builtins(&mut ctx).await;
    tools::register_all(&mut ctx).await;
    
    // Register workflow tools if available
    // kore_workflow::tools::register_all(&mut ctx).await;
    
    println!("╔══════════════════════════════════════╗");
    println!("║         Kore Agent v0.1.0            ║");
    println!("╠══════════════════════════════════════╣");
    println!("║  Autonomous LLM-driven runtime       ║");
    println!("║  Type 'quit' to exit                 ║");
    println!("╚══════════════════════════════════════╝");
    println!();
    
    // Check for API key
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        eprintln!("⚠️  ANTHROPIC_API_KEY not set!");
        eprintln!("   Set it with: export ANTHROPIC_API_KEY=your-key");
        std::process::exit(1);
    }
    
    // Genesis: First LLM call with system prompt
    println!("🚀 Initializing genesis workflow...\n");
    
    let stack = Stack::new();
    
    // Build genesis program: call LLM with system prompt, then enter REPL loop
    let genesis_ops = vec![
        // Push system prompt and initial user message
        Op::push(SYSTEM_PROMPT.to_string()),
        Op::push("Initialize and introduce yourself.".to_string()),
        Op::call("llm-system"),
        Op::call("print"),
    ];
    
    // Execute genesis
    let (mut stack, mut ctx) = match execute(&genesis_ops, stack, ctx).await {
        Ok(result) => result,
        Err(e) => {
            eprintln!("❌ Genesis failed: {}", e);
            std::process::exit(1);
        }
    };
    
    println!("\n");
    
    // REPL loop
    loop {
        print!("kore> ");
        use std::io::Write;
        std::io::stdout().flush().unwrap();
        
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_err() {
            break;
        }
        
        let input = input.trim();
        
        if input.is_empty() {
            continue;
        }
        
        if input == "quit" || input == "exit" {
            println!("Goodbye!");
            break;
        }
        
        if input == "stack" {
            println!("Stack: {:?}", stack.values());
            continue;
        }
        
        if input.starts_with("/llm ") {
            // Direct LLM call with context
            let prompt = &input[5..];
            let ops = vec![
                Op::push(SYSTEM_PROMPT.to_string()),
                Op::push(prompt.to_string()),
                Op::call("llm-system"),
                Op::call("print"),
            ];
            
            match execute(&ops, stack.clone(), ctx.clone()).await {
                Ok((new_stack, new_ctx)) => {
                    stack = new_stack;
                    ctx = new_ctx;
                }
                Err(e) => {
                    eprintln!("❌ Error: {}", e);
                }
            }
            println!();
            continue;
        }
        
        // Parse and execute kore code
        match Op::parse(input) {
            Ok(ops) => {
                match execute(&ops, stack.clone(), ctx.clone()).await {
                    Ok((new_stack, new_ctx)) => {
                        stack = new_stack;
                        ctx = new_ctx;
                        // Show stack if not empty
                        if !stack.is_empty() {
                            println!("→ {:?}", stack.values());
                        }
                    }
                    Err(e) => {
                        eprintln!("❌ Error: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("❌ Parse error: {}", e);
            }
        }
    }
}
