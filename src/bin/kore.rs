//! Kore CLI - Run kore programs from text
//!
//! One thing: execute a kore program and print the result.

use kore::{Context, Stack, Op, execute};
use kore::builtins::register_builtins;
use std::env;
use std::fs;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    
    match args.len() {
        1 => {
            // No args - show usage
            eprintln!("kore - stack-based language runtime");
            eprintln!();
            eprintln!("Usage:");
            eprintln!("  kore <file.kore>     Run a file");
            eprintln!("  kore -e '<code>'     Run inline code");
            eprintln!();
            eprintln!("Examples:");
            eprintln!("  kore -e '5 3 add'");
            eprintln!("  kore -e '[1 2 3] [10 mul] map'");
            std::process::exit(0);
        }
        2 => {
            // Single arg - it's a file
            let path = &args[1];
            match fs::read_to_string(path) {
                Ok(source) => run(&source).await,
                Err(e) => {
                    eprintln!("Error reading {}: {}", path, e);
                    std::process::exit(1);
                }
            }
        }
        3 if args[1] == "-e" => {
            // -e '<code>' - inline execution
            run(&args[2]).await;
        }
        _ => {
            eprintln!("Unknown arguments. Use: kore -e '<code>' or kore <file>");
            std::process::exit(1);
        }
    }
}

async fn run(source: &str) {
    // Parse source to ops
    let ops = match Op::parse(source) {
        Ok(ops) => ops,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            std::process::exit(1);
        }
    };
    
    // Setup context with builtins
    let mut ctx = Context::new();
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
    }
}
