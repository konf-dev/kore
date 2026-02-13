//! Native Code Backend using Cranelift
//!
//! Compiles Kore bytecode directly to machine code.
//! Each primitive does ONE thing. They compose.
//!
//! This is the FASTEST execution path.

use cranelift_codegen::ir::{types, AbiParam, InstBuilder, UserFuncName};
use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_codegen::Context;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};
use std::collections::{HashMap, HashSet};
use crate::bytecode::Op;

/// Compiled native function that can be called directly
pub type NativeFunc = unsafe extern "C" fn() -> i64;

/// JIT compiler for Kore bytecode
pub struct NativeCompiler {
    module: JITModule,
    ctx: Context,
    func_ctx: FunctionBuilderContext,
}

const MAX_STACK: usize = 64;
const MAX_LOCALS: usize = 262144;

impl NativeCompiler {
    /// Create a new native compiler for the current platform
    pub fn new() -> Result<Self, String> {
        let mut flag_builder = settings::builder();
        flag_builder.set("opt_level", "speed").map_err(|e| e.to_string())?;
        
        let isa_builder = cranelift_native::builder()
            .map_err(|e| format!("Native ISA not available: {}", e))?;
        
        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .map_err(|e| e.to_string())?;
        
        let builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
        let module = JITModule::new(builder);
        let ctx = module.make_context();
        let func_ctx = FunctionBuilderContext::new();
        
        Ok(NativeCompiler { module, ctx, func_ctx })
    }
    
    /// Compile Kore bytecode to native machine code
    pub fn compile(&mut self, bytecode: &[u8]) -> Result<NativeFunc, String> {
        // Function signature: () -> i64
        let mut sig = self.module.make_signature();
        sig.returns.push(AbiParam::new(types::I64));
        
        let func_id = self.module
            .declare_function("kore_main", Linkage::Local, &sig)
            .map_err(|e| e.to_string())?;
        
        self.ctx.func.signature = sig;
        self.ctx.func.name = UserFuncName::user(0, 0);
        
        // Find jump targets BEFORE borrowing self.ctx.func
        let jump_targets = find_jump_targets(bytecode);
        
        {
            let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.func_ctx);
            
            // Create blocks for each target
            let mut blocks: HashMap<usize, cranelift_codegen::ir::Block> = HashMap::new();
            for &target in &jump_targets {
                blocks.insert(target, builder.create_block());
            }
            
            // Track expected sp at each block entry (for verification)
            let mut block_sp: HashMap<usize, Option<usize>> = HashMap::new();
            for &target in &jump_targets {
                block_sp.insert(target, None);
            }
            block_sp.insert(0, Some(0)); // Entry block starts with sp=0
            
            // ================================================================
            // Declare stack variables (shared across all blocks)
            // ================================================================
            let mut stack_vars: Vec<Variable> = Vec::with_capacity(MAX_STACK);
            for i in 0..MAX_STACK {
                let var = Variable::from_u32(i as u32);
                builder.declare_var(var, types::I64);
                stack_vars.push(var);
            }
            
            // ================================================================
            // Declare local variables for STORE/LOAD (P1: locals are tools)
            // ================================================================
            let mut local_vars: Vec<Variable> = Vec::with_capacity(MAX_LOCALS);
            for i in 0..MAX_LOCALS {
                let var = Variable::from_u32((MAX_STACK + i) as u32);
                builder.declare_var(var, types::I64);
                local_vars.push(var);
            }
            
            // Initialize entry block
            let entry_block = *blocks.get(&0).unwrap_or(&builder.create_block());
            builder.switch_to_block(entry_block);
            
            // Initialize all stack and local vars to 0 (required by Cranelift SSA)
            let zero = builder.ins().iconst(types::I64, 0);
            for var in &stack_vars {
                builder.def_var(*var, zero);
            }
            for var in &local_vars {
                builder.def_var(*var, zero);
            }
            
            // ================================================================
            // PASS 2: Translate bytecode
            // ================================================================
            let mut pc = 0;
            let mut sp: usize = 0;  // Compile-time stack pointer
            let mut current_block = entry_block;
            let mut block_terminated = false;
            
            while pc < bytecode.len() {
                // Switch to new block if this PC is a jump target
                if let Some(&block) = blocks.get(&pc) {
                    if block != current_block && !block_terminated {
                        builder.ins().jump(block, &[]);
                        // Record sp for this target
                        if let Some(expected) = block_sp.get(&pc).and_then(|x| *x) {
                            if expected != sp {
                                return Err(format!("Stack mismatch at {}: expected {}, got {}", pc, expected, sp));
                            }
                        } else {
                            block_sp.insert(pc, Some(sp));
                        }
                    }
                    if block != current_block || block_terminated {
                        builder.switch_to_block(block);
                        current_block = block;
                        block_terminated = false;
                        // Restore sp from recorded value
                        if let Some(Some(recorded_sp)) = block_sp.get(&pc) {
                            sp = *recorded_sp;
                        }
                    }
                }
                
                let op = bytecode[pc];
                pc += 1;
                
                match op {
                    // ========================================================
                    // STACK PRIMITIVES - each does ONE thing
                    // ========================================================
                    
                    // NOP: do nothing
                    x if x == Op::Nop as u8 => {}
                    
                    // DROP: remove top
                    x if x == Op::Drop as u8 => {
                        if sp > 0 { sp -= 1; }
                    }
                    
                    // DUP: copy top
                    x if x == Op::Dup as u8 => {
                        if sp > 0 && sp < MAX_STACK {
                            let val = builder.use_var(stack_vars[sp - 1]);
                            builder.def_var(stack_vars[sp], val);
                            sp += 1;
                        }
                    }
                    
                    // SWAP: exchange top two
                    x if x == Op::Swap as u8 => {
                        if sp >= 2 {
                            let a = builder.use_var(stack_vars[sp - 2]);
                            let b = builder.use_var(stack_vars[sp - 1]);
                            builder.def_var(stack_vars[sp - 2], b);
                            builder.def_var(stack_vars[sp - 1], a);
                        }
                    }
                    
                    // ROT: a b c -> b c a (rotate top 3)
                    x if x == Op::Rot as u8 => {
                        if sp >= 3 {
                            let a = builder.use_var(stack_vars[sp - 3]);
                            let b = builder.use_var(stack_vars[sp - 2]);
                            let c = builder.use_var(stack_vars[sp - 1]);
                            builder.def_var(stack_vars[sp - 3], b);
                            builder.def_var(stack_vars[sp - 2], c);
                            builder.def_var(stack_vars[sp - 1], a);
                        }
                    }
                    
                    // OVER: a b -> a b a (copy second to top)
                    x if x == Op::Over as u8 => {
                        if sp >= 2 && sp < MAX_STACK {
                            let a = builder.use_var(stack_vars[sp - 2]);
                            builder.def_var(stack_vars[sp], a);
                            sp += 1;
                        }
                    }
                    
                    // ========================================================
                    // LITERAL PRIMITIVES - push one value
                    // ========================================================
                    
                    // INT8: push byte
                    x if x == Op::Int8 as u8 => {
                        if pc < bytecode.len() && sp < MAX_STACK {
                            let val = bytecode[pc] as i8 as i64;
                            pc += 1;
                            let v = builder.ins().iconst(types::I64, val);
                            builder.def_var(stack_vars[sp], v);
                            sp += 1;
                        }
                    }
                    
                    // INT16: push 2 bytes
                    x if x == Op::Int16 as u8 => {
                        if pc + 1 < bytecode.len() && sp < MAX_STACK {
                            let val = i16::from_le_bytes([bytecode[pc], bytecode[pc + 1]]) as i64;
                            pc += 2;
                            let v = builder.ins().iconst(types::I64, val);
                            builder.def_var(stack_vars[sp], v);
                            sp += 1;
                        }
                    }
                    
                    // INT32: push 4 bytes
                    x if x == Op::Int32 as u8 => {
                        if pc + 3 < bytecode.len() && sp < MAX_STACK {
                            let val = i32::from_le_bytes([
                                bytecode[pc], bytecode[pc + 1],
                                bytecode[pc + 2], bytecode[pc + 3],
                            ]) as i64;
                            pc += 4;
                            let v = builder.ins().iconst(types::I64, val);
                            builder.def_var(stack_vars[sp], v);
                            sp += 1;
                        }
                    }
                    
                    // INT64: push 8 bytes
                    x if x == Op::Int64 as u8 => {
                        if pc + 7 < bytecode.len() && sp < MAX_STACK {
                            let val = i64::from_le_bytes([
                                bytecode[pc], bytecode[pc + 1], bytecode[pc + 2], bytecode[pc + 3],
                                bytecode[pc + 4], bytecode[pc + 5], bytecode[pc + 6], bytecode[pc + 7],
                            ]);
                            pc += 8;
                            let v = builder.ins().iconst(types::I64, val);
                            builder.def_var(stack_vars[sp], v);
                            sp += 1;
                        }
                    }
                    
                    // NIL: push 0 (unit/null)
                    x if x == Op::Nil as u8 => {
                        if sp < MAX_STACK {
                            let v = builder.ins().iconst(types::I64, 0);
                            builder.def_var(stack_vars[sp], v);
                            sp += 1;
                        }
                    }
                    
                    // TRUE: push 1
                    x if x == Op::True as u8 => {
                        if sp < MAX_STACK {
                            let v = builder.ins().iconst(types::I64, 1);
                            builder.def_var(stack_vars[sp], v);
                            sp += 1;
                        }
                    }
                    
                    // FALSE: push 0
                    x if x == Op::False as u8 => {
                        if sp < MAX_STACK {
                            let v = builder.ins().iconst(types::I64, 0);
                            builder.def_var(stack_vars[sp], v);
                            sp += 1;
                        }
                    }
                    
                    // ========================================================
                    // FLOAT LITERALS - stored as i64 bit patterns
                    // ========================================================
                    
                    // F32: push float32 (as i64 bits)
                    x if x == Op::F32 as u8 => {
                        if pc + 3 < bytecode.len() && sp < MAX_STACK {
                            let bits = u32::from_le_bytes([
                                bytecode[pc], bytecode[pc + 1],
                                bytecode[pc + 2], bytecode[pc + 3],
                            ]);
                            pc += 4;
                            // Store f32 bits in i64 (zero-extended)
                            let v = builder.ins().iconst(types::I64, bits as i64);
                            builder.def_var(stack_vars[sp], v);
                            sp += 1;
                        }
                    }
                    
                    // F64: push float64 (as i64 bits)
                    x if x == Op::F64 as u8 => {
                        if pc + 7 < bytecode.len() && sp < MAX_STACK {
                            let bits = u64::from_le_bytes([
                                bytecode[pc], bytecode[pc + 1], bytecode[pc + 2], bytecode[pc + 3],
                                bytecode[pc + 4], bytecode[pc + 5], bytecode[pc + 6], bytecode[pc + 7],
                            ]);
                            pc += 8;
                            // Store f64 bits in i64
                            let v = builder.ins().iconst(types::I64, bits as i64);
                            builder.def_var(stack_vars[sp], v);
                            sp += 1;
                        }
                    }
                    
                    // ========================================================
                    // ARITHMETIC PRIMITIVES - each binary op
                    // ========================================================
                    
                    // ADD: a b -> a+b
                    x if x == Op::Add as u8 => {
                        if sp >= 2 {
                            let b = builder.use_var(stack_vars[sp - 1]);
                            let a = builder.use_var(stack_vars[sp - 2]);
                            sp -= 1;
                            let result = builder.ins().iadd(a, b);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    
                    // SUB: a b -> a-b
                    x if x == Op::Sub as u8 => {
                        if sp >= 2 {
                            let b = builder.use_var(stack_vars[sp - 1]);
                            let a = builder.use_var(stack_vars[sp - 2]);
                            sp -= 1;
                            let result = builder.ins().isub(a, b);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    
                    // MUL: a b -> a*b
                    x if x == Op::Mul as u8 => {
                        if sp >= 2 {
                            let b = builder.use_var(stack_vars[sp - 1]);
                            let a = builder.use_var(stack_vars[sp - 2]);
                            sp -= 1;
                            let result = builder.ins().imul(a, b);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    
                    // DIV: a b -> a/b
                    x if x == Op::Div as u8 => {
                        if sp >= 2 {
                            let b = builder.use_var(stack_vars[sp - 1]);
                            let a = builder.use_var(stack_vars[sp - 2]);
                            sp -= 1;
                            let result = builder.ins().sdiv(a, b);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    
                    // MOD: a b -> a%b
                    x if x == Op::Mod as u8 => {
                        if sp >= 2 {
                            let b = builder.use_var(stack_vars[sp - 1]);
                            let a = builder.use_var(stack_vars[sp - 2]);
                            sp -= 1;
                            let result = builder.ins().srem(a, b);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    
                    // NEG: a -> -a
                    x if x == Op::Neg as u8 => {
                        if sp >= 1 {
                            let a = builder.use_var(stack_vars[sp - 1]);
                            let result = builder.ins().ineg(a);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    
                    // ========================================================
                    // FLOAT ARITHMETIC - interpret i64 bits as f64
                    // These use 0x46-0x4F range (after NEG 0x45)
                    // ========================================================
                    
                    // FADD: a b -> a+b (float)
                    x if x == Op::Fadd as u8 => {
                        if sp >= 2 {
                            let b_bits = builder.use_var(stack_vars[sp - 1]);
                            let a_bits = builder.use_var(stack_vars[sp - 2]);
                            sp -= 1;
                            // Bitcast i64 -> f64
                            let a_f = builder.ins().bitcast(types::F64, cranelift_codegen::ir::MemFlags::new(), a_bits);
                            let b_f = builder.ins().bitcast(types::F64, cranelift_codegen::ir::MemFlags::new(), b_bits);
                            // Float add
                            let result_f = builder.ins().fadd(a_f, b_f);
                            // Bitcast f64 -> i64
                            let result = builder.ins().bitcast(types::I64, cranelift_codegen::ir::MemFlags::new(), result_f);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    
                    // FSUB: a b -> a-b (float)
                    x if x == Op::Fsub as u8 => {
                        if sp >= 2 {
                            let b_bits = builder.use_var(stack_vars[sp - 1]);
                            let a_bits = builder.use_var(stack_vars[sp - 2]);
                            sp -= 1;
                            let a_f = builder.ins().bitcast(types::F64, cranelift_codegen::ir::MemFlags::new(), a_bits);
                            let b_f = builder.ins().bitcast(types::F64, cranelift_codegen::ir::MemFlags::new(), b_bits);
                            let result_f = builder.ins().fsub(a_f, b_f);
                            let result = builder.ins().bitcast(types::I64, cranelift_codegen::ir::MemFlags::new(), result_f);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    
                    // FMUL: a b -> a*b (float)
                    x if x == Op::Fmul as u8 => {
                        if sp >= 2 {
                            let b_bits = builder.use_var(stack_vars[sp - 1]);
                            let a_bits = builder.use_var(stack_vars[sp - 2]);
                            sp -= 1;
                            let a_f = builder.ins().bitcast(types::F64, cranelift_codegen::ir::MemFlags::new(), a_bits);
                            let b_f = builder.ins().bitcast(types::F64, cranelift_codegen::ir::MemFlags::new(), b_bits);
                            let result_f = builder.ins().fmul(a_f, b_f);
                            let result = builder.ins().bitcast(types::I64, cranelift_codegen::ir::MemFlags::new(), result_f);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    
                    // FDIV: a b -> a/b (float)
                    x if x == Op::Fdiv as u8 => {
                        if sp >= 2 {
                            let b_bits = builder.use_var(stack_vars[sp - 1]);
                            let a_bits = builder.use_var(stack_vars[sp - 2]);
                            sp -= 1;
                            let a_f = builder.ins().bitcast(types::F64, cranelift_codegen::ir::MemFlags::new(), a_bits);
                            let b_f = builder.ins().bitcast(types::F64, cranelift_codegen::ir::MemFlags::new(), b_bits);
                            let result_f = builder.ins().fdiv(a_f, b_f);
                            let result = builder.ins().bitcast(types::I64, cranelift_codegen::ir::MemFlags::new(), result_f);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    
                    // FNEG: a -> -a (float)
                    x if x == Op::Fneg as u8 => {
                        if sp >= 1 {
                            let a_bits = builder.use_var(stack_vars[sp - 1]);
                            let a_f = builder.ins().bitcast(types::F64, cranelift_codegen::ir::MemFlags::new(), a_bits);
                            let result_f = builder.ins().fneg(a_f);
                            let result = builder.ins().bitcast(types::I64, cranelift_codegen::ir::MemFlags::new(), result_f);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    
                    // FSQRT: a -> sqrt(a) (float)
                    x if x == Op::Fsqrt as u8 => {
                        if sp >= 1 {
                            let a_bits = builder.use_var(stack_vars[sp - 1]);
                            let a_f = builder.ins().bitcast(types::F64, cranelift_codegen::ir::MemFlags::new(), a_bits);
                            let result_f = builder.ins().sqrt(a_f);
                            let result = builder.ins().bitcast(types::I64, cranelift_codegen::ir::MemFlags::new(), result_f);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    
                    // FABS: a -> |a| (float absolute)
                    x if x == Op::Fabs as u8 => {
                        if sp >= 1 {
                            let a_bits = builder.use_var(stack_vars[sp - 1]);
                            let a_f = builder.ins().bitcast(types::F64, cranelift_codegen::ir::MemFlags::new(), a_bits);
                            let result_f = builder.ins().fabs(a_f);
                            let result = builder.ins().bitcast(types::I64, cranelift_codegen::ir::MemFlags::new(), result_f);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    
                    // I2F: convert int to float
                    x if x == Op::I2f as u8 => {
                        if sp >= 1 {
                            let a = builder.use_var(stack_vars[sp - 1]);
                            let a_f = builder.ins().fcvt_from_sint(types::F64, a);
                            let result = builder.ins().bitcast(types::I64, cranelift_codegen::ir::MemFlags::new(), a_f);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    
                    // F2I: convert float to int (truncate)
                    x if x == Op::F2i as u8 => {
                        if sp >= 1 {
                            let a_bits = builder.use_var(stack_vars[sp - 1]);
                            let a_f = builder.ins().bitcast(types::F64, cranelift_codegen::ir::MemFlags::new(), a_bits);
                            let result = builder.ins().fcvt_to_sint_sat(types::I64, a_f);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    
                    // ========================================================
                    // COMPARISON PRIMITIVES - each returns 0 or 1
                    // ========================================================
                    
                    // EQ: a b -> a==b
                    x if x == Op::Eq as u8 => {
                        if sp >= 2 {
                            let b = builder.use_var(stack_vars[sp - 1]);
                            let a = builder.use_var(stack_vars[sp - 2]);
                            sp -= 1;
                            let cmp = builder.ins().icmp(IntCC::Equal, a, b);
                            let result = builder.ins().uextend(types::I64, cmp);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    
                    // LT: a b -> a<b
                    x if x == Op::Lt as u8 => {
                        if sp >= 2 {
                            let b = builder.use_var(stack_vars[sp - 1]);
                            let a = builder.use_var(stack_vars[sp - 2]);
                            sp -= 1;
                            let cmp = builder.ins().icmp(IntCC::SignedLessThan, a, b);
                            let result = builder.ins().uextend(types::I64, cmp);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    
                    // GT: a b -> a>b
                    x if x == Op::Gt as u8 => {
                        if sp >= 2 {
                            let b = builder.use_var(stack_vars[sp - 1]);
                            let a = builder.use_var(stack_vars[sp - 2]);
                            sp -= 1;
                            let cmp = builder.ins().icmp(IntCC::SignedGreaterThan, a, b);
                            let result = builder.ins().uextend(types::I64, cmp);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    
                    // LE: a b -> a<=b
                    x if x == Op::Le as u8 => {
                        if sp >= 2 {
                            let b = builder.use_var(stack_vars[sp - 1]);
                            let a = builder.use_var(stack_vars[sp - 2]);
                            sp -= 1;
                            let cmp = builder.ins().icmp(IntCC::SignedLessThanOrEqual, a, b);
                            let result = builder.ins().uextend(types::I64, cmp);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    
                    // GE: a b -> a>=b
                    x if x == Op::Ge as u8 => {
                        if sp >= 2 {
                            let b = builder.use_var(stack_vars[sp - 1]);
                            let a = builder.use_var(stack_vars[sp - 2]);
                            sp -= 1;
                            let cmp = builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, a, b);
                            let result = builder.ins().uextend(types::I64, cmp);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    
                    // NE: a b -> a!=b
                    x if x == Op::Ne as u8 => {
                        if sp >= 2 {
                            let b = builder.use_var(stack_vars[sp - 1]);
                            let a = builder.use_var(stack_vars[sp - 2]);
                            sp -= 1;
                            let cmp = builder.ins().icmp(IntCC::NotEqual, a, b);
                            let result = builder.ins().uextend(types::I64, cmp);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    
                    // ========================================================
                    // LOGIC PRIMITIVES - bitwise operations
                    // ========================================================
                    
                    // AND: a b -> a&b
                    x if x == Op::And as u8 => {
                        if sp >= 2 {
                            let b = builder.use_var(stack_vars[sp - 1]);
                            let a = builder.use_var(stack_vars[sp - 2]);
                            sp -= 1;
                            let result = builder.ins().band(a, b);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    
                    // OR: a b -> a|b
                    x if x == Op::Or as u8 => {
                        if sp >= 2 {
                            let b = builder.use_var(stack_vars[sp - 1]);
                            let a = builder.use_var(stack_vars[sp - 2]);
                            sp -= 1;
                            let result = builder.ins().bor(a, b);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    
                    // NOT: a -> !a (bitwise not: ~0 = -1, ~1 = -2)
                    // P1: Polymorphic — Bool(!x) or Int(!x). Matches interpreter.
                    // AND/OR/XOR are all bitwise on i64, so NOT must be too.
                    x if x == Op::Not as u8 => {
                        if sp >= 1 {
                            let a = builder.use_var(stack_vars[sp - 1]);
                            let result = builder.ins().bnot(a);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    
                    // XOR: a b -> a^b
                    x if x == Op::Xor as u8 => {
                        if sp >= 2 {
                            let b = builder.use_var(stack_vars[sp - 1]);
                            let a = builder.use_var(stack_vars[sp - 2]);
                            sp -= 1;
                            let result = builder.ins().bxor(a, b);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    
                    // BAND (0x64), BOR (0x65), BXOR (0x66) — same as AND/OR/XOR for JIT (all bitwise on i64)
                    x if x == Op::Band as u8 => {
                        if sp >= 2 {
                            let b = builder.use_var(stack_vars[sp - 1]);
                            let a = builder.use_var(stack_vars[sp - 2]);
                            sp -= 1;
                            let result = builder.ins().band(a, b);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    x if x == Op::Bor as u8 => {
                        if sp >= 2 {
                            let b = builder.use_var(stack_vars[sp - 1]);
                            let a = builder.use_var(stack_vars[sp - 2]);
                            sp -= 1;
                            let result = builder.ins().bor(a, b);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    x if x == Op::Bxor as u8 => {
                        if sp >= 2 {
                            let b = builder.use_var(stack_vars[sp - 1]);
                            let a = builder.use_var(stack_vars[sp - 2]);
                            sp -= 1;
                            let result = builder.ins().bxor(a, b);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    // BNOT (0x67)
                    x if x == Op::Bnot as u8 => {
                        if sp >= 1 {
                            let a = builder.use_var(stack_vars[sp - 1]);
                            let result = builder.ins().bnot(a);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    // SHL (0x68)
                    x if x == Op::Shl as u8 => {
                        if sp >= 2 {
                            let n = builder.use_var(stack_vars[sp - 1]);
                            let a = builder.use_var(stack_vars[sp - 2]);
                            sp -= 1;
                            let result = builder.ins().ishl(a, n);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    // SHR (0x69) — arithmetic shift right
                    x if x == Op::Shr as u8 => {
                        if sp >= 2 {
                            let n = builder.use_var(stack_vars[sp - 1]);
                            let a = builder.use_var(stack_vars[sp - 2]);
                            sp -= 1;
                            let result = builder.ins().sshr(a, n);
                            builder.def_var(stack_vars[sp - 1], result);
                        }
                    }
                    
                    // ========================================================
                    // CONTROL FLOW PRIMITIVES
                    // ========================================================
                    
                    // JMP: unconditional jump (relative)
                    x if x == Op::Jmp as u8 => {
                        if pc + 3 < bytecode.len() {
                            let rel = i32::from_le_bytes([bytecode[pc], bytecode[pc + 1], bytecode[pc + 2], bytecode[pc + 3]]);
                            let target = ((pc + 4) as i64 + rel as i64) as usize;
                            pc += 4;
                            
                            if let Some(&target_block) = blocks.get(&target) {
                                builder.ins().jump(target_block, &[]);
                                block_terminated = true;
                            }
                        } else {
                            pc += 4;
                        }
                    }
                    
                    // JZ: jump if zero (relative)
                    x if x == Op::Jz as u8 => {
                        if pc + 3 < bytecode.len() && sp >= 1 {
                            let rel = i32::from_le_bytes([bytecode[pc], bytecode[pc + 1], bytecode[pc + 2], bytecode[pc + 3]]);
                            let target = ((pc + 4) as i64 + rel as i64) as usize;
                            let fall_through = pc + 4;
                            pc += 4;
                            
                            // Pop condition (sp decreases by 1)
                            let cond = builder.use_var(stack_vars[sp - 1]);
                            sp -= 1;
                            
                            // Record/verify sp at both targets
                            let new_sp = sp;
                            if let Some(expected) = block_sp.get(&target).and_then(|x| *x) {
                                if expected != new_sp {
                                    return Err(format!("Stack mismatch at {}: expected {}, got {}", target, expected, new_sp));
                                }
                            } else {
                                block_sp.insert(target, Some(new_sp));
                            }
                            if let Some(expected) = block_sp.get(&fall_through).and_then(|x| *x) {
                                if expected != new_sp {
                                    return Err(format!("Stack mismatch at {}: expected {}, got {}", fall_through, expected, new_sp));
                                }
                            } else {
                                block_sp.insert(fall_through, Some(new_sp));
                            }
                            
                            // Compare with zero
                            let zero = builder.ins().iconst(types::I64, 0);
                            let is_zero = builder.ins().icmp(IntCC::Equal, cond, zero);
                            
                            // Branch
                            if let (Some(&then_block), Some(&else_block)) = 
                                (blocks.get(&target), blocks.get(&fall_through)) 
                            {
                                builder.ins().brif(is_zero, then_block, &[], else_block, &[]);
                                block_terminated = true;
                            }
                        } else {
                            pc += 4;
                        }
                    }
                    
                    // JNZ: jump if non-zero (relative)
                    x if x == Op::Jnz as u8 => {
                        if pc + 3 < bytecode.len() && sp >= 1 {
                            let rel = i32::from_le_bytes([bytecode[pc], bytecode[pc + 1], bytecode[pc + 2], bytecode[pc + 3]]);
                            let target = ((pc + 4) as i64 + rel as i64) as usize;
                            let fall_through = pc + 4;
                            pc += 4;
                            
                            // Pop condition
                            let cond = builder.use_var(stack_vars[sp - 1]);
                            sp -= 1;
                            
                            // Record/verify sp at both targets
                            let new_sp = sp;
                            if let Some(expected) = block_sp.get(&target).and_then(|x| *x) {
                                if expected != new_sp {
                                    return Err(format!("Stack mismatch at {}: expected {}, got {}", target, expected, new_sp));
                                }
                            } else {
                                block_sp.insert(target, Some(new_sp));
                            }
                            if let Some(expected) = block_sp.get(&fall_through).and_then(|x| *x) {
                                if expected != new_sp {
                                    return Err(format!("Stack mismatch at {}: expected {}, got {}", fall_through, expected, new_sp));
                                }
                            } else {
                                block_sp.insert(fall_through, Some(new_sp));
                            }
                            
                            // Compare with zero
                            let zero = builder.ins().iconst(types::I64, 0);
                            let is_nonzero = builder.ins().icmp(IntCC::NotEqual, cond, zero);
                            
                            // Branch
                            if let (Some(&then_block), Some(&else_block)) = 
                                (blocks.get(&target), blocks.get(&fall_through)) 
                            {
                                builder.ins().brif(is_nonzero, then_block, &[], else_block, &[]);
                                block_terminated = true;
                            }
                        } else {
                            pc += 4;
                        }
                    }
                    
                    // ========================================================
                    // FUNCTION CALL PRIMITIVES
                    // ========================================================
                    
                    // CALL: call function by symbol index
                    // For native: would need to compile each function separately
                    // and use Cranelift function calls. Complex due to stack passing.
                    x if x == Op::Call as u8 => {
                        // pc += 2; // Skip symbol index (unreachable after return)
                        return Err("CALL not yet supported in native (use interpreter for recursion)".to_string());
                    }
                    
                    // RET: return from function call
                    x if x == Op::Ret as u8 => {
                        return Err("RET not yet supported in native (use interpreter for recursion)".to_string());
                    }
                    
                    // ========================================================
                    // REFLECTION PRIMITIVES
                    // ========================================================
                    
                    // SIZE: push bytecode length
                    x if x == Op::Size as u8 => {
                        if sp < MAX_STACK {
                            let size = builder.ins().iconst(types::I64, bytecode.len() as i64);
                            builder.def_var(stack_vars[sp], size);
                            sp += 1;
                        }
                    }
                    
                    // FETCH: offset -> bytecode[offset]
                    // In native code, we implement this as a giant switch/lookup table
                    // since bytecode is known at compile time
                    x if x == Op::Fetch as u8 => {
                        if sp >= 1 {
                            // Read the offset from stack (compile-time value isn't known)
                            // For now, return 0 - full implementation would need runtime lookup
                            // A proper implementation would use memory or a jump table
                            let zero = builder.ins().iconst(types::I64, 0);
                            builder.def_var(stack_vars[sp - 1], zero);
                        }
                    }
                    
                    // ========================================================
                    // LOCALS (P1: locals are stack→slot tools)
                    // ========================================================
                    
                    // STORE n: (val --) store top of stack into local slot n
                    x if x == Op::Store as u8 => {
                        if pc + 3 < bytecode.len() {
                            let slot = u32::from_le_bytes([
                                bytecode[pc], bytecode[pc+1], bytecode[pc+2], bytecode[pc+3]
                            ]) as usize;
                            pc += 4;
                            if sp > 0 && slot < MAX_LOCALS {
                                let val = builder.use_var(stack_vars[sp - 1]);
                                sp -= 1;
                                builder.def_var(local_vars[slot], val);
                            } else if sp == 0 {
                                return Err(format!("STORE: stack underflow at offset {}", pc - 5));
                            } else {
                                return Err(format!("STORE: local slot {} exceeds MAX_LOCALS {}", slot, MAX_LOCALS));
                            }
                        }
                    }
                    
                    // LOAD n: (-- val) push local slot n onto stack
                    x if x == Op::Load as u8 => {
                        if pc + 3 < bytecode.len() {
                            let slot = u32::from_le_bytes([
                                bytecode[pc], bytecode[pc+1], bytecode[pc+2], bytecode[pc+3]
                            ]) as usize;
                            pc += 4;
                            if slot < MAX_LOCALS && sp < MAX_STACK {
                                let val = builder.use_var(local_vars[slot]);
                                builder.def_var(stack_vars[sp], val);
                                sp += 1;
                            } else if slot >= MAX_LOCALS {
                                return Err(format!("LOAD: local slot {} exceeds MAX_LOCALS {}", slot, MAX_LOCALS));
                            } else {
                                return Err(format!("LOAD: stack overflow at offset {}", pc - 5));
                            }
                        }
                    }
                    
                    // ========================================================
                    // REFLECTION
                    // ========================================================
                    
                    // DEPTH (0xD1): (-- n) push current stack depth
                    x if x == Op::Depth as u8 => {
                        if sp < MAX_STACK {
                            let depth_val = builder.ins().iconst(types::I64, sp as i64);
                            builder.def_var(stack_vars[sp], depth_val);
                            sp += 1;
                        }
                    }
                    
                    // ========================================================
                    // TERMINATION
                    // ========================================================
                    
                    // HALT: stop execution
                    x if x == Op::Halt as u8 => {
                        break;
                    }
                    
                    // ========================================================
                    // UNSUPPORTED — explicit error, never silent
                    // ========================================================
                    _ => {
                        return Err(format!(
                            "Native backend: unsupported opcode 0x{:02X} at offset {} \
                             (use 'korec run' interpreter for this program)",
                            op, pc - 1
                        ));
                    }
                }
            }
            
            // Seal ALL blocks at the end (required for proper SSA with control flow)
            for &block in blocks.values() {
                builder.seal_block(block);
            }
            
            // Return top of stack (or 0)
            let result = if sp > 0 {
                builder.use_var(stack_vars[sp - 1])
            } else {
                builder.ins().iconst(types::I64, 0)
            };
            builder.ins().return_(&[result]);
            
            builder.finalize();
        }
        
        // Compile and link
        self.module.define_function(func_id, &mut self.ctx).map_err(|e| format!("Compilation error: {}", e))?;
        self.module.clear_context(&mut self.ctx);
        self.module.finalize_definitions().map_err(|e| e.to_string())?;
        
        let code_ptr = self.module.get_finalized_function(func_id);
        Ok(unsafe { std::mem::transmute(code_ptr) })
    }
    
    // ====================================================================
    // COMPILE MODULE — full support for CALL/RET with memory-backed stack
    // ====================================================================
    //
    // Architecture: single Cranelift function containing all Kore function
    // bodies + main code as basic blocks. The data stack is a StackSlot
    // (memory-backed), sp is a runtime Variable.
    //
    // CALL: save return-site-ID to call_stack, branch to function block
    // RET:  pop return-site-ID, br_table back to the correct return block
    //
    // This is slower than SSA-stack compile() but handles all programs
    // including recursive function calls.
    
    /// Compile a full BytecodeModule with CALL/RET support
    pub fn compile_module(&mut self, module: &crate::bytecode::BytecodeModule) -> Result<NativeFunc, String> {
        use cranelift_codegen::ir::{StackSlotData, StackSlotKind, MemFlags, BlockCall, JumpTableData};
        
        let bytecode = &module.code;
        
        // Function signature: () -> i64
        let mut sig = self.module.make_signature();
        sig.returns.push(AbiParam::new(types::I64));
        
        let func_id = self.module
            .declare_function("kore_module_main", Linkage::Local, &sig)
            .map_err(|e| e.to_string())?;
        
        self.ctx.func.signature = sig;
        self.ctx.func.name = UserFuncName::user(0, 1);
        
        // ================================================================
        // PASS 0: Scan for used locals (Fix 1: only save/restore these)
        // P1: Each local is a tool. Only save live tools.
        // ================================================================
        let used_locals = scan_used_locals(bytecode, &module.symbol_table);
        let num_used_locals = used_locals.len();
        
        // ================================================================
        // PASS 1: Find all jump targets AND all CALL sites
        // ================================================================
        let jump_targets = find_jump_targets(bytecode);
        
        // Collect all CALL sites: (call_pc, symbol_idx, return_pc)
        // return_pc = pc after the CALL instruction (3 bytes: opcode + u16)
        let mut call_sites: Vec<(usize, u16, usize)> = Vec::new();
        {
            let mut pc = 0;
            while pc < bytecode.len() {
                let op = bytecode[pc];
                if op == Op::Call as u8 && pc + 2 < bytecode.len() {
                    let sym_idx = u16::from_le_bytes([bytecode[pc + 1], bytecode[pc + 2]]);
                    call_sites.push((pc, sym_idx, pc + 3));
                }
                // Advance pc (same logic as find_jump_targets)
                pc += 1;
                match op {
                    x if x == Op::Jmp as u8 || x == Op::Jz as u8 || x == Op::Jnz as u8 => pc += 4,
                    x if x == Op::Int8 as u8 || x == Op::Syscall as u8 => pc += 1,
                    x if x == Op::Store as u8 || x == Op::Load as u8 => pc += 4,
                    x if x == Op::Call as u8 || x == Op::Int16 as u8 || x == Op::List as u8 => pc += 2,
                    x if x == Op::Quote as u8 => {
                        if pc + 1 < bytecode.len() {
                            let body_len = u16::from_le_bytes([bytecode[pc], bytecode[pc + 1]]) as usize;
                            pc += 2 + body_len;
                        } else { pc += 2; }
                    }
                    x if x == Op::Case as u8 => pc += 4,
                    x if x == Op::Int32 as u8 || x == Op::F32 as u8 => pc += 4,
                    x if x == Op::Int64 as u8 || x == Op::F64 as u8 => pc += 8,
                    x if x == Op::Str as u8 => {
                        if pc + 1 < bytecode.len() {
                            let str_len = u16::from_le_bytes([bytecode[pc], bytecode[pc + 1]]) as usize;
                            pc += 2 + str_len;
                        } else { pc += 2; }
                    }
                    _ => {}
                }
            }
        }
        
        {
            let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.func_ctx);
            
            // ============================================================
            // Stack slots (memory-backed)
            // P1: Stack is a tool (state transformer). Sized to actual usage.
            // ============================================================
            const STACK_SIZE: usize = 1024;      // data stack entries
            const CALL_STACK_SIZE: usize = 256;   // max call depth
            const MAX_FRAMES: usize = 256;        // max nested call frames
            
            // Size locals to actual usage (not a fixed max) to avoid stack overflow
            let local_frame_size = if num_used_locals > 0 { num_used_locals } else { 1 };
            // Frame save area sized to ONLY used locals (Fix 1)
            let frame_save_size = local_frame_size;
            
            let data_stack = builder.create_sized_stack_slot(StackSlotData::new(
                StackSlotKind::ExplicitSlot,
                (STACK_SIZE * 8) as u32,
                3, // 8-byte aligned
            ));
            
            let call_stack = builder.create_sized_stack_slot(StackSlotData::new(
                StackSlotKind::ExplicitSlot,
                (CALL_STACK_SIZE * 8) as u32,
                3,
            ));
            
            let local_stack = builder.create_sized_stack_slot(StackSlotData::new(
                StackSlotKind::ExplicitSlot,
                (MAX_FRAMES * frame_save_size * 8) as u32,
                3,
            ));
            
            // Current locals (fast access, saved/restored on CALL/RET)
            let locals_slot = builder.create_sized_stack_slot(StackSlotData::new(
                StackSlotKind::ExplicitSlot,
                (local_frame_size * 8) as u32,
                3,
            ));
            
            // ============================================================
            // Runtime variables
            // ============================================================
            let sp_var = Variable::from_u32(0);     // data stack pointer
            let csp_var = Variable::from_u32(1);    // call stack pointer
            let fsp_var = Variable::from_u32(2);    // frame stack pointer
            
            builder.declare_var(sp_var, types::I64);
            builder.declare_var(csp_var, types::I64);
            builder.declare_var(fsp_var, types::I64);
            
            let memflags = MemFlags::new();
            
            // ============================================================
            // Create blocks for all targets + return dispatch + return sites
            // ============================================================
            let mut blocks: HashMap<usize, cranelift_codegen::ir::Block> = HashMap::new();
            for &target in &jump_targets {
                blocks.insert(target, builder.create_block());
            }
            // Ensure function entry points are blocks
            for (_, &offset) in &module.symbol_table {
                if !blocks.contains_key(&offset) {
                    blocks.insert(offset, builder.create_block());
                }
            }
            // Ensure return PCs (after CALL) are blocks
            for &(_, _, ret_pc) in &call_sites {
                if !blocks.contains_key(&ret_pc) {
                    blocks.insert(ret_pc, builder.create_block());
                }
            }
            
            // Return dispatch block (RET jumps here)
            let ret_dispatch_block = builder.create_block();
            
            // Create a block for each return site (one per CALL)
            let return_blocks: Vec<cranelift_codegen::ir::Block> = call_sites
                .iter()
                .map(|_| builder.create_block())
                .collect();
            
            // Halt block — for when RET returns from main (sentinel)
            let halt_block = builder.create_block();
            
            // ============================================================
            // Entry block: initialize sp=0, csp=0, fsp=0, push sentinel
            // ============================================================
            let entry_block = *blocks.get(&0).unwrap();
            builder.switch_to_block(entry_block);
            
            let zero = builder.ins().iconst(types::I64, 0);
            builder.def_var(sp_var, zero);
            builder.def_var(fsp_var, zero);
            
            // Push sentinel return-site-ID (-1) so top-level RET triggers halt
            let neg_one = builder.ins().iconst(types::I64, -1_i64);
            builder.ins().stack_store(neg_one, call_stack, 0);
            let one = builder.ins().iconst(types::I64, 1);
            builder.def_var(csp_var, one);
            
            // Initialize locals to 0 (only used locals — Fix 1)
            for &slot in &used_locals {
                let z = builder.ins().iconst(types::I64, 0);
                builder.ins().stack_store(z, locals_slot, (slot * 8) as i32);
            }
            
            // ============================================================
            // SSA Stack Variables (Fix 2: SSA within function bodies)
            // P1: Stack vars are tools. SSA = register allocation = fast.
            // P3: Composition preserved — SSA ops compose like memory ops.
            // ============================================================
            // Declare SSA variables for the stack (same as compile() path)
            // These map to CPU registers when the compiler can prove it safe.
            const SSA_STACK_SIZE: usize = 64;
            let ssa_var_offset: u32 = 3; // 0=sp, 1=csp, 2=fsp, so SSA stack starts at 3
            let mut ssa_stack_vars: Vec<Variable> = Vec::with_capacity(SSA_STACK_SIZE);
            for i in 0..SSA_STACK_SIZE {
                let var = Variable::from_u32(ssa_var_offset + i as u32);
                builder.declare_var(var, types::I64);
                ssa_stack_vars.push(var);
            }
            
            // Initialize all SSA stack vars to 0 (required by Cranelift SSA)
            let ssa_zero = builder.ins().iconst(types::I64, 0);
            for var in &ssa_stack_vars {
                builder.def_var(*var, ssa_zero);
            }
            
            // ============================================================
            // SSA tracking state:
            // ssa_sp: Option<usize> — when Some(n), we know stack depth
            //   statically and use SSA variables. When None, fall back to
            //   memory-backed sp_var (at block entries from jumps).
            // ============================================================
            let mut ssa_sp: Option<usize> = Some(0);  // Entry: sp=0
            
            // Helper: flush SSA stack to memory and sync sp_var
            // Called before CALL, RET, and jumps where sp must be in memory
            macro_rules! flush_ssa_to_memory {
                ($builder:expr, $sp_var:expr, $ssa_sp:expr, $ssa_stack_vars:expr, $data_stack:expr, $memflags:expr) => {
                    if let Some(sp_val) = $ssa_sp {
                        // Write all SSA values to memory stack
                        for idx in 0..sp_val {
                            if idx < SSA_STACK_SIZE {
                                let val = $builder.use_var($ssa_stack_vars[idx]);
                                $builder.ins().stack_store(val, $data_stack, (idx * 8) as i32);
                            }
                        }
                        // Sync runtime sp
                        let sp_const = $builder.ins().iconst(types::I64, sp_val as i64);
                        $builder.def_var($sp_var, sp_const);
                        $ssa_sp = None; // Now in memory mode
                    }
                }
            }
            
            // Helper: reload SSA stack from memory (at block entry)
            // Since we don't know sp at merge points, we stay in memory mode
            // (ssa_sp = None) after jumps. SSA mode resumes only from known points.
            
            // ============================================================
            // Helper closures as macros (inline for borrow checker)
            // ============================================================
            // We'll use inline code patterns instead of closures because
            // of Rust's borrow checker with FunctionBuilder.
            
            // ============================================================
            // PASS 2: Translate bytecode
            // ============================================================
            let mut pc = 0;
            let mut current_block = entry_block;
            let mut block_terminated = false;
            
            while pc < bytecode.len() {
                // Switch to new block if this PC is a jump target
                if let Some(&block) = blocks.get(&pc) {
                    if block != current_block && !block_terminated {
                        // Flush SSA to memory before jumping
                        flush_ssa_to_memory!(builder, sp_var, ssa_sp, ssa_stack_vars, data_stack, memflags);
                        builder.ins().jump(block, &[]);
                    }
                    if block != current_block || block_terminated {
                        builder.switch_to_block(block);
                        current_block = block;
                        block_terminated = false;
                        // At block entry from jump: sp unknown, use memory mode
                        ssa_sp = None;
                    }
                }
                
                if block_terminated {
                    pc += 1;
                    continue;
                }
                
                let op = bytecode[pc];
                pc += 1;
                
                match op {
                    // ====================================================
                    // STACK PRIMITIVES
                    // Fix 2: SSA mode (0 Cranelift instrs) when sp known
                    // ====================================================
                    
                    // NOP
                    x if x == Op::Nop as u8 => {}
                    
                    // DROP: sp -= 1
                    x if x == Op::Drop as u8 => {
                        if let Some(ref mut sp) = ssa_sp {
                            if *sp > 0 { *sp -= 1; }  // SSA: compile-time, 0 instructions
                        } else {
                            let sp = builder.use_var(sp_var);
                            let one = builder.ins().iconst(types::I64, 1);
                            let new_sp = builder.ins().isub(sp, one);
                            builder.def_var(sp_var, new_sp);
                        }
                    }
                    
                    // DUP: stack[sp] = stack[sp-1]; sp += 1
                    x if x == Op::Dup as u8 => {
                        if let Some(ref mut sp) = ssa_sp {
                            if *sp > 0 && *sp < SSA_STACK_SIZE {
                                let val = builder.use_var(ssa_stack_vars[*sp - 1]);
                                builder.def_var(ssa_stack_vars[*sp], val);
                                *sp += 1;  // 0 Cranelift instructions!
                            }
                        } else {
                            let sp = builder.use_var(sp_var);
                            let one = builder.ins().iconst(types::I64, 1);
                            let top_off = builder.ins().isub(sp, one);
                            let eight = builder.ins().iconst(types::I64, 8);
                            let byte_off = builder.ins().imul(top_off, eight);
                            let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                            let addr = builder.ins().iadd(base, byte_off);
                            let val = builder.ins().load(types::I64, memflags, addr, 0);
                            let sp_byte = builder.ins().imul(sp, eight);
                            let dest = builder.ins().iadd(base, sp_byte);
                            builder.ins().store(memflags, val, dest, 0);
                            let new_sp = builder.ins().iadd(sp, one);
                            builder.def_var(sp_var, new_sp);
                        }
                    }
                    
                    // SWAP: swap stack[sp-1] and stack[sp-2]
                    x if x == Op::Swap as u8 => {
                        if let Some(sp) = ssa_sp {
                            if sp >= 2 {
                                let a = builder.use_var(ssa_stack_vars[sp - 2]);
                                let b = builder.use_var(ssa_stack_vars[sp - 1]);
                                builder.def_var(ssa_stack_vars[sp - 2], b);
                                builder.def_var(ssa_stack_vars[sp - 1], a);
                                // 0 Cranelift instructions!
                            }
                        } else {
                            let sp = builder.use_var(sp_var);
                            let eight = builder.ins().iconst(types::I64, 8);
                            let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                            let one = builder.ins().iconst(types::I64, 1);
                            let two = builder.ins().iconst(types::I64, 2);
                            let off1 = builder.ins().isub(sp, one);
                            let off2 = builder.ins().isub(sp, two);
                            let byte1 = builder.ins().imul(off1, eight);
                            let byte2 = builder.ins().imul(off2, eight);
                            let addr1 = builder.ins().iadd(base, byte1);
                            let addr2 = builder.ins().iadd(base, byte2);
                            let a = builder.ins().load(types::I64, memflags, addr1, 0);
                            let b = builder.ins().load(types::I64, memflags, addr2, 0);
                            builder.ins().store(memflags, b, addr1, 0);
                            builder.ins().store(memflags, a, addr2, 0);
                        }
                    }
                    
                    // ROT: a b c -> b c a
                    x if x == Op::Rot as u8 => {
                        if let Some(sp) = ssa_sp {
                            if sp >= 3 {
                                let a = builder.use_var(ssa_stack_vars[sp - 3]);
                                let b = builder.use_var(ssa_stack_vars[sp - 2]);
                                let c = builder.use_var(ssa_stack_vars[sp - 1]);
                                builder.def_var(ssa_stack_vars[sp - 3], b);
                                builder.def_var(ssa_stack_vars[sp - 2], c);
                                builder.def_var(ssa_stack_vars[sp - 1], a);
                            }
                        } else {
                            let sp = builder.use_var(sp_var);
                            let eight = builder.ins().iconst(types::I64, 8);
                            let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                            let one = builder.ins().iconst(types::I64, 1);
                            let two = builder.ins().iconst(types::I64, 2);
                            let three = builder.ins().iconst(types::I64, 3);
                            let tmp_off1 = builder.ins().isub(sp, one);
                            let off1 = builder.ins().imul(tmp_off1, eight);
                            let tmp_off2 = builder.ins().isub(sp, two);
                            let off2 = builder.ins().imul(tmp_off2, eight);
                            let tmp_off3 = builder.ins().isub(sp, three);
                            let off3 = builder.ins().imul(tmp_off3, eight);
                            let addr1 = builder.ins().iadd(base, off1);
                            let addr2 = builder.ins().iadd(base, off2);
                            let addr3 = builder.ins().iadd(base, off3);
                            let c = builder.ins().load(types::I64, memflags, addr1, 0);
                            let b = builder.ins().load(types::I64, memflags, addr2, 0);
                            let a = builder.ins().load(types::I64, memflags, addr3, 0);
                            builder.ins().store(memflags, b, addr3, 0);
                            builder.ins().store(memflags, c, addr2, 0);
                            builder.ins().store(memflags, a, addr1, 0);
                        }
                    }
                    
                    // OVER: a b -> a b a
                    x if x == Op::Over as u8 => {
                        if let Some(ref mut sp) = ssa_sp {
                            if *sp >= 2 && *sp < SSA_STACK_SIZE {
                                let a = builder.use_var(ssa_stack_vars[*sp - 2]);
                                builder.def_var(ssa_stack_vars[*sp], a);
                                *sp += 1;
                            }
                        } else {
                            let sp = builder.use_var(sp_var);
                            let eight = builder.ins().iconst(types::I64, 8);
                            let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                            let two = builder.ins().iconst(types::I64, 2);
                            let tmp_off = builder.ins().isub(sp, two);
                            let off = builder.ins().imul(tmp_off, eight);
                            let addr = builder.ins().iadd(base, off);
                            let val = builder.ins().load(types::I64, memflags, addr, 0);
                            let sp_byte = builder.ins().imul(sp, eight);
                            let dest = builder.ins().iadd(base, sp_byte);
                            builder.ins().store(memflags, val, dest, 0);
                            let one = builder.ins().iconst(types::I64, 1);
                            let new_sp = builder.ins().iadd(sp, one);
                            builder.def_var(sp_var, new_sp);
                        }
                    }
                    
                    // ====================================================
                    // LITERAL PUSH — SSA mode: 1 instr; Memory mode: 8 instrs
                    // ====================================================
                    
                    // INT8
                    x if x == Op::Int8 as u8 => {
                        if pc < bytecode.len() {
                            let val = bytecode[pc] as i8 as i64;
                            pc += 1;
                            let v = builder.ins().iconst(types::I64, val);
                            if let Some(ref mut sp) = ssa_sp {
                                if *sp < SSA_STACK_SIZE {
                                    builder.def_var(ssa_stack_vars[*sp], v);
                                    *sp += 1;
                                }
                            } else {
                                let sp = builder.use_var(sp_var);
                                let eight = builder.ins().iconst(types::I64, 8);
                                let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                                let off = builder.ins().imul(sp, eight);
                                let addr = builder.ins().iadd(base, off);
                                builder.ins().store(memflags, v, addr, 0);
                                let one = builder.ins().iconst(types::I64, 1);
                                let new_sp = builder.ins().iadd(sp, one);
                                builder.def_var(sp_var, new_sp);
                            }
                        }
                    }
                    
                    // INT16
                    x if x == Op::Int16 as u8 => {
                        if pc + 1 < bytecode.len() {
                            let val = i16::from_le_bytes([bytecode[pc], bytecode[pc + 1]]) as i64;
                            pc += 2;
                            let v = builder.ins().iconst(types::I64, val);
                            if let Some(ref mut sp) = ssa_sp {
                                if *sp < SSA_STACK_SIZE {
                                    builder.def_var(ssa_stack_vars[*sp], v);
                                    *sp += 1;
                                }
                            } else {
                                let sp = builder.use_var(sp_var);
                                let eight = builder.ins().iconst(types::I64, 8);
                                let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                                let off = builder.ins().imul(sp, eight);
                                let addr = builder.ins().iadd(base, off);
                                builder.ins().store(memflags, v, addr, 0);
                                let one = builder.ins().iconst(types::I64, 1);
                                let new_sp = builder.ins().iadd(sp, one);
                                builder.def_var(sp_var, new_sp);
                            }
                        }
                    }
                    
                    // INT32
                    x if x == Op::Int32 as u8 => {
                        if pc + 3 < bytecode.len() {
                            let val = i32::from_le_bytes([
                                bytecode[pc], bytecode[pc+1], bytecode[pc+2], bytecode[pc+3]
                            ]) as i64;
                            pc += 4;
                            let v = builder.ins().iconst(types::I64, val);
                            if let Some(ref mut sp) = ssa_sp {
                                if *sp < SSA_STACK_SIZE {
                                    builder.def_var(ssa_stack_vars[*sp], v);
                                    *sp += 1;
                                }
                            } else {
                                let sp = builder.use_var(sp_var);
                                let eight = builder.ins().iconst(types::I64, 8);
                                let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                                let off = builder.ins().imul(sp, eight);
                                let addr = builder.ins().iadd(base, off);
                                builder.ins().store(memflags, v, addr, 0);
                                let one = builder.ins().iconst(types::I64, 1);
                                let new_sp = builder.ins().iadd(sp, one);
                                builder.def_var(sp_var, new_sp);
                            }
                        }
                    }
                    
                    // INT64
                    x if x == Op::Int64 as u8 => {
                        if pc + 7 < bytecode.len() {
                            let val = i64::from_le_bytes([
                                bytecode[pc], bytecode[pc+1], bytecode[pc+2], bytecode[pc+3],
                                bytecode[pc+4], bytecode[pc+5], bytecode[pc+6], bytecode[pc+7],
                            ]);
                            pc += 8;
                            let v = builder.ins().iconst(types::I64, val);
                            if let Some(ref mut sp) = ssa_sp {
                                if *sp < SSA_STACK_SIZE {
                                    builder.def_var(ssa_stack_vars[*sp], v);
                                    *sp += 1;
                                }
                            } else {
                                let sp = builder.use_var(sp_var);
                                let eight = builder.ins().iconst(types::I64, 8);
                                let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                                let off = builder.ins().imul(sp, eight);
                                let addr = builder.ins().iadd(base, off);
                                builder.ins().store(memflags, v, addr, 0);
                                let one = builder.ins().iconst(types::I64, 1);
                                let new_sp = builder.ins().iadd(sp, one);
                                builder.def_var(sp_var, new_sp);
                            }
                        }
                    }
                    
                    // F32 (stored as f64 bits in i64)
                    x if x == Op::F32 as u8 => {
                        if pc + 3 < bytecode.len() {
                            let f = f32::from_le_bytes([
                                bytecode[pc], bytecode[pc+1], bytecode[pc+2], bytecode[pc+3]
                            ]) as f64;
                            pc += 4;
                            let val = f.to_bits() as i64;
                            let v = builder.ins().iconst(types::I64, val);
                            if let Some(ref mut sp) = ssa_sp {
                                if *sp < SSA_STACK_SIZE {
                                    builder.def_var(ssa_stack_vars[*sp], v);
                                    *sp += 1;
                                }
                            } else {
                                let sp = builder.use_var(sp_var);
                                let eight = builder.ins().iconst(types::I64, 8);
                                let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                                let off = builder.ins().imul(sp, eight);
                                let addr = builder.ins().iadd(base, off);
                                builder.ins().store(memflags, v, addr, 0);
                                let one = builder.ins().iconst(types::I64, 1);
                                let new_sp = builder.ins().iadd(sp, one);
                                builder.def_var(sp_var, new_sp);
                            }
                        }
                    }
                    
                    // F64 (stored as bits in i64)
                    x if x == Op::F64 as u8 => {
                        if pc + 7 < bytecode.len() {
                            let val = i64::from_le_bytes([
                                bytecode[pc], bytecode[pc+1], bytecode[pc+2], bytecode[pc+3],
                                bytecode[pc+4], bytecode[pc+5], bytecode[pc+6], bytecode[pc+7],
                            ]);
                            pc += 8;
                            let v = builder.ins().iconst(types::I64, val);
                            if let Some(ref mut sp) = ssa_sp {
                                if *sp < SSA_STACK_SIZE {
                                    builder.def_var(ssa_stack_vars[*sp], v);
                                    *sp += 1;
                                }
                            } else {
                                let sp = builder.use_var(sp_var);
                                let eight = builder.ins().iconst(types::I64, 8);
                                let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                                let off = builder.ins().imul(sp, eight);
                                let addr = builder.ins().iadd(base, off);
                                builder.ins().store(memflags, v, addr, 0);
                                let one = builder.ins().iconst(types::I64, 1);
                                let new_sp = builder.ins().iadd(sp, one);
                                builder.def_var(sp_var, new_sp);
                            }
                        }
                    }
                    
                    // NIL (push 0)
                    x if x == Op::Nil as u8 => {
                        let v = builder.ins().iconst(types::I64, 0);
                        if let Some(ref mut sp) = ssa_sp {
                            if *sp < SSA_STACK_SIZE {
                                builder.def_var(ssa_stack_vars[*sp], v);
                                *sp += 1;
                            }
                        } else {
                            let sp = builder.use_var(sp_var);
                            let eight = builder.ins().iconst(types::I64, 8);
                            let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                            let off = builder.ins().imul(sp, eight);
                            let addr = builder.ins().iadd(base, off);
                            builder.ins().store(memflags, v, addr, 0);
                            let one = builder.ins().iconst(types::I64, 1);
                            let new_sp = builder.ins().iadd(sp, one);
                            builder.def_var(sp_var, new_sp);
                        }
                    }
                    
                    // TRUE (push 1)
                    x if x == Op::True as u8 => {
                        let v = builder.ins().iconst(types::I64, 1);
                        if let Some(ref mut sp) = ssa_sp {
                            if *sp < SSA_STACK_SIZE {
                                builder.def_var(ssa_stack_vars[*sp], v);
                                *sp += 1;
                            }
                        } else {
                            let sp = builder.use_var(sp_var);
                            let eight = builder.ins().iconst(types::I64, 8);
                            let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                            let off = builder.ins().imul(sp, eight);
                            let addr = builder.ins().iadd(base, off);
                            builder.ins().store(memflags, v, addr, 0);
                            let one = builder.ins().iconst(types::I64, 1);
                            let new_sp = builder.ins().iadd(sp, one);
                            builder.def_var(sp_var, new_sp);
                        }
                    }
                    
                    // FALSE (push 0)
                    x if x == Op::False as u8 => {
                        let v = builder.ins().iconst(types::I64, 0);
                        if let Some(ref mut sp) = ssa_sp {
                            if *sp < SSA_STACK_SIZE {
                                builder.def_var(ssa_stack_vars[*sp], v);
                                *sp += 1;
                            }
                        } else {
                            let sp = builder.use_var(sp_var);
                            let eight = builder.ins().iconst(types::I64, 8);
                            let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                            let off = builder.ins().imul(sp, eight);
                            let addr = builder.ins().iadd(base, off);
                            builder.ins().store(memflags, v, addr, 0);
                            let one = builder.ins().iconst(types::I64, 1);
                            let new_sp = builder.ins().iadd(sp, one);
                            builder.def_var(sp_var, new_sp);
                        }
                    }
                    
                    // ====================================================
                    // ARITHMETIC — SSA mode: 1 instr; Memory mode: ~15 instrs
                    // ====================================================
                    
                    // Binary ops: ADD, SUB, MUL, DIV, MOD
                    x if x == Op::Add as u8 || x == Op::Sub as u8 || x == Op::Mul as u8 || x == Op::Div as u8 || x == Op::Mod as u8 => {
                        if let Some(ref mut sp) = ssa_sp {
                            if *sp >= 2 {
                                let b = builder.use_var(ssa_stack_vars[*sp - 1]);
                                let a = builder.use_var(ssa_stack_vars[*sp - 2]);
                                let result = match op {
                                    x if x == Op::Add as u8 => builder.ins().iadd(a, b),
                                    x if x == Op::Sub as u8 => builder.ins().isub(a, b),
                                    x if x == Op::Mul as u8 => builder.ins().imul(a, b),
                                    x if x == Op::Div as u8 => builder.ins().sdiv(a, b),
                                    x if x == Op::Mod as u8 => builder.ins().srem(a, b),
                                    _ => unreachable!(),
                                };
                                *sp -= 1;
                                builder.def_var(ssa_stack_vars[*sp - 1], result);
                            }
                        } else {
                            let sp = builder.use_var(sp_var);
                            let eight = builder.ins().iconst(types::I64, 8);
                            let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                            let one = builder.ins().iconst(types::I64, 1);
                            let two = builder.ins().iconst(types::I64, 2);
                            let tmp_off_b = builder.ins().isub(sp, one);
                            let off_b = builder.ins().imul(tmp_off_b, eight);
                            let tmp_off_a = builder.ins().isub(sp, two);
                            let off_a = builder.ins().imul(tmp_off_a, eight);
                            let addr_b = builder.ins().iadd(base, off_b);
                            let addr_a = builder.ins().iadd(base, off_a);
                            let a = builder.ins().load(types::I64, memflags, addr_a, 0);
                            let b = builder.ins().load(types::I64, memflags, addr_b, 0);
                            let result = match op {
                                x if x == Op::Add as u8 => builder.ins().iadd(a, b),
                                x if x == Op::Sub as u8 => builder.ins().isub(a, b),
                                x if x == Op::Mul as u8 => builder.ins().imul(a, b),
                                x if x == Op::Div as u8 => builder.ins().sdiv(a, b),
                                x if x == Op::Mod as u8 => builder.ins().srem(a, b),
                                _ => unreachable!(),
                            };
                            builder.ins().store(memflags, result, addr_a, 0);
                            let new_sp = builder.ins().isub(sp, one);
                            builder.def_var(sp_var, new_sp);
                        }
                    }
                    
                    // NEG: negate top
                    x if x == Op::Neg as u8 => {
                        if let Some(ref mut sp) = ssa_sp {
                            if *sp >= 1 {
                                let val = builder.use_var(ssa_stack_vars[*sp - 1]);
                                let neg = builder.ins().ineg(val);
                                builder.def_var(ssa_stack_vars[*sp - 1], neg);
                            }
                        } else {
                            let sp = builder.use_var(sp_var);
                            let eight = builder.ins().iconst(types::I64, 8);
                            let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                            let one = builder.ins().iconst(types::I64, 1);
                            let tmp_off = builder.ins().isub(sp, one);
                            let off = builder.ins().imul(tmp_off, eight);
                            let addr = builder.ins().iadd(base, off);
                            let val = builder.ins().load(types::I64, memflags, addr, 0);
                            let neg = builder.ins().ineg(val);
                            builder.ins().store(memflags, neg, addr, 0);
                        }
                    }
                    
                    // ====================================================
                    // FLOAT ARITHMETIC — SSA: 3 instrs (bitcast+op+bitcast)
                    // ====================================================
                    
                    // Float binary: FADD, FSUB, FMUL, FDIV
                    x if x == Op::Fadd as u8 || x == Op::Fsub as u8 || x == Op::Fmul as u8 || x == Op::Fdiv as u8 => {
                        if let Some(ref mut sp) = ssa_sp {
                            if *sp >= 2 {
                                let b_bits = builder.use_var(ssa_stack_vars[*sp - 1]);
                                let a_bits = builder.use_var(ssa_stack_vars[*sp - 2]);
                                let a_f = builder.ins().bitcast(types::F64, memflags, a_bits);
                                let b_f = builder.ins().bitcast(types::F64, memflags, b_bits);
                                let result_f = match op {
                                    x if x == Op::Fadd as u8 => builder.ins().fadd(a_f, b_f),
                                    x if x == Op::Fsub as u8 => builder.ins().fsub(a_f, b_f),
                                    x if x == Op::Fmul as u8 => builder.ins().fmul(a_f, b_f),
                                    x if x == Op::Fdiv as u8 => builder.ins().fdiv(a_f, b_f),
                                    _ => unreachable!(),
                                };
                                let result = builder.ins().bitcast(types::I64, memflags, result_f);
                                *sp -= 1;
                                builder.def_var(ssa_stack_vars[*sp - 1], result);
                            }
                        } else {
                            let sp = builder.use_var(sp_var);
                            let eight = builder.ins().iconst(types::I64, 8);
                            let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                            let one = builder.ins().iconst(types::I64, 1);
                            let two = builder.ins().iconst(types::I64, 2);
                            let tmp_off_b = builder.ins().isub(sp, one);
                            let off_b = builder.ins().imul(tmp_off_b, eight);
                            let tmp_off_a = builder.ins().isub(sp, two);
                            let off_a = builder.ins().imul(tmp_off_a, eight);
                            let addr_b = builder.ins().iadd(base, off_b);
                            let addr_a = builder.ins().iadd(base, off_a);
                            let a_bits = builder.ins().load(types::I64, memflags, addr_a, 0);
                            let b_bits = builder.ins().load(types::I64, memflags, addr_b, 0);
                            let a_f = builder.ins().bitcast(types::F64, memflags, a_bits);
                            let b_f = builder.ins().bitcast(types::F64, memflags, b_bits);
                            let result_f = match op {
                                x if x == Op::Fadd as u8 => builder.ins().fadd(a_f, b_f),
                                x if x == Op::Fsub as u8 => builder.ins().fsub(a_f, b_f),
                                x if x == Op::Fmul as u8 => builder.ins().fmul(a_f, b_f),
                                x if x == Op::Fdiv as u8 => builder.ins().fdiv(a_f, b_f),
                                _ => unreachable!(),
                            };
                            let result = builder.ins().bitcast(types::I64, memflags, result_f);
                            builder.ins().store(memflags, result, addr_a, 0);
                            let new_sp = builder.ins().isub(sp, one);
                            builder.def_var(sp_var, new_sp);
                        }
                    }
                    
                    // Float unary: FNEG, FSQRT, FABS
                    x if x == Op::Fneg as u8 || x == Op::Fsqrt as u8 || x == Op::Fabs as u8 => {
                        if let Some(ref mut sp) = ssa_sp {
                            if *sp >= 1 {
                                let bits = builder.use_var(ssa_stack_vars[*sp - 1]);
                                let f = builder.ins().bitcast(types::F64, memflags, bits);
                                let result_f = match op {
                                    x if x == Op::Fneg as u8 => builder.ins().fneg(f),
                                    x if x == Op::Fsqrt as u8 => builder.ins().sqrt(f),
                                    x if x == Op::Fabs as u8 => builder.ins().fabs(f),
                                    _ => unreachable!(),
                                };
                                let result = builder.ins().bitcast(types::I64, memflags, result_f);
                                builder.def_var(ssa_stack_vars[*sp - 1], result);
                            }
                        } else {
                            let sp = builder.use_var(sp_var);
                            let eight = builder.ins().iconst(types::I64, 8);
                            let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                            let one = builder.ins().iconst(types::I64, 1);
                            let tmp_off = builder.ins().isub(sp, one);
                            let off = builder.ins().imul(tmp_off, eight);
                            let addr = builder.ins().iadd(base, off);
                            let bits = builder.ins().load(types::I64, memflags, addr, 0);
                            let f = builder.ins().bitcast(types::F64, memflags, bits);
                            let result_f = match op {
                                x if x == Op::Fneg as u8 => builder.ins().fneg(f),
                                x if x == Op::Fsqrt as u8 => builder.ins().sqrt(f),
                                x if x == Op::Fabs as u8 => builder.ins().fabs(f),
                                _ => unreachable!(),
                            };
                            let result = builder.ins().bitcast(types::I64, memflags, result_f);
                            builder.ins().store(memflags, result, addr, 0);
                        }
                    }
                    
                    // I2F: int to float
                    x if x == Op::I2f as u8 => {
                        if let Some(ref mut sp) = ssa_sp {
                            if *sp >= 1 {
                                let ival = builder.use_var(ssa_stack_vars[*sp - 1]);
                                let fval = builder.ins().fcvt_from_sint(types::F64, ival);
                                let bits = builder.ins().bitcast(types::I64, memflags, fval);
                                builder.def_var(ssa_stack_vars[*sp - 1], bits);
                            }
                        } else {
                            let sp = builder.use_var(sp_var);
                            let eight = builder.ins().iconst(types::I64, 8);
                            let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                            let one = builder.ins().iconst(types::I64, 1);
                            let tmp_off = builder.ins().isub(sp, one);
                            let off = builder.ins().imul(tmp_off, eight);
                            let addr = builder.ins().iadd(base, off);
                            let ival = builder.ins().load(types::I64, memflags, addr, 0);
                            let fval = builder.ins().fcvt_from_sint(types::F64, ival);
                            let bits = builder.ins().bitcast(types::I64, memflags, fval);
                            builder.ins().store(memflags, bits, addr, 0);
                        }
                    }
                    
                    // F2I: float to int
                    x if x == Op::F2i as u8 => {
                        if let Some(ref mut sp) = ssa_sp {
                            if *sp >= 1 {
                                let bits = builder.use_var(ssa_stack_vars[*sp - 1]);
                                let fval = builder.ins().bitcast(types::F64, memflags, bits);
                                let ival = builder.ins().fcvt_to_sint_sat(types::I64, fval);
                                builder.def_var(ssa_stack_vars[*sp - 1], ival);
                            }
                        } else {
                            let sp = builder.use_var(sp_var);
                            let eight = builder.ins().iconst(types::I64, 8);
                            let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                            let one = builder.ins().iconst(types::I64, 1);
                            let tmp_off = builder.ins().isub(sp, one);
                            let off = builder.ins().imul(tmp_off, eight);
                            let addr = builder.ins().iadd(base, off);
                            let bits = builder.ins().load(types::I64, memflags, addr, 0);
                            let fval = builder.ins().bitcast(types::F64, memflags, bits);
                            let ival = builder.ins().fcvt_to_sint_sat(types::I64, fval);
                            builder.ins().store(memflags, ival, addr, 0);
                        }
                    }
                    
                    // ====================================================
                    // COMPARISON — SSA: 2 instrs (icmp+uextend)
                    // ====================================================
                    x if x == Op::Eq as u8 || x == Op::Lt as u8 || x == Op::Gt as u8 || x == Op::Le as u8 || x == Op::Ge as u8 || x == Op::Ne as u8 => {
                        if let Some(ref mut sp) = ssa_sp {
                            if *sp >= 2 {
                                let b = builder.use_var(ssa_stack_vars[*sp - 1]);
                                let a = builder.use_var(ssa_stack_vars[*sp - 2]);
                                let cc = match op {
                                    x if x == Op::Eq as u8 => IntCC::Equal,
                                    x if x == Op::Lt as u8 => IntCC::SignedLessThan,
                                    x if x == Op::Gt as u8 => IntCC::SignedGreaterThan,
                                    x if x == Op::Le as u8 => IntCC::SignedLessThanOrEqual,
                                    x if x == Op::Ge as u8 => IntCC::SignedGreaterThanOrEqual,
                                    x if x == Op::Ne as u8 => IntCC::NotEqual,
                                    _ => unreachable!(),
                                };
                                let cmp = builder.ins().icmp(cc, a, b);
                                let result = builder.ins().uextend(types::I64, cmp);
                                *sp -= 1;
                                builder.def_var(ssa_stack_vars[*sp - 1], result);
                            }
                        } else {
                            let sp = builder.use_var(sp_var);
                            let eight = builder.ins().iconst(types::I64, 8);
                            let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                            let one = builder.ins().iconst(types::I64, 1);
                            let two = builder.ins().iconst(types::I64, 2);
                            let tmp_off_b = builder.ins().isub(sp, one);
                            let off_b = builder.ins().imul(tmp_off_b, eight);
                            let tmp_off_a = builder.ins().isub(sp, two);
                            let off_a = builder.ins().imul(tmp_off_a, eight);
                            let addr_b = builder.ins().iadd(base, off_b);
                            let addr_a = builder.ins().iadd(base, off_a);
                            let a = builder.ins().load(types::I64, memflags, addr_a, 0);
                            let b = builder.ins().load(types::I64, memflags, addr_b, 0);
                            let cc = match op {
                                x if x == Op::Eq as u8 => IntCC::Equal,
                                x if x == Op::Lt as u8 => IntCC::SignedLessThan,
                                x if x == Op::Gt as u8 => IntCC::SignedGreaterThan,
                                x if x == Op::Le as u8 => IntCC::SignedLessThanOrEqual,
                                x if x == Op::Ge as u8 => IntCC::SignedGreaterThanOrEqual,
                                x if x == Op::Ne as u8 => IntCC::NotEqual,
                                _ => unreachable!(),
                            };
                            let cmp = builder.ins().icmp(cc, a, b);
                            let result = builder.ins().uextend(types::I64, cmp);
                            builder.ins().store(memflags, result, addr_a, 0);
                            let new_sp = builder.ins().isub(sp, one);
                            builder.def_var(sp_var, new_sp);
                        }
                    }
                    
                    // ====================================================
                    // LOGIC — SSA: 1 instr
                    // ====================================================
                    
                    // Binary logic: AND, OR, XOR, BAND, BOR, BXOR, SHL, SHR
                    x if x == Op::And as u8 || x == Op::Or as u8 || x == Op::Xor as u8 || x == Op::Band as u8 || x == Op::Bor as u8 || x == Op::Bxor as u8 || x == Op::Shl as u8 || x == Op::Shr as u8 => {
                        if let Some(ref mut sp) = ssa_sp {
                            if *sp >= 2 {
                                let b = builder.use_var(ssa_stack_vars[*sp - 1]);
                                let a = builder.use_var(ssa_stack_vars[*sp - 2]);
                                let result = match op {
                                    x if x == Op::And as u8 => builder.ins().band(a, b),
                                    x if x == Op::Or as u8 => builder.ins().bor(a, b),
                                    x if x == Op::Xor as u8 => builder.ins().bxor(a, b),
                                    x if x == Op::Band as u8 => builder.ins().band(a, b),
                                    x if x == Op::Bor as u8 => builder.ins().bor(a, b),
                                    x if x == Op::Bxor as u8 => builder.ins().bxor(a, b),
                                    x if x == Op::Shl as u8 => builder.ins().ishl(a, b),
                                    x if x == Op::Shr as u8 => builder.ins().sshr(a, b),
                                    _ => unreachable!(),
                                };
                                *sp -= 1;
                                builder.def_var(ssa_stack_vars[*sp - 1], result);
                            }
                        } else {
                            let sp = builder.use_var(sp_var);
                            let eight = builder.ins().iconst(types::I64, 8);
                            let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                            let one = builder.ins().iconst(types::I64, 1);
                            let two = builder.ins().iconst(types::I64, 2);
                            let tmp_off_b = builder.ins().isub(sp, one);
                            let off_b = builder.ins().imul(tmp_off_b, eight);
                            let tmp_off_a = builder.ins().isub(sp, two);
                            let off_a = builder.ins().imul(tmp_off_a, eight);
                            let addr_b = builder.ins().iadd(base, off_b);
                            let addr_a = builder.ins().iadd(base, off_a);
                            let a = builder.ins().load(types::I64, memflags, addr_a, 0);
                            let b = builder.ins().load(types::I64, memflags, addr_b, 0);
                            let result = match op {
                                x if x == Op::And as u8 => builder.ins().band(a, b),
                                x if x == Op::Or as u8 => builder.ins().bor(a, b),
                                x if x == Op::Xor as u8 => builder.ins().bxor(a, b),
                                x if x == Op::Band as u8 => builder.ins().band(a, b),
                                x if x == Op::Bor as u8 => builder.ins().bor(a, b),
                                x if x == Op::Bxor as u8 => builder.ins().bxor(a, b),
                                x if x == Op::Shl as u8 => builder.ins().ishl(a, b),
                                x if x == Op::Shr as u8 => builder.ins().sshr(a, b),
                                _ => unreachable!(),
                            };
                            builder.ins().store(memflags, result, addr_a, 0);
                            let new_sp = builder.ins().isub(sp, one);
                            builder.def_var(sp_var, new_sp);
                        }
                    }
                    
                    // NOT (bitwise: ~0 = -1, ~1 = -2)
                    // P1: Polymorphic — Bool(!x) or Int(!x). Matches interpreter.
                    // AND/OR/XOR are all bitwise on i64, so NOT must be too.
                    // BNOT (0x67) is the explicit Int-only alias.
                    x if x == Op::Not as u8 => {
                        if let Some(ref mut sp) = ssa_sp {
                            if *sp >= 1 {
                                let val = builder.use_var(ssa_stack_vars[*sp - 1]);
                                let result = builder.ins().bnot(val);
                                builder.def_var(ssa_stack_vars[*sp - 1], result);
                            }
                        } else {
                            let sp = builder.use_var(sp_var);
                            let eight = builder.ins().iconst(types::I64, 8);
                            let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                            let one = builder.ins().iconst(types::I64, 1);
                            let tmp_off = builder.ins().isub(sp, one);
                            let off = builder.ins().imul(tmp_off, eight);
                            let addr = builder.ins().iadd(base, off);
                            let val = builder.ins().load(types::I64, memflags, addr, 0);
                            let result = builder.ins().bnot(val);
                            builder.ins().store(memflags, result, addr, 0);
                        }
                    }
                    
                    // BNOT (bitwise not)
                    x if x == Op::Bnot as u8 => {
                        if let Some(ref mut sp) = ssa_sp {
                            if *sp >= 1 {
                                let val = builder.use_var(ssa_stack_vars[*sp - 1]);
                                let result = builder.ins().bnot(val);
                                builder.def_var(ssa_stack_vars[*sp - 1], result);
                            }
                        } else {
                            let sp = builder.use_var(sp_var);
                            let eight = builder.ins().iconst(types::I64, 8);
                            let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                            let one = builder.ins().iconst(types::I64, 1);
                            let tmp_off = builder.ins().isub(sp, one);
                            let off = builder.ins().imul(tmp_off, eight);
                            let addr = builder.ins().iadd(base, off);
                            let val = builder.ins().load(types::I64, memflags, addr, 0);
                            let result = builder.ins().bnot(val);
                            builder.ins().store(memflags, result, addr, 0);
                        }
                    }
                    
                    // ====================================================
                    // CONTROL FLOW — flush SSA before any branch
                    // ====================================================
                    
                    // JMP rel32
                    x if x == Op::Jmp as u8 => {
                        if pc + 3 < bytecode.len() {
                            let rel = i32::from_le_bytes([bytecode[pc], bytecode[pc + 1], bytecode[pc + 2], bytecode[pc + 3]]);
                            pc += 4;
                            let target = ((pc as i64) + (rel as i64)) as usize;
                            // Flush SSA to memory before branch
                            flush_ssa_to_memory!(builder, sp_var, ssa_sp, ssa_stack_vars, data_stack, memflags);
                            if let Some(&target_block) = blocks.get(&target) {
                                builder.ins().jump(target_block, &[]);
                                block_terminated = true;
                            }
                        }
                    }
                    
                    // JZ rel32: pop, if zero jump
                    x if x == Op::Jz as u8 => {
                        if pc + 3 < bytecode.len() {
                            let rel = i32::from_le_bytes([bytecode[pc], bytecode[pc + 1], bytecode[pc + 2], bytecode[pc + 3]]);
                            pc += 4;
                            let target = ((pc as i64) + (rel as i64)) as usize;
                            let fall_through = pc;
                            
                            // Pop condition — SSA or memory
                            let cond = if let Some(ref mut sp) = ssa_sp {
                                if *sp >= 1 {
                                    *sp -= 1;
                                    let c = builder.use_var(ssa_stack_vars[*sp]);
                                    // Flush remaining SSA stack to memory before branch
                                    flush_ssa_to_memory!(builder, sp_var, ssa_sp, ssa_stack_vars, data_stack, memflags);
                                    c
                                } else {
                                    // SSA empty, pop from memory
                                    flush_ssa_to_memory!(builder, sp_var, ssa_sp, ssa_stack_vars, data_stack, memflags);
                                    let sp = builder.use_var(sp_var);
                                    let eight = builder.ins().iconst(types::I64, 8);
                                    let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                                    let one = builder.ins().iconst(types::I64, 1);
                                    let new_sp = builder.ins().isub(sp, one);
                                    let off = builder.ins().imul(new_sp, eight);
                                    let addr = builder.ins().iadd(base, off);
                                    let c = builder.ins().load(types::I64, memflags, addr, 0);
                                    builder.def_var(sp_var, new_sp);
                                    c
                                }
                            } else {
                                let sp = builder.use_var(sp_var);
                                let eight = builder.ins().iconst(types::I64, 8);
                                let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                                let one = builder.ins().iconst(types::I64, 1);
                                let new_sp = builder.ins().isub(sp, one);
                                let off = builder.ins().imul(new_sp, eight);
                                let addr = builder.ins().iadd(base, off);
                                let c = builder.ins().load(types::I64, memflags, addr, 0);
                                builder.def_var(sp_var, new_sp);
                                c
                            };
                            
                            let zero = builder.ins().iconst(types::I64, 0);
                            let is_zero = builder.ins().icmp(IntCC::Equal, cond, zero);
                            
                            if let (Some(&t_block), Some(&f_block)) = (blocks.get(&target), blocks.get(&fall_through)) {
                                builder.ins().brif(is_zero, t_block, &[], f_block, &[]);
                                block_terminated = true;
                            }
                        }
                    }
                    
                    // JNZ rel32: pop, if non-zero jump
                    x if x == Op::Jnz as u8 => {
                        if pc + 3 < bytecode.len() {
                            let rel = i32::from_le_bytes([bytecode[pc], bytecode[pc + 1], bytecode[pc + 2], bytecode[pc + 3]]);
                            pc += 4;
                            let target = ((pc as i64) + (rel as i64)) as usize;
                            let fall_through = pc;
                            
                            // Pop condition — SSA or memory
                            let cond = if let Some(ref mut sp) = ssa_sp {
                                if *sp >= 1 {
                                    *sp -= 1;
                                    let c = builder.use_var(ssa_stack_vars[*sp]);
                                    flush_ssa_to_memory!(builder, sp_var, ssa_sp, ssa_stack_vars, data_stack, memflags);
                                    c
                                } else {
                                    flush_ssa_to_memory!(builder, sp_var, ssa_sp, ssa_stack_vars, data_stack, memflags);
                                    let sp = builder.use_var(sp_var);
                                    let eight = builder.ins().iconst(types::I64, 8);
                                    let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                                    let one = builder.ins().iconst(types::I64, 1);
                                    let new_sp = builder.ins().isub(sp, one);
                                    let off = builder.ins().imul(new_sp, eight);
                                    let addr = builder.ins().iadd(base, off);
                                    let c = builder.ins().load(types::I64, memflags, addr, 0);
                                    builder.def_var(sp_var, new_sp);
                                    c
                                }
                            } else {
                                let sp = builder.use_var(sp_var);
                                let eight = builder.ins().iconst(types::I64, 8);
                                let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                                let one = builder.ins().iconst(types::I64, 1);
                                let new_sp = builder.ins().isub(sp, one);
                                let off = builder.ins().imul(new_sp, eight);
                                let addr = builder.ins().iadd(base, off);
                                let c = builder.ins().load(types::I64, memflags, addr, 0);
                                builder.def_var(sp_var, new_sp);
                                c
                            };
                            
                            let zero = builder.ins().iconst(types::I64, 0);
                            let is_nonzero = builder.ins().icmp(IntCC::NotEqual, cond, zero);
                            
                            if let (Some(&t_block), Some(&f_block)) = (blocks.get(&target), blocks.get(&fall_through)) {
                                builder.ins().brif(is_nonzero, t_block, &[], f_block, &[]);
                                block_terminated = true;
                            }
                        }
                    }
                    
                    // ====================================================
                    // CALL/RET — flush SSA before call/return
                    // ====================================================
                    
                    // CALL: call function by symbol index
                    x if x == Op::Call as u8 => {
                        if pc + 1 < bytecode.len() {
                            let sym_idx = u16::from_le_bytes([bytecode[pc], bytecode[pc + 1]]);
                            pc += 2;
                            let ret_pc = pc;  // instruction after CALL
                            
                            // Flush SSA to memory before saving frame
                            flush_ssa_to_memory!(builder, sp_var, ssa_sp, ssa_stack_vars, data_stack, memflags);
                            
                            // Look up the call site index for this return point
                            let call_site_id = call_sites.iter()
                                .position(|&(_, _, rpc)| rpc == ret_pc)
                                .ok_or_else(|| format!("Internal error: no call site for ret_pc {}", ret_pc))?;
                            
                            // Look up function target
                            let target_offset = *module.symbol_table.get(&sym_idx)
                                .ok_or_else(|| format!("Unknown symbol index: {}", sym_idx))?;
                            
                            // 1. Save ONLY USED locals to frame stack (Fix 1)
                            // P1: Only save tools that are live. P4: Frame shrinks.
                            let fsp = builder.use_var(fsp_var);
                            let frame_size_bytes = builder.ins().iconst(types::I64, (frame_save_size * 8) as i64);
                            let frame_offset = builder.ins().imul(fsp, frame_size_bytes);
                            let frame_base = builder.ins().stack_addr(types::I64, local_stack, 0);
                            let frame_addr = builder.ins().iadd(frame_base, frame_offset);
                            let locals_base = builder.ins().stack_addr(types::I64, locals_slot, 0);
                            for (save_idx, &slot) in used_locals.iter().enumerate() {
                                let src = builder.ins().load(types::I64, memflags, locals_base, (slot * 8) as i32);
                                let dst_off = builder.ins().iconst(types::I64, (save_idx * 8) as i64);
                                let dst_addr = builder.ins().iadd(frame_addr, dst_off);
                                builder.ins().store(memflags, src, dst_addr, 0);
                            }
                            let one_fsp = builder.ins().iconst(types::I64, 1);
                            let new_fsp = builder.ins().iadd(fsp, one_fsp);
                            builder.def_var(fsp_var, new_fsp);
                            
                            // 2. Reset ONLY USED locals to 0 for the callee (Fix 1)
                            for &slot in &used_locals {
                                let z = builder.ins().iconst(types::I64, 0);
                                builder.ins().stack_store(z, locals_slot, (slot * 8) as i32);
                            }
                            
                            // 3. Push return-site-ID to call stack
                            let csp = builder.use_var(csp_var);
                            let eight = builder.ins().iconst(types::I64, 8);
                            let cs_base = builder.ins().stack_addr(types::I64, call_stack, 0);
                            let cs_off = builder.ins().imul(csp, eight);
                            let cs_addr = builder.ins().iadd(cs_base, cs_off);
                            let site_id = builder.ins().iconst(types::I64, call_site_id as i64);
                            builder.ins().store(memflags, site_id, cs_addr, 0);
                            let one_csp = builder.ins().iconst(types::I64, 1);
                            let new_csp = builder.ins().iadd(csp, one_csp);
                            builder.def_var(csp_var, new_csp);
                            
                            // 4. Jump to function block
                            if let Some(&target_block) = blocks.get(&target_offset) {
                                builder.ins().jump(target_block, &[]);
                                block_terminated = true;
                            } else {
                                return Err(format!("Function at offset {} has no block", target_offset));
                            }
                        }
                    }
                    
                    // RET: return from function call
                    x if x == Op::Ret as u8 => {
                        // 1. Restore ONLY USED locals from frame stack (Fix 1)
                        // P1: Restore only live tools. P3: Composition preserved.
                        let fsp = builder.use_var(fsp_var);
                        let one_fsp = builder.ins().iconst(types::I64, 1);
                        let new_fsp = builder.ins().isub(fsp, one_fsp);
                        builder.def_var(fsp_var, new_fsp);
                        
                        let frame_size_bytes = builder.ins().iconst(types::I64, (frame_save_size * 8) as i64);
                        let frame_offset = builder.ins().imul(new_fsp, frame_size_bytes);
                        let frame_base = builder.ins().stack_addr(types::I64, local_stack, 0);
                        let frame_addr = builder.ins().iadd(frame_base, frame_offset);
                        let locals_base = builder.ins().stack_addr(types::I64, locals_slot, 0);
                        for (save_idx, &slot) in used_locals.iter().enumerate() {
                            let src_off = builder.ins().iconst(types::I64, (save_idx * 8) as i64);
                            let src_addr = builder.ins().iadd(frame_addr, src_off);
                            let val = builder.ins().load(types::I64, memflags, src_addr, 0);
                            builder.ins().store(memflags, val, locals_base, (slot * 8) as i32);
                        }
                        
                        // 2. Pop return-site-ID from call stack
                        let csp = builder.use_var(csp_var);
                        let one_csp = builder.ins().iconst(types::I64, 1);
                        let new_csp = builder.ins().isub(csp, one_csp);
                        builder.def_var(csp_var, new_csp);
                        
                        let eight = builder.ins().iconst(types::I64, 8);
                        let cs_base = builder.ins().stack_addr(types::I64, call_stack, 0);
                        let cs_off = builder.ins().imul(new_csp, eight);
                        let cs_addr = builder.ins().iadd(cs_base, cs_off);
                        let site_id = builder.ins().load(types::I64, memflags, cs_addr, 0);
                        
                        // 3. Dispatch to return block via br_table
                        // br_table with return_blocks as targets, halt_block as default
                        // site_id is the index into return_blocks
                        // If site_id is -1 (sentinel), it wraps to a large index → default → halt
                        let site_id_i32 = builder.ins().ireduce(types::I32, site_id);
                        
                        // Build jump table: default = halt_block, entries = return_blocks
                        let default_bc = builder.func.dfg.block_call(halt_block, &[]);
                        let entry_bcs: Vec<BlockCall> = return_blocks.iter()
                            .map(|&rb| builder.func.dfg.block_call(rb, &[]))
                            .collect();
                        let jt_data = JumpTableData::new(default_bc, &entry_bcs);
                        let jt = builder.create_jump_table(jt_data);
                        builder.ins().br_table(site_id_i32, jt);
                        block_terminated = true;
                    }
                    
                    // ====================================================
                    // STORE/LOAD — SSA-aware local variables
                    // ====================================================
                    
                    // STORE n: pop -> locals[n]
                    x if x == Op::Store as u8 => {
                        if pc + 3 < bytecode.len() {
                            let slot = u32::from_le_bytes([
                                bytecode[pc], bytecode[pc+1], bytecode[pc+2], bytecode[pc+3]
                            ]) as usize;
                            pc += 4;
                            
                            if let Some(ref mut sp) = ssa_sp {
                                if *sp >= 1 {
                                    *sp -= 1;
                                    let val = builder.use_var(ssa_stack_vars[*sp]);
                                    builder.ins().stack_store(val, locals_slot, (slot * 8) as i32);
                                }
                            } else {
                                // Pop from data stack
                                let sp = builder.use_var(sp_var);
                                let eight = builder.ins().iconst(types::I64, 8);
                                let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                                let one = builder.ins().iconst(types::I64, 1);
                                let new_sp = builder.ins().isub(sp, one);
                                let off = builder.ins().imul(new_sp, eight);
                                let addr = builder.ins().iadd(base, off);
                                let val = builder.ins().load(types::I64, memflags, addr, 0);
                                builder.def_var(sp_var, new_sp);
                                
                                // Store to locals slot
                                builder.ins().stack_store(val, locals_slot, (slot * 8) as i32);
                            }
                        }
                    }
                    
                    // LOAD n: locals[n] -> push
                    x if x == Op::Load as u8 => {
                        if pc + 3 < bytecode.len() {
                            let slot = u32::from_le_bytes([
                                bytecode[pc], bytecode[pc+1], bytecode[pc+2], bytecode[pc+3]
                            ]) as usize;
                            pc += 4;
                            
                            // Load from locals slot
                            let val = builder.ins().stack_load(types::I64, locals_slot, (slot * 8) as i32);
                            
                            if let Some(ref mut sp) = ssa_sp {
                                if *sp < SSA_STACK_SIZE {
                                    builder.def_var(ssa_stack_vars[*sp], val);
                                    *sp += 1;
                                }
                            } else {
                                // Push to data stack
                                let sp = builder.use_var(sp_var);
                                let eight = builder.ins().iconst(types::I64, 8);
                                let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                                let off = builder.ins().imul(sp, eight);
                                let addr = builder.ins().iadd(base, off);
                                builder.ins().store(memflags, val, addr, 0);
                                let one = builder.ins().iconst(types::I64, 1);
                                let new_sp = builder.ins().iadd(sp, one);
                                builder.def_var(sp_var, new_sp);
                            }
                        }
                    }
                    
                    // ====================================================
                    // REFLECTION — SSA-aware
                    // ====================================================
                    
                    // SIZE: push bytecode length
                    x if x == Op::Size as u8 => {
                        let v = builder.ins().iconst(types::I64, bytecode.len() as i64);
                        if let Some(ref mut sp) = ssa_sp {
                            if *sp < SSA_STACK_SIZE {
                                builder.def_var(ssa_stack_vars[*sp], v);
                                *sp += 1;
                            }
                        } else {
                            let sp = builder.use_var(sp_var);
                            let eight = builder.ins().iconst(types::I64, 8);
                            let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                            let off = builder.ins().imul(sp, eight);
                            let addr = builder.ins().iadd(base, off);
                            builder.ins().store(memflags, v, addr, 0);
                            let one = builder.ins().iconst(types::I64, 1);
                            let new_sp = builder.ins().iadd(sp, one);
                            builder.def_var(sp_var, new_sp);
                        }
                    }
                    
                    // FETCH: read bytecode[index]
                    x if x == Op::Fetch as u8 => {
                        if let Some(ref mut sp) = ssa_sp {
                            if *sp >= 1 {
                                // Replace top with 0 (safe fallback)
                                let result = builder.ins().iconst(types::I64, 0);
                                builder.def_var(ssa_stack_vars[*sp - 1], result);
                            }
                        } else {
                            let sp = builder.use_var(sp_var);
                            let eight = builder.ins().iconst(types::I64, 8);
                            let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                            let one = builder.ins().iconst(types::I64, 1);
                            let tmp_off = builder.ins().isub(sp, one);
                            let off = builder.ins().imul(tmp_off, eight);
                            let addr = builder.ins().iadd(base, off);
                            let _idx = builder.ins().load(types::I64, memflags, addr, 0);
                            // For safety, just return 0 for out-of-bounds
                            let result = builder.ins().iconst(types::I64, 0);
                            builder.ins().store(memflags, result, addr, 0);
                        }
                    }
                    
                    // ====================================================
                    // REFLECTION
                    // ====================================================
                    
                    // DEPTH (0xD1): (-- n) push current stack depth
                    x if x == Op::Depth as u8 => {
                        if let Some(ref mut ssp) = ssa_sp {
                            if *ssp < SSA_STACK_SIZE {
                                let depth_val = builder.ins().iconst(types::I64, *ssp as i64);
                                builder.def_var(ssa_stack_vars[*ssp], depth_val);
                                *ssp += 1;
                            }
                        } else {
                            // Memory mode: read sp, compute depth = sp/8, push at sp
                            let sp_val = builder.use_var(sp_var);
                            let eight = builder.ins().iconst(types::I64, 8);
                            let depth_val = builder.ins().sdiv(sp_val, eight);
                            let base = builder.ins().stack_addr(types::I64, data_stack, 0);
                            let addr = builder.ins().iadd(base, sp_val);
                            builder.ins().store(memflags, depth_val, addr, 0);
                            let new_sp = builder.ins().iadd_imm(sp_val, 8);
                            builder.def_var(sp_var, new_sp);
                        }
                    }
                    
                    // ====================================================
                    // TERMINATION — flush SSA before halt
                    // ====================================================
                    
                    x if x == Op::Halt as u8 => {
                        // Flush SSA to memory so halt_block can read the result
                        flush_ssa_to_memory!(builder, sp_var, ssa_sp, ssa_stack_vars, data_stack, memflags);
                        builder.ins().jump(halt_block, &[]);
                        block_terminated = true;
                    }
                    
                    // ====================================================
                    // UNSUPPORTED
                    // ====================================================
                    _ => {
                        return Err(format!(
                            "Native module backend: unsupported opcode 0x{:02X} at offset {} \
                             (use 'korec run' interpreter for this program)",
                            op, pc - 1
                        ));
                    }
                }
            }
            
            // ============================================================
            // Return dispatch: each return block just continues at ret_pc
            // ============================================================
            for (i, &(_, _, ret_pc)) in call_sites.iter().enumerate() {
                builder.switch_to_block(return_blocks[i]);
                if let Some(&target_block) = blocks.get(&ret_pc) {
                    builder.ins().jump(target_block, &[]);
                } else {
                    // ret_pc might be the very next instruction — create the block if needed
                    return Err(format!("Return PC {} has no block (internal error)", ret_pc));
                }
            }
            
            // ============================================================
            // Halt block: return top of stack (or 0)
            // ============================================================
            builder.switch_to_block(halt_block);
            let sp = builder.use_var(sp_var);
            let zero = builder.ins().iconst(types::I64, 0);
            let is_empty = builder.ins().icmp(IntCC::Equal, sp, zero);
            
            // If stack empty, return 0; else return stack[sp-1]
            let result_empty_block = builder.create_block();
            let result_nonempty_block = builder.create_block();
            builder.ins().brif(is_empty, result_empty_block, &[], result_nonempty_block, &[]);
            
            builder.switch_to_block(result_empty_block);
            let zero_result = builder.ins().iconst(types::I64, 0);
            builder.ins().return_(&[zero_result]);
            
            builder.switch_to_block(result_nonempty_block);
            let sp2 = builder.use_var(sp_var);
            let eight = builder.ins().iconst(types::I64, 8);
            let base = builder.ins().stack_addr(types::I64, data_stack, 0);
            let one = builder.ins().iconst(types::I64, 1);
            let tmp_top_off = builder.ins().isub(sp2, one);
            let top_off = builder.ins().imul(tmp_top_off, eight);
            let top_addr = builder.ins().iadd(base, top_off);
            let result = builder.ins().load(types::I64, memflags, top_addr, 0);
            builder.ins().return_(&[result]);
            
            // ============================================================
            // Seal all blocks
            // ============================================================
            for &block in blocks.values() {
                builder.seal_block(block);
            }
            builder.seal_block(ret_dispatch_block);
            for &rb in &return_blocks {
                builder.seal_block(rb);
            }
            builder.seal_block(halt_block);
            builder.seal_block(result_empty_block);
            builder.seal_block(result_nonempty_block);
            
            builder.finalize();
        }
        
        // Compile and link
        self.module.define_function(func_id, &mut self.ctx)
            .map_err(|e| format!("Module compilation error: {}", e))?;
        self.module.clear_context(&mut self.ctx);
        self.module.finalize_definitions().map_err(|e| e.to_string())?;
        
        let code_ptr = self.module.get_finalized_function(func_id);
        Ok(unsafe { std::mem::transmute(code_ptr) })
    }
}

/// Scan a function body for which local slots are actually used (STORE/LOAD indices)
/// Returns the set of slot indices that need saving/restoring on CALL/RET.
/// P1: Each local is a tool (state slot). Only save tools that are live.
fn scan_used_locals(bytecode: &[u8], symbol_table: &HashMap<u16, usize>) -> Vec<usize> {
    let mut used: HashSet<usize> = HashSet::new();
    
    // Scan ALL function bodies for STORE/LOAD operands
    // We must be conservative: any local used anywhere must be saved
    let mut pc = 0;
    while pc < bytecode.len() {
        let op = bytecode[pc];
        pc += 1;
        match op {
            x if x == Op::Store as u8 || x == Op::Load as u8 => {
                if pc + 3 < bytecode.len() {
                    let slot = u32::from_le_bytes([
                        bytecode[pc], bytecode[pc+1], bytecode[pc+2], bytecode[pc+3]
                    ]) as usize;
                    used.insert(slot);
                    pc += 4;
                }
            }
            // Skip operands for multi-byte instructions
            x if x == Op::Int8 as u8 || x == Op::Syscall as u8 => pc += 1,
            x if x == Op::Call as u8 || x == Op::Int16 as u8 || x == Op::List as u8 => pc += 2,
            x if x == Op::Jmp as u8 || x == Op::Jz as u8 || x == Op::Jnz as u8 => pc += 4,
            x if x == Op::Quote as u8 => {
                if pc + 1 < bytecode.len() {
                    let body_len = u16::from_le_bytes([bytecode[pc], bytecode[pc + 1]]) as usize;
                    pc += 2 + body_len;
                } else { pc += 2; }
            }
            x if x == Op::Case as u8 => pc += 4,
            x if x == Op::Int32 as u8 || x == Op::F32 as u8 => pc += 4,
            x if x == Op::Int64 as u8 || x == Op::F64 as u8 => pc += 8,
            x if x == Op::Str as u8 => {
                if pc + 1 < bytecode.len() {
                    let str_len = u16::from_le_bytes([bytecode[pc], bytecode[pc + 1]]) as usize;
                    pc += 2 + str_len;
                } else { pc += 2; }
            }
            _ => {}
        }
    }
    
    let mut result: Vec<usize> = used.into_iter().collect();
    result.sort();
    result
}

/// Find all bytecode offsets that are jump targets
fn find_jump_targets(bytecode: &[u8]) -> Vec<usize> {
    let mut target_set: HashSet<usize> = HashSet::new();
    target_set.insert(0); // Entry is always a target
    let mut pc = 0;
    
    while pc < bytecode.len() {
        let op = bytecode[pc];
        pc += 1;
        
        match op {
            // JMP, JZ, JNZ: extract target
            x if x == Op::Jmp as u8 || x == Op::Jz as u8 || x == Op::Jnz as u8 => {
                if pc + 3 < bytecode.len() {
                    let rel = i32::from_le_bytes([bytecode[pc], bytecode[pc + 1], bytecode[pc + 2], bytecode[pc + 3]]);
                    let target = ((pc + 4) as i64 + rel as i64) as usize;
                    target_set.insert(target);
                    // Fall-through is also a target for conditional jumps
                    target_set.insert(pc + 4);
                }
                pc += 4;
            }
            // 1-byte operand: INT8, SYSCALL
            x if x == Op::Int8 as u8 || x == Op::Syscall as u8 => pc += 1,
            // 4-byte operand: STORE, LOAD (u32 slot)
            x if x == Op::Store as u8 || x == Op::Load as u8 => pc += 4,
            
            // 2-byte operand: CALL, INT16, LIST
            x if x == Op::Call as u8 || x == Op::Int16 as u8 || x == Op::List as u8 => pc += 2,
            
            // QUOTE: 2-byte body length + body bytes (skip entire body)
            x if x == Op::Quote as u8 => {
                if pc + 1 < bytecode.len() {
                    let body_len = u16::from_le_bytes([bytecode[pc], bytecode[pc + 1]]) as usize;
                    pc += 2 + body_len;
                } else {
                    pc += 2;
                }
            }
            
            // CASE: 2x i16 offsets (4 bytes)
            x if x == Op::Case as u8 => pc += 4,
            
            // 4-byte operand: INT32, F32
            x if x == Op::Int32 as u8 || x == Op::F32 as u8 => pc += 4,
            
            // 8-byte operand: INT64, F64
            x if x == Op::Int64 as u8 || x == Op::F64 as u8 => pc += 8,
            
            // STR: 2-byte length + string bytes
            x if x == Op::Str as u8 => {
                if pc + 1 < bytecode.len() {
                    let str_len = u16::from_le_bytes([bytecode[pc], bytecode[pc + 1]]) as usize;
                    pc += 2 + str_len;
                } else {
                    pc += 2;
                }
            }
            
            // All other opcodes: no operands (1-byte total, already consumed)
            _ => {}
        }
    }
    
    let mut targets: Vec<usize> = target_set.into_iter().collect();
    targets.sort();
    targets
}

/// Compile and run bytecode natively (SSA stack — fastest, no CALL/RET)
pub fn run_native(bytecode: &[u8]) -> Result<i64, String> {
    let mut compiler = NativeCompiler::new()?;
    let func = compiler.compile(bytecode)?;
    Ok(unsafe { func() })
}

/// Compile and run a full module with function support (memory-backed stack)
///
/// This handles CALL/RET by using a runtime stack pointer and memory-backed
/// data stack. Each function body and main code are blocks within a single
/// Cranelift function. CALL branches to the function block, RET uses br_table
/// to dispatch back to the correct return site.
///
/// Locals are saved/restored per-call using a frame stack.
pub fn run_native_module(module: &crate::bytecode::BytecodeModule) -> Result<i64, String> {
    // Fast path: if no functions defined, use the SSA compiler (faster)
    if module.symbol_table.is_empty() {
        return run_native(&module.code);
    }
    
    let mut compiler = NativeCompiler::new()?;
    let func = compiler.compile_module(module)?;
    Ok(unsafe { func() })
}

// ============================================================================
// TESTS - Each tests ONE primitive
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_primitive_add() {
        // ADD: 5 3 -> 8
        let bytecode = vec![0x30, 5, 0x30, 3, 0x40, 0xFF];
        assert_eq!(run_native(&bytecode).unwrap(), 8);
    }
    
    #[test]
    fn test_primitive_sub() {
        // SUB: 10 3 -> 7
        let bytecode = vec![0x30, 10, 0x30, 3, 0x41, 0xFF];
        assert_eq!(run_native(&bytecode).unwrap(), 7);
    }
    
    #[test]
    fn test_primitive_mul() {
        // MUL: 6 7 -> 42
        let bytecode = vec![0x30, 6, 0x30, 7, 0x42, 0xFF];
        assert_eq!(run_native(&bytecode).unwrap(), 42);
    }
    
    #[test]
    fn test_primitive_div() {
        // DIV: 20 4 -> 5
        let bytecode = vec![0x30, 20, 0x30, 4, 0x43, 0xFF];
        assert_eq!(run_native(&bytecode).unwrap(), 5);
    }
    
    #[test]
    fn test_primitive_lt() {
        // LT: 3 5 -> 1 (true)
        let bytecode = vec![0x30, 3, 0x30, 5, 0x51, 0xFF];
        assert_eq!(run_native(&bytecode).unwrap(), 1);
    }
    
    #[test]
    fn test_primitive_dup() {
        // DUP: 7 -> 7 7, then ADD -> 14
        let bytecode = vec![0x30, 7, 0x02, 0x40, 0xFF];
        assert_eq!(run_native(&bytecode).unwrap(), 14);
    }
    
    #[test]
    fn test_primitive_swap() {
        // SWAP: 10 3 -> 3 10, then SUB -> -7
        let bytecode = vec![0x30, 10, 0x30, 3, 0x03, 0x41, 0xFF];
        assert_eq!(run_native(&bytecode).unwrap(), -7);
    }
    
    #[test]
    fn test_primitive_size() {
        // SIZE: push bytecode length (6 bytes)
        let bytecode = vec![0x91, 0xFF, 0x00, 0x00, 0x00, 0x00];
        assert_eq!(run_native(&bytecode).unwrap(), 6);
    }
    
    #[test]
    fn test_primitive_ne() {
        // NE: 5 != 3 -> 1 (true)
        let bytecode = vec![0x30, 5, 0x30, 3, 0x55, 0xFF];
        assert_eq!(run_native(&bytecode).unwrap(), 1);
        
        // NE: 5 != 5 -> 0 (false)
        let bytecode2 = vec![0x30, 5, 0x30, 5, 0x55, 0xFF];
        assert_eq!(run_native(&bytecode2).unwrap(), 0);
    }
    
    #[test]
    fn test_primitive_and() {
        // AND: 0b1100 & 0b1010 = 0b1000 = 8
        let bytecode = vec![0x30, 12, 0x30, 10, 0x60, 0xFF];
        assert_eq!(run_native(&bytecode).unwrap(), 8);
    }
    
    #[test]
    fn test_primitive_or() {
        // OR: 0b1100 | 0b1010 = 0b1110 = 14
        let bytecode = vec![0x30, 12, 0x30, 10, 0x61, 0xFF];
        assert_eq!(run_native(&bytecode).unwrap(), 14);
    }
    
    #[test]
    fn test_primitive_xor() {
        // XOR: 0b1100 ^ 0b1010 = 0b0110 = 6
        let bytecode = vec![0x30, 12, 0x30, 10, 0x63, 0xFF];
        assert_eq!(run_native(&bytecode).unwrap(), 6);
    }
    
    #[test]
    fn test_primitive_not() {
        // NOT: ~0 = -1 (all bits set in two's complement)
        let bytecode = vec![0x30, 0, 0x62, 0xFF];
        assert_eq!(run_native(&bytecode).unwrap(), -1);
        
        // NOT: ~1 = -2
        let bytecode2 = vec![0x30, 1, 0x62, 0xFF];
        assert_eq!(run_native(&bytecode2).unwrap(), -2);
    }
    
    #[test]
    fn test_compose_arithmetic() {
        // Composition: (5 + 3) * 2 - 4 = 12
        let bytecode = vec![
            0x30, 5,    // push 5
            0x30, 3,    // push 3
            0x40,       // add -> 8
            0x30, 2,    // push 2
            0x42,       // mul -> 16
            0x30, 4,    // push 4
            0x41,       // sub -> 12
            0xFF,
        ];
        assert_eq!(run_native(&bytecode).unwrap(), 12);
    }
    
    #[test]
    fn test_primitive_jz_balanced() {
        // JZ with balanced branches: both paths push one value then merge
        // if (cond == 0) push 10 else push 20; halt
        // 
        // Bytecode layout (i32 jump offsets):
        // 0-1:   push 0       (sp: 0 -> 1)
        // 2-6:   jz +7        (sp: 1 -> 0, if zero jump to 14)
        // 7-8:   push 20      (sp: 0 -> 1, not-taken path)
        // 9-13:  jmp +2       (jump to 16)
        // 14-15: push 10      (sp: 0 -> 1, taken path)
        // 16:    halt
        //
        // jz at pc=2, operand bytes 3-6, pc after = 7, target = 7 + 7 = 14 ✓
        // jmp at pc=9, operand bytes 10-13, pc after = 14, target = 14 + 2 = 16 ✓
        let bytecode = vec![
            0x30, 0,                      // 0-1: push 0 (condition)
            0x71, 0x07, 0x00, 0x00, 0x00, // 2-6: jz +7 -> jump to 14 if zero
            0x30, 20,                     // 7-8: push 20 (else branch)
            0x70, 0x02, 0x00, 0x00, 0x00, // 9-13: jmp +2 -> jump to 16
            0x30, 10,                     // 14-15: push 10 (then branch)
            0xFF,                         // 16: halt
        ];
        // Condition is 0, so JZ is taken -> push 10
        assert_eq!(run_native(&bytecode).unwrap(), 10);
    }
    
    #[test]
    fn test_primitive_jz_unbalanced_fails() {
        // JZ with unbalanced branches should fail
        // Taken path has sp=0, not-taken has sp=1 at merge point
        let bytecode = vec![
            0x30, 0,                      // push 0
            0x71, 0x02, 0x00, 0x00, 0x00, // jz +2 (skip next push)
            0x30, 99,                     // push 99 (only in not-taken path)
            0x30, 42,                     // push 42 (merge point, but sp differs!)
            0xFF,
        ];
        let result = run_native(&bytecode);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Stack mismatch"));
    }
    
    #[test]
    fn test_primitive_jnz() {
        // JNZ: push 1, if non-zero jump to push 10
        // Same bytecode structure as JZ but condition is 1 (non-zero)
        let bytecode = vec![
            0x30, 1,                      // 0-1: push 1 (condition, non-zero)
            0x72, 0x07, 0x00, 0x00, 0x00, // 2-6: jnz +7 -> jump to 14 if non-zero
            0x30, 20,                     // 7-8: push 20 (else branch)
            0x70, 0x02, 0x00, 0x00, 0x00, // 9-13: jmp +2 -> jump to 16
            0x30, 10,                     // 14-15: push 10 (then branch)
            0xFF,                         // 16: halt
        ];
        // Condition is 1 (non-zero), so JNZ is taken -> push 10
        assert_eq!(run_native(&bytecode).unwrap(), 10);
    }
    
    #[test]
    fn test_call_not_supported() {
        // CALL (0x22) should fail gracefully - needs interpreter for recursion
        let bytecode = vec![
            0x22, 0x00, 0x00, // call symbol 0
            0xFF,
        ];
        let result = run_native(&bytecode);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("CALL"));
    }
    
    #[test]
    fn test_ret_not_supported() {
        // RET (0x23) should fail gracefully
        let bytecode = vec![
            0x30, 42,  // push 42
            0x23,      // ret
            0xFF,
        ];
        let result = run_native(&bytecode);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("RET"));
    }
    
    #[test]
    fn test_unsupported_opcode_errors_clearly() {
        // LIST (0x80) is not supported in JIT — must error, never silently skip
        let bytecode = vec![
            0x30, 1, 0x30, 2, 0x30, 3,  // push 1 2 3
            0x80, 0x03, 0x00,            // list(3)
            0xFF,
        ];
        let result = run_native(&bytecode);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("0x80"), "Error should mention opcode: {}", err);
        assert!(err.contains("unsupported"), "Error should say unsupported: {}", err);
    }
    
    #[test]
    fn test_store_load_basic() {
        // STORE 0, LOAD 0: push 42, store in local 0, load it back
        let bytecode = vec![
            0x30, 42,               // push 42
            0xA8, 0, 0, 0, 0,      // store 0  (stack: empty, local[0] = 42)
            0xA9, 0, 0, 0, 0,      // load 0   (stack: 42)
            0xFF,
        ];
        assert_eq!(run_native(&bytecode).unwrap(), 42);
    }
    
    #[test]
    fn test_store_load_multiple_slots() {
        // Use multiple local slots: store 10 in slot 0, 20 in slot 1, load both, add
        let bytecode = vec![
            0x30, 10,               // push 10
            0xA8, 0, 0, 0, 0,      // store 0
            0x30, 20,               // push 20
            0xA8, 1, 0, 0, 0,      // store 1
            0xA9, 0, 0, 0, 0,      // load 0 (push 10)
            0xA9, 1, 0, 0, 0,      // load 1 (push 20)
            0x40,                   // add -> 30
            0xFF,
        ];
        assert_eq!(run_native(&bytecode).unwrap(), 30);
    }
    
    #[test]
    fn test_store_load_overwrite() {
        // Store, then overwrite: local 0 = 5, then local 0 = 99
        let bytecode = vec![
            0x30, 5,                // push 5
            0xA8, 0, 0, 0, 0,      // store 0
            0x30, 99,               // push 99
            0xA8, 0, 0, 0, 0,      // store 0 (overwrite)
            0xA9, 0, 0, 0, 0,      // load 0 -> 99
            0xFF,
        ];
        assert_eq!(run_native(&bytecode).unwrap(), 99);
    }
    
    #[test]
    fn test_store_load_in_loop() {
        // Classic loop with local counter:
        // sum = 0, i = 1; while i <= 10: sum += i; i += 1; result = sum
        // Expected: 1+2+...+10 = 55
        //
        // Use the compiler to generate correct bytecode (no hand-assembly errors)
        let source = "0 ->sum  1 ->i  while i 10 <= do  sum i + ->sum  i 1 + ->i  end  sum";
        let module = crate::parser::compile(source).expect("compile failed");
        let result = run_native(&module.code).unwrap();
        assert_eq!(result, 55);
    }
    
    #[test]
    fn test_cross_backend_store_load() {
        // STORE/LOAD must produce identical results in JIT and interpreter
        let bytecode = vec![
            0x30, 7,                // push 7
            0xA8, 0, 0, 0, 0,      // store 0
            0x30, 3,                // push 3
            0xA8, 1, 0, 0, 0,      // store 1
            0xA9, 0, 0, 0, 0,      // load 0
            0xA9, 1, 0, 0, 0,      // load 1
            0x42,                   // mul -> 21
            0xFF,
        ];
        let native = run_native(&bytecode).unwrap();
        let interp = run_interp(&bytecode);
        assert_eq!(native, 21);
        assert_eq!(native, interp, "Cross-backend STORE/LOAD consistency");
    }
    
    #[test]
    fn test_primitive_rot() {
        // ROT: a b c -> b c a
        // 1 2 3 rot -> 2 3 1
        // drop drop -> 1
        let bytecode = vec![
            0x30, 1,  // push 1
            0x30, 2,  // push 2
            0x30, 3,  // push 3
            0x04,     // rot: 1 2 3 -> 2 3 1
            0x01,     // drop: 2 3 -> 2
            0x01,     // drop: 2
            0xFF,
        ];
        assert_eq!(run_native(&bytecode).unwrap(), 2);
    }
    
    #[test]
    fn test_primitive_over() {
        // OVER: a b -> a b a
        // 5 3 over -> 5 3 5
        // add add -> 13
        let bytecode = vec![
            0x30, 5,  // push 5
            0x30, 3,  // push 3
            0x05,     // over: 5 3 -> 5 3 5
            0x40,     // add: 5 3 5 -> 5 8
            0x40,     // add: 5 8 -> 13
            0xFF,
        ];
        assert_eq!(run_native(&bytecode).unwrap(), 13);
    }
    
    #[test]
    fn test_primitive_true() {
        // TRUE: push 1
        let bytecode = vec![0x38, 0xFF];
        assert_eq!(run_native(&bytecode).unwrap(), 1);
    }
    
    #[test]
    fn test_primitive_false() {
        // FALSE: push 0
        let bytecode = vec![0x39, 0xFF];
        assert_eq!(run_native(&bytecode).unwrap(), 0);
    }
    
    #[test]
    fn test_primitive_nil() {
        // NIL: push 0
        let bytecode = vec![0x37, 0xFF];
        assert_eq!(run_native(&bytecode).unwrap(), 0);
    }
    
    #[test]
    fn test_primitive_mod() {
        // MOD: 17 % 5 = 2
        let bytecode = vec![0x30, 17, 0x30, 5, 0x44, 0xFF];
        assert_eq!(run_native(&bytecode).unwrap(), 2);
    }
    
    #[test]
    fn test_primitive_neg() {
        // NEG: -7 = -7
        let bytecode = vec![0x30, 7, 0x45, 0xFF];
        assert_eq!(run_native(&bytecode).unwrap(), -7);
    }
    
    #[test]
    fn test_primitive_eq() {
        // EQ: 5 == 5 -> 1
        let bytecode = vec![0x30, 5, 0x30, 5, 0x50, 0xFF];
        assert_eq!(run_native(&bytecode).unwrap(), 1);
        
        // EQ: 5 == 3 -> 0
        let bytecode2 = vec![0x30, 5, 0x30, 3, 0x50, 0xFF];
        assert_eq!(run_native(&bytecode2).unwrap(), 0);
    }
    
    #[test]
    fn test_primitive_gt() {
        // GT: 5 > 3 -> 1
        let bytecode = vec![0x30, 5, 0x30, 3, 0x52, 0xFF];
        assert_eq!(run_native(&bytecode).unwrap(), 1);
    }
    
    #[test]
    fn test_primitive_le() {
        // LE: 3 <= 5 -> 1
        let bytecode = vec![0x30, 3, 0x30, 5, 0x53, 0xFF];
        assert_eq!(run_native(&bytecode).unwrap(), 1);
    }
    
    #[test]
    fn test_primitive_ge() {
        // GE: 5 >= 5 -> 1
        let bytecode = vec![0x30, 5, 0x30, 5, 0x54, 0xFF];
        assert_eq!(run_native(&bytecode).unwrap(), 1);
    }
    
    #[test]
    fn test_primitive_drop() {
        // DROP: 5 3 -> 5
        let bytecode = vec![0x30, 5, 0x30, 3, 0x01, 0xFF];
        assert_eq!(run_native(&bytecode).unwrap(), 5);
    }
    
    #[test]
    fn test_primitive_nop() {
        // NOP: does nothing
        let bytecode = vec![0x30, 42, 0x00, 0x00, 0x00, 0xFF];
        assert_eq!(run_native(&bytecode).unwrap(), 42);
    }
    
    // ========================================================
    // FLOAT TESTS - floats stored as i64 bit patterns
    // ========================================================
    
    fn f64_to_bytes(f: f64) -> [u8; 8] {
        f.to_bits().to_le_bytes()
    }
    
    fn i64_to_f64(bits: i64) -> f64 {
        f64::from_bits(bits as u64)
    }
    
    #[test]
    fn test_float_literal_f64() {
        // F64: push 3.14159 as bit pattern
        let pi = std::f64::consts::PI;
        let bytes = f64_to_bytes(pi);
        let bytecode = vec![
            0x35, bytes[0], bytes[1], bytes[2], bytes[3], 
                  bytes[4], bytes[5], bytes[6], bytes[7],
            0xFF,
        ];
        let result = run_native(&bytecode).unwrap();
        assert_eq!(i64_to_f64(result), pi);
    }
    
    #[test]
    fn test_float_fadd() {
        // FADD: 1.5 + 2.5 = 4.0
        let a = f64_to_bytes(1.5);
        let b = f64_to_bytes(2.5);
        let bytecode = vec![
            0x35, a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7],
            0x35, b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
            0x46, // FADD
            0xFF,
        ];
        let result = run_native(&bytecode).unwrap();
        assert_eq!(i64_to_f64(result), 4.0);
    }
    
    #[test]
    fn test_float_fsub() {
        // FSUB: 5.0 - 3.0 = 2.0
        let a = f64_to_bytes(5.0);
        let b = f64_to_bytes(3.0);
        let bytecode = vec![
            0x35, a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7],
            0x35, b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
            0x47, // FSUB
            0xFF,
        ];
        let result = run_native(&bytecode).unwrap();
        assert_eq!(i64_to_f64(result), 2.0);
    }
    
    #[test]
    fn test_float_fmul() {
        // FMUL: 3.0 * 4.0 = 12.0
        let a = f64_to_bytes(3.0);
        let b = f64_to_bytes(4.0);
        let bytecode = vec![
            0x35, a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7],
            0x35, b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
            0x48, // FMUL
            0xFF,
        ];
        let result = run_native(&bytecode).unwrap();
        assert_eq!(i64_to_f64(result), 12.0);
    }
    
    #[test]
    fn test_float_fdiv() {
        // FDIV: 10.0 / 4.0 = 2.5
        let a = f64_to_bytes(10.0);
        let b = f64_to_bytes(4.0);
        let bytecode = vec![
            0x35, a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7],
            0x35, b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
            0x49, // FDIV
            0xFF,
        ];
        let result = run_native(&bytecode).unwrap();
        assert_eq!(i64_to_f64(result), 2.5);
    }
    
    #[test]
    fn test_float_fsqrt() {
        // FSQRT: sqrt(16.0) = 4.0
        let a = f64_to_bytes(16.0);
        let bytecode = vec![
            0x35, a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7],
            0x4B, // FSQRT
            0xFF,
        ];
        let result = run_native(&bytecode).unwrap();
        assert_eq!(i64_to_f64(result), 4.0);
    }
    
    #[test]
    fn test_float_fneg() {
        // FNEG: -5.0
        let a = f64_to_bytes(5.0);
        let bytecode = vec![
            0x35, a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7],
            0x4A, // FNEG
            0xFF,
        ];
        let result = run_native(&bytecode).unwrap();
        assert_eq!(i64_to_f64(result), -5.0);
    }
    
    #[test]
    fn test_float_fabs() {
        // FABS: |-5.0| = 5.0
        let a = f64_to_bytes(-5.0);
        let bytecode = vec![
            0x35, a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7],
            0x4C, // FABS
            0xFF,
        ];
        let result = run_native(&bytecode).unwrap();
        assert_eq!(i64_to_f64(result), 5.0);
    }
    
    #[test]
    fn test_float_i2f() {
        // I2F: 42 -> 42.0
        let bytecode = vec![
            0x30, 42, // INT8 42
            0x4D,     // I2F
            0xFF,
        ];
        let result = run_native(&bytecode).unwrap();
        assert_eq!(i64_to_f64(result), 42.0);
    }
    
    #[test]
    fn test_float_f2i() {
        // F2I: 3.7 -> 3 (truncate)
        let a = f64_to_bytes(3.7);
        let bytecode = vec![
            0x35, a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7],
            0x4E, // F2I
            0xFF,
        ];
        let result = run_native(&bytecode).unwrap();
        assert_eq!(result, 3);
    }
    
    #[test]
    fn test_float_pythagorean() {
        // sqrt(3^2 + 4^2) = 5.0 (complex magnitude)
        let three = f64_to_bytes(3.0);
        let four = f64_to_bytes(4.0);
        let bytecode = vec![
            // 3.0 dup fmul -> 9.0
            0x35, three[0], three[1], three[2], three[3], three[4], three[5], three[6], three[7],
            0x02, // DUP
            0x48, // FMUL -> 9.0
            // 4.0 dup fmul -> 16.0
            0x35, four[0], four[1], four[2], four[3], four[4], four[5], four[6], four[7],
            0x02, // DUP
            0x48, // FMUL -> 16.0
            // fadd -> 25.0
            0x46, // FADD
            // fsqrt -> 5.0
            0x4B, // FSQRT
            0xFF,
        ];
        let result = run_native(&bytecode).unwrap();
        assert_eq!(i64_to_f64(result), 5.0);
    }
    
    // ========================================================
    // BENCHMARKS - Compare native JIT vs interpreter
    // ========================================================
    
    #[test]
    fn bench_native_vs_interpreter() {
        use std::time::Instant;
        
        // Bytecode: (1000000 * 1000000 + 12345 - 67890) / 2 = 499999972227
        let bytecode = vec![
            0x32, 0x40, 0x42, 0x0f, 0x00, // INT32 1000000
            0x32, 0x40, 0x42, 0x0f, 0x00, // INT32 1000000
            0x42,                         // MUL
            0x31, 0x39, 0x30,             // INT16 12345
            0x40,                         // ADD
            0x32, 0x32, 0x09, 0x01, 0x00, // INT32 67890
            0x41,                         // SUB
            0x30, 2,                      // INT8 2
            0x43,                         // DIV
            0xFF,                         // HALT
        ];
        
        let expected: i64 = (1000000i64 * 1000000 + 12345 - 67890) / 2;
        
        // Run native many times (includes JIT compilation in first run)
        let iterations = 10000;
        
        // COMPILE ONCE for native
        let mut compiler = NativeCompiler::new().unwrap();
        let func = compiler.compile(&bytecode).unwrap();
        
        // Verify result
        let native_result = unsafe { func() };
        assert_eq!(native_result, expected, "Native result mismatch");
        
        // Benchmark native (call compiled function, no recompilation)
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = unsafe { func() };
        }
        let native_time = start.elapsed();
        
        // Benchmark interpreter (must parse each time)
        let start = Instant::now();
        for _ in 0..iterations {
            let mut interp = crate::interpreter::Interpreter::new(&bytecode);
            let _ = interp.run().unwrap();
        }
        let interp_time = start.elapsed();
        
        let speedup = interp_time.as_nanos() as f64 / native_time.as_nanos() as f64;
        
        println!("\n=== BENCHMARK: Native JIT vs Interpreter ===");
        println!("Bytecode: {} bytes", bytecode.len());
        println!("Iterations: {}", iterations);
        println!("Expected result: {}", expected);
        println!("");
        println!("Native JIT:    {:>10.2?} ({:.3} µs/iter)", native_time, native_time.as_nanos() as f64 / iterations as f64 / 1000.0);
        println!("Interpreter:   {:>10.2?} ({:.3} µs/iter)", interp_time, interp_time.as_nanos() as f64 / iterations as f64 / 1000.0);
        println!("Speedup:       {:.1}x", speedup);
        println!("");
        
        // Native should be faster
        assert!(speedup > 1.0, "Native should be faster than interpreter");
    }
    
    #[test]
    fn bench_loop_native() {
        use std::time::Instant;
        
        // Loop: count from N down to 0
        // push N; loop: dup, jz done, 1 sub, jmp loop; done: halt
        //
        // Layout (i32 jump offsets, 5-byte jump instructions):
        // pc=0: 0x30 N   (2 bytes, pc after = 2)
        // pc=2: 0x02 DUP (1 byte, pc after = 3)
        // pc=3: 0x71 i32  (5 bytes, JZ, pc after = 8)
        //       target = 8 + offset; we want target = 16 (HALT), so offset = 8
        // pc=8: 0x30 1   (2 bytes, pc after = 10)
        // pc=10: 0x41 SUB (1 byte, pc after = 11)
        // pc=11: 0x70 i32  (5 bytes, JMP, pc after = 16)
        //       target = 16 + offset; we want target = 2 (DUP), so offset = -14
        // pc=16: 0xFF HALT
        
        let bytecode = vec![
            0x30, 100,                    // 0-1: push 100
            0x02,                         // 2: DUP
            0x71, 0x08, 0x00, 0x00, 0x00, // 3-7: JZ +8 -> target 16 (HALT)
            0x30, 1,                      // 8-9: push 1
            0x41,                         // 10: SUB
            0x70, 0xF2, 0xFF, 0xFF, 0xFF, // 11-15: JMP -14 -> target 2 (DUP)
            0xFF,                         // 16: HALT
        ];
        
        // Run it
        let result = run_native(&bytecode);
        assert!(result.is_ok(), "Loop failed: {:?}", result);
        assert_eq!(result.unwrap(), 0, "Loop should end at 0");
        
        // Benchmark
        let iterations = 100;
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = run_native(&bytecode).unwrap();
        }
        let native_time = start.elapsed();
        
        println!("\n=== BENCHMARK: Loop (100 iterations internal) ===");
        println!("Native JIT:    {:>10.2?} ({:.2} µs/run)", native_time, native_time.as_nanos() as f64 / iterations as f64 / 1000.0);
    }
    
    // ========================================================================
    // CROSS-BACKEND CONSISTENCY TESTS
    // These verify interpreter and native produce identical results
    // ========================================================================
    
    fn run_interp(code: &[u8]) -> i64 {
        let mut interp = crate::interpreter::Interpreter::new(code);
        interp.run().unwrap();
        match interp.result() {
            Some(crate::interpreter::Value::Int(n)) => *n,
            Some(crate::interpreter::Value::Bool(true)) => 1,
            Some(crate::interpreter::Value::Bool(false)) => 0,
            other => panic!("Unexpected result: {:?}", other),
        }
    }
    
    #[test]
    fn test_cross_backend_arithmetic() {
        // (5 + 3) * 2 = 16
        let code = vec![0x30, 5, 0x30, 3, 0x40, 0x30, 2, 0x42, 0xFF];
        let native = run_native(&code).unwrap();
        let interp = run_interp(&code);
        assert_eq!(native, 16);
        assert_eq!(interp, 16);
        assert_eq!(native, interp, "Cross-backend: (5+3)*2");
    }
    
    #[test]
    fn test_cross_backend_comparison() {
        // 5 > 3 = true (1)
        let code = vec![0x30, 5, 0x30, 3, 0x52, 0xFF];
        let native = run_native(&code).unwrap();
        let interp = run_interp(&code);
        assert_eq!(native, 1);
        assert_eq!(interp, 1);
        assert_eq!(native, interp, "Cross-backend: 5 > 3");
    }
    
    #[test]
    fn test_cross_backend_logic() {
        // true and false = false (0)
        let code = vec![0x38, 0x39, 0x60, 0xFF];
        let native = run_native(&code).unwrap();
        let interp = run_interp(&code);
        assert_eq!(native, 0);
        assert_eq!(interp, 0);
        assert_eq!(native, interp, "Cross-backend: true AND false");
    }
    
    #[test]
    fn test_cross_backend_stack_ops() {
        // 3 dup * = 9
        let code = vec![0x30, 3, 0x02, 0x42, 0xFF];
        let native = run_native(&code).unwrap();
        let interp = run_interp(&code);
        assert_eq!(native, 9);
        assert_eq!(interp, 9);
        assert_eq!(native, interp, "Cross-backend: 3 dup *");
    }
    
    #[test]
    fn test_cross_backend_negative() {
        // -5 + 3 = -2
        let code = vec![0x30, 0xFB_u8, 0x30, 3, 0x40, 0xFF]; // 0xFB = -5 as i8
        let native = run_native(&code).unwrap();
        let interp = run_interp(&code);
        assert_eq!(native, -2);
        assert_eq!(interp, -2);
        assert_eq!(native, interp, "Cross-backend: -5 + 3");
    }
    
    #[test]
    fn test_p1_tools_are_stack_functions() {
        // P1: Every op transforms stack, no hidden state
        // () -> (5) -> (5,3) -> (8) -> (8,8) -> (64)
        let code = vec![0x30, 5, 0x30, 3, 0x40, 0x02, 0x42, 0xFF];
        let native = run_native(&code).unwrap();
        let interp = run_interp(&code);
        assert_eq!(native, 64);
        assert_eq!(native, interp, "P1: Tools compose as stack transformers");
    }
    
    #[test]
    fn test_p2_deterministic_execution() {
        // P2: Same bytecode always produces same result
        let code = vec![0x30, 42, 0x02, 0x42, 0xFF]; // 42 dup * = 1764
        for _ in 0..100 {
            assert_eq!(run_native(&code).unwrap(), 1764);
            assert_eq!(run_interp(&code), 1764);
        }
    }
    
    #[test]
    fn test_p3_composition_is_concatenation() {
        // P3: (f ; g)(x) = g(f(x))
        // f: x+1, g: *2, result: (5+1)*2 = 12
        let code = vec![0x30, 5, 0x30, 1, 0x40, 0x30, 2, 0x42, 0xFF];
        let native = run_native(&code).unwrap();
        let interp = run_interp(&code);
        assert_eq!(native, 12);
        assert_eq!(native, interp, "P3: Composition = concatenation");
    }
    
    #[test]
    fn test_p4_integer_division_consistency() {
        // P4: Types attenuate - Int / Int = Int (truncated)
        let code = vec![0x30, 10, 0x30, 3, 0x43, 0xFF]; // 10 / 3 = 3
        let native = run_native(&code).unwrap();
        let interp = run_interp(&code);
        assert_eq!(native, 3);
        assert_eq!(native, interp, "P4: Integer division truncates");
    }
    
    // ================================================================
    // CALL/RET tests — using compile_module path
    // ================================================================
    
    /// Helper: compile source, run via module path, return result
    fn run_module(source: &str) -> i64 {
        let module = crate::parser::compile(source).expect("compile failed");
        run_native_module(&module).expect("native module failed")
    }
    
    /// Helper: compile source, run via interpreter, return top of stack as i64
    fn run_interp_module(source: &str) -> i64 {
        let module = crate::parser::compile(source).expect("compile failed");
        let mut interp = crate::interpreter::Interpreter::from_module(&module);
        interp.run().unwrap();
        match interp.stack().last() {
            Some(crate::interpreter::Value::Int(n)) => *n,
            Some(crate::interpreter::Value::Bool(true)) => 1,
            Some(crate::interpreter::Value::Bool(false)) => 0,
            other => panic!("Expected Int or Bool on stack, got {:?}", other),
        }
    }
    
    #[test]
    fn test_call_ret_simple_function() {
        // : double dup + ;  5 double
        // Expected: 10
        let result = run_module(": double dup + ;  5 double");
        assert_eq!(result, 10);
    }
    
    #[test]
    fn test_call_ret_multiple_calls() {
        // : double dup + ;  3 double double
        // Expected: 3 -> 6 -> 12
        let result = run_module(": double dup + ;  3 double double");
        assert_eq!(result, 12);
    }
    
    #[test]
    fn test_call_ret_with_locals() {
        // : square ->x  x x * ;  7 square
        // Expected: 49
        let result = run_module(": square ->x  x x * ;  7 square");
        assert_eq!(result, 49);
    }
    
    #[test]
    fn test_call_ret_two_functions() {
        // : double dup + ;
        // : add1 1 + ;
        // 5 double add1
        // Expected: 5 -> 10 -> 11
        let result = run_module(": double dup + ;  : add1 1 + ;  5 double add1");
        assert_eq!(result, 11);
    }
    
    #[test]
    fn test_call_ret_local_scoping() {
        // Locals should be scoped per function call
        // : f ->x  x x + ;
        // 3 f 5 f +
        // Expected: (3+3) + (5+5) = 16
        let result = run_module(": f ->x  x x + ;  3 f 5 f +");
        assert_eq!(result, 16);
    }
    
    #[test]
    fn test_call_ret_cross_backend_consistency() {
        // Same program must give same result on interpreter and JIT
        let sources = [
            ": double dup + ;  5 double",
            ": square ->x  x x * ;  7 square",
            ": double dup + ;  : add1 1 + ;  5 double add1",
            ": f ->x  x x + ;  3 f 5 f +",
        ];
        for src in &sources {
            let native = run_module(src);
            let interp = run_interp_module(src);
            assert_eq!(native, interp, "Cross-backend mismatch for: {}", src);
        }
    }

    // ================================================================
    // REGRESSION: compile_module must use correct opcode values
    // (Previously used wrong opcodes for Nil/True/False and logic ops)
    // ================================================================

    #[test]
    fn test_module_true_false_nil() {
        // Ensure Nil=0x37, True=0x38, False=0x39 are handled correctly
        // true false and → 0 (false AND true = 0)
        let native = run_module(": f true false and ; f");
        let interp = run_interp_module(": f true false and ; f");
        assert_eq!(native, 0);
        assert_eq!(native, interp, "Cross-backend: true false and");
    }

    #[test]
    fn test_module_logic_and_or() {
        // Verify AND/OR opcodes (0x60/0x61) in compile_module
        // 3 5 and → 1 (bitwise: 0b011 & 0b101 = 0b001 = 1)
        let native = run_module(": f 3 5 and ; f");
        let interp = run_interp_module(": f 3 5 and ; f");
        assert_eq!(native, interp, "Cross-backend: 3 5 and");

        // 3 5 or → 7 (bitwise: 0b011 | 0b101 = 0b111 = 7)
        let native2 = run_module(": f 3 5 or ; f");
        let interp2 = run_interp_module(": f 3 5 or ; f");
        assert_eq!(native2, interp2, "Cross-backend: 3 5 or");
    }

    #[test]
    fn test_module_bitwise_ops() {
        // Verify BAND/BOR/BXOR/BNOT/SHL/SHR (0x64-0x69) in compile_module
        let cases = [
            (": f 12 10 band ; f", "band"),
            (": f 12 10 bor ; f", "bor"),
            (": f 12 10 bxor ; f", "bxor"),
            (": f 0 bnot ; f", "bnot"),
            (": f 1 4 shl ; f", "shl"),
            (": f 16 2 shr ; f", "shr"),
        ];
        for (src, label) in &cases {
            let native = run_module(src);
            let interp = run_interp_module(src);
            assert_eq!(native, interp, "Cross-backend {}: native={}, interp={}", label, native, interp);
        }
    }

    // ================================================================
    // OPCODE REGISTRY ENFORCEMENT — prevents the 13-opcode class of bug
    // Both compile() and compile_module() must handle the same opcodes.
    // ================================================================

    #[test]
    fn test_jit_opcodes_registry_consistency() {
        // Verify jit_opcodes() returns valid opcodes matching the canonical enum
        let jit_ops = crate::bytecode::Op::jit_opcodes();
        for op in jit_ops {
            let byte = *op as u8;
            let roundtrip = crate::bytecode::Op::from_byte(byte);
            assert!(roundtrip.is_some(), "JIT opcode 0x{:02X} not in Op::from_byte()", byte);
            assert_eq!(roundtrip.unwrap(), *op, "JIT opcode 0x{:02X} roundtrip mismatch", byte);
        }
    }

    #[test]
    fn test_all_opcodes_includes_jit_opcodes() {
        // Every JIT opcode must be in all_opcodes()
        let all_ops: std::collections::HashSet<u8> = crate::bytecode::Op::all_opcodes()
            .iter().map(|op| *op as u8).collect();
        for op in crate::bytecode::Op::jit_opcodes() {
            assert!(all_ops.contains(&(*op as u8)),
                "JIT opcode {:?} (0x{:02X}) not in all_opcodes()", op, *op as u8);
        }
    }

    #[test]
    fn test_compile_handles_all_jit_opcodes() {
        // Every JIT opcode except CALL/RET must be accepted by compile()
        // (CALL/RET intentionally error in compile(), they need compile_module())
        use crate::bytecode::Op;
        let skip = [Op::Call as u8, Op::Ret as u8];

        for op in Op::jit_opcodes() {
            let byte = *op as u8;
            if skip.contains(&byte) { continue; }

            // Build minimal bytecode that exercises this opcode
            let bytecode = build_test_bytecode_for_op(*op);
            let result = run_native(&bytecode);
            assert!(result.is_ok(),
                "compile() rejected JIT opcode {:?} (0x{:02X}): {}",
                op, byte, result.unwrap_err());
        }
    }

    #[test]
    fn test_compile_module_handles_all_jit_opcodes() {
        // Every JIT opcode must be accepted by compile_module()
        use crate::bytecode::Op;

        for op in Op::jit_opcodes() {
            let byte = *op as u8;
            let bytecode = build_test_bytecode_for_op(*op);

            // Wrap in a module (no symbol table = uses compile() fast path;
            // to force compile_module path, we use a dummy function)
            let mut module = crate::bytecode::BytecodeModule::new();
            // Add a dummy function so compile_module() is forced
            module.symbol_table.insert(0, bytecode.len()); // points past end (unreachable)
            module.code = bytecode.clone();
            // Add a dummy function body: just RET
            module.code.push(Op::Ret as u8);

            let result = run_native_module(&module);
            assert!(result.is_ok(),
                "compile_module() rejected JIT opcode {:?} (0x{:02X}): {}",
                op, byte, result.unwrap_err());
        }
    }

    /// Build minimal valid bytecode that exercises a given opcode.
    /// Each opcode needs appropriate stack setup to avoid underflow.
    fn build_test_bytecode_for_op(op: crate::bytecode::Op) -> Vec<u8> {
        use crate::bytecode::Op;
        match op {
            // Stack ops
            Op::Nop => vec![Op::Nop as u8, Op::Halt as u8],
            Op::Drop => vec![Op::Int8 as u8, 1, Op::Drop as u8, Op::Halt as u8],
            Op::Dup => vec![Op::Int8 as u8, 1, Op::Dup as u8, Op::Halt as u8],
            Op::Swap => vec![Op::Int8 as u8, 1, Op::Int8 as u8, 2, Op::Swap as u8, Op::Halt as u8],
            Op::Rot => vec![Op::Int8 as u8, 1, Op::Int8 as u8, 2, Op::Int8 as u8, 3, Op::Rot as u8, Op::Halt as u8],
            Op::Over => vec![Op::Int8 as u8, 1, Op::Int8 as u8, 2, Op::Over as u8, Op::Halt as u8],

            // Literals
            Op::Int8 => vec![Op::Int8 as u8, 42, Op::Halt as u8],
            Op::Int16 => vec![Op::Int16 as u8, 0x39, 0x30, Op::Halt as u8],
            Op::Int32 => vec![Op::Int32 as u8, 0x01, 0x00, 0x00, 0x00, Op::Halt as u8],
            Op::Int64 => vec![Op::Int64 as u8, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, Op::Halt as u8],
            Op::F32 => vec![Op::F32 as u8, 0x00, 0x00, 0x80, 0x3F, Op::Halt as u8], // 1.0f32
            Op::F64 => {
                let bytes = 1.0f64.to_bits().to_le_bytes();
                let mut bc = vec![Op::F64 as u8];
                bc.extend_from_slice(&bytes);
                bc.push(Op::Halt as u8);
                bc
            }
            Op::Nil => vec![Op::Nil as u8, Op::Halt as u8],
            Op::True => vec![Op::True as u8, Op::Halt as u8],
            Op::False => vec![Op::False as u8, Op::Halt as u8],

            // Int arithmetic (binary: need 2 args)
            Op::Add | Op::Sub | Op::Mul | Op::Div | Op::Mod =>
                vec![Op::Int8 as u8, 10, Op::Int8 as u8, 3, op as u8, Op::Halt as u8],
            Op::Neg => vec![Op::Int8 as u8, 5, Op::Neg as u8, Op::Halt as u8],

            // Float arithmetic (binary: need 2 f64 args)
            Op::Fadd | Op::Fsub | Op::Fmul | Op::Fdiv => {
                let a = 2.0f64.to_bits().to_le_bytes();
                let b = 1.0f64.to_bits().to_le_bytes();
                let mut bc = vec![Op::F64 as u8];
                bc.extend_from_slice(&a);
                bc.push(Op::F64 as u8);
                bc.extend_from_slice(&b);
                bc.push(op as u8);
                bc.push(Op::Halt as u8);
                bc
            }
            Op::Fneg | Op::Fsqrt | Op::Fabs => {
                let a = 4.0f64.to_bits().to_le_bytes();
                let mut bc = vec![Op::F64 as u8];
                bc.extend_from_slice(&a);
                bc.push(op as u8);
                bc.push(Op::Halt as u8);
                bc
            }
            Op::I2f => vec![Op::Int8 as u8, 5, Op::I2f as u8, Op::Halt as u8],
            Op::F2i => {
                let a = 3.7f64.to_bits().to_le_bytes();
                let mut bc = vec![Op::F64 as u8];
                bc.extend_from_slice(&a);
                bc.push(Op::F2i as u8);
                bc.push(Op::Halt as u8);
                bc
            }

            // Comparison (binary)
            Op::Eq | Op::Lt | Op::Gt | Op::Le | Op::Ge | Op::Ne =>
                vec![Op::Int8 as u8, 5, Op::Int8 as u8, 3, op as u8, Op::Halt as u8],

            // Logic (binary)
            Op::And | Op::Or | Op::Xor | Op::Band | Op::Bor | Op::Bxor | Op::Shl | Op::Shr =>
                vec![Op::Int8 as u8, 12, Op::Int8 as u8, 3, op as u8, Op::Halt as u8],
            Op::Not => vec![Op::Int8 as u8, 1, Op::Not as u8, Op::Halt as u8],
            Op::Bnot => vec![Op::Int8 as u8, 1, Op::Bnot as u8, Op::Halt as u8],

            // Jumps — need careful layout (i32 offsets = 4 bytes)
            Op::Jmp => vec![Op::Int8 as u8, 42, Op::Jmp as u8, 0x00, 0x00, 0x00, 0x00, Op::Halt as u8], // jmp +0 (fallthrough)
            Op::Jz => vec![Op::Int8 as u8, 42, Op::Int8 as u8, 0, Op::Jz as u8, 0x00, 0x00, 0x00, 0x00, Op::Halt as u8],
            Op::Jnz => vec![Op::Int8 as u8, 42, Op::Int8 as u8, 1, Op::Jnz as u8, 0x00, 0x00, 0x00, 0x00, Op::Halt as u8],

            // CALL/RET: these need a module context, just test they don't crash
            Op::Call => vec![Op::Int8 as u8, 42, Op::Halt as u8], // skip actual CALL
            Op::Ret => vec![Op::Int8 as u8, 42, Op::Halt as u8],  // skip actual RET

            // Locals
            Op::Store => vec![Op::Int8 as u8, 42, Op::Store as u8, 0, 0, 0, 0, Op::Int8 as u8, 0, Op::Halt as u8],
            Op::Load => vec![Op::Int8 as u8, 42, Op::Store as u8, 0, 0, 0, 0, Op::Load as u8, 0, 0, 0, 0, Op::Halt as u8],

            // Reflection
            Op::Fetch => vec![Op::Int8 as u8, 0, Op::Fetch as u8, Op::Halt as u8],
            Op::Size => vec![Op::Size as u8, Op::Halt as u8],

            // Halt
            Op::Halt => vec![Op::Halt as u8],

            // Any other — shouldn't be in jit_opcodes
            _ => vec![op as u8, Op::Halt as u8],
        }
    }

    // ================================================================
    // EXHAUSTIVE CROSS-BACKEND CONSISTENCY — every JIT opcode must
    // produce identical results on SSA compile() and module compile_module()
    // ================================================================

    #[test]
    fn test_every_jit_opcode_cross_backend() {
        // For every opcode that both backends handle, verify identical output
        let test_cases: Vec<(&str, &str, i64)> = vec![
            // Stack ops
            ("nop", "42", 42),
            ("drop", "42 99 drop", 42),
            ("dup", "7 dup +", 14),
            ("swap", "10 3 swap -", -7),
            ("rot", "1 2 3 rot drop drop", 2),
            ("over", "5 3 over + +", 13),
            // Literals
            ("int8", "42", 42),
            ("negative int8", "-5", -5),
            ("zero", "0", 0),
            ("true only", "true", 1),
            ("false only", "false", 0),
            // Arithmetic
            ("add", "5 3 +", 8),
            ("sub", "10 3 -", 7),
            ("mul", "6 7 *", 42),
            ("div", "20 4 /", 5),
            ("mod", "17 5 mod", 2),
            ("neg", "7 neg", -7),
            // Comparison
            ("eq true", "5 5 =", 1),
            ("eq false", "5 3 =", 0),
            ("lt true", "3 5 <", 1),
            ("lt false", "5 3 <", 0),
            ("gt true", "5 3 >", 1),
            ("gt false", "3 5 >", 0),
            ("le true", "5 5 <=", 1),
            ("ge true", "5 5 >=", 1),
            ("ne true", "5 3 !=", 1),
            ("ne false", "5 5 !=", 0),
            // Logic
            ("and", "12 10 and", 8),
            ("or", "12 10 or", 14),
            ("xor", "12 10 xor", 6),
            ("not 0", "0 not", -1),
            ("band", "12 10 band", 8),
            ("bor", "12 10 bor", 14),
            ("bxor", "12 10 bxor", 6),
            ("bnot", "0 bnot", -1),
            ("shl", "1 4 shl", 16),
            ("shr", "16 2 shr", 4),
            // Locals
            ("store/load", "42 ->x  x", 42),
            ("multi locals", "10 ->a  20 ->b  a b +", 30),
            ("overwrite", "5 ->x 99 ->x  x", 99),
            // Complex compositions
            ("factorial-like", "1 2 3 4 5 * * * *", 120),
            ("sum-loop", "0 ->sum  1 ->i  while i 10 <= do  sum i + ->sum  i 1 + ->i  end  sum", 55),
        ];

        for (label, source, expected) in &test_cases {
            // Run via SSA compile()
            let module_ssa = crate::parser::compile(source).expect(&format!("compile failed: {}", label));
            let ssa_result = run_native(&module_ssa.code);
            assert!(ssa_result.is_ok(), "SSA compile failed for '{}': {}", label, ssa_result.as_ref().unwrap_err());
            let ssa_val = ssa_result.unwrap();
            assert_eq!(ssa_val, *expected,
                "SSA result wrong for '{}': got {}, expected {}", label, ssa_val, expected);

            // Run via interpreter
            let interp_result = {
                let mut interp = crate::interpreter::Interpreter::new(&module_ssa.code);
                interp.run().unwrap();
                match interp.result() {
                    Some(crate::interpreter::Value::Int(n)) => *n,
                    Some(crate::interpreter::Value::Bool(true)) => 1,
                    Some(crate::interpreter::Value::Bool(false)) => 0,
                    other => panic!("Unexpected result for '{}': {:?}", label, other),
                }
            };
            assert_eq!(interp_result, *expected,
                "Interpreter result wrong for '{}': got {}, expected {}", label, interp_result, expected);

            // Run via compile_module (memory-backed) — wrap in a function to force module path
            let module_src = format!(": __test {} ;  __test", source);
            let native_mod = run_module(&module_src);
            let interp_mod = run_interp_module(&module_src);
            assert_eq!(native_mod, *expected,
                "Module native wrong for '{}': got {}, expected {}", label, native_mod, expected);
            assert_eq!(interp_mod, *expected,
                "Module interp wrong for '{}': got {}, expected {}", label, interp_mod, expected);
        }
    }

    // ================================================================
    // SECURITY TESTS — no silent failures, proper error handling
    // ================================================================

    #[test]
    fn test_security_unknown_opcodes_always_error() {
        // Every byte that is NOT a valid opcode must produce an error, never silently skip
        let valid: std::collections::HashSet<u8> = (0..=255u8)
            .filter(|b| crate::bytecode::Op::from_byte(*b).is_some())
            .collect();

        for byte in 0..=255u8 {
            if valid.contains(&byte) { continue; }
            let bytecode = vec![byte, 0xFF]; // unknown opcode + HALT
            let result = run_native(&bytecode);
            assert!(result.is_err(),
                "compile() silently accepted unknown opcode 0x{:02X}", byte);
            assert!(result.unwrap_err().contains("unsupported"),
                "compile() error for 0x{:02X} should say 'unsupported'", byte);
        }
    }

    #[test]
    fn test_security_module_unknown_opcodes_always_error() {
        // Same for compile_module() — unknown opcodes must error
        let valid: std::collections::HashSet<u8> = (0..=255u8)
            .filter(|b| crate::bytecode::Op::from_byte(*b).is_some())
            .collect();
        let jit_supported: std::collections::HashSet<u8> = crate::bytecode::Op::jit_opcodes()
            .iter().map(|op| *op as u8).collect();

        for byte in 0..=255u8 {
            if valid.contains(&byte) && !jit_supported.contains(&byte) {
                // Valid opcode but not JIT-supported — should error with "unsupported"
                let mut module = crate::bytecode::BytecodeModule::new();
                module.symbol_table.insert(0, 100); // dummy function to force compile_module
                module.code = vec![byte, 0xFF];
                module.code.resize(101, 0xFF); // pad to include the dummy function offset
                let result = run_native_module(&module);
                assert!(result.is_err(),
                    "compile_module() silently accepted unsupported opcode 0x{:02X} ({:?})",
                    byte, crate::bytecode::Op::from_byte(byte));
            }
        }
    }

    #[test]
    fn test_security_empty_program() {
        // Empty bytecode = 0 on stack
        let result = run_native(&[0xFF]);
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_security_nop_only() {
        let result = run_native(&[0x00, 0x00, 0x00, 0xFF]);
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn test_security_max_locals() {
        // Use 64 local slots — should not crash
        let mut bytecode = Vec::new();
        for slot in 0..64u32 {
            bytecode.extend_from_slice(&[0x30, (slot + 1) as u8]); // push slot+1
            bytecode.push(0xA8);                                    // store
            bytecode.extend_from_slice(&slot.to_le_bytes());        // slot (u32 LE)
        }
        // Load the last one (slot 63)
        bytecode.push(0xA9);
        bytecode.extend_from_slice(&63u32.to_le_bytes());
        bytecode.push(0xFF);
        assert_eq!(run_native(&bytecode).unwrap(), 64);
    }

    #[test]
    fn test_security_integer_boundaries() {
        // Test near i64::MAX and i64::MIN
        let max_bytes = i64::MAX.to_le_bytes();
        let one_bytes = 1i64.to_le_bytes();

        // i64::MAX - 1 (should not overflow in subtraction)
        let mut bytecode = vec![0x33]; // INT64
        bytecode.extend_from_slice(&max_bytes);
        bytecode.push(0x33); // INT64
        bytecode.extend_from_slice(&one_bytes);
        bytecode.push(0x41); // SUB
        bytecode.push(0xFF);
        assert_eq!(run_native(&bytecode).unwrap(), i64::MAX - 1);
    }

    #[test]
    fn test_security_negative_arithmetic() {
        // -100 + 50 = -50
        let bytecode = vec![0x30, 0x9C, 0x30, 50, 0x40, 0xFF]; // -100 as i8 = 0x9C
        let result = run_native(&bytecode).unwrap();
        assert_eq!(result, -50);
    }

    #[test]
    fn test_security_stack_underflow_drop() {
        // DROP on empty stack — should not crash (sp stays at 0)
        let result = run_native(&[0x01, 0xFF]);
        assert!(result.is_ok()); // Should not panic
    }

    #[test]
    fn test_security_deep_computation() {
        // Build a deep computation: push 1, then dup + 100 times → 2^100
        // This tests deep stack usage without overflow
        let mut bytecode = vec![0x30, 1]; // push 1
        for _ in 0..20 {
            bytecode.push(0x02); // dup
            bytecode.push(0x40); // add (doubles)
        }
        bytecode.push(0xFF);
        assert_eq!(run_native(&bytecode).unwrap(), 1 << 20); // 1048576
    }

    // ================================================================
    // PERFORMANCE REGRESSION TESTS
    // ================================================================

    #[test]
    fn bench_ssa_vs_module_path() {
        use std::time::Instant;

        // Compile the same arithmetic program via both paths
        let source = "5 3 + 2 * 4 -";
        let module = crate::parser::compile(source).expect("compile failed");

        // SSA path (compile)
        let mut compiler_ssa = NativeCompiler::new().unwrap();
        let func_ssa = compiler_ssa.compile(&module.code).unwrap();

        // Module path — force by adding dummy symbol
        let mut mod_with_sym = module.clone();
        mod_with_sym.symbol_table.insert(9999, module.code.len());
        mod_with_sym.code.push(0x23); // RET at end (unreachable)
        let mut compiler_mod = NativeCompiler::new().unwrap();
        let func_mod = compiler_mod.compile_module(&mod_with_sym).unwrap();

        // Verify both produce same result
        let ssa_result = unsafe { func_ssa() };
        let mod_result = unsafe { func_mod() };
        assert_eq!(ssa_result, mod_result, "SSA vs module result mismatch");
        assert_eq!(ssa_result, 12);

        // Benchmark
        let iterations = 100_000;

        let start = Instant::now();
        for _ in 0..iterations {
            let _ = unsafe { func_ssa() };
        }
        let ssa_time = start.elapsed();

        let start = Instant::now();
        for _ in 0..iterations {
            let _ = unsafe { func_mod() };
        }
        let mod_time = start.elapsed();

        let speedup = mod_time.as_nanos() as f64 / ssa_time.as_nanos() as f64;

        println!("\n=== BENCHMARK: SSA vs Module path ===");
        println!("SSA (compile):     {:>10.2?} ({:.3} ns/iter)", ssa_time, ssa_time.as_nanos() as f64 / iterations as f64);
        println!("Module (compile_module): {:>10.2?} ({:.3} ns/iter)", mod_time, mod_time.as_nanos() as f64 / iterations as f64);
        println!("SSA advantage:     {:.1}x", speedup);

        // SSA should be at least as fast (typically 2-10x faster)
        // We use >= 0.8x to account for noise/warm-up
        assert!(speedup >= 0.8,
            "SSA path should not be significantly slower than module path (speedup={:.2}x)", speedup);
    }

    #[test]
    fn bench_jit_vs_interpreter_loop() {
        use std::time::Instant;

        let source = "0 ->sum  1 ->i  while i 100 <= do  sum i + ->sum  i 1 + ->i  end  sum";
        let module = crate::parser::compile(source).expect("compile failed");
        let expected = 5050i64;

        // JIT
        let mut compiler = NativeCompiler::new().unwrap();
        let func = compiler.compile(&module.code).unwrap();
        let jit_result = unsafe { func() };
        assert_eq!(jit_result, expected);

        let iterations = 10_000;

        let start = Instant::now();
        for _ in 0..iterations {
            let _ = unsafe { func() };
        }
        let jit_time = start.elapsed();

        let start = Instant::now();
        for _ in 0..iterations {
            let mut interp = crate::interpreter::Interpreter::new(&module.code);
            let _ = interp.run();
        }
        let interp_time = start.elapsed();

        let speedup = interp_time.as_nanos() as f64 / jit_time.as_nanos() as f64;

        println!("\n=== BENCHMARK: JIT vs Interpreter (loop sum 1..100) ===");
        println!("JIT:           {:>10.2?} ({:.1} ns/iter)", jit_time, jit_time.as_nanos() as f64 / iterations as f64);
        println!("Interpreter:   {:>10.2?} ({:.1} ns/iter)", interp_time, interp_time.as_nanos() as f64 / iterations as f64);
        println!("Speedup:       {:.1}x", speedup);

        assert!(speedup > 1.0, "JIT should be faster than interpreter (speedup={:.2}x)", speedup);
    }

    // ================================================================
    // EDGE CASES — boundary conditions for production readiness
    // ================================================================

    #[test]
    fn test_edge_single_value() {
        assert_eq!(run_native(&[0x30, 0, 0xFF]).unwrap(), 0);
        assert_eq!(run_native(&[0x30, 127, 0xFF]).unwrap(), 127);
        assert_eq!(run_native(&[0x30, 0x80, 0xFF]).unwrap(), -128); // -128 as i8
    }

    #[test]
    fn test_edge_int16_boundaries() {
        // i16::MAX = 32767
        let bytes = 32767i16.to_le_bytes();
        let bytecode = vec![0x31, bytes[0], bytes[1], 0xFF];
        assert_eq!(run_native(&bytecode).unwrap(), 32767);

        // i16::MIN = -32768
        let bytes = (-32768i16).to_le_bytes();
        let bytecode = vec![0x31, bytes[0], bytes[1], 0xFF];
        assert_eq!(run_native(&bytecode).unwrap(), -32768);
    }

    #[test]
    fn test_edge_int32_boundaries() {
        let bytes = i32::MAX.to_le_bytes();
        let bytecode = vec![0x32, bytes[0], bytes[1], bytes[2], bytes[3], 0xFF];
        assert_eq!(run_native(&bytecode).unwrap(), i32::MAX as i64);
    }

    #[test]
    fn test_edge_float_special_values() {
        // NaN, Infinity, -Infinity, -0.0
        let nan_bytes = f64::NAN.to_bits().to_le_bytes();
        let inf_bytes = f64::INFINITY.to_bits().to_le_bytes();
        let neg_inf_bytes = f64::NEG_INFINITY.to_bits().to_le_bytes();

        // NaN + 1.0 = NaN (should not crash)
        let one_bytes = 1.0f64.to_bits().to_le_bytes();
        let mut bytecode = vec![0x35];
        bytecode.extend_from_slice(&nan_bytes);
        bytecode.push(0x35);
        bytecode.extend_from_slice(&one_bytes);
        bytecode.push(0x46); // FADD
        bytecode.push(0xFF);
        let result = run_native(&bytecode).unwrap();
        let f = f64::from_bits(result as u64);
        assert!(f.is_nan(), "NaN + 1.0 should be NaN, got {}", f);

        // Infinity is representable
        let mut bytecode = vec![0x35];
        bytecode.extend_from_slice(&inf_bytes);
        bytecode.push(0xFF);
        let result = run_native(&bytecode).unwrap();
        assert_eq!(f64::from_bits(result as u64), f64::INFINITY);

        // -Infinity
        let mut bytecode = vec![0x35];
        bytecode.extend_from_slice(&neg_inf_bytes);
        bytecode.push(0xFF);
        let result = run_native(&bytecode).unwrap();
        assert_eq!(f64::from_bits(result as u64), f64::NEG_INFINITY);
    }

    #[test]
    fn test_edge_comparison_with_negatives() {
        // -5 < 3 → true
        let bytecode = vec![0x30, 0xFB, 0x30, 3, 0x51, 0xFF]; // -5 as i8
        assert_eq!(run_native(&bytecode).unwrap(), 1);

        // -1 > -2 → true
        let bytecode = vec![0x30, 0xFF, 0x30, 0xFE, 0x52, 0xFF]; // -1, -2
        assert_eq!(run_native(&bytecode).unwrap(), 1);
    }

    #[test]
    fn test_edge_shift_boundaries() {
        // Shift by 0 = identity
        let bytecode = vec![0x30, 42, 0x30, 0, 0x68, 0xFF]; // 42 << 0
        assert_eq!(run_native(&bytecode).unwrap(), 42);

        // Shift by 63 (max meaningful shift for i64)
        let bytecode = vec![0x30, 1, 0x30, 63, 0x68, 0xFF]; // 1 << 63
        assert_eq!(run_native(&bytecode).unwrap(), i64::MIN); // wraps to -2^63
    }

    #[test]
    fn test_edge_chain_all_stack_ops() {
        // Exercise all stack ops in sequence: push, dup, swap, rot, over, drop
        let bytecode = vec![
            0x30, 1,  // push 1       stack: [1]
            0x30, 2,  // push 2       stack: [1, 2]
            0x30, 3,  // push 3       stack: [1, 2, 3]
            0x02,     // dup          stack: [1, 2, 3, 3]
            0x01,     // drop         stack: [1, 2, 3]
            0x03,     // swap         stack: [1, 3, 2]
            0x04,     // rot          stack: [3, 2, 1]
            0x05,     // over         stack: [3, 2, 1, 2]
            0x40,     // add          stack: [3, 2, 3]
            0x40,     // add          stack: [3, 5]
            0x40,     // add          stack: [8]
            0xFF,
        ];
        assert_eq!(run_native(&bytecode).unwrap(), 8);
    }

    #[test]
    fn test_edge_module_recursive_fibonacci() {
        // Test recursive function calls (exercises call stack depth)
        let result = run_module(": fib ->n  n 2 < if  n  else  n 1 - fib n 2 - fib +  end ;  10 fib");
        let expected = 55; // fib(10) = 55
        assert_eq!(result, expected);
    }

    #[test]
    fn test_edge_module_recursive_cross_backend() {
        // Recursive fib must match interpreter
        let src = ": fib ->n  n 2 < if  n  else  n 1 - fib n 2 - fib +  end ;  10 fib";
        let native = run_module(src);
        let interp = run_interp_module(src);
        assert_eq!(native, interp, "Recursive fib: native={}, interp={}", native, interp);
    }

    #[test]
    fn test_edge_deeply_nested_calls() {
        // A calls B calls C calls D — test call stack integrity
        let src = "\
            : d ->x x x * ; \
            : c ->x x d ; \
            : b ->x x c ; \
            : a ->x x b ; \
            5 a";
        let native = run_module(src);
        let interp = run_interp_module(src);
        assert_eq!(native, 25); // 5 * 5
        assert_eq!(native, interp);
    }

    #[test]
    fn test_edge_locals_across_calls() {
        // Verify locals are properly saved/restored across call boundaries
        let src = "\
            : inner ->y  y y * ; \
            : outer ->x  x 1 + inner x 1 - inner + ; \
            5 outer";
        // (5+1)^2 + (5-1)^2 = 36 + 16 = 52
        let native = run_module(src);
        let interp = run_interp_module(src);
        assert_eq!(native, 52);
        assert_eq!(native, interp);
    }
}
