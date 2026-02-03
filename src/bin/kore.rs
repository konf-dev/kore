//! Kore CLI - The Kore OS interface
//!
//! Modes:
//! - REPL: Interactive mode (no args)
//! - Script: Run a file (kore file.kore)
//! - Inline: Run code (kore -e 'code')
//! - Check: Static analysis (kore --check file.kore)

use kore::{Context, Stack, Op, execute};
use kore::analyzer;
use kore::builtins::register_builtins;
use std::env;
use std::fs;
use std::io::{self, Write};

const PRELUDE_PATH: &str = "lib/prelude.kore";

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    
    match args.len() {
        1 => {
            // No args - REPL mode
            repl().await;
        }
        2 if args[1] == "--help" || args[1] == "-h" => {
            show_help();
        }
        _ if args[1] == "--check" && args.len() >= 3 => {
            // --check <file> - static analysis
            check_file(&args[2]);
        }
        _ if args[1] == "-e" && args.len() >= 3 => {
            // -e '<code>' - inline execution
            run_script(&args[2]).await;
        }
        _ if !args[1].starts_with('-') => {
            // First arg is a file, remaining are script arguments
            // (accessible via 'args' primitive)
            let path = &args[1];
            match fs::read_to_string(path) {
                Ok(source) => run_script(&source).await,
                Err(e) => {
                    eprintln!("Error reading {}: {}", path, e);
                    std::process::exit(1);
                }
            }
        }
        _ => {
            eprintln!("Unknown arguments. Use: kore --help");
            std::process::exit(1);
        }
    }
}

fn show_help() {
    eprintln!("kore - Kore OS runtime");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  kore                      Start REPL (interactive mode)");
    eprintln!("  kore <file.kore> [args]   Run a script with arguments");
    eprintln!("  kore -e '<code>'          Run inline code");
    eprintln!("  kore --check <file.kore>  Static stack analysis (detect errors before running)");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  kore                           # Start REPL");
    eprintln!("  kore -e '5 3 add'              # => 8");
    eprintln!("  kore examples/ls.kore /tmp     # List /tmp directory");
    eprintln!("  kore --check script.kore       # Check for stack errors");
}

fn check_file(path: &str) {
    match fs::read_to_string(path) {
        Ok(source) => {
            match Op::parse(&source) {
                Ok(ops) => {
                    let analysis = analyzer::analyze(&ops);
                    print!("{}", analyzer::format_analysis(&analysis));
                    
                    if analysis.has_errors() {
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("Parse error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("Error reading {}: {}", path, e);
            std::process::exit(1);
        }
    }
}

async fn repl() {
    eprintln!("Kore OS v0.1.0");
    eprintln!("Type 'exit' to quit, 'help' for commands.");
    eprintln!();
    
    // Setup context with builtins
    let mut ctx = Context::new();
    register_builtins(&mut ctx).await;
    
    // Try to load prelude
    if fs::metadata(PRELUDE_PATH).is_ok() {
        if let Ok(source) = fs::read_to_string(PRELUDE_PATH) {
            match Op::parse(&source) {
                Ok(ops) => {
                    let stack = Stack::new();
                    match execute(&ops, stack, ctx.clone()).await {
                        Ok((_, new_ctx)) => {
                            ctx = new_ctx;
                            eprintln!("Loaded prelude.");
                        }
                        Err(e) => eprintln!("Prelude error: {}", e),
                    }
                }
                Err(e) => eprintln!("Prelude parse error: {}", e),
            }
        }
    }
    
    // Persistent stack across REPL iterations
    let mut stack = Stack::new();
    
    loop {
        // Show prompt with stack depth
        let depth = stack.depth();
        if depth > 0 {
            print!("[{}] > ", depth);
        } else {
            print!("> ");
        }
        io::stdout().flush().unwrap();
        
        // Read line
        let mut line = String::new();
        match io::stdin().read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(e) => {
                eprintln!("Read error: {}", e);
                break;
            }
        }
        
        let line = line.trim();
        
        // Handle special commands
        match line {
            "" => continue,
            "exit" | "quit" => break,
            "help" => {
                eprintln!("Commands:");
                eprintln!("  exit, quit  - Exit REPL");
                eprintln!("  clear       - Clear stack");
                eprintln!("  stack       - Show full stack");
                eprintln!("  .s          - Show stack (short)");
                eprintln!("  help        - Show this help");
                continue;
            }
            "clear" => {
                stack = Stack::new();
                eprintln!("Stack cleared.");
                continue;
            }
            "stack" | ".s" => {
                if stack.depth() == 0 {
                    eprintln!("(empty stack)");
                } else {
                    for (i, v) in stack.values().iter().enumerate() {
                        eprintln!("  {}: {}", i, format_value(v));
                    }
                }
                continue;
            }
            _ => {}
        }
        
        // Parse and execute
        match Op::parse(line) {
            Ok(ops) => {
                match execute(&ops, stack.clone(), ctx.clone()).await {
                    Ok((new_stack, new_ctx)) => {
                        // Show new values on stack
                        let old_depth = stack.depth();
                        let new_depth = new_stack.depth();
                        
                        if new_depth > old_depth {
                            // Print new values
                            for i in old_depth..new_depth {
                                println!("{}", format_value(&new_stack.values()[i]));
                            }
                        } else if new_depth < old_depth {
                            // Values were consumed
                            eprintln!("({} values consumed)", old_depth - new_depth);
                        }
                        
                        stack = new_stack;
                        ctx = new_ctx;
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Parse error: {}", e);
            }
        }
    }
    
    eprintln!("Goodbye.");
}

async fn run_script(source: &str) {
    // Parse source to ops
    let ops = match Op::parse(source) {
        Ok(ops) => ops,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            std::process::exit(1);
        }
    };
    
    // Setup context with builtins (trusted mode for full access)
    let mut ctx = Context::trusted();
    register_builtins(&mut ctx).await;
    
    // Execute
    let stack = Stack::new();
    match execute(&ops, stack, ctx).await {
        Ok((result_stack, _)) => {
            // Print each value on the stack
            for value in result_stack.values() {
                println!("{}", format_value(value));
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

fn format_value(v: &kore::Value) -> String {
    match v {
        kore::Value::Null => "null".to_string(),
        kore::Value::Bool(b) => b.to_string(),
        kore::Value::Int(n) => n.to_string(),
        kore::Value::Float(f) => f.to_string(),
        kore::Value::Text(s) => format!("\"{}\"", s),
        kore::Value::List(items) => {
            let inner: Vec<String> = items.iter().map(format_value).collect();
            format!("[{}]", inner.join(" "))
        }
        kore::Value::Map(m) => {
            let pairs: Vec<String> = m.iter()
                .map(|(k, v)| format!("{}: {}", k, format_value(v)))
                .collect();
            format!("{{{}}}", pairs.join(", "))
        }
        kore::Value::Quote(ops) => format!("[...{} ops]", ops.len()),
        kore::Value::Handle(h) => format!("<handle:{:?}:{}>", h.kind, h.id),
        kore::Value::Error(e) => format!("Error({}: {})", e.code, e.message),
        kore::Value::Ext(e) => format!("<ext:{}:{}>", e.kind, format_value(&e.data)),
    }
}
