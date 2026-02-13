//! Bytecode-level greedy pruner for Kore programs.
//!
//! Strategy:
//! 1. Compile source → bytecode once
//! 2. Decompose bytecode into instruction stream, compute per-instruction stack effects
//! 3. Build prefix sums, find all spans [i,j) where net stack effect = 0
//! 4. Sort spans by size descending (try removing largest chunks first)
//! 5. For each candidate: NOP out the span, re-run interpreter, compare outputs
//! 6. Keep removal if outputs stay within epsilon
//!
//! Speed: ~1ms per interpreter run (Rust VM on ~200K bytecodes),
//! 10 images × ~1ms = ~10ms per candidate check.
//! With ~500 candidates = ~5 seconds total.

use std::collections::HashMap;
use crate::bytecode::{BytecodeModule, Op};
use crate::interpreter::{Interpreter, Value};
use crate::optimizer;

// ============================================================================
// INSTRUCTION REPRESENTATION
// ============================================================================

/// A decoded instruction with its position and size in the bytecode
#[derive(Debug, Clone)]
struct Instr {
    offset: usize,     // byte offset in code[]
    size: usize,       // total bytes (opcode + operands)
    op: Op,
    effect: i32,       // net stack effect
}

/// Compute the size of an instruction at the given offset
fn instruction_size(code: &[u8], offset: usize) -> usize {
    if offset >= code.len() { return 0; }
    let op = code[offset];
    match Op::from_byte(op) {
        Some(Op::Int8) => 2,
        Some(Op::Int16) => 3,
        Some(Op::Int32) => 5,
        Some(Op::Int64) => 9,
        Some(Op::F32) => 5,
        Some(Op::F64) => 9,
        Some(Op::Str) => {
            if offset + 3 <= code.len() {
                let len = u16::from_le_bytes([code[offset+1], code[offset+2]]) as usize;
                3 + len
            } else { 1 }
        }
        Some(Op::Quote) => 3,  // opcode + u16 length prefix (body follows inline)
        Some(Op::Call) => 3,
        Some(Op::Case) => 5,   // opcode + i16 left_off + i16 right_off
        Some(Op::Jmp) | Some(Op::Jz) | Some(Op::Jnz) => 5,
        Some(Op::Store) | Some(Op::Load) => 5,
        Some(Op::List) => 3,
        Some(Op::Syscall) => 2,
        Some(_) => 1,
        None => 1,
    }
}

/// Compute the net stack effect of an opcode.
/// For structural ops (jumps, calls, control flow) the value doesn't matter
/// since they act as segment boundaries in span finding.
fn stack_effect(op: Op) -> i32 {
    match op {
        // Stack
        Op::Nop => 0,
        Op::Drop => -1,
        Op::Dup => 1,
        Op::Swap => 0,
        Op::Rot => 0,
        Op::Over => 1,

        // Data types
        Op::Pair => -1,     // pop 2, push 1
        Op::Unpair => 1,    // pop 1, push 2
        Op::Left => 0,      // pop 1, push 1
        Op::Right => 0,     // pop 1, push 1

        // Literals — all push 1
        Op::Int8 | Op::Int16 | Op::Int32 | Op::Int64 => 1,
        Op::F32 | Op::F64 => 1,
        Op::Nil | Op::True | Op::False => 1,

        // Arithmetic (binary: pop 2, push 1 = -1)
        Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Mod => -1,
        Op::Neg => 0,
        Op::Fadd | Op::Fsub | Op::Fmul | Op::Fdiv => -1,
        Op::Fneg | Op::Fsqrt | Op::Fabs | Op::Fexp | Op::Flog => 0,
        Op::Fsin | Op::Fcos => 0,
        Op::Fatan2 | Op::Fpow => -1,
        Op::Ffloor | Op::Fceil | Op::Fround => 0,
        Op::I2f | Op::F2i => 0,

        // Comparison (binary: pop 2, push 1 = -1)
        Op::Eq | Op::Lt | Op::Gt | Op::Le | Op::Ge | Op::Ne => -1,

        // Logic
        Op::And | Op::Or | Op::Xor => -1,
        Op::Not => 0,
        Op::Band | Op::Bor | Op::Bxor => -1,
        Op::Bnot => 0,
        Op::Shl | Op::Shr => -1,

        // Locals
        Op::Store => -1,  // pop value, store to slot
        Op::Load => 1,    // push value from slot

        // IO
        Op::Print | Op::Println => -1,  // pop 1, push 0
        Op::Rand => 1,                  // pop 0, push 1

        // Reflection
        Op::Fetch => 0,  // pop 1 (offset), push 1 (byte)
        Op::Size => 1,   // pop 0, push 1
        Op::Depth => 1,  // pop 0, push 1

        // List ops (these are structural-ish but effect is known)
        Op::Get => -1,       // pop 2 (list, i), push 1
        Op::Set => -2,       // pop 3 (list, i, v), push 1
        Op::Len => 1,        // peeks list (no pop!), pushes 1
        Op::Append => -1,    // pop 2, push 1
        Op::Reverse => 0,    // pop 1, push 1
        Op::Zip => -1,       // pop 2, push 1
        Op::Head => 0,       // pop 1, push 1
        Op::Tail => 0,       // pop 1, push 1
        Op::Range => -1,     // pop 2, push 1
        Op::First => 1,      // peek pair, push a
        Op::Second => 1,     // peek pair, push b
        Op::ListConcat => -1,// pop 2, push 1
        Op::IsEmpty => 1,    // peek list, push bool
        Op::Filter => -1,    // pop 2 (list, quote), push 1
        Op::Map => -1,       // pop 2 (list, quote), push 1
        Op::Fold => -2,      // pop 3 (list, init, quote), push 1

        // Array ops
        Op::ArrayNew => 0,   // pop 1 (n), push 1 (array)
        Op::ArrayGet => 0,   // pop 2 (array, i), push 2 (array, elem)
        Op::ArraySet => -2,  // pop 3 (array, i, v), push 1 (array)
        Op::ArrayLen => 1,   // peek array, push n
        Op::ArrayPush => -1, // pop 2 (array, v), push 1 (array)
        Op::ArrayFrom => 0,  // pop 1 (list), push 1 (array)

        // HashMap ops
        Op::MapNew => 1,     // push 1
        Op::MapGet => 0,     // pop 2 (map, key), push 2 (map, value)
        Op::MapSet => -2,    // pop 3 (map, key, value), push 1 (map)
        Op::MapKeys => 1,    // peek map, push list
        Op::MapHas => 0,     // pop 2 (map, key), push 2 (map, bool)

        // Error handling
        Op::IsError => 1,    // peek value, push bool
        Op::MakeError => 0,  // pop 1 (str), push 1 (error)
        Op::Propagate => 0,  // pop 1, push 1 (or aborts)

        // Fiber ops
        Op::FiberNew => 0,   // pop 1 (quote), push 1 (fiber)
        Op::FiberStep => 1,  // pop 1 (fiber), push 2 (fiber', bool)
        Op::FiberPush => -1, // pop 2 (fiber, value), push 1 (fiber')
        Op::FiberStack => 1, // pop 1 (fiber), push 2 (fiber, list)
        Op::FiberStatus => 1,// pop 1 (fiber), push 2 (fiber, bool)
        Op::Spawn => -1,     // pop 2 (quote, caps), push 1 (result)
        Op::ChanNew => 1,    // push 1
        Op::ChanSend => -2,  // pop 2 (chan, value), push 0
        Op::ChanRecv => 0,   // pop 1 (chan), push 1 (value)

        // Linear types
        Op::Linear | Op::Affine | Op::Consume => 0,  // pop 1, push 1
        Op::IsLinear | Op::IsAffine => 1,             // peek, push bool

        // Introspection
        Op::TypeOf => 1,     // peek value, push type str
        Op::Describe => 0,   // pop 1, push 1

        // String ops
        Op::StrLen => 1,     // peek str, push n
        Op::StrGet => -1,    // pop 2 (str, i), push 1 (char-str)
        Op::StrConcat => -1, // pop 2, push 1
        Op::StrSlice => -2,  // pop 3 (str, start, end), push 1
        Op::ToStr => 0,      // pop 1, push 1
        Op::StrFind => -1,   // pop 2 (str, pattern), push 1 (int)
        Op::StrSplit => -1,  // pop 2 (str, delim), push 1 (list)
        Op::StrReplace => -2,// pop 3 (str, from, to), push 1
        Op::StrUpper | Op::StrLower | Op::StrTrim => 0,  // pop 1, push 1
        Op::ParseFloat | Op::ParseInt => 0,  // pop 1, push 1

        // Control flow — these are "structural", effect doesn't matter
        // since they act as segment boundaries
        Op::Jmp => 0,
        Op::Jz | Op::Jnz => -1,  // pop condition
        Op::Call | Op::Ret => 0,
        Op::Halt => 0,
        Op::Quote => 1,   // pushes a quote value
        Op::Apply => -1,  // pops a quote, executes inline

        // Complex control flow (structural — segment boundaries)
        Op::Case => 0,
        Op::Cond => -3,   // pops bool + 2 quotes, but dynamic
        Op::Loop => -1,   // pops quote, but dynamic
        Op::Times => -2,  // pops n + quote, but dynamic
        Op::Try => -1,    // pops quote, dynamic
        Op::Fail => -1,   // pops str, unwinds

        // Variable-effect ops (structural — can't predict)
        Op::List => 0,    // pops N, pushes 1 — variable
        Op::Unlist => 0,  // pops 1, pushes N — variable
        Op::Syscall => 0, // varies by call_id

        // Everything else: treat as 0 (conservative)
        _ => 0,
    }
}

// ============================================================================
// INSTRUCTION STREAM DECODER
// ============================================================================

/// Decode bytecode into a list of instructions
fn decode_instructions(code: &[u8]) -> Vec<Instr> {
    let mut instrs = Vec::new();
    let mut offset = 0;
    while offset < code.len() {
        let size = instruction_size(code, offset);
        let op = Op::from_byte(code[offset]).unwrap_or(Op::Nop);
        let effect = stack_effect(op);
        instrs.push(Instr { offset, size, op, effect });
        offset += size;
    }
    instrs
}

// ============================================================================
// STRUCTURAL DETECTION
// ============================================================================

/// Check if an instruction is "structural" — acts as a segment boundary
/// for span finding. These are never included in removable spans because
/// they have control-flow effects, variable stack effects, or inline data.
fn is_structural(op: Op) -> bool {
    matches!(op,
        // Control flow
        Op::Jmp | Op::Jz | Op::Jnz |
        Op::Call | Op::Ret | Op::Halt |
        Op::Quote | Op::Apply |
        // Complex/dynamic control flow
        Op::Cond | Op::Loop | Op::Times | Op::Case |
        Op::Try | Op::Fail |
        // Variable stack effect
        Op::List | Op::Unlist | Op::Syscall |
        // Inline data (must not corrupt)
        Op::Str
    )
}

// ============================================================================
// SPAN FINDING
// ============================================================================

/// Check if a span [byte_start, byte_end) is entirely NOP'd already
fn is_all_nops(code: &[u8], byte_start: usize, byte_end: usize) -> bool {
    let nop_byte = Op::Nop as u8;
    for i in byte_start..byte_end.min(code.len()) {
        if code[i] != nop_byte {
            return false;
        }
    }
    true
}

/// Count how many non-NOP bytes are in a span
fn non_nop_bytes(code: &[u8], byte_start: usize, byte_end: usize) -> usize {
    let nop_byte = Op::Nop as u8;
    let mut count = 0;
    for i in byte_start..byte_end.min(code.len()) {
        if code[i] != nop_byte {
            count += 1;
        }
    }
    count
}

/// Find all net-0 spans in the instruction stream, sorted by size descending.
/// A span [i, j) of instructions has net effect 0 if prefix[i] == prefix[j].
/// Only considers spans within structural-free segments.
/// Filters out spans that are entirely NOP'd already.
fn find_net0_spans(instrs: &[Instr], code: &[u8]) -> Vec<(usize, usize, usize)> {
    // Split into segments between structural instructions
    let n = instrs.len();

    // Compute prefix sums of stack effects
    let mut prefix = vec![0i32; n + 1];
    for i in 0..n {
        prefix[i + 1] = prefix[i] + instrs[i].effect;
    }

    // Find structural-free segments
    let mut segments: Vec<(usize, usize)> = Vec::new();
    let mut seg_start = 0;
    for i in 0..n {
        if is_structural(instrs[i].op) {
            if i > seg_start {
                segments.push((seg_start, i));
            }
            seg_start = i + 1;
        }
    }
    if n > seg_start {
        segments.push((seg_start, n));
    }

    let mut spans: Vec<(usize, usize, usize)> = Vec::new(); // (byte_size, instr_start, instr_end)

    for (seg_start, seg_end) in &segments {
        let seg_start = *seg_start;
        let seg_end = *seg_end;

        // Build hashmap: prefix_value -> list of instruction indices
        let mut val_to_pos: HashMap<i32, Vec<usize>> = HashMap::new();
        for i in seg_start..=seg_end {
            if i <= n {
                val_to_pos.entry(prefix[i]).or_default().push(i);
            }
        }

        // For each start position, find the farthest end with same prefix value
        for i in seg_start..seg_end {
            let val = prefix[i];
            if let Some(positions) = val_to_pos.get(&val) {
                // Find farthest j within this segment
                let j = *positions.last().unwrap();
                if j > i && j <= seg_end {
                    // Compute byte size of the span
                    let byte_start = instrs[i].offset;
                    let byte_end = if j < instrs.len() {
                        instrs[j].offset
                    } else {
                        instrs.last().map(|x| x.offset + x.size).unwrap_or(0)
                    };
                    let byte_size = byte_end - byte_start;

                    // Skip spans that are entirely NOP'd already
                    if !is_all_nops(code, byte_start, byte_end) {
                        spans.push((byte_size, i, j));
                    }
                }
            }
        }
    }

    // Sort by non-NOP byte count descending (= actual bytes we'd remove)
    // This way we try removing the most *real* code first
    spans.sort_by(|a, b| {
        let a_start = instrs[a.1].offset;
        let a_end = if a.2 < instrs.len() { instrs[a.2].offset }
                    else { instrs.last().map(|x| x.offset + x.size).unwrap_or(0) };
        let b_start = instrs[b.1].offset;
        let b_end = if b.2 < instrs.len() { instrs[b.2].offset }
                    else { instrs.last().map(|x| x.offset + x.size).unwrap_or(0) };
        let a_real = non_nop_bytes(code, a_start, a_end);
        let b_real = non_nop_bytes(code, b_start, b_end);
        b_real.cmp(&a_real)
    });

    // Deduplicate: keep only the largest span per start position
    let mut seen_starts = std::collections::HashSet::new();
    spans.retain(|&(_, start, _)| seen_starts.insert(start));

    spans
}

// ============================================================================
// BYTECODE COMPACTION
// ============================================================================

/// Strip all NOP instructions from bytecode and rewrite jump/quote/case offsets.
/// Returns (compacted_code, offset_map) where offset_map[old_pos] = new_pos.
fn compact_bytecode(code: &[u8]) -> (Vec<u8>, Vec<usize>) {
    let instrs = decode_instructions(code);

    // Build removed_before[pos] = cumulative NOP bytes before this position
    let mut removed_before = vec![0usize; code.len() + 1];
    let mut cum_removed = 0usize;
    for instr in &instrs {
        for i in instr.offset..instr.offset + instr.size {
            if i < removed_before.len() {
                removed_before[i] = cum_removed;
            }
        }
        if instr.op == Op::Nop {
            cum_removed += instr.size;
        }
    }
    removed_before[code.len()] = cum_removed;

    // offset_map: old_pos → new_pos
    let offset_map: Vec<usize> = (0..=code.len())
        .map(|i| i - removed_before[i])
        .collect();

    if cum_removed == 0 {
        return (code.to_vec(), offset_map);
    }

    // Build new code, rewriting offsets for jumps/quotes/cases
    let mut new_code = Vec::with_capacity(code.len() - cum_removed);

    for instr in &instrs {
        if instr.op == Op::Nop {
            continue;
        }

        let pos = instr.offset;

        match instr.op {
            Op::Jmp | Op::Jz | Op::Jnz => {
                // opcode(1) + i32 relative offset(4)
                // target = pos + 5 + old_rel
                new_code.push(code[pos]);
                let old_rel = i32::from_le_bytes([
                    code[pos+1], code[pos+2], code[pos+3], code[pos+4]
                ]);
                let old_target = (pos as i64 + 5 + old_rel as i64) as usize;
                let new_pos = offset_map[pos];
                let new_target = if old_target <= code.len() {
                    offset_map[old_target]
                } else {
                    old_target - removed_before[code.len()]
                };
                let new_rel = (new_target as i64 - (new_pos as i64 + 5)) as i32;
                new_code.extend_from_slice(&new_rel.to_le_bytes());
            }
            Op::Quote => {
                // opcode(1) + u16 body_len(2), body follows inline
                new_code.push(code[pos]);
                let old_len = u16::from_le_bytes([code[pos+1], code[pos+2]]) as usize;
                let body_start = pos + 3;
                let body_end = body_start + old_len;
                let new_body_start = offset_map[body_start.min(code.len())];
                let new_body_end = offset_map[body_end.min(code.len())];
                let new_len = (new_body_end - new_body_start) as u16;
                new_code.extend_from_slice(&new_len.to_le_bytes());
            }
            Op::Case => {
                // opcode(1) + i16 left_off(2) + i16 right_off(2)
                // Both offsets relative to pos + 5
                new_code.push(code[pos]);
                let old_left = i16::from_le_bytes([code[pos+1], code[pos+2]]) as i64;
                let old_right = i16::from_le_bytes([code[pos+3], code[pos+4]]) as i64;
                let old_left_target = (pos as i64 + 5 + old_left) as usize;
                let old_right_target = (pos as i64 + 5 + old_right) as usize;
                let new_pos = offset_map[pos];
                let new_left_target = offset_map[old_left_target.min(code.len())];
                let new_right_target = offset_map[old_right_target.min(code.len())];
                let new_left = (new_left_target as i64 - (new_pos as i64 + 5)) as i16;
                let new_right = (new_right_target as i64 - (new_pos as i64 + 5)) as i16;
                new_code.extend_from_slice(&new_left.to_le_bytes());
                new_code.extend_from_slice(&new_right.to_le_bytes());
            }
            _ => {
                // Copy instruction bytes as-is
                new_code.extend_from_slice(&code[pos..pos + instr.size]);
            }
        }
    }

    (new_code, offset_map)
}

// ============================================================================
// CANDIDATE EVALUATION
// ============================================================================

/// Build a patched bytecode by NOP-ing out a byte range [byte_start, byte_end).
/// We replace with NOP instructions so jump offsets remain valid.
fn nop_out(code: &[u8], byte_start: usize, byte_end: usize) -> Vec<u8> {
    let mut patched = code.to_vec();
    for i in byte_start..byte_end {
        patched[i] = Op::Nop as u8;
    }
    patched
}

/// Run interpreter on bytecode with the given module's symbol table.
/// Returns the final stack as Vec<f64>, or None on error.
fn run_bytecode(code: &[u8], symbols: &HashMap<u16, usize>, max_steps: usize) -> Option<Vec<f64>> {
    let mut module = BytecodeModule::new();
    module.code = code.to_vec();
    module.symbol_table = symbols.clone();

    let mut interp = Interpreter::from_module_with_caps(&module, 0);
    if max_steps > 0 {
        interp.set_max_steps(max_steps);
    }

    match interp.run() {
        Ok(()) => {
            let floats: Vec<f64> = interp.stack().iter().filter_map(|v| match v {
                Value::Float(f) => Some(*f),
                Value::Int(i) => Some(*i as f64),
                _ => None,
            }).collect();
            Some(floats)
        }
        Err(_) => None,
    }
}

// ============================================================================
// MAIN PRUNING LOOP
// ============================================================================

/// Configuration for the pruner
pub struct PruneConfig {
    pub eps: f64,
    pub max_steps: usize,
    pub batch_file: String,
    pub labels_file: String,
    pub output_file: String,
    pub source_file: String,
}

/// Load reference images: pixel lines from batch file, one per class
fn load_reference_images(batch_file: &str, labels_file: &str) -> Result<Vec<(usize, u8, String)>, String> {
    let labels: Vec<u8> = std::fs::read_to_string(labels_file)
        .map_err(|e| format!("Failed to read labels: {}", e))?
        .lines()
        .filter_map(|l| l.trim().parse().ok())
        .collect();

    let batch_lines: Vec<String> = std::fs::read_to_string(batch_file)
        .map_err(|e| format!("Failed to read batch: {}", e))?
        .lines()
        .map(|l| l.to_string())
        .collect();

    // Pick first occurrence of each class
    let mut seen = std::collections::HashSet::new();
    let mut images = Vec::new();
    for (i, &label) in labels.iter().enumerate() {
        if seen.insert(label) && i < batch_lines.len() {
            images.push((i, label, batch_lines[i].clone()));
        }
    }
    images.sort_by_key(|x| x.1);

    eprintln!("Selected {} reference images: {:?}",
        images.len(),
        images.iter().map(|(i, l, _)| (*i, *l)).collect::<Vec<_>>()
    );
    Ok(images)
}

/// Run the pruner
pub fn run_prune(config: PruneConfig) -> Result<(), String> {
    use std::time::Instant;

    // ── Read and compile source ──
    let source = std::fs::read_to_string(&config.source_file)
        .map_err(|e| format!("Failed to read source: {}", e))?;

    eprintln!("Compiling {}...", config.source_file);
    let base_dir = std::path::Path::new(&config.source_file)
        .parent().unwrap_or(std::path::Path::new("."));
    let mut module = crate::parser::compile_with_imports(&source, base_dir)?;

    // Optimize
    let (optimized, offset_map) = optimizer::optimize_with_offsets(&module.code);
    module.code = optimized;
    let mut new_st = HashMap::new();
    for (idx, old_off) in &module.symbol_table {
        let new_off = offset_map.get(old_off).copied().unwrap_or(*old_off);
        new_st.insert(*idx, new_off);
    }
    module.symbol_table = new_st;

    let original_size = module.code.len();
    eprintln!("Compiled: {} bytes of bytecode", original_size);

    // ── Load reference images ──
    let images = load_reference_images(&config.batch_file, &config.labels_file)?;

    // ── Build per-image bytecodes (model + pixels + forward call) ──
    // The model is compiled once. For each image, we append: pixel_source + " forward"
    // and compile the full thing as one program.
    eprintln!("Building per-image programs...");
    let mut image_programs: Vec<(BytecodeModule, u8)> = Vec::new();
    for (idx, label, pixels) in &images {
        let full_source = format!("{}\n{} forward", source, pixels);
        let mut img_module = crate::parser::compile_with_imports(&full_source, base_dir)?;
        let (opt, omap) = optimizer::optimize_with_offsets(&img_module.code);
        img_module.code = opt;
        let mut st = HashMap::new();
        for (k, v) in &img_module.symbol_table {
            st.insert(*k, omap.get(v).copied().unwrap_or(*v));
        }
        img_module.symbol_table = st;
        eprintln!("  img {} (label={}): {} bytes", idx, label, img_module.code.len());
        image_programs.push((img_module, *label));
    }

    // ── Get reference outputs ──
    eprintln!("\nComputing reference outputs...");
    let mut ref_outputs: Vec<Vec<f64>> = Vec::new();
    for (img_module, label) in &image_programs {
        let result = run_bytecode(&img_module.code, &img_module.symbol_table, config.max_steps);
        match result {
            Some(floats) => {
                let preview: Vec<String> = floats.iter().map(|f| format!("{:.4}", f)).collect();
                eprintln!("  label={}: [{}]", label, preview.join(", "));
                ref_outputs.push(floats);
            }
            None => {
                return Err(format!("Reference execution failed for label={}", label));
            }
        }
    }
    let n_ref: usize = ref_outputs.iter().map(|r| r.len()).sum();
    eprintln!("Got {} reference values\n", n_ref);

    // ── Find the model bytecode region ──
    // The model is compiled as the prefix of each image program.
    // We need to identify which byte range in the image programs corresponds
    // to the model, so we can NOP out spans there.
    //
    // Strategy: the model-only module (without pixels) gives us the model bytecode.
    // In the full image program, the model bytecode is a prefix (definitions come first).
    // We find the CALL to "forward" and NOP within the function body.
    //
    // Simpler approach: work on the image programs directly.
    // The model definitions are compiled into all image programs identically.
    // We identify the model region by comparing the first N bytes across all programs.
    let model_code_len = module.code.len();
    eprintln!("Model bytecode: {} bytes", model_code_len);

    // Find the shared prefix length across all image programs
    let _min_prog_len = image_programs.iter().map(|(m, _)| m.code.len()).min().unwrap_or(0);

    // ── Decode instructions and find spans in the model region ──
    // Use the first image program as reference for instruction boundaries
    let ref_code = &image_programs[0].0.code;
    let all_instrs = decode_instructions(ref_code);
    eprintln!("Decoded {} instructions ({} bytes)", all_instrs.len(), ref_code.len());

    // Find instructions that belong to the model region
    // Skip: initial JMP (to main), function definitions' structural parts
    // We want to find CALL/RET boundaries for function bodies

    // Find all net-0 spans, sorted largest first
    let t0 = Instant::now();
    let spans = find_net0_spans(&all_instrs, ref_code);
    let dt = t0.elapsed();
    eprintln!("Found {} net-0 spans in {:.1}ms (largest={} bytes)\n",
        spans.len(), dt.as_secs_f64() * 1000.0,
        spans.first().map(|s| s.0).unwrap_or(0));

    // ── Greedy pruning loop ──
    eprintln!("============================================================");
    eprintln!("Greedy pruning (eps={}, largest-first)", config.eps);
    eprintln!("============================================================\n");

    // We work on mutable copies of all image program bytecodes
    let mut programs: Vec<(Vec<u8>, HashMap<u16, usize>)> = image_programs.iter()
        .map(|(m, _)| (m.code.clone(), m.symbol_table.clone()))
        .collect();

    let mut removed_total: usize = 0;
    let mut removals: Vec<(usize, usize, f64)> = Vec::new(); // (byte_start, byte_size, max_diff)
    let mut total_checks: usize = 0;
    let mut round = 0;
    let t_start = Instant::now();

    loop {
        round += 1;
        let round_t0 = Instant::now();

        // Re-decode instructions from current bytecode
        let ref_code = programs[0].0.clone();
        let all_instrs = decode_instructions(&ref_code);

        // Find net-0 spans (filters out already-NOP'd spans)
        let spans = find_net0_spans(&all_instrs, &ref_code);
        if spans.is_empty() {
            eprintln!("Round {}: No more net-0 spans found.", round);
            break;
        }

        eprintln!("Round {}: {} spans (largest={} bytes, smallest={} bytes)",
            round, spans.len(), spans[0].0,
            spans.last().map(|s| s.0).unwrap_or(0));

        let mut round_removals = 0;
        let mut round_checks = 0;
        // Track NOP'd byte ranges this round to skip overlapping candidates
        let mut nopd_ranges: Vec<(usize, usize)> = Vec::new();

        for &(byte_size, instr_start, instr_end) in &spans {
            let byte_start = all_instrs[instr_start].offset;
            let byte_end = if instr_end < all_instrs.len() {
                all_instrs[instr_end].offset
            } else {
                ref_code.len()
            };

            // Skip if this span overlaps with an already-NOP'd range this round
            let overlaps = nopd_ranges.iter().any(|&(s, e)| byte_start < e && byte_end > s);
            if overlaps {
                continue;
            }

            // Skip if there are no real (non-NOP) bytes to remove
            let real_bytes = non_nop_bytes(&programs[0].0, byte_start, byte_end);
            if real_bytes == 0 {
                continue;
            }

            // Check candidate: NOP out the span in all image programs, run each
            total_checks += 1;
            round_checks += 1;
            let mut ok = true;
            let mut max_diff: f64 = 0.0;

            // Progress every 100 checks
            if round_checks % 100 == 0 {
                let dt = t_start.elapsed();
                eprintln!("  ... checked {}/{} in {:.1}s", round_checks, spans.len(), dt.as_secs_f64());
            }

            for (img_idx, (code, syms)) in programs.iter().enumerate() {
                // Only NOP if span is within this program's bytecode
                if byte_end > code.len() {
                    ok = false;
                    break;
                }

                let patched = nop_out(code, byte_start, byte_end);
                let result = run_bytecode(&patched, syms, config.max_steps);

                match result {
                    Some(floats) => {
                        let ref_vals = &ref_outputs[img_idx];
                        if floats.len() != ref_vals.len() {
                            ok = false;
                            break;
                        }
                        for k in 0..ref_vals.len() {
                            let diff = (floats[k] - ref_vals[k]).abs();
                            if diff > max_diff { max_diff = diff; }
                            if diff > config.eps {
                                ok = false;
                                break;
                            }
                        }
                        if !ok { break; }
                    }
                    None => {
                        ok = false;
                        break;
                    }
                }
            }

            if ok {
                // Apply the removal to all programs
                for (code, _) in programs.iter_mut() {
                    for i in byte_start..byte_end.min(code.len()) {
                        code[i] = Op::Nop as u8;
                    }
                }

                removed_total += real_bytes;  // count only actually-removed bytes
                round_removals += 1;
                removals.push((byte_start, byte_size, max_diff));
                nopd_ranges.push((byte_start, byte_end));

                let dt = t_start.elapsed();
                eprintln!("  [{:4}] REMOVED {:5} bytes ({} real) at 0x{:04X} (Δ={:.4}) [{} NOP'd, {:.1}% removed] {:.1}s",
                    total_checks, byte_size, real_bytes, byte_start, max_diff,
                    removed_total, removed_total as f64 / original_size as f64 * 100.0,
                    dt.as_secs_f64());

                // Continue checking non-overlapping spans (don't break)
            }
        }

        let round_dt = round_t0.elapsed();
        eprintln!("  Round {} done: {} checks, {} removals, {:.1}s",
            round, round_checks, round_removals, round_dt.as_secs_f64());

        if round_removals == 0 {
            eprintln!("\nNo more removals possible.");
            break;
        }

        // If we got removals, do another round — some previously-blocked spans
        // may now be removable since the program behavior with NOP'd regions
        // may have shifted enough. But this rarely helps, so limit rounds.
        if round >= 20 {
            eprintln!("\nReached round limit (20).");
            break;
        }
    }

    let dt = t_start.elapsed();
    eprintln!("\n============================================================");
    eprintln!("PRUNING DONE in {:.1}s", dt.as_secs_f64());
    eprintln!("============================================================");
    eprintln!("Original:  {} bytes", original_size);
    eprintln!("NOP'd:     {} bytes ({:.1}%)", removed_total, removed_total as f64 / original_size as f64 * 100.0);
    eprintln!("Removals:  {}", removals.len());
    eprintln!("Checks:    {}", total_checks);

    // ── Compact: strip NOPs, rewrite offsets ──
    eprintln!("\nCompacting bytecode (stripping NOPs, rewriting offsets)...");
    let nop_code = &programs[0].0;
    let nop_syms = &programs[0].1;
    let (compacted_code, offset_map) = compact_bytecode(nop_code);

    // Remap symbol table (absolute offsets into code)
    let mut compacted_syms: HashMap<u16, usize> = HashMap::new();
    for (idx, old_off) in nop_syms {
        let new_off = if *old_off <= offset_map.len() - 1 {
            offset_map[*old_off]
        } else {
            *old_off
        };
        compacted_syms.insert(*idx, new_off);
    }

    let compacted_size = compacted_code.len();
    let saved_bytes = nop_code.len() - compacted_size;
    eprintln!("Compacted: {} → {} bytes ({} bytes removed, {:.1}% smaller)",
        nop_code.len(), compacted_size, saved_bytes,
        saved_bytes as f64 / nop_code.len() as f64 * 100.0);

    // ── Verify compacted bytecode on all reference images ──
    eprintln!("\nVerifying compacted bytecode...");
    let mut verify_ok = true;
    for (img_idx, _) in programs.iter().enumerate() {
        // Build compacted version of this image's program
        let (img_compacted, img_omap) = compact_bytecode(&programs[img_idx].0);
        let mut img_csyms: HashMap<u16, usize> = HashMap::new();
        for (idx, old_off) in &programs[img_idx].1 {
            let new_off = if *old_off < img_omap.len() { img_omap[*old_off] } else { *old_off };
            img_csyms.insert(*idx, new_off);
        }

        let result = run_bytecode(&img_compacted, &img_csyms, config.max_steps);
        match result {
            Some(floats) => {
                let ref_vals = &ref_outputs[img_idx];
                let max_d: f64 = floats.iter().zip(ref_vals.iter())
                    .map(|(a, b)| (a - b).abs()).fold(0.0f64, f64::max);
                let status = if max_d <= config.eps { "OK" } else { "FAIL" };
                if max_d > config.eps { verify_ok = false; }
                eprintln!("  img {} (label={}): {} (Δ={:.4})",
                    img_idx, image_programs[img_idx].1, status, max_d);
            }
            None => {
                eprintln!("  img {} (label={}): FAIL (execution error)", img_idx, image_programs[img_idx].1);
                verify_ok = false;
            }
        }
    }

    if !verify_ok {
        eprintln!("\nWARNING: Compacted bytecode verification failed on some images!");
        eprintln!("Saving NOP'd version as fallback.");
        // Fall back to saving NOP'd version
        let output_korec = config.output_file.replace(".kore", ".korec");
        let mut out_module = BytecodeModule::new();
        out_module.code = nop_code.to_vec();
        out_module.symbol_table = nop_syms.clone();
        let binary = out_module.encode();
        std::fs::write(&output_korec, &binary)
            .map_err(|e| format!("Failed to write {}: {}", output_korec, e))?;
        eprintln!("Saved (NOP'd fallback): {} ({} bytes)", output_korec, binary.len());
    } else {
        // Save compacted bytecode
        let output_korec = config.output_file.replace(".kore", ".korec");
        let mut out_module = BytecodeModule::new();
        out_module.code = compacted_code;
        out_module.symbol_table = compacted_syms;
        let binary = out_module.encode();
        std::fs::write(&output_korec, &binary)
            .map_err(|e| format!("Failed to write {}: {}", output_korec, e))?;
        eprintln!("Saved (compacted): {} ({} bytes)", output_korec, binary.len());
    }

    eprintln!("\n============================================================");
    eprintln!("SUMMARY");
    eprintln!("============================================================");
    eprintln!("Original model:    {} bytes", original_size);
    eprintln!("After pruning:     {} bytes NOP'd ({:.1}%)", removed_total, removed_total as f64 / original_size as f64 * 100.0);
    eprintln!("After compaction:  {} bytes ({:.1}% of original)",
        compacted_size, compacted_size as f64 / nop_code.len() as f64 * 100.0);
    eprintln!("Total reduction:   {} bytes removed ({:.1}%)",
        saved_bytes, saved_bytes as f64 / nop_code.len() as f64 * 100.0);

    // ── Save removal log ──
    let log_file = config.output_file.replace(".kore", "_prune_log.json");
    let log = format!(
        "{{\n  \"original_bytes\": {},\n  \"removed_bytes\": {},\n  \"removed_pct\": {:.1},\n  \"compacted_bytes\": {},\n  \"compacted_pct\": {:.1},\n  \"removals\": {},\n  \"checks\": {},\n  \"elapsed_s\": {:.1},\n  \"eps\": {}\n}}",
        original_size, removed_total,
        removed_total as f64 / original_size as f64 * 100.0,
        compacted_size,
        compacted_size as f64 / nop_code.len() as f64 * 100.0,
        removals.len(), total_checks, dt.as_secs_f64(), config.eps
    );
    let _ = std::fs::write(&log_file, &log);
    eprintln!("Log: {}", log_file);

    Ok(())
}

// ============================================================================
// BATCH EVALUATION
// ============================================================================

/// Configuration for batch evaluation
pub struct EvalConfig {
    pub source_file: String,
    pub korec_file: Option<String>,   // if provided, use this pre-compiled bytecode
    pub batch_file: String,
    pub labels_file: String,
    pub max_steps: usize,
    pub output_json: String,
    pub label: String,                // label for this run (e.g. "original" or "pruned_eps1")
}

/// Run batch evaluation: execute program on all test images, measure accuracy and speed
pub fn run_eval(config: EvalConfig) -> Result<(), String> {
    use std::time::Instant;

    // ── Load labels ──
    let labels: Vec<u8> = std::fs::read_to_string(&config.labels_file)
        .map_err(|e| format!("Failed to read labels: {}", e))?
        .lines()
        .filter_map(|l| l.trim().parse().ok())
        .collect();

    // ── Load batch pixel lines ──
    let batch_lines: Vec<String> = std::fs::read_to_string(&config.batch_file)
        .map_err(|e| format!("Failed to read batch: {}", e))?
        .lines()
        .map(|l| l.to_string())
        .collect();

    let n_images = labels.len().min(batch_lines.len());
    eprintln!("Eval '{}': {} images", config.label, n_images);

    // ── Read source ──
    let source = std::fs::read_to_string(&config.source_file)
        .map_err(|e| format!("Failed to read source: {}", e))?;

    let base_dir = std::path::Path::new(&config.source_file)
        .parent().unwrap_or(std::path::Path::new("."));

    // ── Build template bytecode ──
    // If a korec file is provided, load it directly (it's already compacted).
    // Otherwise, compile from source + first image.
    let mut template_module: BytecodeModule;

    if let Some(ref korec_path) = config.korec_file {
        // Load the compacted korec directly — it already has pixel data baked in
        eprintln!("Loading compacted model from {}...", korec_path);
        let data = std::fs::read(korec_path)
            .map_err(|e| format!("Failed to read korec: {}", e))?;
        template_module = BytecodeModule::decode(&data)
            .map_err(|e| format!("Failed to decode korec: {}", e))?;
        eprintln!("Template bytecode: {} bytes (from korec)", template_module.code.len());
    } else {
        // Compile from source + first image
        eprintln!("Compiling template program (source + first image)...");
        let first_pixels = &batch_lines[0];
        let template_source = format!("{}\n{} forward", source, first_pixels);
        template_module = crate::parser::compile_with_imports(&template_source, base_dir)?;
        let (template_opt, template_omap) = optimizer::optimize_with_offsets(&template_module.code);
        template_module.code = template_opt;
        let mut template_st = HashMap::new();
        for (k, v) in &template_module.symbol_table {
            template_st.insert(*k, template_omap.get(v).copied().unwrap_or(*v));
        }
        template_module.symbol_table = template_st;
        eprintln!("Template bytecode: {} bytes (compiled)", template_module.code.len());
    }

    // ── Find the 784 F64 pixel slots in the template bytecode ──
    let f64_opcode = Op::F64 as u8;
    let mut pixel_offsets: Vec<usize> = Vec::new();
    {
        let code = &template_module.code;
        let mut best_run_start = 0usize;
        let mut best_run_count = 0usize;
        let mut current_run_start = 0usize;
        let mut current_run_count = 0usize;
        let mut pos = 0usize;

        while pos < code.len() {
            let op_byte = code[pos];
            let isz = instruction_size(code, pos);
            if op_byte == f64_opcode {
                if current_run_count == 0 {
                    current_run_start = pos;
                }
                current_run_count += 1;
            } else {
                if current_run_count > best_run_count {
                    best_run_count = current_run_count;
                    best_run_start = current_run_start;
                }
                current_run_count = 0;
            }
            pos += isz;
        }
        if current_run_count > best_run_count {
            best_run_count = current_run_count;
            best_run_start = current_run_start;
        }

        eprintln!("Longest F64 run: {} instructions starting at offset 0x{:X}",
            best_run_count, best_run_start);

        if best_run_count >= 784 {
            let mut p = best_run_start;
            for _ in 0..784 {
                pixel_offsets.push(p + 1); // offset of the 8-byte value
                p += 9;
            }
        }
    }
    eprintln!("Found {} F64 pixel slots in template (expected 784)", pixel_offsets.len());

    if pixel_offsets.len() != 784 {
        return Err(format!(
            "Expected 784 F64 pixel slots but found {}. Cannot use fast pixel patching.",
            pixel_offsets.len()
        ));
    }

    // ── Parse all pixel lines into float arrays ──
    eprintln!("Parsing pixel data...");
    let all_pixels: Vec<Vec<f64>> = batch_lines[..n_images].iter()
        .map(|line| {
            line.split_whitespace()
                .filter_map(|s| s.parse::<f64>().ok())
                .collect()
        })
        .collect();

    // ── Run all images with fast bytecode patching ──
    let mut correct = 0usize;
    let mut total = 0usize;
    let mut per_class_correct = [0usize; 10];
    let mut per_class_total = [0usize; 10];
    let mut inference_times: Vec<f64> = Vec::with_capacity(n_images);
    let mut errors = 0usize;

    let t_start = Instant::now();

    for i in 0..n_images {
        let label = labels[i];
        let pixels = &all_pixels[i];

        if pixels.len() != 784 {
            errors += 1;
            per_class_total[label as usize] += 1;
            total += 1;
            continue;
        }

        // Patch pixel values directly into the template bytecode
        let mut code = template_module.code.clone();
        for (slot, &val) in pixel_offsets.iter().zip(pixels.iter()) {
            let bytes = val.to_le_bytes();
            code[*slot..*slot + 8].copy_from_slice(&bytes);
        }

        // Run inference and time it
        let t_infer = Instant::now();
        let result = run_bytecode(&code, &template_module.symbol_table, config.max_steps);
        let infer_ms = t_infer.elapsed().as_secs_f64() * 1000.0;
        inference_times.push(infer_ms);

        match result {
            Some(floats) => {
                let predicted = floats.iter()
                    .enumerate()
                    .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(idx, _)| idx as u8)
                    .unwrap_or(255);

                if predicted == label {
                    correct += 1;
                    per_class_correct[label as usize] += 1;
                }
                per_class_total[label as usize] += 1;
                total += 1;
            }
            None => {
                errors += 1;
                per_class_total[label as usize] += 1;
                total += 1;
            }
        }

        // Progress every 500 images
        if (i + 1) % 500 == 0 || i + 1 == n_images {
            let dt = t_start.elapsed().as_secs_f64();
            let acc = if total > 0 { correct as f64 / total as f64 * 100.0 } else { 0.0 };
            let avg_ms = if !inference_times.is_empty() {
                inference_times.iter().sum::<f64>() / inference_times.len() as f64
            } else { 0.0 };
            eprintln!("  [{}/{}] acc={:.2}% avg_infer={:.2}ms errors={} ({:.1}s elapsed)",
                i + 1, n_images, acc, avg_ms, errors, dt);
        }
    }

    let total_time = t_start.elapsed().as_secs_f64();

    // ── Compute stats ──
    let accuracy = if total > 0 { correct as f64 / total as f64 * 100.0 } else { 0.0 };
    let avg_infer_ms = if !inference_times.is_empty() {
        inference_times.iter().sum::<f64>() / inference_times.len() as f64
    } else { 0.0 };
    let median_infer_ms = {
        let mut sorted = inference_times.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        if sorted.is_empty() { 0.0 }
        else { sorted[sorted.len() / 2] }
    };
    let p95_infer_ms = {
        let mut sorted = inference_times.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        if sorted.is_empty() { 0.0 }
        else { sorted[(sorted.len() as f64 * 0.95) as usize] }
    };
    let total_infer_ms: f64 = inference_times.iter().sum();

    eprintln!("\n============================================================");
    eprintln!("EVAL RESULTS: {}", config.label);
    eprintln!("============================================================");
    eprintln!("Images:     {}", total);
    eprintln!("Correct:    {}", correct);
    eprintln!("Accuracy:   {:.2}%", accuracy);
    eprintln!("Errors:     {}", errors);
    eprintln!("Avg infer:  {:.3} ms", avg_infer_ms);
    eprintln!("Median:     {:.3} ms", median_infer_ms);
    eprintln!("P95:        {:.3} ms", p95_infer_ms);
    eprintln!("Total time: {:.1}s (infer: {:.1}s, compile overhead: {:.1}s)",
        total_time, total_infer_ms / 1000.0, total_time - total_infer_ms / 1000.0);
    eprintln!("\nPer-class accuracy:");
    for c in 0..10 {
        if per_class_total[c] > 0 {
            eprintln!("  {}: {}/{} ({:.1}%)", c,
                per_class_correct[c], per_class_total[c],
                per_class_correct[c] as f64 / per_class_total[c] as f64 * 100.0);
        }
    }

    // ── Save JSON ──
    let per_class_json: Vec<String> = (0..10).map(|c| {
        format!("    {{ \"class\": {}, \"correct\": {}, \"total\": {}, \"accuracy\": {:.4} }}",
            c, per_class_correct[c], per_class_total[c],
            if per_class_total[c] > 0 { per_class_correct[c] as f64 / per_class_total[c] as f64 } else { 0.0 })
    }).collect();

    let json = format!(
r#"{{
  "label": "{}",
  "source_file": "{}",
  "korec_file": {},
  "n_images": {},
  "correct": {},
  "accuracy": {:.4},
  "errors": {},
  "avg_infer_ms": {:.4},
  "median_infer_ms": {:.4},
  "p95_infer_ms": {:.4},
  "total_infer_s": {:.3},
  "total_time_s": {:.3},
  "per_class": [
{}
  ]
}}"#,
        config.label,
        config.source_file,
        config.korec_file.as_ref().map_or("null".to_string(), |f| format!("\"{}\"", f)),
        total,
        correct,
        accuracy / 100.0,
        errors,
        avg_infer_ms,
        median_infer_ms,
        p95_infer_ms,
        total_infer_ms / 1000.0,
        total_time,
        per_class_json.join(",\n")
    );

    std::fs::write(&config.output_json, &json)
        .map_err(|e| format!("Failed to write {}: {}", config.output_json, e))?;
    eprintln!("\nResults saved: {}", config.output_json);

    Ok(())
}
