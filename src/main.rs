//! Kore Bytecode Compiler/Disassembler/Interpreter CLI
//!
//! Usage:
//!   korec compile input.kore -o output.korec
//!   korec disasm input.korec
//!   korec run input.korec
//!   korec native-run input.korec      (JIT compile & run - FASTEST)
//!   korec check input.korec
//!   korec wasm input.korec -o output.wasm
//!   korec spirv input.korec -o output.spv
//!   korec gpu-run input.spv [--input "1 2 3"]
//!   korec gpu-info
//!   korec example

use std::env;
use std::fs;

mod bytecode;
mod interpreter;
mod optimizer;
mod parser;
mod proof_checker;
mod spirv_backend;
mod wasm_backend;

#[cfg(feature = "gpu")]
mod gpu_runtime;

#[cfg(feature = "native")]
mod native_backend;

#[cfg(test)]
mod benchmarks;

#[cfg(test)]
mod industrial_benchmarks;

use bytecode::*;
use interpreter::*;
use proof_checker::{ProofChecker, Type};

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        return;
    }

    match args[1].as_str() {
        "compile" => {
            if args.len() < 3 {
                eprintln!("Usage: korec compile <file.kore> [-o output.korec]");
                return;
            }
            let output = if args.len() >= 5 && args[3] == "-o" {
                args[4].clone()
            } else {
                args[2].replace(".kore", ".korec")
            };
            compile_source(&args[2], &output);
        }
        "disasm" => {
            if args.len() < 3 {
                eprintln!("Usage: korec disasm <file.korec>");
                return;
            }
            disasm_file(&args[2]);
        }
        "run" => {
            if args.len() < 3 {
                eprintln!("Usage: korec run <file.korec> [--allow io] [--trace]");
                return;
            }
            let allowed_caps = parse_allow_flags(&args);
            run_file(&args[2], args.contains(&"--trace".to_string()), allowed_caps);
        }
        #[cfg(feature = "native")]
        "native-run" => {
            if args.len() < 3 {
                eprintln!("Usage: korec native-run <file.korec>");
                return;
            }
            native_run_file(&args[2]);
        }
        "check" => {
            if args.len() < 3 {
                eprintln!("Usage: korec check <file.korec>");
                return;
            }
            check_file(&args[2]);
        }
        "wasm" => {
            if args.len() < 3 {
                eprintln!("Usage: korec wasm <file.korec> [-o output.wasm]");
                return;
            }
            let output = if args.len() >= 5 && args[3] == "-o" {
                args[4].clone()
            } else {
                args[2].replace(".korec", ".wasm")
            };
            compile_wasm(&args[2], &output);
        }
        "spirv" => {
            if args.len() < 3 {
                eprintln!("Usage: korec spirv <file.korec> [-o output.spv]");
                return;
            }
            let output = if args.len() >= 5 && args[3] == "-o" {
                args[4].clone()
            } else {
                args[2].replace(".korec", ".spv")
            };
            compile_spirv(&args[2], &output);
        }
        #[cfg(feature = "gpu")]
        "gpu-run" => {
            if args.len() < 3 {
                eprintln!("Usage: korec gpu-run <file.spv> [--input \"1 2 3\"]");
                return;
            }
            let input_data: Vec<i32> = if let Some(idx) = args.iter().position(|a| a == "--input") {
                if idx + 1 < args.len() {
                    args[idx + 1]
                        .split_whitespace()
                        .filter_map(|s| s.parse().ok())
                        .collect()
                } else {
                    vec![]
                }
            } else {
                vec![]
            };
            run_gpu(&args[2], &input_data);
        }
        #[cfg(feature = "gpu")]
        "gpu-info" => {
            show_gpu_info();
        }
        #[cfg(feature = "gpu")]
        "gpu-map" => {
            if args.len() < 4 {
                eprintln!("Usage: korec gpu-map <body.kore> --input \"1 2 3 4 5\"");
                eprintln!();
                eprintln!("Applies the Kore expression to each input element in parallel on GPU.");
                eprintln!("The .kore file should contain just the per-element body, e.g.:");
                eprintln!("  dup mul         -- square each element");
                eprintln!("  dup add         -- double each element");
                eprintln!("  3 add           -- add 3 to each element");
                return;
            }
            let input_data: Vec<i32> = if let Some(idx) = args.iter().position(|a| a == "--input") {
                if idx + 1 < args.len() {
                    args[idx + 1]
                        .split_whitespace()
                        .filter_map(|s| s.parse().ok())
                        .collect()
                } else {
                    vec![]
                }
            } else {
                vec![]
            };
            if input_data.is_empty() {
                eprintln!("Error: --input required with space-separated integers");
                return;
            }
            run_gpu_map(&args[2], &input_data);
        }
        "example" => {
            run_example();
        }
        "repl" => {
            let allowed_caps = parse_allow_flags(&args);
            run_repl(allowed_caps);
        }
        "serve" => {
            // Parse optional --prelude <file> flag
            let prelude_source = if let Some(idx) = args.iter().position(|a| a == "--prelude") {
                if idx + 1 < args.len() {
                    match fs::read_to_string(&args[idx + 1]) {
                        Ok(content) => Some(content),
                        Err(e) => {
                            eprintln!("Failed to read prelude file '{}': {}", args[idx + 1], e);
                            return;
                        }
                    }
                } else {
                    eprintln!("--prelude requires a file path argument");
                    return;
                }
            } else {
                None
            };
            run_serve(prelude_source);
        }
        "test" => {
            run_tests();
        }
        "help" | "--help" | "-h" => {
            print_usage();
        }
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            print_usage();
        }
    }
}

fn print_usage() {
    eprintln!("Kore Bytecode Toolchain v0.1");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  korec compile <file.kore> [-o out]  Compile source to bytecode");
    eprintln!("  korec disasm <file.korec>           Disassemble bytecode file");
    eprintln!("  korec run <file.korec> [--allow io] [--trace]    Execute bytecode (interpreter)");
    #[cfg(feature = "native")]
    eprintln!("  korec native-run <file.korec>       Execute bytecode (JIT - FASTEST)");
    eprintln!("  korec check <file.korec>            Type-check bytecode (P4)");
    eprintln!("  korec wasm <file.korec> [-o out]    Compile to WebAssembly");
    eprintln!("  korec spirv <file.korec> [-o out]   Compile to SPIR-V (GPU)");
    #[cfg(feature = "gpu")]
    {
        eprintln!("  korec gpu-run <file.spv> [--input]  Run SPIR-V on GPU");
        eprintln!("  korec gpu-map <body.korec> --input   Parallel MAP on GPU");
        eprintln!("  korec gpu-info                      Show GPU information");
    }
    eprintln!("  korec repl [--allow io]              Interactive REPL");
    eprintln!("  korec serve                         Persistent eval server (stdin/stdout)");
    eprintln!("  korec example                       Run factorial example");
    eprintln!("  korec test                          Run test suite");
    eprintln!("  korec help                          Show this help");
}

fn compile_source(input: &str, output: &str) {
    let source = match fs::read_to_string(input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading {}: {}", input, e);
            return;
        }
    };

    println!("Compiling {}...", input);

    // Resolve base directory for imports
    let input_path = std::path::Path::new(input);
    let base_dir = input_path.parent().unwrap_or(std::path::Path::new("."));

    match parser::compile_with_imports(&source, base_dir) {
        Ok(mut module) => {
            // OPTIMIZER: Apply algebraic optimizations before proof checking
            // These are provably correct rewrites (involutions, identity elements,
            // constant folding) that reduce instruction count.
            let original_len = module.code.len();
            let (optimized_code, offset_map) = optimizer::optimize_with_offsets(&module.code);
            let optimized_len = optimized_code.len();
            module.code = optimized_code;
            
            // Patch symbol table: function offsets may have shifted
            let mut new_symbol_table = std::collections::HashMap::new();
            for (idx, old_offset) in &module.symbol_table {
                let new_offset = offset_map.get(old_offset).copied().unwrap_or(*old_offset);
                new_symbol_table.insert(*idx, new_offset);
            }
            module.symbol_table = new_symbol_table;
            
            if optimized_len < original_len {
                println!("  optimizer: {} → {} bytes (saved {})", 
                    original_len, optimized_len, original_len - optimized_len);
            }
            
            // MANDATORY: Proof check before accepting bytecode
            // This is what makes Kore's safety guarantees REAL
            let mut checker = ProofChecker::new();
            let errors = checker.check(&module.code);
            
            if !errors.is_empty() {
                eprintln!("✗ Type errors (safety violation):\n");
                for err in &errors {
                    eprintln!("  {:04X}: {}", err.address, err.message);
                }
                eprintln!("\nCompilation rejected. Fix type errors first.");
                std::process::exit(1);
            }
            
            // Only write if proof check passes
            let binary = module.encode();
            fs::write(output, &binary).expect("Failed to write bytecode file");
            let cap_str = format_caps(module.cap_flags);
            println!("✓ Proof checked and written {} ({} bytes){}", output, binary.len(), cap_str);
            println!("\nDisassembly:");
            println!("{}", disassemble(&module.code));
        }
        Err(e) => {
            eprintln!("✗ Compilation error: {}", e);
            std::process::exit(1);
        }
    }
}

fn format_caps(caps: u8) -> String {
    if caps == 0 {
        return " [pure]".into();
    }
    let mut parts = Vec::new();
    if caps & crate::bytecode::CAP_IO != 0 { parts.push("io"); }
    if caps & crate::bytecode::CAP_FS != 0 { parts.push("fs"); }
    if caps & crate::bytecode::CAP_NET != 0 { parts.push("net"); }
    if caps & crate::bytecode::CAP_EXEC != 0 { parts.push("exec"); }
    format!(" [caps: {}]", parts.join(", "))
}

fn disasm_file(path: &str) {
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error reading {}: {}", path, e);
            return;
        }
    };

    let module = match BytecodeModule::decode(&data) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error decoding bytecode: {}", e);
            return;
        }
    };

    println!("Kore Bytecode v{}.{}", module.version.0, module.version.1);
    println!("Code size: {} bytes", module.code.len());
    println!();
    println!("{}", disassemble(&module.code));
}

fn run_file(path: &str, trace: bool, allowed_caps: u8) {
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error reading {}: {}", path, e);
            return;
        }
    };

    let module = match BytecodeModule::decode(&data) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error decoding bytecode: {}", e);
            return;
        }
    };

    // P4: Runtime capability check — constraints attenuate, never escalate
    let denied = module.cap_flags & !allowed_caps;
    if denied != 0 {
        eprintln!("\u{2717} Capability denied: module requires{} but only{} allowed",
            format_caps(module.cap_flags), 
            if allowed_caps == 0 { " [none]".into() } else { format_caps(allowed_caps) }
        );
        eprintln!("  Hint: use --allow io to grant IO capabilities");
        std::process::exit(1);
    }

    let mut interp = Interpreter::from_module_with_caps(&module, allowed_caps);
    if trace {
        interp.enable_trace();
    }

    match interp.run() {
        Ok(()) => {
            println!("Result: {:?}", interp.result());
            if interp.stack().len() > 1 {
                println!("Full stack: {:?}", interp.stack());
            }
        }
        Err(e) => {
            eprintln!("Runtime error: {}", e);
        }
    }
}

fn parse_allow_flags(args: &[String]) -> u8 {
    let mut caps: u8 = 0;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--allow" && i + 1 < args.len() {
            for cap in args[i + 1].split(',') {
                match cap.trim() {
                    "io" => caps |= crate::bytecode::CAP_IO,
                    "fs" => caps |= crate::bytecode::CAP_FS,
                    "net" => caps |= crate::bytecode::CAP_NET,
                    "exec" => caps |= crate::bytecode::CAP_EXEC,
                    "all" => caps = 0xFF,
                    _ => eprintln!("Warning: unknown capability '{}'", cap),
                }
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    caps
}

#[cfg(feature = "native")]
fn native_run_file(path: &str) {
    use std::time::Instant;
    
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error reading {}: {}", path, e);
            return;
        }
    };

    let module = match BytecodeModule::decode(&data) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error decoding bytecode: {}", e);
            return;
        }
    };

    println!("JIT compiling {}...", path);
    let compile_start = Instant::now();
    
    match native_backend::run_native_module(&module) {
        Ok(result) => {
            let elapsed = compile_start.elapsed();
            println!("✓ Native execution complete ({} μs)", elapsed.as_micros());
            println!("Result: {}", result);
        }
        Err(e) => {
            eprintln!("Native execution error: {}", e);
        }
    }
}

fn check_file(path: &str) {
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error reading {}: {}", path, e);
            return;
        }
    };

    let module = match BytecodeModule::decode(&data) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error decoding bytecode: {}", e);
            return;
        }
    };

    println!("Type-checking {} ({} bytes)...\n", path, module.code.len());

    let mut checker = ProofChecker::new();
    let errors = checker.check_with_caps(&module.code, module.cap_flags);

    if errors.is_empty() {
        println!("✓ Type check passed!");
        println!("\nFinal stack types:");
        for (i, t) in checker.final_stack().iter().enumerate() {
            println!("  [{}] {}", i, t.name());
        }
    } else {
        println!("✗ Type errors found:\n");
        for err in &errors {
            println!("  {:04X}: {}", err.address, err.message);
        }
        std::process::exit(1);
    }
}

fn compile_wasm(input: &str, output: &str) {
    let data = match fs::read(input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error reading {}: {}", input, e);
            return;
        }
    };

    let module = match BytecodeModule::decode(&data) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error decoding bytecode: {}", e);
            return;
        }
    };

    println!("Compiling {} to WebAssembly...", input);

    match wasm_backend::compile_to_wasm(&module.code) {
        Ok(wasm) => {
            fs::write(output, &wasm).expect("Failed to write WASM file");
            println!("✓ Written {} ({} bytes)", output, wasm.len());
            println!("\nTo test in browser:");
            println!("  const bytes = await fetch('{}').then(r => r.arrayBuffer());", output);
            println!("  const {{ instance }} = await WebAssembly.instantiate(bytes);");
            println!("  console.log(instance.exports.main());");
        }
        Err(e) => {
            eprintln!("✗ Compilation failed: {}", e);
            std::process::exit(1);
        }
    }
}

fn compile_spirv(input: &str, output: &str) {
    let data = match fs::read(input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error reading {}: {}", input, e);
            return;
        }
    };

    let module = match BytecodeModule::decode(&data) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error decoding bytecode: {}", e);
            return;
        }
    };

    println!("Compiling {} to SPIR-V...", input);

    match spirv_backend::compile_to_spirv(&module.code) {
        Ok(spirv) => {
            fs::write(output, &spirv).expect("Failed to write SPIR-V file");
            println!("✓ Written {} ({} bytes)", output, spirv.len());
            println!("\nTo validate with spirv-val:");
            println!("  spirv-val {}", output);
            println!("\nTo disassemble with spirv-dis:");
            println!("  spirv-dis {}", output);
            println!("\nTo run on GPU:");
            println!("  korec gpu-run {}", output);
        }
        Err(e) => {
            eprintln!("✗ Compilation failed: {}", e);
            std::process::exit(1);
        }
    }
}

#[cfg(feature = "gpu")]
fn run_gpu(input: &str, input_data: &[i32]) {
    let spirv_bytes = match fs::read(input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error reading {}: {}", input, e);
            return;
        }
    };
    
    println!("Running {} on GPU...", input);
    
    if let Some(info) = gpu_runtime::gpu_info() {
        println!("GPU: {}", info);
    }
    
    println!("Input: {:?}", input_data);
    
    match gpu_runtime::run_compute(&spirv_bytes, input_data, 1) {
        Ok(result) => {
            println!("✓ GPU execution complete ({} μs)", result.elapsed_us);
            println!("Output: {:?}", result.output);
            
            // Show first result prominently
            if !result.output.is_empty() {
                println!("\nResult[0] = {}", result.output[0]);
            }
        }
        Err(e) => {
            eprintln!("✗ GPU execution failed: {}", e);
            std::process::exit(1);
        }
    }
}

#[cfg(feature = "gpu")]
fn run_gpu_map(body_path: &str, input_data: &[i32]) {
    // Read the source file containing the per-element body
    let source = match fs::read_to_string(body_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading {}: {}", body_path, e);
            return;
        }
    };
    
    // Compile the body to bytecode (skip proof checker — input comes from GPU)
    // Add "halt" if not present
    let full_source = if source.contains("halt") { source } else { format!("{} halt", source) };
    let module = match parser::compile(&full_source) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error compiling {}: {}", body_path, e);
            return;
        }
    };
    
    // Compile to SPIR-V parallel MAP shader
    let spirv_bytes = match spirv_backend::compile_to_spirv_parallel_map(&module.code) {
        Ok(spv) => spv,
        Err(e) => {
            eprintln!("Error compiling to SPIR-V: {}", e);
            return;
        }
    };
    
    let n = input_data.len();
    println!("GPU parallel MAP: {} elements", n);
    
    if let Some(info) = gpu_runtime::gpu_info() {
        println!("GPU: {}", info);
    }
    
    println!("Input:  {:?}", input_data);
    
    match gpu_runtime::run_compute(&spirv_bytes, input_data, n as u32) {
        Ok(result) => {
            let output: Vec<i32> = result.output.iter().take(n).copied().collect();
            println!("Output: {:?}", output);
            println!("✓ GPU MAP complete ({} μs, {} elements)", result.elapsed_us, n);
        }
        Err(e) => {
            eprintln!("✗ GPU MAP failed: {}", e);
            std::process::exit(1);
        }
    }
}

#[cfg(feature = "gpu")]
fn show_gpu_info() {
    println!("GPU Information:");
    println!();
    
    if gpu_runtime::gpu_available() {
        if let Some(info) = gpu_runtime::gpu_info() {
            println!("  Adapter: {}", info);
            println!("  Status:  Available ✓");
        }
    } else {
        println!("  Status:  No GPU available");
        println!();
        println!("Supported backends:");
        println!("  - Vulkan (Linux, Windows, Android)");
        println!("  - Metal (macOS, iOS)");
        println!("  - DirectX 12 (Windows)");
        println!("  - WebGPU (browsers)");
    }
}

fn run_repl(allowed_caps: u8) {
    use std::io::{self, Write};
    
    println!("Kore REPL v0.16 — type 'exit' or Ctrl-D to quit");
    println!("Postulates: P1(tool) P2(apply) P3(concat) P4(attenuate)");
    println!("Caps: {}", if allowed_caps == 0 { "none (use --allow io,fs,net,exec,all)".into() } else { format!("0x{:02X}", allowed_caps) });
    println!();
    
    // Persistent function definitions accumulate across lines
    let mut definitions = String::new();
    
    loop {
        print!("kore> ");
        io::stdout().flush().unwrap_or(());
        
        let mut line = String::new();
        match io::stdin().read_line(&mut line) {
            Ok(0) => { println!(); break; } // EOF
            Ok(_) => {}
            Err(e) => { eprintln!("read error: {}", e); break; }
        }
        
        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }
        if trimmed == "exit" || trimmed == "quit" { break; }
        
        // Special REPL commands
        if trimmed == ":stack" || trimmed == ":s" {
            println!("(stack shown after each expression)");
            continue;
        }
        if trimmed == ":help" || trimmed == ":h" {
            println!("  :help     Show this help");
            println!("  :clear    Clear definitions");
            println!("  exit      Quit REPL");
            continue;
        }
        if trimmed == ":clear" {
            definitions.clear();
            println!("Definitions cleared.");
            continue;
        }
        
        // If it's a function definition, accumulate it
        if trimmed.starts_with(':') && !trimmed.starts_with(":") {
            // Not a REPL command, might be a colon-definition
        }
        if trimmed.starts_with(": ") || trimmed.starts_with(":") && trimmed.contains(';') {
            definitions.push_str(trimmed);
            definitions.push('\n');
            println!("  defined.");
            continue;
        }
        
        // Compile and run: definitions + this expression
        let full_source = format!("{}\n{}", definitions, trimmed);
        
        match parser::compile(&full_source) {
            Ok(mut module) => {
                // Optimize
                let (optimized, offset_map) = optimizer::optimize_with_offsets(&module.code);
                module.code = optimized;
                
                // Patch symbol table
                let mut new_symbol_table = std::collections::HashMap::new();
                for (idx, old_offset) in &module.symbol_table {
                    let new_offset = offset_map.get(old_offset).copied().unwrap_or(*old_offset);
                    new_symbol_table.insert(*idx, new_offset);
                }
                module.symbol_table = new_symbol_table;
                
                // Type check
                let mut checker = ProofChecker::new();
                let errors = checker.check(&module.code);
                if !errors.is_empty() {
                    for err in &errors {
                        eprintln!("  type error at {:04X}: {}", err.address, err.message);
                    }
                    continue;
                }
                
                // Run
                let mut interp = Interpreter::from_module_with_caps(&module, allowed_caps);
                match interp.run() {
                    Ok(()) => {
                        // Print the stack
                        let stack = interp.stack();
                        if stack.is_empty() {
                            println!("  (empty stack)");
                        } else {
                            for (i, v) in stack.iter().enumerate() {
                                println!("  [{}] {:?}", i, v);
                            }
                        }
                    }
                    Err(e) => eprintln!("  error: {}", e),
                }
            }
            Err(e) => eprintln!("  parse error: {}", e),
        }
    }
}

/// Persistent eval server: reads Kore source programs from stdin, one per line,
/// compiles + proof-checks + runs each in-process, writes result to stdout.
///
/// Protocol (newline-delimited):
///   → input:  Kore source code  (e.g. "3 5 + dup *")
///             OR JSON: {"source": "3 5 +", "max_steps": 10000}
///   ← output: JSON with rich feedback:
///     {"ok":true, "stage":"run", "result":"Int(8)", "stack":["Int(8)"],
///      "steps":5, "compile_ok":true, "typecheck_ok":true}
///     {"ok":false, "stage":"typecheck", "error":"...", "type_errors":["..."],
///      "steps":0, "compile_ok":true, "typecheck_ok":false}
///
/// Eliminates OS process-creation overhead (~1.3 ms per call → ~0 ms).
/// Start one `korec serve` per worker, keep it alive for the whole training run.
fn run_serve(prelude_source: Option<String>) {
    use std::io::{self, BufRead, Write};

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());

    // Default max steps (0 = unlimited, but for training safety use 100K)
    let default_max_steps: usize = 100_000;

    // Pre-compile the prelude once to validate it
    if let Some(ref prelude) = prelude_source {
        match parser::compile(prelude) {
            Ok(_) => {
                let _ = writeln!(io::stderr(), "[serve] prelude loaded ({} bytes)", prelude.len());
            }
            Err(e) => {
                let _ = writeln!(io::stderr(), "[serve] WARNING: prelude has compile errors: {}", e);
            }
        }
    }

    for line in stdin.lock().lines() {
        let source = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let trimmed = source.trim();
        if trimmed.is_empty() {
            let _ = writeln!(out, "{{\"ok\":true,\"stage\":\"run\",\"result\":\"Nil\",\"stack\":[],\"steps\":0,\"compile_ok\":true,\"typecheck_ok\":true}}");
            let _ = out.flush();
            continue;
        }

        // Parse input: either raw source or JSON {"source": "...", "max_steps": N}
        let (program_source, max_steps) = if trimmed.starts_with('{') {
            // Try JSON parse (minimal — avoid serde dependency)
            let src = extract_json_string(trimmed, "source").unwrap_or_default();
            let steps = extract_json_int(trimmed, "max_steps").unwrap_or(default_max_steps as i64) as usize;
            if src.is_empty() {
                // Not valid JSON input, treat as raw source
                (trimmed.to_string(), default_max_steps)
            } else {
                (src, steps)
            }
        } else {
            (trimmed.to_string(), default_max_steps)
        };

        // Prepend prelude to program source if available
        let full_source = if let Some(ref prelude) = prelude_source {
            format!("{}\n{}", prelude, program_source)
        } else {
            program_source
        };

        // Stage 1: Compile from source string (no disk I/O)
        let compile_result = parser::compile(&full_source);
        let mut module = match compile_result {
            Ok(m) => m,
            Err(e) => {
                let escaped = e.replace('\\', "\\\\").replace('"', "\\\"");
                let _ = writeln!(out, "{{\"ok\":false,\"stage\":\"compile\",\"error\":\"{}\",\"steps\":0,\"compile_ok\":false,\"typecheck_ok\":false}}", escaped);
                let _ = out.flush();
                continue;
            }
        };

        // Optimize
        let (optimized, offset_map) = optimizer::optimize_with_offsets(&module.code);
        module.code = optimized;
        let mut new_symbol_table = std::collections::HashMap::new();
        for (idx, old_offset) in &module.symbol_table {
            let new_offset = offset_map.get(old_offset).copied().unwrap_or(*old_offset);
            new_symbol_table.insert(*idx, new_offset);
        }
        module.symbol_table = new_symbol_table;

        // Stage 2: Proof check
        let mut checker = ProofChecker::new();
        let errors = checker.check(&module.code);
        if !errors.is_empty() {
            let type_errors: Vec<String> = errors.iter()
                .map(|e| e.message.replace('\\', "\\\\").replace('"', "\\\""))
                .collect();
            let type_errors_json: Vec<String> = type_errors.iter()
                .map(|s| format!("\"{}\"", s))
                .collect();
            let msgs: Vec<String> = errors.iter()
                .map(|e| format!("{:04X}: {}", e.address, e.message))
                .collect();
            let joined = msgs.join("; ").replace('\\', "\\\\").replace('"', "\\\"");
            let _ = writeln!(out, "{{\"ok\":false,\"stage\":\"typecheck\",\"error\":\"{}\",\"type_errors\":[{}],\"steps\":0,\"compile_ok\":true,\"typecheck_ok\":false}}",
                joined, type_errors_json.join(","));
            let _ = out.flush();
            continue;
        }

        // Stage 3: Run (in-process, no OS process creation)
        let mut interp = Interpreter::from_module_with_caps(&module, 0); // pure — no caps
        if max_steps > 0 {
            interp.set_max_steps(max_steps);
        }
        match interp.run() {
            Ok(()) => {
                let steps = interp.steps_executed();
                let stack: Vec<String> = interp.stack().iter()
                    .map(|v| format!("{:?}", v))
                    .collect();
                let result = if let Some(top) = interp.result() {
                    format!("{:?}", top)
                } else {
                    "Nil".to_string()
                };
                let result_escaped = result.replace('\\', "\\\\").replace('"', "\\\"");
                let stack_json: Vec<String> = stack.iter()
                    .map(|s| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")))
                    .collect();
                let _ = writeln!(out, "{{\"ok\":true,\"stage\":\"run\",\"result\":\"{}\",\"stack\":[{}],\"steps\":{},\"compile_ok\":true,\"typecheck_ok\":true}}",
                    result_escaped, stack_json.join(","), steps);
                let _ = out.flush();
            }
            Err(e) => {
                let steps = interp.steps_executed();
                let escaped = e.replace('\\', "\\\\").replace('"', "\\\"");
                // Include partial stack for debugging
                let stack: Vec<String> = interp.stack().iter()
                    .map(|v| format!("{:?}", v))
                    .collect();
                let stack_json: Vec<String> = stack.iter()
                    .map(|s| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")))
                    .collect();
                let _ = writeln!(out, "{{\"ok\":false,\"stage\":\"run\",\"error\":\"{}\",\"partial_stack\":[{}],\"steps\":{},\"compile_ok\":true,\"typecheck_ok\":true}}",
                    escaped, stack_json.join(","), steps);
                let _ = out.flush();
            }
        }
    }
}

/// Minimal JSON string field extractor (avoids serde dependency for serve mode)
fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    let pos = json.find(&pattern)?;
    let after_key = &json[pos + pattern.len()..];
    let colon = after_key.find(':')?;
    let after_colon = after_key[colon + 1..].trim_start();
    if !after_colon.starts_with('"') { return None; }
    let content = &after_colon[1..];
    let mut result = String::new();
    let mut chars = content.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => return Some(result),
            '\\' => {
                if let Some(escaped) = chars.next() {
                    match escaped {
                        'n' => result.push('\n'),
                        't' => result.push('\t'),
                        '\\' => result.push('\\'),
                        '"' => result.push('"'),
                        _ => { result.push('\\'); result.push(escaped); }
                    }
                }
            }
            _ => result.push(ch),
        }
    }
    None
}

/// Minimal JSON integer field extractor
fn extract_json_int(json: &str, key: &str) -> Option<i64> {
    let pattern = format!("\"{}\"", key);
    let pos = json.find(&pattern)?;
    let after_key = &json[pos + pattern.len()..];
    let colon = after_key.find(':')?;
    let after_colon = after_key[colon + 1..].trim_start();
    let num_str: String = after_colon.chars().take_while(|c| c.is_ascii_digit() || *c == '-').collect();
    num_str.parse().ok()
}

fn run_example() {
    println!("Building factorial(5) example...\n");

    let mut asm = Assembler::new();

    // Main: 5 factorial halt
    asm.emit_i8(5);
    asm.emit_call(0); // call factorial
    asm.emit(Op::Halt);

    // factorial function (symbol 0, starts at offset 6)
    asm.label("factorial");
    asm.emit(Op::Dup);       // n n
    asm.emit_i8(1);          // n n 1
    asm.emit(Op::Le);        // n (n<=1)
    asm.emit_jz("recurse");  // n  (jump if false)
    
    // base case: n <= 1, return 1
    asm.emit(Op::Drop);      // 
    asm.emit_i8(1);          // 1
    asm.emit(Op::Ret);       // return 1
    
    // recursive case
    asm.label("recurse");
    asm.emit(Op::Dup);       // n n
    asm.emit_i8(1);          // n n 1
    asm.emit(Op::Sub);       // n (n-1)
    asm.emit_call(0);        // n factorial(n-1)
    asm.emit(Op::Mul);       // n * factorial(n-1)
    asm.emit(Op::Ret);

    let module = asm.finalize().expect("Assembly failed");

    println!("Disassembly:");
    println!("{}", disassemble(&module.code));

    // Save to file
    let binary = module.encode();
    fs::write("factorial.korec", &binary).expect("Failed to write file");
    println!("Written to factorial.korec ({} bytes)\n", binary.len());

    // Run it!
    println!("Executing with trace...\n");
    let mut interp = Interpreter::new(&module.code);
    interp.enable_trace();
    
    match interp.run() {
        Ok(()) => {
            println!("\n✓ Result: {:?}", interp.result());
        }
        Err(e) => {
            eprintln!("\n✗ Error: {}", e);
        }
    }
}

fn run_tests() {
    println!("Running Kore Bytecode Tests\n");
    println!("============================\n");

    let mut passed = 0;
    let mut failed = 0;

    // Test 1: Simple addition
    {
        print!("Test 1: 3 + 4 = 7 ... ");
        let mut asm = Assembler::new();
        asm.emit_i8(3);
        asm.emit_i8(4);
        asm.emit(Op::Add);
        asm.emit(Op::Halt);
        
        let module = asm.finalize().unwrap();
        let mut interp = Interpreter::new(&module.code);
        interp.run().unwrap();
        
        match interp.result() {
            Some(Value::Int(7)) => { println!("✓"); passed += 1; }
            other => { println!("✗ got {:?}", other); failed += 1; }
        }
    }

    // Test 2: Factorial
    {
        print!("Test 2: factorial(5) = 120 ... ");
        let mut asm = Assembler::new();
        asm.emit_i8(5);
        asm.emit_call_label("factorial");
        asm.emit(Op::Halt);
        asm.label("factorial");
        asm.emit(Op::Dup);
        asm.emit_i8(1);
        asm.emit(Op::Le);
        asm.emit_jz("recurse");
        asm.emit(Op::Drop);
        asm.emit_i8(1);
        asm.emit(Op::Ret);
        asm.label("recurse");
        asm.emit(Op::Dup);
        asm.emit_i8(1);
        asm.emit(Op::Sub);
        asm.emit_call_label("factorial");
        asm.emit(Op::Mul);
        asm.emit(Op::Ret);

        let module = asm.finalize().unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        match interp.result() {
            Some(Value::Int(120)) => { println!("✓"); passed += 1; }
            other => { println!("✗ got {:?}", other); failed += 1; }
        }
    }

    // Test 3: Pair/Unpair
    {
        print!("Test 3: unpair(pair(1,2)) then add = 3 ... ");
        let mut asm = Assembler::new();
        asm.emit_i8(1);
        asm.emit_i8(2);
        asm.emit(Op::Pair);
        asm.emit(Op::Unpair);
        asm.emit(Op::Add);
        asm.emit(Op::Halt);
        
        let module = asm.finalize().unwrap();
        let mut interp = Interpreter::new(&module.code);
        interp.run().unwrap();
        
        match interp.result() {
            Some(Value::Int(3)) => { println!("✓"); passed += 1; }
            other => { println!("✗ got {:?}", other); failed += 1; }
        }
    }

    // Test 4: Dup and Mul (square)
    {
        print!("Test 4: 7 dup mul = 49 ... ");
        let mut asm = Assembler::new();
        asm.emit_i8(7);
        asm.emit(Op::Dup);
        asm.emit(Op::Mul);
        asm.emit(Op::Halt);
        
        let module = asm.finalize().unwrap();
        let mut interp = Interpreter::new(&module.code);
        interp.run().unwrap();
        
        match interp.result() {
            Some(Value::Int(49)) => { println!("✓"); passed += 1; }
            other => { println!("✗ got {:?}", other); failed += 1; }
        }
    }

    // Test 5: Swap
    {
        print!("Test 5: 10 3 swap sub = -7 ... ");
        let mut asm = Assembler::new();
        asm.emit_i8(10);
        asm.emit_i8(3);
        asm.emit(Op::Swap);
        asm.emit(Op::Sub);
        asm.emit(Op::Halt);
        
        let module = asm.finalize().unwrap();
        let mut interp = Interpreter::new(&module.code);
        interp.run().unwrap();
        
        // 10 3 swap → 3 10, then sub → 3-10 = -7
        match interp.result() {
            Some(Value::Int(-7)) => { println!("✓"); passed += 1; }
            other => { println!("✗ got {:?}", other); failed += 1; }
        }
    }

    // Test 6: Left construction
    {
        print!("Test 6: Left(42) ... ");
        let mut asm = Assembler::new();
        asm.emit_i8(42);
        asm.emit(Op::Left);
        asm.emit(Op::Halt);
        
        let module = asm.finalize().unwrap();
        let mut interp = Interpreter::new(&module.code);
        interp.run().unwrap();
        
        match interp.result() {
            Some(Value::Left(v)) => match v.as_ref() {
                Value::Int(42) => { println!("✓"); passed += 1; }
                _ => { println!("✗ wrong inner value"); failed += 1; }
            }
            other => { println!("✗ got {:?}", other); failed += 1; }
        }
    }

    // Test 7: Comparison
    {
        print!("Test 7: 5 < 10 = true ... ");
        let mut asm = Assembler::new();
        asm.emit_i8(5);
        asm.emit_i8(10);
        asm.emit(Op::Lt);
        asm.emit(Op::Halt);
        
        let module = asm.finalize().unwrap();
        let mut interp = Interpreter::new(&module.code);
        interp.run().unwrap();
        
        match interp.result() {
            Some(Value::Bool(true)) => { println!("✓"); passed += 1; }
            other => { println!("✗ got {:?}", other); failed += 1; }
        }
    }

    // Test 8: Nested pairs (complex number simulation)
    {
        print!("Test 8: Complex (3,4) magnitude² = 25 ... ");
        // (3,4) → 3² + 4² = 9 + 16 = 25
        let mut asm = Assembler::new();
        asm.emit_i8(3);      // real
        asm.emit_i8(4);      // imag
        asm.emit(Op::Pair);  // (3,4)
        asm.emit(Op::Unpair);// 3 4
        asm.emit(Op::Dup);   // 3 4 4
        asm.emit(Op::Mul);   // 3 16
        asm.emit(Op::Swap);  // 16 3
        asm.emit(Op::Dup);   // 16 3 3
        asm.emit(Op::Mul);   // 16 9
        asm.emit(Op::Add);   // 25
        asm.emit(Op::Halt);
        
        let module = asm.finalize().unwrap();
        let mut interp = Interpreter::new(&module.code);
        interp.run().unwrap();
        
        match interp.result() {
            Some(Value::Int(25)) => { println!("✓"); passed += 1; }
            other => { println!("✗ got {:?}", other); failed += 1; }
        }
    }

    println!("\n--- Type Checker Tests (P4) ---\n");

    // Test 9: Type check valid code
    {
        print!("Test 9: Type check 3+4 (valid) ... ");
        let mut asm = Assembler::new();
        asm.emit_i8(3);
        asm.emit_i8(4);
        asm.emit(Op::Add);
        asm.emit(Op::Halt);
        
        let module = asm.finalize().unwrap();
        let mut checker = ProofChecker::new();
        let errors = checker.check(&module.code);
        
        if errors.is_empty() {
            println!("✓");
            passed += 1;
        } else {
            println!("✗ unexpected errors: {:?}", errors);
            failed += 1;
        }
    }

    // Test 10: Detect stack underflow
    {
        print!("Test 10: Detect stack underflow ... ");
        let mut asm = Assembler::new();
        asm.emit(Op::Add);  // Stack empty!
        asm.emit(Op::Halt);
        
        let module = asm.finalize().unwrap();
        let mut checker = ProofChecker::new();
        let errors = checker.check(&module.code);
        
        if !errors.is_empty() {
            println!("✓ (caught: {})", errors[0].message);
            passed += 1;
        } else {
            println!("✗ should have detected underflow");
            failed += 1;
        }
    }

    // Test 11: Detect type mismatch
    {
        print!("Test 11: Detect type mismatch (Bool AND Int) ... ");
        let mut asm = Assembler::new();
        asm.emit(Op::True);
        asm.emit_i8(5);
        asm.emit(Op::And);  // Expects Bool, Bool
        asm.emit(Op::Halt);
        
        let module = asm.finalize().unwrap();
        let mut checker = ProofChecker::new();
        let errors = checker.check(&module.code);
        
        if !errors.is_empty() {
            println!("✓ (caught: {})", errors[0].message);
            passed += 1;
        } else {
            println!("✗ should have detected type mismatch");
            failed += 1;
        }
    }

    // Test 12: Type check pair/unpair
    {
        print!("Test 12: Type check pair/unpair ... ");
        let mut asm = Assembler::new();
        asm.emit_i8(1);
        asm.emit_i8(2);
        asm.emit(Op::Pair);
        asm.emit(Op::Unpair);
        asm.emit(Op::Add);
        asm.emit(Op::Halt);
        
        let module = asm.finalize().unwrap();
        let mut checker = ProofChecker::new();
        let errors = checker.check(&module.code);
        
        if errors.is_empty() && checker.final_stack() == &[Type::Int] {
            println!("✓");
            passed += 1;
        } else {
            println!("✗ errors={:?} stack={:?}", errors, checker.final_stack());
            failed += 1;
        }
    }

    println!("\n============================");
    println!("Passed: {}, Failed: {}", passed, failed);
    
    if failed > 0 {
        std::process::exit(1);
    }
}
