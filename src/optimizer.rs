//! Kore Algebraic Optimizer
//!
//! Runs between parsing and execution. Applies provably-correct
//! algebraic identities to bytecode, reducing instruction count
//! without changing semantics.
//!
//! All identities are mathematically provable:
//! - Group involutions: swap swap → ε, neg neg → ε
//! - Rot order 3: rot rot rot → ε
//! - Retraction: dup drop → ε
//! - Arithmetic identities: 0 add → ε, 1 mul → ε
//! - Constant folding: INT a; INT b; ADD → INT (a+b)
//!
//! P3: Composition = concatenation. Optimization rewrites
//! subsequences of bytecode, preserving compositional semantics.
//!
//! IMPORTANT: All optimization passes replace removed instructions
//! with NOPs (length-preserving). A final compaction pass strips
//! NOPs and adjusts all offset-dependent references (JMP, JZ, JNZ,
//! QUOTE lengths) to maintain correctness.

use crate::bytecode::Op;
use std::collections::HashMap;

/// Optimize a bytecode sequence by applying algebraic identities.
/// Returns the optimized bytecode and an old→new offset map for
/// patching external references (e.g., symbol_table).
pub fn optimize(code: &[u8]) -> Vec<u8> {
    let (optimized, _offset_map) = optimize_with_offsets(code);
    optimized
}

/// Optimize and also return the offset relocation map.
/// The map translates old bytecode offsets → new offsets after compaction.
/// Callers must use this to update symbol tables, etc.
pub fn optimize_with_offsets(code: &[u8]) -> (Vec<u8>, HashMap<usize, usize>) {
    let mut result = code.to_vec();
    let mut changed = true;
    
    // Fixed-point iteration: each pass replaces removed bytes with NOPs
    // (length-preserving), so no offsets are invalidated during iteration.
    while changed {
        changed = false;
        
        let new = eliminate_involutions(&result);
        if new != result {
            result = new;
            changed = true;
            continue;
        }
        
        let new = eliminate_retractions(&result);
        if new != result {
            result = new;
            changed = true;
            continue;
        }
        
        let new = eliminate_identity_arithmetic(&result);
        if new != result {
            result = new;
            changed = true;
            continue;
        }
        
        let new = fold_constants(&result);
        if new != result {
            result = new;
            changed = true;
            continue;
        }
        
        let new = eliminate_training_identities(&result);
        if new != result {
            result = new;
            changed = true;
        }
    }
    
    // Final compaction: remove NOPs, adjust all embedded offsets,
    // and build the old→new offset map for external fixups.
    compact_nops(&result)
}

/// Remove NOP instructions and adjust all offset-dependent references.
/// Returns (compacted_code, old_offset → new_offset map).
fn compact_nops(code: &[u8]) -> (Vec<u8>, HashMap<usize, usize>) {
    // Phase 1: Build offset map (old position → new position)
    let mut offset_map: HashMap<usize, usize> = HashMap::new();
    let mut new_pos = 0usize;
    let mut i = 0usize;
    
    while i < code.len() {
        offset_map.insert(i, new_pos);
        let size = instruction_size(code, i);
        if code[i] == Op::Nop as u8 {
            // NOP will be removed — don't advance new_pos
        } else {
            new_pos += size;
        }
        i += size;
    }
    // Map the end-of-code position too
    offset_map.insert(i, new_pos);
    
    // Phase 2: Copy non-NOP bytes and patch embedded offsets
    let mut result = Vec::with_capacity(new_pos);
    i = 0;
    
    while i < code.len() {
        let size = instruction_size(code, i);
        
        if code[i] == Op::Nop as u8 {
            i += size;
            continue;
        }
        
        match code[i] {
            // JMP: opcode(1) + relative_i32(4)
            // The offset is relative to the position AFTER the instruction
            // (i.e., after opcode + 4 bytes = position i+5)
            0x70 => {
                let old_operand_pos = i + 1;
                let old_rel = i32::from_le_bytes([code[old_operand_pos], code[old_operand_pos + 1], code[old_operand_pos + 2], code[old_operand_pos + 3]]);
                let old_target = (i + 5) as i64 + old_rel as i64;
                let new_target = *offset_map.get(&(old_target as usize)).unwrap_or(&(old_target as usize));
                let new_instr_end = offset_map[&i] + 5;
                let new_rel = (new_target as i64 - new_instr_end as i64) as i32;
                result.push(code[i]);
                result.extend_from_slice(&new_rel.to_le_bytes());
            }
            
            // JZ, JNZ: same layout as JMP
            0x71 | 0x72 => {
                let old_operand_pos = i + 1;
                let old_rel = i32::from_le_bytes([code[old_operand_pos], code[old_operand_pos + 1], code[old_operand_pos + 2], code[old_operand_pos + 3]]);
                let old_target = (i + 5) as i64 + old_rel as i64;
                let new_target = *offset_map.get(&(old_target as usize)).unwrap_or(&(old_target as usize));
                let new_instr_end = offset_map[&i] + 5;
                let new_rel = (new_target as i64 - new_instr_end as i64) as i32;
                result.push(code[i]);
                result.extend_from_slice(&new_rel.to_le_bytes());
            }
            
            // QUOTE: opcode(1) + body_len_u16(2) + body(body_len) 
            // body_len must be adjusted for NOPs removed within the body
            0x20 => {
                let old_body_len = u16::from_le_bytes([code[i + 1], code[i + 2]]) as usize;
                let body_start = i + 3;
                let body_end = body_start + old_body_len;
                let new_body_start = offset_map[&body_start];
                let new_body_end = *offset_map.get(&body_end).unwrap_or(&offset_map[&i]);
                let new_body_len = (new_body_end - new_body_start) as u16;
                result.push(code[i]);
                result.extend_from_slice(&new_body_len.to_le_bytes());
                // Body bytes will be copied by subsequent iterations
            }
            
            // All other instructions: copy verbatim
            _ => {
                for j in 0..size {
                    result.push(code[i + j]);
                }
            }
        }
        
        i += size;
    }
    
    (result, offset_map)
}

/// Determine the byte size of an instruction at position `pos` in `code`.
fn instruction_size(code: &[u8], pos: usize) -> usize {
    if pos >= code.len() {
        return 1;
    }
    match code[pos] {
        // 1-byte instructions (no operands)
        0x00..=0x05 |  // NOP, DROP, DUP, SWAP, ROT, OVER
        0x10..=0x13 |  // PAIR, UNPAIR, LEFT, RIGHT
        0x21 |         // APPLY
        0x23 |         // RET
        0x37..=0x39 |  // NIL, TRUE, FALSE
        0x40..=0x5D |  // Arithmetic, Comparison, Float ops
        0x60..=0x63 |  // Logic: AND, OR, NOT, XOR
        0x64..=0x69 |  // Bitwise: BAND, BOR, BXOR, BNOT, SHL, SHR
        0x81..=0x89 |  // UNLIST, LEN, GET, SET, MAP, FOLD, ZIP, APPEND, REVERSE
        0x90..=0x91 |  // FETCH, SIZE
        0xA0..=0xA2 |  // PRINT, PRINTLN, RAND
        0xA3..=0xA7 |  // TRY, FAIL, IS_ERROR, PROPAGATE, MAKE_ERROR
        0xAA..=0xAE |  // MAP_NEW..MAP_HAS
        0xB0..=0xB8 |  // FIBER_*, SPAWN, CHAN_*
        0xC0..=0xC4 |  // LINEAR, AFFINE, CONSUME, IS_LINEAR, IS_AFFINE
        0xC5..=0xCD |  // Training: TIMES, FILTER, HEAD, TAIL, RANGE, FIRST, SECOND, LIST_CONCAT, IS_EMPTY
        0xD0..=0xD2 |  // TYPE_OF, DEPTH, DESCRIBE
        0xE0..=0xEA |  // String ops
        0xF0 |         // EXT
        0xFF           // HALT
        => 1,
        
        // 2-byte instructions (1-byte operand)
        0x30 |         // INT8: opcode + i8
        0xAF           // SYSCALL: opcode + call_id_u8
        => 2,
        
        // 3-byte instructions (2-byte operand)
        0x20 |         // QUOTE: opcode + body_len_u16 (body follows but is separate)
        0x22 |         // CALL: opcode + symbol_idx_u16
        0x31 |         // INT16: opcode + i16
        0x80           // LIST: opcode + count_u16
        => 3,
        
        // 4-byte instructions: CASE (opcode + 2x i16)
        0x14 => 5,     // CASE: opcode + off_l(i16) + off_r(i16)
        
        // 5-byte instructions
        0x32 => 5,     // INT32: opcode + i32
        0x70..=0x72 => 5,  // JMP, JZ, JNZ: opcode + relative_i32
        0xA8 | 0xA9 => 5,  // STORE, LOAD: opcode + slot_u32
        
        // 9-byte instructions
        0x33 => 9,     // INT64: opcode + i64
        0x34 => 5,     // F32: opcode + f32
        0x35 => 9,     // F64: opcode + f64
        
        // STR: opcode + len_u16 + bytes
        0x36 => {
            if pos + 2 < code.len() {
                let len = u16::from_le_bytes([code[pos + 1], code[pos + 2]]) as usize;
                3 + len
            } else {
                1 // malformed, treat as 1
            }
        }
        
        // Unknown: treat as 1 byte (safe fallback)
        _ => 1,
    }
}

/// Remove involution pairs: op op → NOP NOP
/// swap swap → ε, neg neg → ε, fneg fneg → ε, not not → ε
/// Replaces with NOPs to preserve bytecode length.
fn eliminate_involutions(code: &[u8]) -> Vec<u8> {
    let mut result = code.to_vec();
    let mut i = 0;
    
    while i < result.len() {
        if i + 1 < result.len() {
            let a = result[i];
            let b = result[i + 1];
            
            let is_involution = matches!(
                (Op::from_byte(a), Op::from_byte(b)),
                (Some(Op::Swap), Some(Op::Swap)) |
                (Some(Op::Neg), Some(Op::Neg)) |
                (Some(Op::Fneg), Some(Op::Fneg)) |
                (Some(Op::Not), Some(Op::Not)) |
                (Some(Op::Bnot), Some(Op::Bnot))
            );
            
            if is_involution {
                result[i] = Op::Nop as u8;
                result[i + 1] = Op::Nop as u8;
                i += 2;
                continue;
            }
        }
        
        // Check for rot rot rot → ε (order 3)
        if i + 2 < result.len() {
            let (a, b, c) = (result[i], result[i + 1], result[i + 2]);
            if matches!(
                (Op::from_byte(a), Op::from_byte(b), Op::from_byte(c)),
                (Some(Op::Rot), Some(Op::Rot), Some(Op::Rot))
            ) {
                result[i] = Op::Nop as u8;
                result[i + 1] = Op::Nop as u8;
                result[i + 2] = Op::Nop as u8;
                i += 3;
                continue;
            }
        }
        
        i += instruction_size(&result, i);
    }
    
    result
}

/// Remove retraction: dup drop → NOP NOP
/// Replaces with NOPs to preserve bytecode length.
fn eliminate_retractions(code: &[u8]) -> Vec<u8> {
    let mut result = code.to_vec();
    let mut i = 0;
    
    while i < result.len() {
        if i + 1 < result.len() {
            if matches!(
                (Op::from_byte(result[i]), Op::from_byte(result[i + 1])),
                (Some(Op::Dup), Some(Op::Drop))
            ) {
                result[i] = Op::Nop as u8;
                result[i + 1] = Op::Nop as u8;
                i += 2;
                continue;
            }
        }
        
        i += instruction_size(&result, i);
    }
    
    result
}

/// Remove arithmetic identity elements:
/// INT8 0; ADD → NOP NOP NOP  (additive identity)
/// INT8 1; MUL → NOP NOP NOP  (multiplicative identity)
/// Replaces with NOPs to preserve bytecode length.
fn eliminate_identity_arithmetic(code: &[u8]) -> Vec<u8> {
    let mut result = code.to_vec();
    let mut i = 0;
    
    while i < result.len() {
        // Pattern: INT8 0 ADD → NOP NOP NOP
        if i + 2 < result.len()
            && result[i] == Op::Int8 as u8
            && result[i + 1] == 0
            && result[i + 2] == Op::Add as u8
        {
            result[i] = Op::Nop as u8;
            result[i + 1] = Op::Nop as u8;
            result[i + 2] = Op::Nop as u8;
            i += 3;
            continue;
        }
        
        // Pattern: INT8 1 MUL → NOP NOP NOP
        if i + 2 < result.len()
            && result[i] == Op::Int8 as u8
            && result[i + 1] == 1
            && result[i + 2] == Op::Mul as u8
        {
            result[i] = Op::Nop as u8;
            result[i + 1] = Op::Nop as u8;
            result[i + 2] = Op::Nop as u8;
            i += 3;
            continue;
        }
        
        // Pattern: INT8 0 MUL → DROP INT8 0 (annihilation, same size)
        if i + 2 < result.len()
            && result[i] == Op::Int8 as u8
            && result[i + 1] == 0
            && result[i + 2] == Op::Mul as u8
        {
            result[i] = Op::Drop as u8;
            result[i + 1] = Op::Int8 as u8;
            result[i + 2] = 0;
            i += 3;
            continue;
        }
        
        i += instruction_size(&result, i);
    }
    
    result
}

/// Constant folding: evaluate operations on known constants at compile time.
///
/// Uses abstract interpretation with a virtual stack to fold arbitrary-length
/// chains of pure operations. For example:
///   INT8 1; DUP; ADD; DUP; ADD; DUP; ADD; I2F; INT8 1; DUP; ADD; ...
///   ... DUP; ADD; INT8 1; ADD; DUP; ADD; INT8 1; ADD; I2F; FDIV; FNEG
/// All collapse to a single F64 literal.
///
/// This is fully general — works on any Kore program, not just ML models.
/// The pass identifies maximal chains of "pure" instructions (those that only
/// push/pop known constants, with no loads, stores, jumps, or side effects)
/// and replaces each chain with the minimal push instruction for its result.
fn fold_constants(code: &[u8]) -> Vec<u8> {
    #[derive(Clone, Debug)]
    enum KVal {
        Int(i64),
        Float(f64),
    }
    
    #[derive(Clone, Debug)]
    struct StackEntry {
        val: KVal,
        /// Byte offset in `result` where this value's computation chain begins.
        chain_start: usize,
    }
    
    /// Calculate how many bytes a push instruction needs for this value.
    fn emit_size(val: &KVal) -> usize {
        match val {
            KVal::Int(v) => {
                if *v >= -128 && *v <= 127 { 2 }       // INT8
                else if *v >= -32768 && *v <= 32767 { 3 } // INT16
                else if *v >= i32::MIN as i64 && *v <= i32::MAX as i64 { 5 } // INT32
                else { 9 }                               // INT64
            }
            KVal::Float(_) => 9, // F64
        }
    }
    
    /// Emit the optimal push instruction for a value into result at write_pos.
    fn emit_value(val: &KVal, result: &mut Vec<u8>, pos: usize) -> usize {
        match val {
            KVal::Int(v) => {
                if *v >= -128 && *v <= 127 {
                    result[pos] = Op::Int8 as u8;
                    result[pos + 1] = *v as u8;
                    pos + 2
                } else if *v >= -32768 && *v <= 32767 {
                    result[pos] = Op::Int16 as u8;
                    let bytes = (*v as i16).to_le_bytes();
                    result[pos + 1] = bytes[0];
                    result[pos + 2] = bytes[1];
                    pos + 3
                } else if *v >= i32::MIN as i64 && *v <= i32::MAX as i64 {
                    result[pos] = Op::Int32 as u8;
                    let bytes = (*v as i32).to_le_bytes();
                    for j in 0..4 { result[pos + 1 + j] = bytes[j]; }
                    pos + 5
                } else {
                    result[pos] = Op::Int64 as u8;
                    let bytes = v.to_le_bytes();
                    for j in 0..8 { result[pos + 1 + j] = bytes[j]; }
                    pos + 9
                }
            }
            KVal::Float(v) => {
                result[pos] = Op::F64 as u8;
                let bytes = v.to_le_bytes();
                for j in 0..8 { result[pos + 1 + j] = bytes[j]; }
                pos + 9
            }
        }
    }
    
    /// Flush virtual stack entries: replace their instruction chains with
    /// NOPs + optimal push instructions.
    fn fold_stack_entries(
        result: &mut Vec<u8>,
        stack: &mut Vec<StackEntry>,
        fence: usize,
    ) -> bool {
        if stack.is_empty() {
            return false;
        }
        
        let earliest = stack.iter().map(|e| e.chain_start).min().unwrap();
        let chain_len = fence - earliest;
        let push_bytes: usize = stack.iter().map(|e| emit_size(&e.val)).sum();
        
        if push_bytes >= chain_len {
            return false;
        }
        
        // NOP out the entire chain
        for j in earliest..fence {
            result[j] = Op::Nop as u8;
        }
        
        // Write push instructions at the end of the NOP region
        let mut write_pos = fence - push_bytes;
        for entry in stack.iter() {
            write_pos = emit_value(&entry.val, result, write_pos);
        }
        
        true
    }
    
    let mut result = code.to_vec();
    let mut stack: Vec<StackEntry> = Vec::new();
    let mut i = 0;
    
    while i < result.len() {
        let op_byte = result[i];
        let isize_val = instruction_size(&result, i);
        let next_i = i + isize_val;
        
        let op = Op::from_byte(op_byte);
        
        // Try to process this instruction on the virtual stack
        let handled = match op {
            // --- Push literal constants ---
            Some(Op::Int8) if i + 1 < result.len() => {
                let v = result[i + 1] as i8 as i64;
                stack.push(StackEntry { val: KVal::Int(v), chain_start: i });
                true
            }
            Some(Op::Int16) if i + 2 < result.len() => {
                let v = i16::from_le_bytes([result[i+1], result[i+2]]) as i64;
                stack.push(StackEntry { val: KVal::Int(v), chain_start: i });
                true
            }
            Some(Op::Int32) if i + 4 < result.len() => {
                let v = i32::from_le_bytes([result[i+1], result[i+2], result[i+3], result[i+4]]) as i64;
                stack.push(StackEntry { val: KVal::Int(v), chain_start: i });
                true
            }
            Some(Op::Int64) if i + 8 < result.len() => {
                let v = i64::from_le_bytes([
                    result[i+1], result[i+2], result[i+3], result[i+4],
                    result[i+5], result[i+6], result[i+7], result[i+8],
                ]);
                stack.push(StackEntry { val: KVal::Int(v), chain_start: i });
                true
            }
            Some(Op::F32) if i + 4 < result.len() => {
                let v = f32::from_le_bytes([result[i+1], result[i+2], result[i+3], result[i+4]]) as f64;
                stack.push(StackEntry { val: KVal::Float(v), chain_start: i });
                true
            }
            Some(Op::F64) if i + 8 < result.len() => {
                let v = f64::from_le_bytes([
                    result[i+1], result[i+2], result[i+3], result[i+4],
                    result[i+5], result[i+6], result[i+7], result[i+8],
                ]);
                stack.push(StackEntry { val: KVal::Float(v), chain_start: i });
                true
            }
            
            // --- Stack manipulation ---
            Some(Op::Dup) if !stack.is_empty() => {
                let top = stack.last().unwrap().clone();
                stack.push(top);
                true
            }
            Some(Op::Swap) if stack.len() >= 2 => {
                let len = stack.len();
                stack.swap(len - 1, len - 2);
                true
            }
            Some(Op::Over) if stack.len() >= 2 => {
                let second = stack[stack.len() - 2].clone();
                stack.push(second);
                true
            }
            Some(Op::Rot) if stack.len() >= 3 => {
                let len = stack.len();
                let c = stack.remove(len - 3);
                stack.push(c);
                true
            }
            Some(Op::Drop) if !stack.is_empty() => {
                // Dropping a known constant — don't emit anything for it
                // but we need to be careful: if the dropped value was pushed
                // by real instructions, those instructions still ran.
                // For constant folding, we only fold when the FINAL result matters.
                // DROP breaks the chain — we can't fold across it unless
                // we track the full stack effect.
                // Conservative: just drop from virtual stack, don't fold.
                stack.pop();
                true
            }
            
            // --- Integer arithmetic (binary) ---
            Some(Op::Add) if stack.len() >= 2 => {
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let chain_start = a.chain_start.min(b.chain_start);
                match (&a.val, &b.val) {
                    (KVal::Int(x), KVal::Int(y)) => {
                        stack.push(StackEntry { val: KVal::Int(x.wrapping_add(*y)), chain_start });
                        true
                    }
                    _ => { stack.push(a); stack.push(b); false }
                }
            }
            Some(Op::Sub) if stack.len() >= 2 => {
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let chain_start = a.chain_start.min(b.chain_start);
                match (&a.val, &b.val) {
                    (KVal::Int(x), KVal::Int(y)) => {
                        stack.push(StackEntry { val: KVal::Int(x.wrapping_sub(*y)), chain_start });
                        true
                    }
                    _ => { stack.push(a); stack.push(b); false }
                }
            }
            Some(Op::Mul) if stack.len() >= 2 => {
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let chain_start = a.chain_start.min(b.chain_start);
                match (&a.val, &b.val) {
                    (KVal::Int(x), KVal::Int(y)) => {
                        stack.push(StackEntry { val: KVal::Int(x.wrapping_mul(*y)), chain_start });
                        true
                    }
                    _ => { stack.push(a); stack.push(b); false }
                }
            }
            Some(Op::Div) if stack.len() >= 2 => {
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let chain_start = a.chain_start.min(b.chain_start);
                match (&a.val, &b.val) {
                    (KVal::Int(x), KVal::Int(y)) if *y != 0 => {
                        stack.push(StackEntry { val: KVal::Int(x.wrapping_div(*y)), chain_start });
                        true
                    }
                    _ => { stack.push(a); stack.push(b); false }
                }
            }
            Some(Op::Mod) if stack.len() >= 2 => {
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let chain_start = a.chain_start.min(b.chain_start);
                match (&a.val, &b.val) {
                    (KVal::Int(x), KVal::Int(y)) if *y != 0 => {
                        stack.push(StackEntry { val: KVal::Int(x.wrapping_rem(*y)), chain_start });
                        true
                    }
                    _ => { stack.push(a); stack.push(b); false }
                }
            }
            Some(Op::Neg) if !stack.is_empty() => {
                let a = stack.pop().unwrap();
                match &a.val {
                    KVal::Int(x) => {
                        stack.push(StackEntry { val: KVal::Int(x.wrapping_neg()), chain_start: a.chain_start });
                        true
                    }
                    _ => { stack.push(a); false }
                }
            }
            
            // --- Conversion ---
            Some(Op::I2f) if !stack.is_empty() => {
                let a = stack.pop().unwrap();
                match &a.val {
                    KVal::Int(x) => {
                        stack.push(StackEntry { val: KVal::Float(*x as f64), chain_start: a.chain_start });
                        true
                    }
                    _ => { stack.push(a); false }
                }
            }
            Some(Op::F2i) if !stack.is_empty() => {
                let a = stack.pop().unwrap();
                match &a.val {
                    KVal::Float(x) => {
                        stack.push(StackEntry { val: KVal::Int(*x as i64), chain_start: a.chain_start });
                        true
                    }
                    _ => { stack.push(a); false }
                }
            }
            
            // --- Float arithmetic ---
            Some(Op::Fadd) if stack.len() >= 2 => {
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let chain_start = a.chain_start.min(b.chain_start);
                match (&a.val, &b.val) {
                    (KVal::Float(x), KVal::Float(y)) => {
                        stack.push(StackEntry { val: KVal::Float(x + y), chain_start });
                        true
                    }
                    _ => { stack.push(a); stack.push(b); false }
                }
            }
            Some(Op::Fsub) if stack.len() >= 2 => {
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let chain_start = a.chain_start.min(b.chain_start);
                match (&a.val, &b.val) {
                    (KVal::Float(x), KVal::Float(y)) => {
                        stack.push(StackEntry { val: KVal::Float(x - y), chain_start });
                        true
                    }
                    _ => { stack.push(a); stack.push(b); false }
                }
            }
            Some(Op::Fmul) if stack.len() >= 2 => {
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let chain_start = a.chain_start.min(b.chain_start);
                match (&a.val, &b.val) {
                    (KVal::Float(x), KVal::Float(y)) => {
                        stack.push(StackEntry { val: KVal::Float(x * y), chain_start });
                        true
                    }
                    _ => { stack.push(a); stack.push(b); false }
                }
            }
            Some(Op::Fdiv) if stack.len() >= 2 => {
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let chain_start = a.chain_start.min(b.chain_start);
                match (&a.val, &b.val) {
                    (KVal::Float(x), KVal::Float(y)) if *y != 0.0 => {
                        stack.push(StackEntry { val: KVal::Float(x / y), chain_start });
                        true
                    }
                    _ => { stack.push(a); stack.push(b); false }
                }
            }
            Some(Op::Fneg) if !stack.is_empty() => {
                let a = stack.pop().unwrap();
                match &a.val {
                    KVal::Float(x) => {
                        stack.push(StackEntry { val: KVal::Float(-x), chain_start: a.chain_start });
                        true
                    }
                    _ => { stack.push(a); false }
                }
            }
            Some(Op::Fabs) if !stack.is_empty() => {
                let a = stack.pop().unwrap();
                match &a.val {
                    KVal::Float(x) => {
                        stack.push(StackEntry { val: KVal::Float(x.abs()), chain_start: a.chain_start });
                        true
                    }
                    _ => { stack.push(a); false }
                }
            }
            Some(Op::Fsqrt) if !stack.is_empty() => {
                let a = stack.pop().unwrap();
                match &a.val {
                    KVal::Float(x) if *x >= 0.0 => {
                        stack.push(StackEntry { val: KVal::Float(x.sqrt()), chain_start: a.chain_start });
                        true
                    }
                    _ => { stack.push(a); false }
                }
            }
            Some(Op::Fexp) if !stack.is_empty() => {
                let a = stack.pop().unwrap();
                match &a.val {
                    KVal::Float(x) => {
                        stack.push(StackEntry { val: KVal::Float(x.exp()), chain_start: a.chain_start });
                        true
                    }
                    _ => { stack.push(a); false }
                }
            }
            Some(Op::Flog) if !stack.is_empty() => {
                let a = stack.pop().unwrap();
                match &a.val {
                    KVal::Float(x) if *x > 0.0 => {
                        stack.push(StackEntry { val: KVal::Float(x.ln()), chain_start: a.chain_start });
                        true
                    }
                    _ => { stack.push(a); false }
                }
            }
            Some(Op::Fsin) if !stack.is_empty() => {
                let a = stack.pop().unwrap();
                match &a.val {
                    KVal::Float(x) => {
                        stack.push(StackEntry { val: KVal::Float(x.sin()), chain_start: a.chain_start });
                        true
                    }
                    _ => { stack.push(a); false }
                }
            }
            Some(Op::Fcos) if !stack.is_empty() => {
                let a = stack.pop().unwrap();
                match &a.val {
                    KVal::Float(x) => {
                        stack.push(StackEntry { val: KVal::Float(x.cos()), chain_start: a.chain_start });
                        true
                    }
                    _ => { stack.push(a); false }
                }
            }
            Some(Op::Fpow) if stack.len() >= 2 => {
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let chain_start = a.chain_start.min(b.chain_start);
                match (&a.val, &b.val) {
                    (KVal::Float(x), KVal::Float(y)) => {
                        stack.push(StackEntry { val: KVal::Float(x.powf(*y)), chain_start });
                        true
                    }
                    _ => { stack.push(a); stack.push(b); false }
                }
            }
            Some(Op::Fatan2) if stack.len() >= 2 => {
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let chain_start = a.chain_start.min(b.chain_start);
                match (&a.val, &b.val) {
                    (KVal::Float(y), KVal::Float(x)) => {
                        stack.push(StackEntry { val: KVal::Float(y.atan2(*x)), chain_start });
                        true
                    }
                    _ => { stack.push(a); stack.push(b); false }
                }
            }
            Some(Op::Ffloor) if !stack.is_empty() => {
                let a = stack.pop().unwrap();
                match &a.val {
                    KVal::Float(x) => {
                        stack.push(StackEntry { val: KVal::Float(x.floor()), chain_start: a.chain_start });
                        true
                    }
                    _ => { stack.push(a); false }
                }
            }
            Some(Op::Fceil) if !stack.is_empty() => {
                let a = stack.pop().unwrap();
                match &a.val {
                    KVal::Float(x) => {
                        stack.push(StackEntry { val: KVal::Float(x.ceil()), chain_start: a.chain_start });
                        true
                    }
                    _ => { stack.push(a); false }
                }
            }
            Some(Op::Fround) if !stack.is_empty() => {
                let a = stack.pop().unwrap();
                match &a.val {
                    KVal::Float(x) => {
                        stack.push(StackEntry { val: KVal::Float(x.round()), chain_start: a.chain_start });
                        true
                    }
                    _ => { stack.push(a); false }
                }
            }
            
            // --- Bitwise ops ---
            Some(Op::Band) if stack.len() >= 2 => {
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let chain_start = a.chain_start.min(b.chain_start);
                match (&a.val, &b.val) {
                    (KVal::Int(x), KVal::Int(y)) => {
                        stack.push(StackEntry { val: KVal::Int(x & y), chain_start });
                        true
                    }
                    _ => { stack.push(a); stack.push(b); false }
                }
            }
            Some(Op::Bor) if stack.len() >= 2 => {
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let chain_start = a.chain_start.min(b.chain_start);
                match (&a.val, &b.val) {
                    (KVal::Int(x), KVal::Int(y)) => {
                        stack.push(StackEntry { val: KVal::Int(x | y), chain_start });
                        true
                    }
                    _ => { stack.push(a); stack.push(b); false }
                }
            }
            Some(Op::Bxor) if stack.len() >= 2 => {
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let chain_start = a.chain_start.min(b.chain_start);
                match (&a.val, &b.val) {
                    (KVal::Int(x), KVal::Int(y)) => {
                        stack.push(StackEntry { val: KVal::Int(x ^ y), chain_start });
                        true
                    }
                    _ => { stack.push(a); stack.push(b); false }
                }
            }
            Some(Op::Bnot) if !stack.is_empty() => {
                let a = stack.pop().unwrap();
                match &a.val {
                    KVal::Int(x) => {
                        stack.push(StackEntry { val: KVal::Int(!x), chain_start: a.chain_start });
                        true
                    }
                    _ => { stack.push(a); false }
                }
            }
            Some(Op::Shl) if stack.len() >= 2 => {
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let chain_start = a.chain_start.min(b.chain_start);
                match (&a.val, &b.val) {
                    (KVal::Int(x), KVal::Int(y)) if *y >= 0 && *y < 64 => {
                        stack.push(StackEntry { val: KVal::Int(x.wrapping_shl(*y as u32)), chain_start });
                        true
                    }
                    _ => { stack.push(a); stack.push(b); false }
                }
            }
            Some(Op::Shr) if stack.len() >= 2 => {
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let chain_start = a.chain_start.min(b.chain_start);
                match (&a.val, &b.val) {
                    (KVal::Int(x), KVal::Int(y)) if *y >= 0 && *y < 64 => {
                        stack.push(StackEntry { val: KVal::Int(x.wrapping_shr(*y as u32)), chain_start });
                        true
                    }
                    _ => { stack.push(a); stack.push(b); false }
                }
            }
            
            // --- Any non-pure instruction: flush the virtual stack ---
            _ => false,
        };
        
        if !handled {
            // This instruction is not foldable. Flush any pending constants
            // on the virtual stack — emit them as optimal push instructions.
            // But only emit (fold) entries whose chain_start < i, meaning they
            // were computed from earlier instructions that we can now replace.
            fold_stack_entries(&mut result, &mut stack, i);
            stack.clear();
        }
        
        i = next_i;
    }
    
    // Flush any remaining entries at end of code
    let end = result.len();
    fold_stack_entries(&mut result, &mut stack, end);
    
    // If we folded anything, we need another pass (the fixed-point loop will
    // call us again). The existing fold_constants behavior is subsumed by this.
    result
}









/// Count the number of optimization passes applied
pub fn optimization_stats(original: &[u8], optimized: &[u8]) -> (usize, usize) {
    (original.len(), optimized.len())
}

/// Eliminate dead training primitives:
/// - INT8 0; TIMES → NOP NOP NOP  (zero iterations = no effect)
/// - NIL; IS_EMPTY → NIL; TRUE    (empty list is always empty)
/// - NIL; HEAD → (leave as-is, would be runtime error — don't "optimize" errors)
/// - BNOT; BNOT → NOP NOP         (bitwise not is involution)
/// P4: Constraints only attenuate. These preserve semantics exactly.
fn eliminate_training_identities(code: &[u8]) -> Vec<u8> {
    let mut result = code.to_vec();
    let mut i = 0;
    
    while i < result.len() {
        // Pattern: INT8 0 TIMES → NOP NOP NOP  (zero iterations = noop)
        if i + 2 < result.len()
            && result[i] == Op::Int8 as u8
            && result[i + 1] == 0
            && result[i + 2] == Op::Times as u8
        {
            result[i] = Op::Nop as u8;
            result[i + 1] = Op::Nop as u8;
            result[i + 2] = Op::Nop as u8;
            i += 3;
            continue;
        }
        
        // Pattern: NIL IS_EMPTY → NIL TRUE  (empty list → always true)
        if i + 1 < result.len()
            && result[i] == Op::Nil as u8
            && result[i + 1] == Op::IsEmpty as u8
        {
            // IS_EMPTY is non-destructive: (list -- list bool)
            // NIL IS_EMPTY = NIL TRUE (pushes nil then true)
            // But IS_EMPTY keeps the list, so result is: nil bool
            // We need: NIL TRUE — but that only leaves nil, true on stack
            // which is correct since NIL IS_EMPTY → nil true
            result[i + 1] = Op::True as u8;
            i += 2;
            continue;
        }
        
        // Pattern: LIST(0) LIST_CONCAT → NOP NOP NOP NOP (concat with empty list = identity)
        // LIST(0) = 0x80 0x00 0x00, LIST_CONCAT = 0xCC → 4 bytes
        if i + 3 < result.len()
            && result[i] == Op::List as u8
            && result[i + 1] == 0x00
            && result[i + 2] == 0x00
            && result[i + 3] == Op::ListConcat as u8
        {
            result[i] = Op::Nop as u8;
            result[i + 1] = Op::Nop as u8;
            result[i + 2] = Op::Nop as u8;
            result[i + 3] = Op::Nop as u8;
            i += 4;
            continue;
        }
        
        // Pattern: INT8 1 TIMES → NOP NOP NOP (one iteration = just run quote)
        if i + 2 < result.len()
            && result[i] == Op::Int8 as u8
            && result[i + 1] == 1
            && result[i + 2] == Op::Times as u8
        {
            // TIMES pops N and the quote, applies it N times
            // With N=1, it just applies once — same as APPLY
            // Replace INT8 1 TIMES → NOP NOP APPLY
            result[i] = Op::Nop as u8;
            result[i + 1] = Op::Nop as u8;
            result[i + 2] = Op::Apply as u8;
            i += 3;
            continue;
        }
        
        i += instruction_size(&result, i);
    }
    
    result
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::*;

    #[test]
    fn test_swap_swap_elimination() {
        let mut asm = Assembler::new();
        asm.emit_i8(1);
        asm.emit_i8(2);
        asm.emit(Op::Swap);
        asm.emit(Op::Swap);
        asm.emit(Op::Add);
        asm.emit(Op::Halt);
        let module = asm.finalize().unwrap();
        
        let optimized = optimize(&module.code);
        
        // swap swap should be eliminated
        // Original: INT8 1 INT8 2 SWAP SWAP ADD HALT = 8 bytes
        // Optimized: INT8 1 INT8 2 ADD HALT = 6 bytes
        assert!(optimized.len() < module.code.len(),
            "Expected optimization to reduce size: {} -> {}", module.code.len(), optimized.len());
        
        // Verify result is the same
        let mut interp = crate::interpreter::Interpreter::new(&optimized);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&crate::interpreter::Value::Int(3)));
    }
    
    #[test]
    fn test_neg_neg_elimination() {
        let mut asm = Assembler::new();
        asm.emit_i8(5);
        asm.emit(Op::Neg);
        asm.emit(Op::Neg);
        asm.emit(Op::Halt);
        let module = asm.finalize().unwrap();
        
        let optimized = optimize(&module.code);
        assert!(optimized.len() < module.code.len());
        
        let mut interp = crate::interpreter::Interpreter::new(&optimized);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&crate::interpreter::Value::Int(5)));
    }
    
    #[test]
    fn test_not_not_elimination() {
        let mut asm = Assembler::new();
        asm.emit(Op::True);
        asm.emit(Op::Not);
        asm.emit(Op::Not);
        asm.emit(Op::Halt);
        let module = asm.finalize().unwrap();
        
        let optimized = optimize(&module.code);
        assert!(optimized.len() < module.code.len());
        
        let mut interp = crate::interpreter::Interpreter::new(&optimized);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&crate::interpreter::Value::Bool(true)));
    }
    
    #[test]
    fn test_rot_rot_rot_elimination() {
        let mut asm = Assembler::new();
        asm.emit_i8(1);
        asm.emit_i8(2);
        asm.emit_i8(3);
        asm.emit(Op::Rot);
        asm.emit(Op::Rot);
        asm.emit(Op::Rot);
        asm.emit(Op::Halt);
        let module = asm.finalize().unwrap();
        
        let optimized = optimize(&module.code);
        assert!(optimized.len() < module.code.len());
        
        // Stack should be [1, 2, 3] unchanged
        let mut interp = crate::interpreter::Interpreter::new(&optimized);
        interp.run().unwrap();
        let stack = interp.stack();
        assert_eq!(stack.len(), 3);
        assert_eq!(stack[2], crate::interpreter::Value::Int(3));
    }
    
    #[test]
    fn test_dup_drop_elimination() {
        let mut asm = Assembler::new();
        asm.emit_i8(42);
        asm.emit(Op::Dup);
        asm.emit(Op::Drop);
        asm.emit(Op::Halt);
        let module = asm.finalize().unwrap();
        
        let optimized = optimize(&module.code);
        assert!(optimized.len() < module.code.len());
        
        let mut interp = crate::interpreter::Interpreter::new(&optimized);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&crate::interpreter::Value::Int(42)));
    }
    
    #[test]
    fn test_add_zero_elimination() {
        let mut asm = Assembler::new();
        asm.emit_i8(7);
        asm.emit_i8(0);
        asm.emit(Op::Add);
        asm.emit(Op::Halt);
        let module = asm.finalize().unwrap();
        
        let optimized = optimize(&module.code);
        // INT8 0 ADD should be eliminated → just INT8 7 HALT
        assert!(optimized.len() < module.code.len());
        
        let mut interp = crate::interpreter::Interpreter::new(&optimized);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&crate::interpreter::Value::Int(7)));
    }
    
    #[test]
    fn test_mul_one_elimination() {
        let mut asm = Assembler::new();
        asm.emit_i8(7);
        asm.emit_i8(1);
        asm.emit(Op::Mul);
        asm.emit(Op::Halt);
        let module = asm.finalize().unwrap();
        
        let optimized = optimize(&module.code);
        assert!(optimized.len() < module.code.len());
        
        let mut interp = crate::interpreter::Interpreter::new(&optimized);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&crate::interpreter::Value::Int(7)));
    }
    
    #[test]
    fn test_constant_fold_add() {
        let mut asm = Assembler::new();
        asm.emit_i8(3);
        asm.emit_i8(4);
        asm.emit(Op::Add);
        asm.emit(Op::Halt);
        let module = asm.finalize().unwrap();
        
        let optimized = optimize(&module.code);
        // INT8 3; INT8 4; ADD → INT8 7
        assert!(optimized.len() < module.code.len());
        
        let mut interp = crate::interpreter::Interpreter::new(&optimized);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&crate::interpreter::Value::Int(7)));
    }
    
    #[test]
    fn test_constant_fold_mul() {
        let mut asm = Assembler::new();
        asm.emit_i8(6);
        asm.emit_i8(7);
        asm.emit(Op::Mul);
        asm.emit(Op::Halt);
        let module = asm.finalize().unwrap();
        
        let optimized = optimize(&module.code);
        assert!(optimized.len() < module.code.len());
        
        let mut interp = crate::interpreter::Interpreter::new(&optimized);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&crate::interpreter::Value::Int(42)));
    }
    
    #[test]
    fn test_constant_fold_f64() {
        let mut asm = Assembler::new();
        asm.emit_f64(2.5);
        asm.emit_f64(3.5);
        asm.emit(Op::Fadd);
        asm.emit(Op::Halt);
        let module = asm.finalize().unwrap();
        
        let optimized = optimize(&module.code);
        // F64 2.5; F64 3.5; FADD → F64 6.0
        assert!(optimized.len() < module.code.len());
        
        let mut interp = crate::interpreter::Interpreter::new(&optimized);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&crate::interpreter::Value::Float(6.0)));
    }
    
    #[test]
    fn test_chained_optimizations() {
        // Multiple optimizations should chain: swap swap + neg neg + constant fold
        let mut asm = Assembler::new();
        asm.emit_i8(3);
        asm.emit_i8(4);
        asm.emit(Op::Swap);
        asm.emit(Op::Swap);
        asm.emit(Op::Add);
        asm.emit(Op::Neg);
        asm.emit(Op::Neg);
        asm.emit(Op::Halt);
        let module = asm.finalize().unwrap();
        
        let optimized = optimize(&module.code);
        // swap swap → ε, neg neg → ε, 3+4 folded → INT8 7 HALT
        assert!(optimized.len() < module.code.len());
        
        let mut interp = crate::interpreter::Interpreter::new(&optimized);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&crate::interpreter::Value::Int(7)));
    }
    
    #[test]
    fn test_no_false_optimization() {
        // A single swap should NOT be eliminated
        let mut asm = Assembler::new();
        asm.emit_i8(1);
        asm.emit_i8(2);
        asm.emit(Op::Swap);
        asm.emit(Op::Sub);
        asm.emit(Op::Halt);
        let module = asm.finalize().unwrap();
        
        let optimized = optimize(&module.code);
        
        // Should produce 2 - 1 = 1 (swap then sub)
        let mut interp = crate::interpreter::Interpreter::new(&optimized);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&crate::interpreter::Value::Int(1)));
    }
    
    #[test]
    fn test_bnot_bnot_elimination() {
        // BNOT BNOT → ε (involution)
        let mut asm = Assembler::new();
        asm.emit_i8(42);
        asm.emit(Op::Bnot);
        asm.emit(Op::Bnot);
        asm.emit(Op::Halt);
        let module = asm.finalize().unwrap();
        
        let optimized = optimize(&module.code);
        assert!(optimized.len() < module.code.len(),
            "BNOT BNOT should be eliminated: {} -> {}", module.code.len(), optimized.len());
        
        let mut interp = crate::interpreter::Interpreter::new(&optimized);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&crate::interpreter::Value::Int(42)));
    }
    
    #[test]
    fn test_zero_times_elimination() {
        // INT8 0 TIMES → NOP NOP NOP (zero iterations)
        // We need a quote on the stack for TIMES to consume, so push one first
        let code = vec![
            Op::Int8 as u8, 0,        // push 0
            Op::Times as u8,          // 0 times → noop
            Op::Halt as u8,
        ];
        let optimized = optimize(&code);
        // After optimization, INT8 0 TIMES should become NOPs
        assert!(optimized.len() < code.len(),
            "INT8 0 TIMES should be eliminated: {} -> {}", code.len(), optimized.len());
    }
    
    #[test]
    fn test_one_times_to_apply() {
        // INT8 1 TIMES → APPLY (single iteration = just apply)
        let code = vec![
            Op::Int8 as u8, 1,        // push 1
            Op::Times as u8,          // 1 times → apply
            Op::Halt as u8,
        ];
        let optimized = optimize(&code);
        // Should rewrite to APPLY HALT
        assert!(optimized.contains(&(Op::Apply as u8)),
            "INT8 1 TIMES should become APPLY");
        assert!(!optimized.contains(&(Op::Times as u8)),
            "TIMES should be eliminated");
    }
    
    #[test]
    fn test_nil_is_empty_optimization() {
        // NIL IS_EMPTY → NIL TRUE
        let code = vec![
            Op::Nil as u8,
            Op::IsEmpty as u8,
            Op::Halt as u8,
        ];
        let optimized = optimize(&code);
        // IS_EMPTY should become TRUE
        assert!(optimized.contains(&(Op::True as u8)),
            "NIL IS_EMPTY should produce TRUE");
        assert!(!optimized.contains(&(Op::IsEmpty as u8)),
            "IS_EMPTY should be eliminated after NIL");
        
        // Verify execution: NIL TRUE → stack has [nil, true]
        let mut interp = crate::interpreter::Interpreter::new(&optimized);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&crate::interpreter::Value::Bool(true)));
    }
    
    #[test]
    fn test_empty_list_concat_elimination() {
        // LIST(0) LIST_CONCAT → ε (concat empty list is identity)
        let code = vec![
            Op::List as u8, 0x00, 0x00,  // LIST(0) = empty list
            Op::ListConcat as u8,         // concat with whatever's below
            Op::Halt as u8,
        ];
        let optimized = optimize(&code);
        assert!(optimized.len() < code.len(),
            "LIST(0) LIST_CONCAT should be eliminated: {} -> {}", code.len(), optimized.len());
    }
}
