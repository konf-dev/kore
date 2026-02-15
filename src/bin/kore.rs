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
use kore::session::{Session, Snapshot, SessionStatus, json_to_value, stack_to_json};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, Write, BufRead};

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
        _ if args[1] == "--serve" => {
            // --serve - JSON protocol over stdin/stdout
            serve().await;
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
    eprintln!("  kore --serve              JSON protocol over stdin/stdout (for MCTS/RL)");
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

// === Serve Mode: JSON Lines protocol over stdin/stdout ===
//
// Commands:
//   {"cmd":"eval","code":"..."}                         → one-shot eval, return stack
//   {"cmd":"session","code":"...","id":"s1"}            → create session from code
//   {"cmd":"push","id":"s1","values":[...]}             → push values onto session stack
//   {"cmd":"step","id":"s1"}                            → step 1 op
//   {"cmd":"step_n","id":"s1","n":10}                   → step up to n ops
//   {"cmd":"run","id":"s1"}                             → run to halt/error
//   {"cmd":"compile_op","id":"s1","code":"..."}         → parse code → append ops
//   {"cmd":"snap","id":"s1","snap_id":"k1"}             → snapshot session
//   {"cmd":"restore","id":"s1","snap_id":"k1"}          → restore session from snapshot
//   {"cmd":"fork","id":"s1","new_id":"s2"}              → fork session
//   {"cmd":"drop","id":"s1"}                            → drop session
//   {"cmd":"drop_snap","snap_id":"k1"}                  → drop snapshot
//   {"cmd":"list"}                                      → list sessions and snapshots

/// State for the serve mode
struct ServeState {
    sessions: HashMap<String, Session>,
    snapshots: HashMap<String, Snapshot>,
    base_ctx: Context,
    next_session_id: u64,
}

impl ServeState {
    fn auto_id(&mut self) -> String {
        let id = format!("s{}", self.next_session_id);
        self.next_session_id += 1;
        id
    }
}

async fn serve() {
    // Setup context
    let mut base_ctx = Context::new();
    register_builtins(&mut base_ctx).await;

    let mut state = ServeState {
        sessions: HashMap::new(),
        snapshots: HashMap::new(),
        base_ctx,
        next_session_id: 0,
    };

    // Signal ready
    let ready = serde_json::json!({"status": "ready", "version": "0.1.0"});
    println!("{}", ready);

    let stdin = io::stdin();
    for line_result in stdin.lock().lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(_) => break,
        };

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let response = handle_serve_command(&mut state, line).await;
        println!("{}", response);
        io::stdout().flush().unwrap();
    }
}

async fn handle_serve_command(state: &mut ServeState, line: &str) -> serde_json::Value {
    use serde_json::json;

    // Parse JSON
    let cmd: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => return json!({"error": format!("JSON parse error: {}", e)}),
    };

    let cmd_name = match cmd.get("cmd").and_then(|c| c.as_str()) {
        Some(c) => c,
        None => return json!({"error": "missing 'cmd' field"}),
    };

    match cmd_name {
        "eval" => serve_eval(state, &cmd).await,
        "session" => serve_session(state, &cmd).await,
        "push" => serve_push(state, &cmd),
        "step" => serve_step(state, &cmd).await,
        "step_n" => serve_step_n(state, &cmd).await,
        "run" => serve_run(state, &cmd).await,
        "compile_op" => serve_compile_op(state, &cmd),
        "snap" => serve_snap(state, &cmd),
        "restore" => serve_restore(state, &cmd),
        "fork" => serve_fork(state, &cmd),
        "drop" => serve_drop(state, &cmd),
        "drop_snap" => serve_drop_snap(state, &cmd),
        "list" => serve_list(state),
        _ => serde_json::json!({"error": format!("unknown command: {}", cmd_name)}),
    }
}

fn session_status_json(sid: &str, session: &Session) -> serde_json::Value {
    use serde_json::json;
    let status_str = match session.status() {
        SessionStatus::Running => "running",
        SessionStatus::Halted => "halted",
        SessionStatus::Error(_) => "error",
    };
    let mut resp = json!({
        "id": sid,
        "status": status_str,
        "pc": session.pc(),
        "steps": session.steps(),
        "ops_len": session.ops_len(),
        "stack": stack_to_json(session.stack()),
        "depth": session.stack().depth(),
    });
    if let SessionStatus::Error(e) = session.status() {
        resp["error_message"] = json!(e);
    }
    resp
}

async fn serve_eval(state: &mut ServeState, cmd: &serde_json::Value) -> serde_json::Value {
    use serde_json::json;
    let code = match cmd.get("code").and_then(|c| c.as_str()) {
        Some(c) => c,
        None => return json!({"error": "eval: missing 'code'"}),
    };

    let ops = match Op::parse(code) {
        Ok(o) => o,
        Err(e) => return json!({"error": format!("parse error: {}", e)}),
    };

    let stack = Stack::new();
    match execute(&ops, stack, state.base_ctx.clone()).await {
        Ok((result_stack, _)) => {
            json!({
                "status": "ok",
                "stack": stack_to_json(&result_stack),
                "depth": result_stack.depth(),
            })
        }
        Err(e) => json!({"status": "error", "error": format!("{}", e)}),
    }
}

async fn serve_session(state: &mut ServeState, cmd: &serde_json::Value) -> serde_json::Value {
    use serde_json::json;

    let id = match cmd.get("id").and_then(|c| c.as_str()) {
        Some(id) => id.to_string(),
        None => state.auto_id(),
    };

    if state.sessions.contains_key(&id) {
        return json!({"error": format!("session '{}' already exists", id)});
    }

    let code = cmd.get("code").and_then(|c| c.as_str()).unwrap_or("");
    let ops = match Op::parse(code) {
        Ok(o) => o,
        Err(e) => return json!({"error": format!("parse error: {}", e)}),
    };

    let max_steps = cmd
        .get("max_steps")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    let session = Session::new(ops, Stack::new(), state.base_ctx.clone())
        .with_max_steps(max_steps);

    state.sessions.insert(id.clone(), session);
    let session = state.sessions.get(&id).unwrap();
    session_status_json(&id, session)
}

fn serve_push(state: &mut ServeState, cmd: &serde_json::Value) -> serde_json::Value {
    use serde_json::json;

    let id = match cmd.get("id").and_then(|c| c.as_str()) {
        Some(id) => id,
        None => return json!({"error": "push: missing 'id'"}),
    };

    let session = match state.sessions.get_mut(id) {
        Some(s) => s,
        None => return json!({"error": format!("session '{}' not found", id)}),
    };

    let values = match cmd.get("values").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return json!({"error": "push: missing 'values' array"}),
    };

    for jval in values {
        match json_to_value(jval) {
            Ok(v) => {
                if let Err(e) = session.push_value(v) {
                    return json!({"error": format!("push error: {}", e)});
                }
            }
            Err(e) => return json!({"error": format!("value conversion error: {}", e)}),
        }
    }

    session_status_json(id, session)
}

async fn serve_step(state: &mut ServeState, cmd: &serde_json::Value) -> serde_json::Value {
    use serde_json::json;

    let id = match cmd.get("id").and_then(|c| c.as_str()) {
        Some(id) => id,
        None => return json!({"error": "step: missing 'id'"}),
    };

    let session = match state.sessions.get_mut(id) {
        Some(s) => s,
        None => return json!({"error": format!("session '{}' not found", id)}),
    };

    session.step().await;
    session_status_json(id, session)
}

async fn serve_step_n(state: &mut ServeState, cmd: &serde_json::Value) -> serde_json::Value {
    use serde_json::json;

    let id = match cmd.get("id").and_then(|c| c.as_str()) {
        Some(id) => id,
        None => return json!({"error": "step_n: missing 'id'"}),
    };

    let n = cmd.get("n").and_then(|v| v.as_u64()).unwrap_or(1) as usize;

    let session = match state.sessions.get_mut(id) {
        Some(s) => s,
        None => return json!({"error": format!("session '{}' not found", id)}),
    };

    let (executed, _) = session.step_n(n).await;
    let mut resp = session_status_json(id, session);
    resp["executed"] = json!(executed);
    resp
}

async fn serve_run(state: &mut ServeState, cmd: &serde_json::Value) -> serde_json::Value {
    use serde_json::json;

    let id = match cmd.get("id").and_then(|c| c.as_str()) {
        Some(id) => id,
        None => return json!({"error": "run: missing 'id'"}),
    };

    let session = match state.sessions.get_mut(id) {
        Some(s) => s,
        None => return json!({"error": format!("session '{}' not found", id)}),
    };

    session.run().await;
    session_status_json(id, session)
}

fn serve_compile_op(state: &mut ServeState, cmd: &serde_json::Value) -> serde_json::Value {
    use serde_json::json;

    let id = match cmd.get("id").and_then(|c| c.as_str()) {
        Some(id) => id,
        None => return json!({"error": "compile_op: missing 'id'"}),
    };

    let code = match cmd.get("code").and_then(|c| c.as_str()) {
        Some(c) => c,
        None => return json!({"error": "compile_op: missing 'code'"}),
    };

    let ops = match Op::parse(code) {
        Ok(o) => o,
        Err(e) => return json!({"error": format!("parse error: {}", e)}),
    };

    let ops_count = ops.len();

    let session = match state.sessions.get_mut(id) {
        Some(s) => s,
        None => return json!({"error": format!("session '{}' not found", id)}),
    };

    session.append_ops(ops);

    let mut resp = session_status_json(id, session);
    resp["appended"] = json!(ops_count);
    resp
}

fn serve_snap(state: &mut ServeState, cmd: &serde_json::Value) -> serde_json::Value {
    use serde_json::json;

    let id = match cmd.get("id").and_then(|c| c.as_str()) {
        Some(id) => id,
        None => return json!({"error": "snap: missing 'id'"}),
    };

    let snap_id = match cmd.get("snap_id").and_then(|c| c.as_str()) {
        Some(sid) => sid.to_string(),
        None => return json!({"error": "snap: missing 'snap_id'"}),
    };

    let session = match state.sessions.get(id) {
        Some(s) => s,
        None => return json!({"error": format!("session '{}' not found", id)}),
    };

    let snap = session.snapshot();
    state.snapshots.insert(snap_id.clone(), snap);

    json!({
        "status": "ok",
        "snap_id": snap_id,
        "session_id": id,
    })
}

fn serve_restore(state: &mut ServeState, cmd: &serde_json::Value) -> serde_json::Value {
    use serde_json::json;

    let id = match cmd.get("id").and_then(|c| c.as_str()) {
        Some(id) => id,
        None => return json!({"error": "restore: missing 'id'"}),
    };

    let snap_id = match cmd.get("snap_id").and_then(|c| c.as_str()) {
        Some(sid) => sid,
        None => return json!({"error": "restore: missing 'snap_id'"}),
    };

    let snap = match state.snapshots.get(snap_id) {
        Some(s) => s.clone(),
        None => return json!({"error": format!("snapshot '{}' not found", snap_id)}),
    };

    let session = match state.sessions.get_mut(id) {
        Some(s) => s,
        None => return json!({"error": format!("session '{}' not found", id)}),
    };

    session.restore(&snap);
    session_status_json(id, session)
}

fn serve_fork(state: &mut ServeState, cmd: &serde_json::Value) -> serde_json::Value {
    use serde_json::json;

    let id = match cmd.get("id").and_then(|c| c.as_str()) {
        Some(id) => id,
        None => return json!({"error": "fork: missing 'id'"}),
    };

    let new_id = match cmd.get("new_id").and_then(|c| c.as_str()) {
        Some(nid) => nid.to_string(),
        None => state.auto_id(),
    };

    if state.sessions.contains_key(&new_id) {
        return json!({"error": format!("session '{}' already exists", new_id)});
    }

    let forked = match state.sessions.get(id) {
        Some(s) => s.fork(),
        None => return json!({"error": format!("session '{}' not found", id)}),
    };

    state.sessions.insert(new_id.clone(), forked);
    let session = state.sessions.get(&new_id).unwrap();
    session_status_json(&new_id, session)
}

fn serve_drop(state: &mut ServeState, cmd: &serde_json::Value) -> serde_json::Value {
    use serde_json::json;

    let id = match cmd.get("id").and_then(|c| c.as_str()) {
        Some(id) => id,
        None => return json!({"error": "drop: missing 'id'"}),
    };

    if state.sessions.remove(id).is_some() {
        json!({"status": "ok", "dropped": id})
    } else {
        json!({"error": format!("session '{}' not found", id)})
    }
}

fn serve_drop_snap(state: &mut ServeState, cmd: &serde_json::Value) -> serde_json::Value {
    use serde_json::json;

    let snap_id = match cmd.get("snap_id").and_then(|c| c.as_str()) {
        Some(sid) => sid,
        None => return json!({"error": "drop_snap: missing 'snap_id'"}),
    };

    if state.snapshots.remove(snap_id).is_some() {
        serde_json::json!({"status": "ok", "dropped": snap_id})
    } else {
        serde_json::json!({"error": format!("snapshot '{}' not found", snap_id)})
    }
}

fn serve_list(state: &ServeState) -> serde_json::Value {
    use serde_json::json;

    let sessions: Vec<serde_json::Value> = state
        .sessions
        .iter()
        .map(|(id, s)| {
            json!({
                "id": id,
                "status": match s.status() {
                    SessionStatus::Running => "running",
                    SessionStatus::Halted => "halted",
                    SessionStatus::Error(_) => "error",
                },
                "pc": s.pc(),
                "steps": s.steps(),
                "depth": s.stack().depth(),
            })
        })
        .collect();

    let snaps: Vec<String> = state.snapshots.keys().cloned().collect();

    json!({
        "sessions": sessions,
        "snapshots": snaps,
    })
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
