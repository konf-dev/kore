//! Kore Bytecode Interpreter
//!
//! This is a DETERMINISTIC execution engine.
//! No decisions - just follows the bytecode mechanically.
//!
//! Postulate compliance:
//! - P1: Every value on stack is a tool (can be applied)
//! - P2: APPLY is the only "real" operation
//! - P3: Execution is sequential concatenation
//! - P4: (Checked at compile time by proof checker)

use std::collections::HashMap;

// ============================================================================
// VALUES (P1: Everything is a Tool)
// ============================================================================

/// A Value is anything that can be on the stack.
/// Per P1, every value IS a tool (S → S transformation).
///
/// ## Compact representation (16 bytes)
///
/// Large payloads (String, Vec, BTreeMap) are Box-wrapped so every variant's
/// payload fits in ≤ 8 bytes.  This brings `size_of::<Value>()` down from 56
/// to 16 — a 3.5× reduction — without sacrificing safe Rust, exhaustive
/// pattern matching, or full i64 integer width.
///
/// Inline (no heap alloc): Nil, Bool, Int, Float, Quote, Channel
/// Heap (one alloc):       Str, Pair, Left, Right, List, Fiber, Linear,
///                         Affine, Error, Map, Array
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Unit/Nil — the trivial tool (identity)
    Nil,

    /// Boolean — tool that pushes true/false
    Bool(bool),

    /// Integer — full 64-bit, tool that pushes this number
    Int(i64),

    /// Float — IEEE 754 f64, tool that pushes this number
    Float(f64),

    /// String — tool that pushes this string  (Box: 24 → 8 bytes payload)
    Str(Box<String>),

    /// Pair — product type (a, b)  (single Box: two allocs → one)
    Pair(Box<(Value, Value)>),

    /// Left — sum type left injection
    Left(Box<Value>),

    /// Right — sum type right injection
    Right(Box<Value>),

    /// Quote — deferred computation (the key to P1!)
    /// u32 offset + u32 len = 8 bytes; max 4 GB code
    Quote { offset: u32, len: u32 },

    /// List — heterogeneous sequence  (Box: 24 → 8 bytes payload)
    List(Box<Vec<Value>>),

    /// Fiber — suspended computation (cooperative multitasking)
    Fiber(Box<FiberState>),

    /// Channel — typed communication between fibers/spawn contexts
    /// u32 supports 4 billion channels — practically unlimited
    Channel(u32),

    /// Linear — value that must be used exactly once (no dup, no drop)
    /// P4: linearity is a constraint that attenuates — you can mark
    /// any value as linear, making it more constrained.
    Linear(Box<Value>),

    /// Affine — value that must be used at most once (no dup, drop OK)
    Affine(Box<Value>),

    /// Error — structured error value  (Box: 24 → 8 bytes payload)
    /// P1: errors are tools too
    Error(Box<String>),

    /// Map — associative (string keys → value)  (Box: ~48 → 8 bytes payload)
    Map(Box<std::collections::BTreeMap<String, Value>>),

    /// Array — contiguous homogeneous i64 storage  (NEW — Tool 4)
    /// GPU-friendly, cache-friendly, 8 bytes per element (vs 16 for List).
    /// P1: arrays are tools (S → S), no special treatment.
    Array(Box<Vec<i64>>),
}

// Compile-time guarantee: Value fits in 16 bytes.
// If a variant grows beyond 8 bytes payload, this fails.
const _: () = assert!(std::mem::size_of::<Value>() == 16);

/// State of a fiber (suspended computation)
#[derive(Debug, Clone, PartialEq)]
pub struct FiberState {
    /// Fiber's own stack
    pub stack: Vec<Value>,
    /// Program counter within the quote's bytecode
    pub pc: usize,
    /// The bytecode offset and length of the fiber's code
    pub offset: usize,
    pub len: usize,
    /// Local variable slots
    pub locals: Vec<Value>,
    /// Whether the fiber has finished execution
    pub done: bool,
}

/// State of a channel (inter-fiber communication)
#[derive(Debug, Clone)]
pub struct ChannelState {
    /// Buffered values waiting to be received
    pub buffer: Vec<Value>,
}

impl Value {
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Nil => "nil",
            Value::Bool(_) => "bool",
            Value::Int(_) => "int",
            Value::Float(_) => "float",
            Value::Str(_) => "str",
            Value::Pair(_) => "pair",
            Value::Left(_) => "left",
            Value::Right(_) => "right",
            Value::Quote { .. } => "quote",
            Value::List(_) => "list",
            Value::Fiber(_) => "fiber",
            Value::Channel(_) => "channel",
            Value::Linear(_) => "linear",
            Value::Affine(_) => "affine",
            Value::Error(_) => "error",
            Value::Map(_) => "map",
            Value::Array(_) => "array",
        }
    }
}

// ============================================================================
// STACK (The universal state)
// ============================================================================

/// The Stack is THE state that tools transform.
/// Stack = () | (Value, Stack) - recursive definition
#[derive(Debug, Clone)]
pub struct Stack {
    data: Vec<Value>,
}

impl Stack {
    pub fn new() -> Self {
        Stack { data: Vec::new() }
    }

    pub fn push(&mut self, v: Value) {
        self.data.push(v);
    }

    pub fn pop(&mut self) -> Result<Value, &'static str> {
        self.data.pop().ok_or("stack underflow")
    }

    pub fn peek(&self) -> Result<&Value, &'static str> {
        self.data.last().ok_or("stack underflow")
    }

    #[allow(dead_code)]
    pub fn depth(&self) -> usize {
        self.data.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

// ============================================================================
// INTERPRETER (P2: One Operation - Apply)
// ============================================================================

/// The Interpreter executes bytecode.
/// It has NO decision points - purely deterministic.
pub struct Interpreter<'a> {
    /// The bytecode being executed
    code: &'a [u8],
    
    /// Program counter (current position in code)
    pc: usize,
    
    /// The stack (THE state)
    stack: Stack,
    
    /// Call stack for CALL/RET
    call_stack: Vec<usize>,
    
    /// Symbol table: index → code offset
    symbols: HashMap<u16, usize>,
    
    /// Local variable slots (for STORE/LOAD)
    locals: Vec<Value>,
    
    /// Saved local frames for nested calls
    local_frames: Vec<Vec<Value>>,
    
    /// Allowed capability flags (P4: runtime cap check)
    allowed_caps: u8,
    
    /// Captured output (for print/println in tests)
    output: Vec<String>,
    
    /// Channel table (for inter-fiber communication)
    channels: Vec<ChannelState>,
    
    /// Execution trace (for debugging)
    trace: bool,

    /// Step counter for gas limit (training safety)
    steps: usize,

    /// Maximum steps before execution is halted (0 = unlimited)
    max_steps: usize,

    /// Peak stack depth observed during execution (for training reward)
    max_stack_depth: usize,
}

impl<'a> Interpreter<'a> {
    pub fn new(code: &'a [u8]) -> Self {
        Interpreter {
            code,
            pc: 0,
            stack: Stack::new(),
            call_stack: Vec::new(),
            symbols: HashMap::new(),
            locals: Vec::new(),
            local_frames: Vec::new(),
            allowed_caps: 0xFF, // permissive by default
            output: Vec::new(),
            channels: Vec::new(),
            trace: false,
            steps: 0,
            max_steps: 0,
            max_stack_depth: 0,
        }
    }
    
    /// Create interpreter from a BytecodeModule
    pub fn from_module(module: &'a crate::bytecode::BytecodeModule) -> Self {
        Interpreter {
            code: &module.code,
            pc: 0,
            stack: Stack::new(),
            call_stack: Vec::new(),
            symbols: module.symbol_table.clone(),
            locals: Vec::new(),
            local_frames: Vec::new(),
            allowed_caps: 0xFF, // permissive by default
            output: Vec::new(),
            channels: Vec::new(),
            trace: false,
            steps: 0,
            max_steps: 0,
            max_stack_depth: 0,
        }
    }
    
    /// Create interpreter with specific capability grants
    pub fn from_module_with_caps(module: &'a crate::bytecode::BytecodeModule, allowed_caps: u8) -> Self {
        Interpreter {
            code: &module.code,
            pc: 0,
            stack: Stack::new(),
            call_stack: Vec::new(),
            symbols: module.symbol_table.clone(),
            locals: Vec::new(),
            local_frames: Vec::new(),
            allowed_caps,
            output: Vec::new(),
            channels: Vec::new(),
            trace: false,
            steps: 0,
            max_steps: 0,
            max_stack_depth: 0,
        }
    }

    pub fn enable_trace(&mut self) {
        self.trace = true;
    }

    /// Set maximum step limit (gas). 0 = unlimited.
    pub fn set_max_steps(&mut self, max: usize) {
        self.max_steps = max;
    }

    /// Get total steps executed so far.
    pub fn steps_executed(&self) -> usize {
        self.steps
    }

    /// Get the peak stack depth observed during execution.
    pub fn max_stack_depth(&self) -> usize {
        self.max_stack_depth
    }
    
    /// Get captured output (from print/println)
    pub fn output(&self) -> &[String] {
        &self.output
    }

    /// Run until HALT or error
    pub fn run(&mut self) -> Result<(), String> {
        while self.pc < self.code.len() {
            if self.max_steps > 0 && self.steps >= self.max_steps {
                return Err(format!("step limit exceeded ({} steps)", self.max_steps));
            }
            if self.trace {
                self.print_state();
            }
            
            let op = self.code[self.pc];
            self.pc += 1;
            self.steps += 1;
            
            self.execute_op(op)?;

            // Track peak stack depth (near-zero overhead: one comparison per step)
            let depth = self.stack.data.len();
            if depth > self.max_stack_depth {
                self.max_stack_depth = depth;
            }
            
            if op == 0xFF {
                // HALT
                break;
            }
        }
        Ok(())
    }
    
    /// Run a quoted code block inline, saving/restoring pc.
    /// The quote body ends with RET which returns us.
    /// Uses a sentinel on the call stack to know when the quote returns.
    fn run_quote_inline(&mut self, offset: usize) -> Result<(), String> {
        let saved_pc = self.pc;
        self.call_stack.push(usize::MAX); // sentinel
        self.pc = offset;
        
        loop {
            if self.pc >= self.code.len() {
                self.pc = saved_pc;
                return Err("Quote ran past end of code".into());
            }
            if self.max_steps > 0 && self.steps >= self.max_steps {
                self.pc = saved_pc;
                return Err(format!("step limit exceeded ({} steps)", self.max_steps));
            }
            
            let op = self.code[self.pc];
            self.pc += 1;
            self.steps += 1;
            
            if op == 0x23 { // RET
                match self.call_stack.pop() {
                    Some(usize::MAX) => {
                        // Our sentinel — quote returned
                        break;
                    }
                    Some(addr) => {
                        // Nested call returned — restore locals frame too
                        self.pc = addr;
                        if let Some(saved) = self.local_frames.pop() {
                            self.locals = saved;
                        }
                    }
                    None => {
                        self.pc = saved_pc;
                        return Err("Call stack underflow in quote".into());
                    }
                }
            } else {
                self.execute_op(op)?;
            }
        }
        
        self.pc = saved_pc;
        Ok(())
    }

    fn print_state(&self) {
        print!("{:04X}: ", self.pc);
        print!("stack=[");
        for (i, v) in self.stack.data.iter().enumerate() {
            if i > 0 { print!(", "); }
            self.print_value_short(v);
        }
        println!("]");
    }

    fn print_value_short(&self, v: &Value) {
        match v {
            Value::Nil => print!("nil"),
            Value::Bool(b) => print!("{}", b),
            Value::Int(n) => print!("{}", n),
            Value::Float(f) => print!("{}", f),
            Value::Str(s) => print!("\"{}\"", s),
            Value::Pair(p) => {
                print!("(");
                self.print_value_short(&p.0);
                print!(",");
                self.print_value_short(&p.1);
                print!(")");
            }
            Value::Left(v) => {
                print!("L(");
                self.print_value_short(v);
                print!(")");
            }
            Value::Right(v) => {
                print!("R(");
                self.print_value_short(v);
                print!(")");
            }
            Value::Quote { offset, len } => print!("[{:04X}+{}]", offset, len),
            Value::List(l) => print!("list({})", l.len()),
            Value::Fiber(f) => print!("fiber({})", if f.done { "done" } else { "running" }),
            Value::Channel(id) => print!("chan({})", id),
            Value::Linear(v) => {
                print!("linear(");
                self.print_value_short(v);
                print!(")");
            }
            Value::Affine(v) => {
                print!("affine(");
                self.print_value_short(v);
                print!(")");
            }
            Value::Error(msg) => print!("error({})", msg),
            Value::Map(m) => print!("map({})", m.len()),
            Value::Array(data) => print!("array({})", data.len()),
        }
    }

    /// Execute a single opcode
    /// This is DETERMINISTIC - no decisions, just pattern matching
    fn execute_op(&mut self, op: u8) -> Result<(), String> {
        match op {
            // ================================================================
            // STACK OPERATIONS (0x00-0x0F)
            // ================================================================
            
            // NOP: do nothing
            0x00 => {}
            
            // DROP: (a -- )
            // P4: linear values cannot be dropped (must be consumed)
            0x01 => {
                let v = self.stack.pop()?;
                if matches!(&v, Value::Linear(_)) {
                    return Err("Cannot drop a linear value — use 'consume' first".into());
                }
                // Affine values CAN be dropped (at most once usage)
            }
            
            // DUP: (a -- a a)
            // P4: linear and affine values cannot be duplicated
            0x02 => {
                let a = self.stack.pop()?;
                if matches!(&a, Value::Linear(_)) {
                    return Err("Cannot dup a linear value — must be used exactly once".into());
                }
                if matches!(&a, Value::Affine(_)) {
                    return Err("Cannot dup an affine value — must be used at most once".into());
                }
                self.stack.push(a.clone());
                self.stack.push(a);
            }
            
            // SWAP: (a b -- b a)
            0x03 => {
                let b = self.stack.pop()?;
                let a = self.stack.pop()?;
                self.stack.push(b);
                self.stack.push(a);
            }
            
            // ROT: (a b c -- b c a)
            0x04 => {
                let c = self.stack.pop()?;
                let b = self.stack.pop()?;
                let a = self.stack.pop()?;
                self.stack.push(b);
                self.stack.push(c);
                self.stack.push(a);
            }
            
            // OVER: (a b -- a b a)
            // P4: cannot duplicate linear/affine values
            0x05 => {
                let b = self.stack.pop()?;
                let a = self.stack.pop()?;
                if matches!(&a, Value::Linear(_)) {
                    return Err("Cannot over (duplicate) a linear value — must be used exactly once".into());
                }
                if matches!(&a, Value::Affine(_)) {
                    return Err("Cannot over (duplicate) an affine value — must be used at most once".into());
                }
                self.stack.push(a.clone());
                self.stack.push(b);
                self.stack.push(a);
            }

            // ================================================================
            // DATA OPERATIONS (0x10-0x1F)
            // ================================================================
            
            // PAIR: (a b -- (a,b))
            0x10 => {
                let b = self.stack.pop()?;
                let a = self.stack.pop()?;
                self.stack.push(Value::Pair(Box::new((a, b))));
            }
            
            // UNPAIR: ((a,b) -- a b)
            0x11 => {
                match self.stack.pop()? {
                    Value::Pair(p) => {
                        let (a, b) = *p;
                        self.stack.push(a);
                        self.stack.push(b);
                    }
                    v => return Err(format!("UNPAIR expects pair, got {}", v.type_name())),
                }
            }
            
            // LEFT: (a -- Left(a))
            0x12 => {
                let a = self.stack.pop()?;
                self.stack.push(Value::Left(Box::new(a)));
            }
            
            // RIGHT: (a -- Right(a))
            0x13 => {
                let a = self.stack.pop()?;
                self.stack.push(Value::Right(Box::new(a)));
            }
            
            // CASE: (Sum left_off right_off -- ...)
            // This is P2 in action: the ONLY branching is via sum types
            0x14 => {
                let left_off = self.read_i16()? as i64;
                let right_off = self.read_i16()? as i64;
                
                match self.stack.pop()? {
                    Value::Left(v) => {
                        self.stack.push(*v);
                        self.pc = (self.pc as i64 + left_off) as usize;
                    }
                    Value::Right(v) => {
                        self.stack.push(*v);
                        self.pc = (self.pc as i64 + right_off) as usize;
                    }
                    v => return Err(format!("CASE expects sum type, got {}", v.type_name())),
                }
            }

            // ================================================================
            // CONTROL OPERATIONS (0x20-0x2F)
            // ================================================================
            
            // QUOTE: push following code as value
            0x20 => {
                let len = self.read_u16()? as usize;
                let offset = self.pc;
                self.stack.push(Value::Quote { offset: offset as u32, len: len as u32 });
                self.pc += len; // skip the quoted code
            }
            
            // APPLY: ([code] -- ...) - THIS IS P2!
            0x21 => {
                match self.stack.pop()? {
                    Value::Quote { offset, len: _ } => {
                        // Save return address
                        self.call_stack.push(self.pc);
                        // Save locals frame so the quote body can use its own
                        // locals (or read copies of the caller's) without
                        // clobbering the caller's slots.  RET already restores
                        // from local_frames, mirroring CALL semantics.
                        self.local_frames.push(self.locals.clone());
                        // Jump to quoted code
                        self.pc = offset as usize;
                    }
                    v => return Err(format!("APPLY expects quote, got {}", v.type_name())),
                }
            }
            
            // COND: (bool then_q else_q -- ...) old-style conditional
            // P1: if is a tool, not syntax
            // true  [A] [B] cond  => executes A
            // false [A] [B] cond  => executes B
            0x24 => {
                let else_q = self.stack.pop()?;
                let then_q = self.stack.pop()?;
                let cond = self.pop_bool()?;
                let chosen = if cond { then_q } else { else_q };
                match chosen {
                    Value::Quote { offset, len: _ } => {
                        self.run_quote_inline(offset as usize)?;
                    }
                    v => return Err(format!("COND expects quote, got {}", v.type_name())),
                }
            }

            // LOOP: (body_q -- ...) repeat quote while top-of-stack is true
            // The quote must leave a Bool on top each iteration
            // true = continue looping, false = stop
            // [dup 0 > swap 1 - swap] loop
            0x25 => {
                match self.stack.pop()? {
                    Value::Quote { offset, len: _ } => {
                        loop {
                            self.run_quote_inline(offset as usize)?;
                            let cont = self.pop_bool()?;
                            if !cont { break; }
                        }
                    }
                    v => return Err(format!("LOOP expects quote, got {}", v.type_name())),
                }
            }

            // CALL: call symbol by index
            0x22 => {
                let idx = self.read_u16()?;
                let target = *self.symbols.get(&idx)
                    .ok_or_else(|| format!("unknown symbol: {}", idx))?;
                self.call_stack.push(self.pc);
                // Save current locals frame
                self.local_frames.push(std::mem::take(&mut self.locals));
                self.pc = target;
            }
            
            // RET: return from call
            0x23 => {
                self.pc = self.call_stack.pop()
                    .ok_or("RET without call")?;
                // Restore previous locals frame
                if let Some(saved) = self.local_frames.pop() {
                    self.locals = saved;
                }
            }

            // ================================================================
            // LITERALS (0x30-0x3F)
            // ================================================================
            
            // INT8
            0x30 => {
                let v = self.code[self.pc] as i8 as i64;
                self.pc += 1;
                self.stack.push(Value::Int(v));
            }
            
            // INT16
            0x31 => {
                let v = self.read_i16()? as i64;
                self.stack.push(Value::Int(v));
            }
            
            // INT32
            0x32 => {
                let v = self.read_i32()? as i64;
                self.stack.push(Value::Int(v));
            }
            
            // INT64
            0x33 => {
                let v = self.read_i64()?;
                self.stack.push(Value::Int(v));
            }
            
            // F32
            0x34 => {
                let bits = self.read_u32()?;
                let v = f32::from_bits(bits) as f64;
                self.stack.push(Value::Float(v));
            }
            
            // F64
            0x35 => {
                let bits = self.read_u64()?;
                let v = f64::from_bits(bits);
                self.stack.push(Value::Float(v));
            }
            
            // STR
            0x36 => {
                let len = self.read_u16()? as usize;
                let s = String::from_utf8_lossy(&self.code[self.pc..self.pc + len]).to_string();
                self.pc += len;
                self.stack.push(Value::Str(Box::new(s)));
            }
            
            // NIL
            0x37 => {
                self.stack.push(Value::Nil);
            }
            
            // TRUE
            0x38 => {
                self.stack.push(Value::Bool(true));
            }
            
            // FALSE
            0x39 => {
                self.stack.push(Value::Bool(false));
            }

            // ================================================================
            // ARITHMETIC (0x40-0x4F)
            // ================================================================
            
            // ADD — wrapping for Int+Int (mod 2^64), float promotion otherwise
            0x40 => {
                let b = self.stack.pop()?;
                let a = self.stack.pop()?;
                match (&a, &b) {
                    (Value::Int(x), Value::Int(y)) => {
                        self.stack.push(Value::Int(x.wrapping_add(*y)));
                    }
                    _ => {
                        let a_f = self.value_to_number(&a)?;
                        let b_f = self.value_to_number(&b)?;
                        self.push_number(a_f + b_f);
                    }
                }
            }
            
            // SUB — wrapping for Int-Int (mod 2^64), float promotion otherwise
            0x41 => {
                let b = self.stack.pop()?;
                let a = self.stack.pop()?;
                match (&a, &b) {
                    (Value::Int(x), Value::Int(y)) => {
                        self.stack.push(Value::Int(x.wrapping_sub(*y)));
                    }
                    _ => {
                        let a_f = self.value_to_number(&a)?;
                        let b_f = self.value_to_number(&b)?;
                        self.push_number(a_f - b_f);
                    }
                }
            }
            
            // MUL — wrapping for Int×Int (mod 2^64), float promotion otherwise
            0x42 => {
                let b = self.stack.pop()?;
                let a = self.stack.pop()?;
                match (&a, &b) {
                    (Value::Int(x), Value::Int(y)) => {
                        self.stack.push(Value::Int(x.wrapping_mul(*y)));
                    }
                    _ => {
                        let a_f = self.value_to_number(&a)?;
                        let b_f = self.value_to_number(&b)?;
                        self.push_number(a_f * b_f);
                    }
                }
            }
            
            // DIV - integer division for integers, float division for floats
            // This matches WASM/GPU backend semantics for determinism
            0x43 => {
                let b = self.stack.pop()?;
                let a = self.stack.pop()?;
                match (&a, &b) {
                    (Value::Int(x), Value::Int(y)) => {
                        if *y == 0 {
                            self.stack.push(Value::Error(Box::new("division by zero".into())));
                        } else {
                            self.stack.push(Value::Int(x / y)); // Integer division
                        }
                    }
                    _ => {
                        let a_f = self.value_to_number(&a)?;
                        let b_f = self.value_to_number(&b)?;
                        if b_f == 0.0 {
                            self.stack.push(Value::Error(Box::new("division by zero".into())));
                        } else {
                            self.push_number(a_f / b_f);
                        }
                    }
                }
            }
            
            // MOD
            0x44 => {
                let b = self.pop_int()?;
                let a = self.pop_int()?;
                if b == 0 {
                    self.stack.push(Value::Error(Box::new("modulo by zero".into())));
                } else {
                    self.stack.push(Value::Int(a % b));
                }
            }
            
            // NEG
            0x45 => {
                let a = self.pop_number()?;
                self.push_number(-a);
            }

            // ================================================================
            // FLOAT ARITHMETIC (0x46-0x4E) — strict f64 operations
            // Unlike ADD/SUB/etc. which auto-promote, these require Float.
            // P4: stricter constraint = more precise type.
            // ================================================================

            // FADD: Float Float → Float
            0x46 => {
                let b = self.pop_float()?;
                let a = self.pop_float()?;
                self.stack.push(Value::Float(a + b));
            }
            // FSUB: Float Float → Float
            0x47 => {
                let b = self.pop_float()?;
                let a = self.pop_float()?;
                self.stack.push(Value::Float(a - b));
            }
            // FMUL: Float Float → Float
            0x48 => {
                let b = self.pop_float()?;
                let a = self.pop_float()?;
                self.stack.push(Value::Float(a * b));
            }
            // FDIV: Float Float → Float
            0x49 => {
                let b = self.pop_float()?;
                let a = self.pop_float()?;
                if b == 0.0 {
                    self.stack.push(Value::Error(Box::new("float division by zero".into())));
                } else {
                    self.stack.push(Value::Float(a / b));
                }
            }
            // FNEG: Float → Float
            0x4A => {
                let a = self.pop_float()?;
                self.stack.push(Value::Float(-a));
            }
            // FSQRT: Float → Float
            0x4B => {
                let a = self.pop_float()?;
                self.stack.push(Value::Float(a.sqrt()));
            }
            // FABS: Float → Float
            0x4C => {
                let a = self.pop_float()?;
                self.stack.push(Value::Float(a.abs()));
            }
            // I2F: Int → Float
            0x4D => {
                let a = self.pop_int()?;
                self.stack.push(Value::Float(a as f64));
            }
            // F2I: Float → Int (truncate toward zero)
            0x4E => {
                let a = self.pop_float()?;
                self.stack.push(Value::Int(a as i64));
            }
            // FEXP: Float → Float (e^x)
            0x4F => {
                let a = self.pop_float()?;
                self.stack.push(Value::Float(a.exp()));
            }

            // ================================================================
            // COMPARISON (0x50-0x5F)
            // ================================================================
            
            // EQ
            0x50 => {
                let b = self.stack.pop()?;
                let a = self.stack.pop()?;
                self.stack.push(Value::Bool(self.values_equal(&a, &b)));
            }
            
            // LT
            0x51 => {
                let b = self.pop_number()?;
                let a = self.pop_number()?;
                self.stack.push(Value::Bool(a < b));
            }
            
            // GT
            0x52 => {
                let b = self.pop_number()?;
                let a = self.pop_number()?;
                self.stack.push(Value::Bool(a > b));
            }
            
            // LE
            0x53 => {
                let b = self.pop_number()?;
                let a = self.pop_number()?;
                self.stack.push(Value::Bool(a <= b));
            }
            
            // GE
            0x54 => {
                let b = self.pop_number()?;
                let a = self.pop_number()?;
                self.stack.push(Value::Bool(a >= b));
            }
            
            // NE
            0x55 => {
                let b = self.stack.pop()?;
                let a = self.stack.pop()?;
                self.stack.push(Value::Bool(!self.values_equal(&a, &b)));
            }
            // FLOG: Float → Float (natural logarithm)
            // P1: returns Error on domain violation instead of aborting
            0x56 => {
                let a = self.pop_float()?;
                if a <= 0.0 {
                    self.stack.push(Value::Error(Box::new("flog: argument must be positive".into())));
                } else {
                    self.stack.push(Value::Float(a.ln()));
                }
            }

            // FSIN: Float → Float (sin(x))
            0x57 => {
                let a = self.pop_float()?;
                self.stack.push(Value::Float(a.sin()));
            }
            // FCOS: Float → Float (cos(x))
            0x58 => {
                let a = self.pop_float()?;
                self.stack.push(Value::Float(a.cos()));
            }
            // FATAN2: Float Float → Float (atan2(y, x))
            0x59 => {
                let x = self.pop_float()?;
                let y = self.pop_float()?;
                self.stack.push(Value::Float(y.atan2(x)));
            }
            // FPOW: Float Float → Float (base^exp)
            0x5A => {
                let exp = self.pop_float()?;
                let base = self.pop_float()?;
                self.stack.push(Value::Float(base.powf(exp)));
            }
            // FFLOOR: Float → Float
            0x5B => {
                let a = self.pop_float()?;
                self.stack.push(Value::Float(a.floor()));
            }
            // FCEIL: Float → Float
            0x5C => {
                let a = self.pop_float()?;
                self.stack.push(Value::Float(a.ceil()));
            }
            // FROUND: Float → Float
            0x5D => {
                let a = self.pop_float()?;
                self.stack.push(Value::Float(a.round()));
            }

            // ================================================================
            // LOGIC (0x60-0x6F)
            // ================================================================
            
            // AND: polymorphic — Bool×Bool→Bool or Int×Int→Int
            0x60 => {
                let b = self.stack.pop()?;
                let a = self.stack.pop()?;
                match (&a, &b) {
                    (Value::Bool(x), Value::Bool(y)) => self.stack.push(Value::Bool(*x && *y)),
                    (Value::Int(x), Value::Int(y)) => self.stack.push(Value::Int(x & y)),
                    _ => return Err(format!("AND expects (bool bool) or (int int), got ({} {})", a.type_name(), b.type_name())),
                }
            }
            
            // OR: polymorphic — Bool×Bool→Bool or Int×Int→Int
            0x61 => {
                let b = self.stack.pop()?;
                let a = self.stack.pop()?;
                match (&a, &b) {
                    (Value::Bool(x), Value::Bool(y)) => self.stack.push(Value::Bool(*x || *y)),
                    (Value::Int(x), Value::Int(y)) => self.stack.push(Value::Int(x | y)),
                    _ => return Err(format!("OR expects (bool bool) or (int int), got ({} {})", a.type_name(), b.type_name())),
                }
            }
            
            // NOT: polymorphic — Bool→Bool or Int→Int
            0x62 => {
                let a = self.stack.pop()?;
                match a {
                    Value::Bool(x) => self.stack.push(Value::Bool(!x)),
                    Value::Int(x) => self.stack.push(Value::Int(!x)),
                    _ => return Err(format!("NOT expects bool or int, got {}", a.type_name())),
                }
            }
            
            // XOR: polymorphic — Bool×Bool→Bool or Int×Int→Int
            0x63 => {
                let b = self.stack.pop()?;
                let a = self.stack.pop()?;
                match (&a, &b) {
                    (Value::Bool(x), Value::Bool(y)) => self.stack.push(Value::Bool(*x ^ *y)),
                    (Value::Int(x), Value::Int(y)) => self.stack.push(Value::Int(x ^ y)),
                    _ => return Err(format!("XOR expects (bool bool) or (int int), got ({} {})", a.type_name(), b.type_name())),
                }
            }

            // BAND: (int int -- int) bitwise AND (Int-only alias)
            0x64 => {
                let b = self.pop_int()?;
                let a = self.pop_int()?;
                self.stack.push(Value::Int(a & b));
            }

            // BOR: (int int -- int) bitwise OR (Int-only alias)
            0x65 => {
                let b = self.pop_int()?;
                let a = self.pop_int()?;
                self.stack.push(Value::Int(a | b));
            }

            // BXOR: (int int -- int) bitwise XOR (Int-only alias)
            0x66 => {
                let b = self.pop_int()?;
                let a = self.pop_int()?;
                self.stack.push(Value::Int(a ^ b));
            }

            // BNOT: (int -- int) bitwise NOT
            0x67 => {
                let a = self.pop_int()?;
                self.stack.push(Value::Int(!a));
            }

            // SHL: (int n -- int) shift left
            0x68 => {
                let n = self.pop_int()?;
                let a = self.pop_int()?;
                self.stack.push(Value::Int(a.wrapping_shl(n as u32)));
            }

            // SHR: (int n -- int) arithmetic shift right
            0x69 => {
                let n = self.pop_int()?;
                let a = self.pop_int()?;
                self.stack.push(Value::Int(a.wrapping_shr(n as u32)));
            }

            // ================================================================
            // JUMPS (0x70-0x7F)
            // ================================================================
            
            // JMP: unconditional jump
            0x70 => {
                let offset = self.read_i32()?;
                self.pc = (self.pc as i64 + offset as i64) as usize;
            }
            
            // JZ: jump if zero/false
            0x71 => {
                let offset = self.read_i32()?;
                let cond = self.pop_bool()?;
                if !cond {
                    self.pc = (self.pc as i64 + offset as i64) as usize;
                }
            }
            
            // JNZ: jump if non-zero/true
            0x72 => {
                let offset = self.read_i32()?;
                let cond = self.pop_bool()?;
                if cond {
                    self.pc = (self.pc as i64 + offset as i64) as usize;
                }
            }

            // ================================================================
            // LIST OPERATIONS (0x80-0x8F)
            // ================================================================
            
            // LIST: collect N items into list
            0x80 => {
                let count = self.read_u16()? as usize;
                let mut items = Vec::with_capacity(count);
                for _ in 0..count {
                    items.push(self.stack.pop()?);
                }
                items.reverse();
                self.stack.push(Value::List(Box::new(items)));
            }
            
            // UNLIST: spread list onto stack
            0x81 => {
                match self.stack.pop()? {
                    Value::List(items) => {
                        for item in *items {
                            self.stack.push(item);
                        }
                    }
                    v => return Err(format!("UNLIST expects list, got {}", v.type_name())),
                }
            }
            
            // LEN: get list/string length
            0x82 => {
                match self.stack.peek()? {
                    Value::List(l) => {
                        let len = l.len() as i64;
                        self.stack.push(Value::Int(len));
                    }
                    Value::Str(s) => {
                        let len = s.len() as i64;
                        self.stack.push(Value::Int(len));
                    }
                    v => return Err(format!("LEN expects list/str, got {}", v.type_name())),
                }
            }
            
            // GET: (list i -- elem)
            // P1: returns Error on out-of-bounds instead of aborting
            0x83 => {
                let i = self.pop_int()? as usize;
                match self.stack.pop()? {
                    Value::List(l) => {
                        match l.get(i) {
                            Some(elem) => self.stack.push(elem.clone()),
                            None => self.stack.push(Value::Error(Box::new(format!("index {} out of bounds (len {})", i, l.len())))),
                        }
                    }
                    v => return Err(format!("GET expects list, got {}", v.type_name())),
                }
            }
            
            // SET: (list i v -- list')
            0x84 => {
                let v = self.stack.pop()?;
                let i = self.pop_int()? as usize;
                match self.stack.pop()? {
                    Value::List(mut l) => {
                        if i >= l.len() {
                            return Err("index out of bounds".into());
                        }
                        l[i] = v;
                        self.stack.push(Value::List(l));
                    }
                    val => return Err(format!("SET expects list, got {}", val.type_name())),
                }
            }
            
            // MAP: (list quote -- list')
            // The functor: f:A→B lifts to map(f):[A]→[B]
            // For each element: push element, execute quote, collect result
            0x85 => {
                let quote = self.stack.pop()?;
                let list = self.stack.pop()?;
                match (list, quote) {
                    (Value::List(items), Value::Quote { offset, len: _ }) => {
                        let mut results = Vec::with_capacity(items.len());
                        
                        for item in *items {
                            self.stack.push(item);
                            self.run_quote_inline(offset as usize)?;
                            results.push(self.stack.pop()?);
                        }
                        
                        self.stack.push(Value::List(Box::new(results)));
                    }
                    (Value::List(_), v) => return Err(format!("MAP expects quote, got {}", v.type_name())),
                    (v, _) => return Err(format!("MAP expects list, got {}", v.type_name())),
                }
            }
            
            // FOLD: (list init quote -- result)
            // The catamorphism: (f:B×A→B, b₀:B) → [A] → B
            // Quote expects (acc elem -- acc') on stack
            0x86 => {
                let quote = self.stack.pop()?;
                let init = self.stack.pop()?;
                let list = self.stack.pop()?;
                match (list, quote) {
                    (Value::List(items), Value::Quote { offset, len: _ }) => {
                        let mut acc = init;
                        
                        for item in *items {
                            self.stack.push(acc);
                            self.stack.push(item);
                            self.run_quote_inline(offset as usize)?;
                            acc = self.stack.pop()?;
                        }
                        
                        self.stack.push(acc);
                    }
                    (Value::List(_), v) => return Err(format!("FOLD expects quote, got {}", v.type_name())),
                    (v, _) => return Err(format!("FOLD expects list, got {}", v.type_name())),
                }
            }
            
            // ZIP: (list list -- list-of-pairs)
            // Product lifting: [A]×[B] → [(A,B)]
            // Pure data operation — no quote execution needed
            0x87 => {
                let b = self.stack.pop()?;
                let a = self.stack.pop()?;
                match (a, b) {
                    (Value::List(as_), Value::List(bs)) => {
                        let len = as_.len().min(bs.len());
                        let mut pairs = Vec::with_capacity(len);
                        for (a, b) in as_.into_iter().zip(bs.into_iter()) {
                            pairs.push(Value::Pair(Box::new((a, b))));
                        }
                        self.stack.push(Value::List(Box::new(pairs)));
                    }
                    (Value::List(_), v) => return Err(format!("ZIP expects list, got {}", v.type_name())),
                    (v, _) => return Err(format!("ZIP expects list, got {}", v.type_name())),
                }
            }

            // APPEND: (list value -- list')
            // Appends a value to the end of a list
            0x88 => {
                let val = self.stack.pop()?;
                let list = self.stack.pop()?;
                match list {
                    Value::List(mut items) => {
                        items.push(val);
                        self.stack.push(Value::List(items));
                    }
                    v => return Err(format!("APPEND expects list, got {}", v.type_name())),
                }
            }

            // REVERSE: (list -- list')
            // Reverses a list
            0x89 => {
                let list = self.stack.pop()?;
                match list {
                    Value::List(mut items) => {
                        items.reverse();
                        self.stack.push(Value::List(items));
                    }
                    v => return Err(format!("REVERSE expects list, got {}", v.type_name())),
                }
            }

            // ================================================================
            // FIBER OPERATIONS (0xB0-0xB5) — Cooperative multitasking
            // ================================================================

            // FIBER_NEW: (quote -- fiber)
            // Create a new fiber from a quote
            0xB0 => {
                let quote = self.stack.pop()?;
                match quote {
                    Value::Quote { offset, len } => {
                        let fiber = FiberState {
                            stack: Vec::new(),
                            pc: offset as usize,
                            offset: offset as usize,
                            len: len as usize,
                            locals: Vec::new(),
                            done: false,
                        };
                        self.stack.push(Value::Fiber(Box::new(fiber)));
                    }
                    v => return Err(format!("FIBER_NEW expects quote, got {}", v.type_name())),
                }
            }

            // FIBER_STEP: (fiber -- fiber' done?)
            // Execute one instruction of the fiber, return whether it's done
            0xB1 => {
                let val = self.stack.pop()?;
                match val {
                    Value::Fiber(mut fiber) => {
                        if fiber.done {
                            self.stack.push(Value::Fiber(fiber));
                            self.stack.push(Value::Bool(true));
                        } else {
                            // Execute one instruction of the fiber
                            let end = fiber.offset + fiber.len;
                            if fiber.pc >= end {
                                fiber.done = true;
                                self.stack.push(Value::Fiber(fiber));
                                self.stack.push(Value::Bool(true));
                            } else {
                                // Execute a single opcode in the fiber's context
                                let op = self.code[fiber.pc];
                                fiber.pc += 1;
                                
                                // RET (0x23) and HALT (0xFF) mark fiber as done
                                if op == 0x23 || op == 0xFF {
                                    fiber.done = true;
                                    self.stack.push(Value::Fiber(fiber));
                                    self.stack.push(Value::Bool(true));
                                } else {
                                    // Swap fiber's state into interpreter
                                    let saved_stack = std::mem::replace(&mut self.stack.data, fiber.stack.clone());
                                    let saved_pc = self.pc;
                                    let saved_locals = std::mem::replace(&mut self.locals, fiber.locals.clone());
                                    let saved_call_stack = std::mem::replace(&mut self.call_stack, Vec::new());
                                    
                                    self.pc = fiber.pc;
                                    
                                    let result = self.execute_op(op);
                                    
                                    // Save fiber state back
                                    fiber.stack = std::mem::replace(&mut self.stack.data, saved_stack);
                                    fiber.pc = self.pc;
                                    fiber.locals = std::mem::replace(&mut self.locals, saved_locals);
                                    self.call_stack = saved_call_stack;
                                    self.pc = saved_pc;
                                    
                                    if fiber.pc >= end {
                                        fiber.done = true;
                                    }
                                    
                                    match result {
                                        Ok(_) => {
                                            let is_done = fiber.done;
                                            self.stack.push(Value::Fiber(fiber));
                                            self.stack.push(Value::Bool(is_done));
                                        }
                                        Err(e) => return Err(format!("Fiber error: {}", e)),
                                    }
                                }
                            }
                        }
                    }
                    v => return Err(format!("FIBER_STEP expects fiber, got {}", v.type_name())),
                }
            }

            // FIBER_PUSH: (fiber value -- fiber')
            // Push a value onto the fiber's stack
            0xB2 => {
                let val = self.stack.pop()?;
                let fib = self.stack.pop()?;
                match fib {
                    Value::Fiber(mut fiber) => {
                        fiber.stack.push(val);
                        self.stack.push(Value::Fiber(fiber));
                    }
                    v => return Err(format!("FIBER_PUSH expects fiber, got {}", v.type_name())),
                }
            }

            // FIBER_STACK: (fiber -- fiber stack)
            // Get the fiber's stack as a list (non-destructive)
            0xB3 => {
                let fib = self.stack.pop()?;
                match fib {
                    Value::Fiber(fiber) => {
                        let stack_list = Value::List(Box::new(fiber.stack.clone()));
                        self.stack.push(Value::Fiber(fiber));
                        self.stack.push(stack_list);
                    }
                    v => return Err(format!("FIBER_STACK expects fiber, got {}", v.type_name())),
                }
            }

            // FIBER_STATUS: (fiber -- fiber done?)
            // Check if the fiber has finished execution
            0xB4 => {
                let fib = self.stack.pop()?;
                match fib {
                    Value::Fiber(fiber) => {
                        let done = fiber.done;
                        self.stack.push(Value::Fiber(fiber));
                        self.stack.push(Value::Bool(done));
                    }
                    v => return Err(format!("FIBER_STATUS expects fiber, got {}", v.type_name())),
                }
            }

            // ================================================================
            // SPAWN (0xB5) — Sandboxed parallel execution (P4: cap attenuation)
            // (quote caps -- result-list)
            //   quote: code to execute in sandboxed context
            //   caps:  Int bitmask of requested capabilities (≤ parent)
            //   result: list of values left on child's stack
            // The child gets attenuated caps = parent_caps & requested_caps
            // ================================================================
            0xB5 => {
                let caps_val = self.stack.pop()?;
                let quote_val = self.stack.pop()?;
                
                let requested_caps = match caps_val {
                    Value::Int(c) => c as u8,
                    v => return Err(format!("SPAWN expects int caps, got {}", v.type_name())),
                };
                
                match quote_val {
                    Value::Quote { offset, len } => {
                        // P4: attenuate — child can never have more caps than parent
                        let child_caps = self.allowed_caps & requested_caps;
                        
                        // Create a child interpreter sharing the same bytecode
                        let child_code = self.code;
                        let mut child = Interpreter::new(child_code);
                        child.symbols = self.symbols.clone();
                        child.allowed_caps = child_caps;
                        child.channels = self.channels.clone();
                        child.pc = offset as usize;
                        
                        // Run the child to completion (within the quote's bounds)
                        // Track call depth so nested function RETs don't kill the spawn
                        let end = offset as usize + len as usize;
                        let initial_call_depth = child.call_stack.len();
                        child.max_steps = self.max_steps;
                        child.steps = self.steps; // inherit parent's step budget
                        loop {
                            // Gas limit check for spawn
                            if child.max_steps > 0 && child.steps >= child.max_steps {
                                self.steps = child.steps; // sync back
                                return Err(format!("step limit exceeded ({} steps)", child.max_steps));
                            }
                            child.steps += 1;
                            // PC can go outside quote bounds during function calls
                            // (call jumps to function body, ret jumps back)
                            let op = child.code[child.pc];
                            child.pc += 1;
                            if op == 0xFF { break; } // HALT always stops
                            // Only break on RET if we're back at the top level
                            // (not inside a nested function call from the quote)
                            if op == 0x23 {
                                if child.call_stack.len() <= initial_call_depth {
                                    // Top-level RET: just stop, don't try to pop empty call stack
                                    break;
                                }
                            }
                            child.execute_op(op)?;
                            // Check if we've returned past the quote's end without pending calls
                            if child.pc >= end && child.call_stack.len() <= initial_call_depth {
                                break;
                            }
                        }
                        
                        // Merge channel state back (channels are shared mutable state)
                        self.channels = child.channels;
                        self.steps = child.steps; // sync step count back to parent
                        
                        // Return child's stack as a list
                        let result = Value::List(Box::new(child.stack.data));
                        self.stack.push(result);
                    }
                    v => return Err(format!("SPAWN expects quote, got {}", v.type_name())),
                }
            }
            
            // ================================================================
            // CHANNELS (0xB6-0xB8) — Inter-fiber/spawn communication
            // ================================================================
            
            // CHAN_NEW: (-- chan) Create a new channel
            0xB6 => {
                let id = self.channels.len() as u32;
                self.channels.push(ChannelState { buffer: Vec::new() });
                self.stack.push(Value::Channel(id));
            }
            
            // CHAN_SEND: (chan value --) Send value to channel
            0xB7 => {
                let val = self.stack.pop()?;
                let chan = self.stack.pop()?;
                match chan {
                    Value::Channel(id) => {
                        if (id as usize) >= self.channels.len() {
                            return Err(format!("chan-send: invalid channel id {}", id));
                        }
                        self.channels[id as usize].buffer.push(val);
                    }
                    v => return Err(format!("CHAN_SEND expects channel, got {}", v.type_name())),
                }
            }
            
            // CHAN_RECV: (chan -- value) Receive value from channel
            // P1: returns Error on empty instead of aborting
            0xB8 => {
                let chan = self.stack.pop()?;
                match chan {
                    Value::Channel(id) => {
                        if (id as usize) >= self.channels.len() {
                            self.stack.push(Value::Error(Box::new(format!("chan-recv: invalid channel id {}", id))));
                        } else if self.channels[id as usize].buffer.is_empty() {
                            self.stack.push(Value::Error(Box::new("chan-recv: channel is empty".into())));
                        } else {
                            let val = self.channels[id as usize].buffer.remove(0);
                            self.stack.push(val);
                        }
                    }
                    v => return Err(format!("CHAN_RECV expects channel, got {}", v.type_name())),
                }
            }

            // ================================================================
            // ERROR HANDLING (0xA3-0xA5) — P4: errors are values, not panics
            // Errors are first-class values. Programs can catch and inspect them.
            // ================================================================

            // TRY: (quote -- result|error) Execute quote; if error, push Error value
            0xA3 => {
                match self.stack.pop()? {
                    Value::Quote { offset, len: _ } => {
                        // Save state for recovery
                        let saved_stack_len = self.stack.data.len();
                        let saved_call_stack_len = self.call_stack.len();
                        let saved_pc = self.pc;
                        
                        match self.run_quote_inline(offset as usize) {
                            Ok(()) => {
                                // Quote succeeded — result is on stack
                            }
                            Err(msg) => {
                                // Quote failed — restore state and push Error
                                self.stack.data.truncate(saved_stack_len);
                                self.call_stack.truncate(saved_call_stack_len);
                                self.pc = saved_pc;
                                self.stack.push(Value::Error(Box::new(msg)));
                            }
                        }
                    }
                    v => return Err(format!("TRY expects quote, got {}", v.type_name())),
                }
            }

            // FAIL: (str -- !) Create an error (unwinds to nearest try)
            0xA4 => {
                match self.stack.pop()? {
                    Value::Str(msg) => {
                        return Err(*msg);
                    }
                    v => return Err(format!("FAIL expects string message, got {}", v.type_name())),
                }
            }

            // IS_ERROR: (a -- a bool) Check if top of stack is an Error
            0xA5 => {
                let a = self.stack.pop()?;
                let is_err = matches!(&a, Value::Error(_));
                self.stack.push(a);
                self.stack.push(Value::Bool(is_err));
            }

            // PROPAGATE: (a -- a) If Error, re-fail (unwind to nearest try); else pass through
            // This is Kore's equivalent of Rust's `?` operator.
            // P1-compliant: it's a total tool (S → S) because it always transforms the stack —
            // either it passes the value through, or it converts Error back to a fail (which
            // try will catch as a value). The unwinding is *structured*, not S → ⊥.
            0xA6 => {
                let a = self.stack.pop()?;
                match a {
                    Value::Error(msg) => {
                        return Err(*msg);
                    }
                    other => {
                        self.stack.push(other);
                    }
                }
            }

            // MAKE-ERROR: (str -- error) Create Error value without unwinding (P1: S→S)
            0xA7 => {
                let a = self.stack.pop()?;
                let msg = match a {
                    Value::Str(s) => *s,
                    other => format!("{}", self.format_value(&other)),
                };
                self.stack.push(Value::Error(Box::new(msg)));
            }

            // ================================================================
            // IO OPERATIONS (0xA0-0xA7) — P4: capability-gated
            // ================================================================
            
            // PRINT: (value --) Write to stdout
            0xA0 => {
                if self.allowed_caps & crate::bytecode::CAP_IO == 0 {
                    return Err("PRINT denied: 'io' capability not granted".into());
                }
                let val = self.stack.pop()?;
                let s = self.format_value(&val);
                print!("{}", s);
                self.output.push(s);
            }
            
            // PRINTLN: (value --) Write to stdout + newline
            0xA1 => {
                if self.allowed_caps & crate::bytecode::CAP_IO == 0 {
                    return Err("PRINTLN denied: 'io' capability not granted".into());
                }
                let val = self.stack.pop()?;
                let s = self.format_value(&val);
                println!("{}", s);
                self.output.push(s);
            }
            
            // RAND: (-- n) Random u64 from OS entropy
            0xA2 => {
                if self.allowed_caps & crate::bytecode::CAP_IO == 0 {
                    return Err("RAND denied: 'io' capability not granted".into());
                }
                // Use std::time for a simple entropy source (no external dep)
                // XOR of time nanos with address for decent randomness
                let t = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64;
                // Mix bits
                let r = t ^ (t >> 17) ^ (t.wrapping_mul(0x2545F4914F6CDD1D));
                self.stack.push(Value::Int(r as i64));
            }

            // ================================================================
            // LOCAL VARIABLES (0xA8-0xA9)
            // ================================================================
            
            // STORE: (value --) Store to local slot N
            0xA8 => {
                let slot = u32::from_le_bytes([
                    self.code[self.pc], self.code[self.pc+1],
                    self.code[self.pc+2], self.code[self.pc+3],
                ]) as usize;
                self.pc += 4;
                let val = self.stack.pop()?;
                // Grow locals vec if needed
                while self.locals.len() <= slot {
                    self.locals.push(Value::Nil);
                }
                self.locals[slot] = val;
            }
            
            // LOAD: (-- value) Load from local slot N
            0xA9 => {
                let slot = u32::from_le_bytes([
                    self.code[self.pc], self.code[self.pc+1],
                    self.code[self.pc+2], self.code[self.pc+3],
                ]) as usize;
                self.pc += 4;
                if slot >= self.locals.len() {
                    return Err(format!("load: local slot {} not initialized", slot));
                }
                self.stack.push(self.locals[slot].clone());
            }

            // ================================================================
            // HASHMAP (0xAA-0xAE) — Associative data structure
            // P1: maps are tools (key→value lookup is S→S)
            // ================================================================

            // MAP_NEW: (-- map) Create empty map
            0xAA => {
                self.stack.push(Value::Map(Box::new(std::collections::BTreeMap::new())));
            }

            // MAP_GET: (map key -- map value) Get value by key (non-destructive on map)
            0xAB => {
                let key = self.stack.pop()?;
                let map = self.stack.pop()?;
                match (&map, &key) {
                    (Value::Map(m), Value::Str(k)) => {
                        let val = m.get(k.as_str()).cloned().unwrap_or(Value::Nil);
                        self.stack.push(map);
                        self.stack.push(val);
                    }
                    (Value::Map(_), v) => return Err(format!("MAP_GET key must be string, got {}", v.type_name())),
                    (v, _) => return Err(format!("MAP_GET expects map, got {}", v.type_name())),
                }
            }

            // MAP_SET: (map key value -- map) Set key-value pair
            0xAC => {
                let val = self.stack.pop()?;
                let key = self.stack.pop()?;
                let map = self.stack.pop()?;
                match (map, key) {
                    (Value::Map(mut m), Value::Str(k)) => {
                        m.insert(*k, val);
                        self.stack.push(Value::Map(m));
                    }
                    (Value::Map(_), v) => return Err(format!("MAP_SET key must be string, got {}", v.type_name())),
                    (v, _) => return Err(format!("MAP_SET expects map, got {}", v.type_name())),
                }
            }

            // MAP_KEYS: (map -- map list) Get all keys as list of strings
            0xAD => {
                let map = self.stack.pop()?;
                match &map {
                    Value::Map(m) => {
                        let keys: Vec<Value> = m.keys().map(|k| Value::Str(Box::new(k.clone()))).collect();
                        self.stack.push(map);
                        self.stack.push(Value::List(Box::new(keys)));
                    }
                    v => return Err(format!("MAP_KEYS expects map, got {}", v.type_name())),
                }
            }

            // MAP_HAS: (map key -- map bool) Check if key exists
            0xAE => {
                let key = self.stack.pop()?;
                let map = self.stack.pop()?;
                match (&map, &key) {
                    (Value::Map(m), Value::Str(k)) => {
                        let has = m.contains_key(k.as_str());
                        self.stack.push(map);
                        self.stack.push(Value::Bool(has));
                    }
                    (Value::Map(_), v) => return Err(format!("MAP_HAS key must be string, got {}", v.type_name())),
                    (v, _) => return Err(format!("MAP_HAS expects map, got {}", v.type_name())),
                }
            }

            // ================================================================
            // SYSCALL (0xAF) — Generalized host calls
            // Each call_id requires specific capabilities.
            // P4: capabilities attenuate — must be granted at runtime.
            // ================================================================
            0xAF => {
                let call_id = self.code[self.pc];
                self.pc += 1;
                match call_id {
                    // 0x01: file-read (path -- str) — cap:fs
                    0x01 => {
                        if self.allowed_caps & crate::bytecode::CAP_FS == 0 {
                            return Err("SYSCALL file-read: requires 'fs' capability (--allow fs)".into());
                        }
                        match self.stack.pop()? {
                            Value::Str(path) => {
                                match std::fs::read_to_string(path.as_str()) {
                                    Ok(contents) => self.stack.push(Value::Str(Box::new(contents))),
                                    Err(e) => return Err(format!("file-read '{}': {}", path, e)),
                                }
                            }
                            v => return Err(format!("file-read expects string path, got {}", v.type_name())),
                        }
                    }
                    // 0x02: file-write (path content --) — cap:fs
                    0x02 => {
                        if self.allowed_caps & crate::bytecode::CAP_FS == 0 {
                            return Err("SYSCALL file-write: requires 'fs' capability (--allow fs)".into());
                        }
                        let content = self.stack.pop()?;
                        let path = self.stack.pop()?;
                        match (path, content) {
                            (Value::Str(p), Value::Str(c)) => {
                                if let Err(e) = std::fs::write(p.as_str(), c.as_str()) {
                                    return Err(format!("file-write '{}': {}", p, e));
                                }
                            }
                            (p, c) => return Err(format!("file-write expects (str str), got ({} {})", p.type_name(), c.type_name())),
                        }
                    }
                    // 0x03: file-exists (path -- bool) — cap:fs
                    0x03 => {
                        if self.allowed_caps & crate::bytecode::CAP_FS == 0 {
                            return Err("SYSCALL file-exists: requires 'fs' capability (--allow fs)".into());
                        }
                        match self.stack.pop()? {
                            Value::Str(path) => {
                                let exists = std::path::Path::new(path.as_str()).exists();
                                self.stack.push(Value::Bool(exists));
                            }
                            v => return Err(format!("file-exists expects string path, got {}", v.type_name())),
                        }
                    }
                    // 0x10: time-now (-- float) — cap:io
                    0x10 => {
                        if self.allowed_caps & crate::bytecode::CAP_IO == 0 {
                            return Err("SYSCALL time-now: requires 'io' capability (--allow io)".into());
                        }
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs_f64();
                        self.stack.push(Value::Float(now));
                    }
                    // 0x20: env-get (name -- str) — cap:io
                    0x20 => {
                        if self.allowed_caps & crate::bytecode::CAP_IO == 0 {
                            return Err("SYSCALL env-get: requires 'io' capability (--allow io)".into());
                        }
                        match self.stack.pop()? {
                            Value::Str(name) => {
                                let val = std::env::var(name.as_str()).unwrap_or_default();
                                self.stack.push(Value::Str(Box::new(val)));
                            }
                            v => return Err(format!("env-get expects string name, got {}", v.type_name())),
                        }
                    }
                    // 0x30: exec (cmd -- (stdout, exit-code)) — cap:exec
                    0x30 => {
                        if self.allowed_caps & crate::bytecode::CAP_EXEC == 0 {
                            return Err("SYSCALL exec: requires 'exec' capability (--allow exec)".into());
                        }
                        match self.stack.pop()? {
                            Value::Str(cmd) => {
                                match std::process::Command::new("sh")
                                    .arg("-c")
                                    .arg(cmd.as_str())
                                    .output()
                                {
                                    Ok(output) => {
                                        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                                        let exit_code = output.status.code().unwrap_or(-1) as i64;
                                        self.stack.push(Value::Pair(Box::new((
                                            Value::Str(Box::new(stdout)),
                                            Value::Int(exit_code),
                                        ))));
                                    }
                                    Err(e) => return Err(format!("exec '{}': {}", cmd, e)),
                                }
                            }
                            v => return Err(format!("exec expects string command, got {}", v.type_name())),
                        }
                    }
                    // 0x04: readline (prompt -- str) — cap:io — read line from stdin
                    0x04 => {
                        if self.allowed_caps & crate::bytecode::CAP_IO == 0 {
                            return Err("SYSCALL readline: requires 'io' capability (--allow io)".into());
                        }
                        match self.stack.pop()? {
                            Value::Str(prompt) => {
                                use std::io::Write;
                                print!("{}", prompt);
                                std::io::stdout().flush().unwrap_or(());
                                let mut line = String::new();
                                match std::io::stdin().read_line(&mut line) {
                                    Ok(_) => {
                                        // Strip trailing newline
                                        if line.ends_with('\n') { line.pop(); }
                                        if line.ends_with('\r') { line.pop(); }
                                        self.stack.push(Value::Str(Box::new(line)));
                                    }
                                    Err(e) => return Err(format!("readline: {}", e)),
                                }
                            }
                            v => return Err(format!("readline expects string prompt, got {}", v.type_name())),
                        }
                    }
                    // 0x05: file-append (path content --) — cap:fs
                    0x05 => {
                        if self.allowed_caps & crate::bytecode::CAP_FS == 0 {
                            return Err("SYSCALL file-append: requires 'fs' capability (--allow fs)".into());
                        }
                        let content = self.stack.pop()?;
                        let path = self.stack.pop()?;
                        match (path, content) {
                            (Value::Str(p), Value::Str(c)) => {
                                use std::io::Write;
                                match std::fs::OpenOptions::new().create(true).append(true).open(p.as_str()) {
                                    Ok(mut f) => {
                                        if let Err(e) = f.write_all(c.as_bytes()) {
                                            return Err(format!("file-append '{}': {}", p, e));
                                        }
                                    }
                                    Err(e) => return Err(format!("file-append '{}': {}", p, e)),
                                }
                            }
                            (p, c) => return Err(format!("file-append expects (str str), got ({} {})", p.type_name(), c.type_name())),
                        }
                    }
                    // 0x06: file-delete (path -- bool) — cap:fs
                    0x06 => {
                        if self.allowed_caps & crate::bytecode::CAP_FS == 0 {
                            return Err("SYSCALL file-delete: requires 'fs' capability (--allow fs)".into());
                        }
                        match self.stack.pop()? {
                            Value::Str(path) => {
                                let ok = std::fs::remove_file(path.as_str()).is_ok();
                                self.stack.push(Value::Bool(ok));
                            }
                            v => return Err(format!("file-delete expects string path, got {}", v.type_name())),
                        }
                    }
                    // 0x07: file-list (path -- list) — cap:fs — list directory entries
                    0x07 => {
                        if self.allowed_caps & crate::bytecode::CAP_FS == 0 {
                            return Err("SYSCALL file-list: requires 'fs' capability (--allow fs)".into());
                        }
                        match self.stack.pop()? {
                            Value::Str(path) => {
                                match std::fs::read_dir(path.as_str()) {
                                    Ok(entries) => {
                                        let mut items = Vec::new();
                                        for entry in entries {
                                            if let Ok(e) = entry {
                                                items.push(Value::Str(Box::new(e.file_name().to_string_lossy().to_string())));
                                            }
                                        }
                                        self.stack.push(Value::List(Box::new(items)));
                                    }
                                    Err(e) => return Err(format!("file-list '{}': {}", path, e)),
                                }
                            }
                            v => return Err(format!("file-list expects string path, got {}", v.type_name())),
                        }
                    }
                    // 0x40: http-get (url -- (body, status)) — cap:net
                    0x40 => {
                        if self.allowed_caps & crate::bytecode::CAP_NET == 0 {
                            return Err("SYSCALL http-get: requires 'net' capability (--allow net)".into());
                        }
                        match self.stack.pop()? {
                            Value::Str(url) => {
                                // Use curl via exec for HTTP (no external deps)
                                match std::process::Command::new("curl")
                                    .arg("-sS")
                                    .arg("-o").arg("-")
                                    .arg("-w").arg("\n%{http_code}")
                                    .arg(url.as_str())
                                    .output()
                                {
                                    Ok(output) => {
                                        let full = String::from_utf8_lossy(&output.stdout).to_string();
                                        // Last line is status code
                                        let (body, status) = if let Some(pos) = full.rfind('\n') {
                                            let code: i64 = full[pos+1..].trim().parse().unwrap_or(0);
                                            (full[..pos].to_string(), code)
                                        } else {
                                            (full, 0)
                                        };
                                        self.stack.push(Value::Pair(Box::new((
                                            Value::Str(Box::new(body)),
                                            Value::Int(status),
                                        ))));
                                    }
                                    Err(e) => return Err(format!("http-get '{}': {}", url, e)),
                                }
                            }
                            v => return Err(format!("http-get expects string URL, got {}", v.type_name())),
                        }
                    }
                    // 0x41: http-post (url body content-type -- (response, status)) — cap:net
                    0x41 => {
                        if self.allowed_caps & crate::bytecode::CAP_NET == 0 {
                            return Err("SYSCALL http-post: requires 'net' capability (--allow net)".into());
                        }
                        let ct = self.stack.pop()?;
                        let body = self.stack.pop()?;
                        let url = self.stack.pop()?;
                        match (url, body, ct) {
                            (Value::Str(u), Value::Str(b), Value::Str(c)) => {
                                match std::process::Command::new("curl")
                                    .arg("-sS")
                                    .arg("-X").arg("POST")
                                    .arg("-H").arg(format!("Content-Type: {}", c))
                                    .arg("-d").arg(b.as_str())
                                    .arg("-o").arg("-")
                                    .arg("-w").arg("\n%{http_code}")
                                    .arg(u.as_str())
                                    .output()
                                {
                                    Ok(output) => {
                                        let full = String::from_utf8_lossy(&output.stdout).to_string();
                                        let (resp, status) = if let Some(pos) = full.rfind('\n') {
                                            let code: i64 = full[pos+1..].trim().parse().unwrap_or(0);
                                            (full[..pos].to_string(), code)
                                        } else {
                                            (full, 0)
                                        };
                                        self.stack.push(Value::Pair(Box::new((
                                            Value::Str(Box::new(resp)),
                                            Value::Int(status),
                                        ))));
                                    }
                                    Err(e) => return Err(format!("http-post '{}': {}", u, e)),
                                }
                            }
                            (u, b, c) => return Err(format!("http-post expects (str str str), got ({} {} {})", u.type_name(), b.type_name(), c.type_name())),
                        }
                    }
                    // 0x50: http-serve (port handler-quote -- ) — cap:net — start HTTP server
                    // The handler quote receives (method path body) on stack and must leave (status body) 
                    0x50 => {
                        if self.allowed_caps & crate::bytecode::CAP_NET == 0 {
                            return Err("SYSCALL http-serve: requires 'net' capability (--allow net)".into());
                        }
                        let handler = self.stack.pop()?;
                        let port_val = self.stack.pop()?;
                        match (port_val, handler) {
                            (Value::Int(port), Value::Quote { offset, len: _ }) => {
                                use std::io::{Read, Write, BufRead, BufReader};
                                let listener = match std::net::TcpListener::bind(format!("0.0.0.0:{}", port)) {
                                    Ok(l) => l,
                                    Err(e) => return Err(format!("http-serve: bind failed on port {}: {}", port, e)),
                                };
                                println!("HTTP server listening on port {}", port);
                                
                                for stream in listener.incoming() {
                                    match stream {
                                        Ok(mut stream) => {
                                            // Parse HTTP request
                                            let mut reader = BufReader::new(stream.try_clone().unwrap());
                                            let mut request_line = String::new();
                                            if reader.read_line(&mut request_line).is_err() { continue; }
                                            
                                            let parts: Vec<&str> = request_line.trim().split_whitespace().collect();
                                            let (method, path) = if parts.len() >= 2 {
                                                (parts[0].to_string(), parts[1].to_string())
                                            } else {
                                                continue;
                                            };
                                            
                                            // Read headers
                                            let mut content_length: usize = 0;
                                            loop {
                                                let mut header = String::new();
                                                if reader.read_line(&mut header).is_err() { break; }
                                                let header = header.trim().to_string();
                                                if header.is_empty() { break; }
                                                if header.to_lowercase().starts_with("content-length:") {
                                                    content_length = header[15..].trim().parse().unwrap_or(0);
                                                }
                                            }
                                            
                                            // Read body
                                            let body = if content_length > 0 {
                                                let mut buf = vec![0u8; content_length];
                                                let _ = reader.read_exact(&mut buf);
                                                String::from_utf8_lossy(&buf).to_string()
                                            } else {
                                                String::new()
                                            };
                                            
                                            // Push request data onto stack and run handler
                                            self.stack.push(Value::Str(Box::new(method)));
                                            self.stack.push(Value::Str(Box::new(path)));
                                            self.stack.push(Value::Str(Box::new(body)));
                                            
                                            match self.run_quote_inline(offset as usize) {
                                                Ok(()) => {
                                                    // Handler should leave (status_code, response_body) on stack
                                                    let resp_body = match self.stack.pop() {
                                                        Ok(Value::Str(s)) => *s,
                                                        Ok(v) => self.format_value(&v),
                                                        Err(_) => "Internal Server Error".to_string(),
                                                    };
                                                    let status = match self.stack.pop() {
                                                        Ok(Value::Int(n)) => n,
                                                        _ => 200,
                                                    };
                                                    
                                                    let response = format!(
                                                        "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n{}",
                                                        status,
                                                        match status { 200 => "OK", 404 => "Not Found", 500 => "Internal Server Error", _ => "OK" },
                                                        resp_body.len(),
                                                        resp_body
                                                    );
                                                    let _ = stream.write_all(response.as_bytes());
                                                }
                                                Err(e) => {
                                                    let response = format!(
                                                        "HTTP/1.1 500 Internal Server Error\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                                                        e.len(), e
                                                    );
                                                    let _ = stream.write_all(response.as_bytes());
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("http-serve: accept error: {}", e);
                                        }
                                    }
                                }
                            }
                            (p, h) => return Err(format!("http-serve expects (int quote), got ({} {})", p.type_name(), h.type_name())),
                        }
                    }
                    _ => return Err(format!("unknown syscall id: 0x{:02X}", call_id)),
                }
            }

            // ================================================================
            // REFLECTION (0x90-0x9F) - Self-hosting primitives
            // These allow a Kore program to read its own bytecode,
            // enabling meta-circular interpretation (Kore in Kore)
            // ================================================================
            
            // FETCH: (offset -- byte) Read own bytecode at offset
            0x90 => {
                let offset = self.pop_int()? as usize;
                if offset >= self.code.len() {
                    return Err(format!("fetch: offset {} out of bounds (code len {})", offset, self.code.len()));
                }
                self.stack.push(Value::Int(self.code[offset] as i64));
            }
            
            // SIZE: ( -- len) Length of own bytecode
            0x91 => {
                self.stack.push(Value::Int(self.code.len() as i64));
            }

            // ================================================================
            // LINEAR TYPES (0xC0-0xC4) — P4: Constraints Attenuate
            // ================================================================
            
            // LINEAR: (a -- Linear(a)) mark value as linear
            // Linear values must be used exactly once: no dup, no drop.
            0xC0 => {
                let a = self.stack.pop()?;
                self.stack.push(Value::Linear(Box::new(a)));
            }
            
            // AFFINE: (a -- Affine(a)) mark value as affine
            // Affine values can be used at most once: no dup, but drop is OK.
            0xC1 => {
                let a = self.stack.pop()?;
                self.stack.push(Value::Affine(Box::new(a)));
            }
            
            // CONSUME: (Linear(a) | Affine(a) -- a) unwrap linearity tag
            // This is the only way to access the inner value.
            0xC2 => {
                match self.stack.pop()? {
                    Value::Linear(v) => self.stack.push(*v),
                    Value::Affine(v) => self.stack.push(*v),
                    v => return Err(format!("CONSUME expects linear or affine value, got {}", v.type_name())),
                }
            }
            
            // IS_LINEAR: (a -- a bool) check if value is linear-tagged
            0xC3 => {
                let a = self.stack.pop()?;
                let is_lin = matches!(&a, Value::Linear(_));
                self.stack.push(a);
                self.stack.push(Value::Bool(is_lin));
            }
            
            // IS_AFFINE: (a -- a bool) check if value is affine-tagged
            0xC4 => {
                let a = self.stack.pop()?;
                let is_aff = matches!(&a, Value::Affine(_));
                self.stack.push(a);
                self.stack.push(Value::Bool(is_aff));
            }

            // ================================================================
            // INTROSPECTION (0xD0-0xD4)
            // ================================================================
            
            // TYPE_OF: (a -- a str) push type name of TOS without consuming it
            0xD0 => {
                let a = self.stack.pop()?;
                let name = a.type_name().to_string();
                self.stack.push(a);
                self.stack.push(Value::Str(Box::new(name)));
            }
            
            // DEPTH: (-- n) push current stack depth
            0xD1 => {
                let d = self.stack.data.len() as i64;
                self.stack.push(Value::Int(d));
            }
            
            // DESCRIBE: (str -- str) look up built-in word description
            0xD2 => {
                match self.stack.pop()? {
                    Value::Str(name) => {
                        let desc = describe_word(&name);
                        self.stack.push(Value::Str(Box::new(desc)));
                    }
                    v => return Err(format!("DESCRIBE expects string, got {}", v.type_name())),
                }
            }

            // ================================================================
            // STRING OPERATIONS (0xE0-0xE5)
            // P1: strings are tools — these are state transformers on strings
            // ================================================================
            
            // STR_LEN: (str -- str n) push length without consuming
            0xE0 => {
                let s = self.stack.pop()?;
                match &s {
                    Value::Str(st) => {
                        let len = st.len() as i64;
                        self.stack.push(s);
                        self.stack.push(Value::Int(len));
                    }
                    _ => return Err(format!("STR_LEN expects string, got {}", s.type_name())),
                }
            }
            
            // STR_GET: (str i -- str) get character at index as 1-char string
            0xE1 => {
                let idx = self.stack.pop()?;
                let s = self.stack.pop()?;
                match (&s, &idx) {
                    (Value::Str(st), Value::Int(i)) => {
                        let i = *i as usize;
                        match st.chars().nth(i) {
                            Some(ch) => self.stack.push(Value::Str(Box::new(ch.to_string()))),
                            None => self.stack.push(Value::Error(Box::new(format!("str-get index {} out of bounds (len {})", i, st.len())))),
                        }
                    }
                    _ => return Err(format!("STR_GET expects (str int), got ({} {})", s.type_name(), idx.type_name())),
                }
            }
            
            // STR_CONCAT: (str str -- str) concatenate two strings
            0xE2 => {
                let b = self.stack.pop()?;
                let a = self.stack.pop()?;
                match (a, b) {
                    (Value::Str(sa), Value::Str(sb)) => {
                        self.stack.push(Value::Str(Box::new(format!("{}{}", sa, sb))));
                    }
                    (a, b) => return Err(format!("STR_CONCAT expects (str str), got ({} {})", a.type_name(), b.type_name())),
                }
            }
            
            // STR_SLICE: (str start end -- str) substring [start..end)
            0xE3 => {
                let end_val = self.stack.pop()?;
                let start_val = self.stack.pop()?;
                let s = self.stack.pop()?;
                match (s, start_val, end_val) {
                    (Value::Str(st), Value::Int(start), Value::Int(end)) => {
                        let start = start as usize;
                        let end = end as usize;
                        let chars: Vec<char> = st.chars().collect();
                        if start > chars.len() || end > chars.len() || start > end {
                            self.stack.push(Value::Error(Box::new(format!("str-slice [{},{}) out of bounds (len {})", start, end, chars.len()))));
                        } else {
                            let result: String = chars[start..end].iter().collect();
                            self.stack.push(Value::Str(Box::new(result)));
                        }
                    }
                    (s, a, b) => return Err(format!("STR_SLICE expects (str int int), got ({} {} {})", s.type_name(), a.type_name(), b.type_name())),
                }
            }
            
            // TO_STR: (a -- str) convert any value to string representation
            0xE4 => {
                let v = self.stack.pop()?;
                let s = self.format_value(&v);
                self.stack.push(Value::Str(Box::new(s)));
            }
            
            // STR_FIND: (str pattern -- int) find substring, -1 if not found
            0xE5 => {
                let pattern = self.stack.pop()?;
                let haystack = self.stack.pop()?;
                match (haystack, pattern) {
                    (Value::Str(h), Value::Str(p)) => {
                        let idx = h.find(p.as_str()).map(|i| i as i64).unwrap_or(-1);
                        self.stack.push(Value::Int(idx));
                    }
                    (h, p) => return Err(format!("STR_FIND expects (str str), got ({} {})", h.type_name(), p.type_name())),
                }
            }

            // STR_SPLIT: (str delim -- list) split string by delimiter
            0xE6 => {
                let delim = self.stack.pop()?;
                let s = self.stack.pop()?;
                match (s, delim) {
                    (Value::Str(st), Value::Str(d)) => {
                        let parts: Vec<Value> = st.split(d.as_str()).map(|p| Value::Str(Box::new(p.to_string()))).collect();
                        self.stack.push(Value::List(Box::new(parts)));
                    }
                    (s, d) => return Err(format!("STR_SPLIT expects (str str), got ({} {})", s.type_name(), d.type_name())),
                }
            }

            // STR_REPLACE: (str from to -- str) replace all occurrences
            0xE7 => {
                let to = self.stack.pop()?;
                let from = self.stack.pop()?;
                let s = self.stack.pop()?;
                match (s, from, to) {
                    (Value::Str(st), Value::Str(f), Value::Str(t)) => {
                        self.stack.push(Value::Str(Box::new(st.replace(f.as_str(), t.as_str()))));
                    }
                    (s, f, t) => return Err(format!("STR_REPLACE expects (str str str), got ({} {} {})", s.type_name(), f.type_name(), t.type_name())),
                }
            }

            // STR_UPPER: (str -- str) convert to uppercase
            0xE8 => {
                match self.stack.pop()? {
                    Value::Str(s) => self.stack.push(Value::Str(Box::new(s.to_uppercase()))),
                    v => return Err(format!("STR_UPPER expects string, got {}", v.type_name())),
                }
            }

            // STR_LOWER: (str -- str) convert to lowercase
            0xE9 => {
                match self.stack.pop()? {
                    Value::Str(s) => self.stack.push(Value::Str(Box::new(s.to_lowercase()))),
                    v => return Err(format!("STR_LOWER expects string, got {}", v.type_name())),
                }
            }

            // STR_TRIM: (str -- str) trim whitespace from both ends
            0xEA => {
                match self.stack.pop()? {
                    Value::Str(s) => self.stack.push(Value::Str(Box::new(s.trim().to_string()))),
                    v => return Err(format!("STR_TRIM expects string, got {}", v.type_name())),
                }
            }

            // PARSE_FLOAT: (str -- float) parse string as float
            0xEB => {
                match self.stack.pop()? {
                    Value::Str(s) => {
                        match s.trim().parse::<f64>() {
                            Ok(f) => self.stack.push(Value::Float(f)),
                            Err(_) => return Err(format!("parse-float: cannot parse '{}' as float", s)),
                        }
                    }
                    v => return Err(format!("parse-float expects string, got {}", v.type_name())),
                }
            }

            // PARSE_INT: (str -- int) parse string as integer
            0xEC => {
                match self.stack.pop()? {
                    Value::Str(s) => {
                        let trimmed = s.trim();
                        // Try decimal first, then hex (0x prefix)
                        let result = if trimmed.starts_with("0x") || trimmed.starts_with("0X") {
                            i64::from_str_radix(&trimmed[2..], 16)
                        } else {
                            trimmed.parse::<i64>()
                        };
                        match result {
                            Ok(i) => self.stack.push(Value::Int(i)),
                            Err(_) => return Err(format!("parse-int: cannot parse '{}' as integer", s)),
                        }
                    }
                    v => return Err(format!("parse-int expects string, got {}", v.type_name())),
                }
            }

            // ================================================================
            // ARRAY OPERATIONS (0x8A-0x8F) — P1: contiguous i64 storage
            // Arrays are tools (S → S). GPU-friendly, cache-friendly.
            // 8 bytes per element (vs 16 for List).
            // ================================================================

            // ARRAY_NEW: (n -- array) create array of n zeros
            0x8A => {
                let n = self.pop_int()? as usize;
                self.stack.push(Value::Array(Box::new(vec![0i64; n])));
            }

            // ARRAY_GET: (array i -- array elem) get element (non-destructive)
            0x8B => {
                let i = self.pop_int()? as usize;
                let arr = self.stack.pop()?;
                match &arr {
                    Value::Array(data) => {
                        match data.get(i) {
                            Some(&elem) => {
                                self.stack.push(arr);
                                self.stack.push(Value::Int(elem));
                            }
                            None => {
                                let len = data.len();
                                self.stack.push(arr);
                                self.stack.push(Value::Error(Box::new(format!("array-get: index {} out of bounds (len {})", i, len))));
                            }
                        }
                    }
                    v => return Err(format!("ARRAY_GET expects array, got {}", v.type_name())),
                }
            }

            // ARRAY_SET: (array i v -- array) set element
            0x8C => {
                let v = self.pop_int()?;
                let i = self.pop_int()? as usize;
                match self.stack.pop()? {
                    Value::Array(mut data) => {
                        if i >= data.len() {
                            return Err(format!("array-set: index {} out of bounds (len {})", i, data.len()));
                        }
                        data[i] = v;
                        self.stack.push(Value::Array(data));
                    }
                    v => return Err(format!("ARRAY_SET expects array, got {}", v.type_name())),
                }
            }

            // ARRAY_LEN: (array -- array n) length (non-destructive)
            0x8D => {
                let arr = self.stack.pop()?;
                match &arr {
                    Value::Array(data) => {
                        let len = data.len() as i64;
                        self.stack.push(arr);
                        self.stack.push(Value::Int(len));
                    }
                    v => return Err(format!("ARRAY_LEN expects array, got {}", v.type_name())),
                }
            }

            // ARRAY_PUSH: (array v -- array) append element
            0x8E => {
                let v = self.pop_int()?;
                match self.stack.pop()? {
                    Value::Array(mut data) => {
                        data.push(v);
                        self.stack.push(Value::Array(data));
                    }
                    val => return Err(format!("ARRAY_PUSH expects array, got {}", val.type_name())),
                }
            }

            // ARRAY_FROM: (list -- array) convert list of ints to array
            // P1: total function — non-int elements become 0
            0x8F => {
                match self.stack.pop()? {
                    Value::List(items) => {
                        let data: Vec<i64> = items.iter().map(|v| {
                            match v {
                                Value::Int(n) => *n,
                                Value::Float(f) => *f as i64,
                                _ => 0,
                            }
                        }).collect();
                        self.stack.push(Value::Array(Box::new(data)));
                    }
                    v => return Err(format!("ARRAY_FROM expects list, got {}", v.type_name())),
                }
            }

            // ================================================================
            // TRAINING PRIMITIVES (0xC5-0xCD) — Reduce token count for RL
            // P1: All are tools (S→S). No hidden state, no exceptions.
            // ================================================================

            // TIMES: (n quote -- ...) execute quote N times
            // P1: tool. P3: composition = concatenation (each iteration concatenates effects)
            0xC5 => {
                let quote = self.stack.pop()?;
                let n_val = self.stack.pop()?;
                match (n_val, quote) {
                    (Value::Int(n), Value::Quote { offset, len: _ }) => {
                        if n < 0 {
                            return Err("TIMES: count must be non-negative".into());
                        }
                        for _ in 0..n {
                            self.run_quote_inline(offset as usize)?;
                        }
                    }
                    (Value::Int(_), v) => return Err(format!("TIMES expects quote, got {}", v.type_name())),
                    (v, _) => return Err(format!("TIMES expects int count, got {}", v.type_name())),
                }
            }

            // FILTER: (list quote -- list') keep elements where quote returns true
            // P1: tool (S→S). The functor predicate lift.
            0xC6 => {
                let quote = self.stack.pop()?;
                let list = self.stack.pop()?;
                match (list, quote) {
                    (Value::List(items), Value::Quote { offset, len: _ }) => {
                        let mut results = Vec::new();
                        for item in *items {
                            self.stack.push(item.clone());
                            self.run_quote_inline(offset as usize)?;
                            let keep = self.pop_bool()?;
                            if keep {
                                results.push(item);
                            }
                        }
                        self.stack.push(Value::List(Box::new(results)));
                    }
                    (Value::List(_), v) => return Err(format!("FILTER expects quote, got {}", v.type_name())),
                    (v, _) => return Err(format!("FILTER expects list, got {}", v.type_name())),
                }
            }

            // HEAD: (list -- elem) first element
            // P1: tool. Returns Error on empty list (errors are values).
            0xC7 => {
                match self.stack.pop()? {
                    Value::List(items) => {
                        if items.is_empty() {
                            self.stack.push(Value::Error(Box::new("head: empty list".into())));
                        } else {
                            self.stack.push(items[0].clone());
                        }
                    }
                    v => return Err(format!("HEAD expects list, got {}", v.type_name())),
                }
            }

            // TAIL: (list -- list') all but first element
            // P1: tool. Returns Error on empty list.
            0xC8 => {
                match self.stack.pop()? {
                    Value::List(items) => {
                        if items.is_empty() {
                            self.stack.push(Value::Error(Box::new("tail: empty list".into())));
                        } else {
                            self.stack.push(Value::List(Box::new(items[1..].to_vec())));
                        }
                    }
                    v => return Err(format!("TAIL expects list, got {}", v.type_name())),
                }
            }

            // RANGE: (start end -- list) build list of integers [start..end)
            // P1: pure tool, no hidden state.
            0xC9 => {
                let end = self.pop_int()?;
                let start = self.pop_int()?;
                let len = if end > start { (end - start) as usize } else { 0 };
                if len > 1_000_000 {
                    return Err(format!("RANGE: too large ({} elements)", len));
                }
                let items: Vec<Value> = (start..end).map(|i| Value::Int(i)).collect();
                self.stack.push(Value::List(Box::new(items)));
            }

            // FIRST: (pair -- pair a) non-destructive first element
            // Equivalent to: dup unpair drop — but 1 token instead of 3
            0xCA => {
                match self.stack.pop()? {
                    Value::Pair(p) => {
                        let a = p.0.clone();
                        self.stack.push(Value::Pair(p));
                        self.stack.push(a);
                    }
                    v => return Err(format!("FIRST expects pair, got {}", v.type_name())),
                }
            }

            // SECOND: (pair -- pair b) non-destructive second element
            // Equivalent to: dup unpair swap drop — but 1 token instead of 4
            0xCB => {
                match self.stack.pop()? {
                    Value::Pair(p) => {
                        let b = p.1.clone();
                        self.stack.push(Value::Pair(p));
                        self.stack.push(b);
                    }
                    v => return Err(format!("SECOND expects pair, got {}", v.type_name())),
                }
            }

            // LIST_CONCAT: (list list -- list') concatenate two lists
            // P1: pure tool, no hidden state.
            0xCC => {
                let b = self.stack.pop()?;
                let a = self.stack.pop()?;
                match (a, b) {
                    (Value::List(mut la), Value::List(lb)) => {
                        la.extend(*lb);
                        self.stack.push(Value::List(la));
                    }
                    (Value::List(_), v) => return Err(format!("LIST_CONCAT expects list, got {}", v.type_name())),
                    (v, _) => return Err(format!("LIST_CONCAT expects list, got {}", v.type_name())),
                }
            }

            // IS_EMPTY: (list -- list bool) non-destructive empty check
            // Equivalent to: dup len 0 = — but 1 token instead of 4
            0xCD => {
                let list = self.stack.pop()?;
                match &list {
                    Value::List(items) => {
                        let empty = items.is_empty();
                        self.stack.push(list);
                        self.stack.push(Value::Bool(empty));
                    }
                    _ => return Err(format!("IS_EMPTY expects list, got {}", list.type_name())),
                }
            }

            // ================================================================
            // HALT (0xFF)
            // ================================================================
            0xFF => {
                // Handled in run() loop
            }

            // Unknown opcode
            _ => {
                return Err(format!("unknown opcode: 0x{:02X}", op));
            }
        }
        
        Ok(())
    }

    // ========================================================================
    // HELPERS (No decisions here either - just mechanical conversions)
    // ========================================================================

    fn read_u16(&mut self) -> Result<u16, String> {
        if self.pc + 2 > self.code.len() {
            return Err("unexpected end of code".into());
        }
        let v = u16::from_le_bytes([self.code[self.pc], self.code[self.pc + 1]]);
        self.pc += 2;
        Ok(v)
    }

    fn read_i16(&mut self) -> Result<i16, String> {
        Ok(self.read_u16()? as i16)
    }

    fn read_u32(&mut self) -> Result<u32, String> {
        if self.pc + 4 > self.code.len() {
            return Err("unexpected end of code".into());
        }
        let v = u32::from_le_bytes([
            self.code[self.pc], self.code[self.pc + 1],
            self.code[self.pc + 2], self.code[self.pc + 3],
        ]);
        self.pc += 4;
        Ok(v)
    }

    fn read_i32(&mut self) -> Result<i32, String> {
        Ok(self.read_u32()? as i32)
    }

    fn read_u64(&mut self) -> Result<u64, String> {
        if self.pc + 8 > self.code.len() {
            return Err("unexpected end of code".into());
        }
        let v = u64::from_le_bytes([
            self.code[self.pc], self.code[self.pc + 1],
            self.code[self.pc + 2], self.code[self.pc + 3],
            self.code[self.pc + 4], self.code[self.pc + 5],
            self.code[self.pc + 6], self.code[self.pc + 7],
        ]);
        self.pc += 8;
        Ok(v)
    }

    fn read_i64(&mut self) -> Result<i64, String> {
        Ok(self.read_u64()? as i64)
    }

    fn pop_int(&mut self) -> Result<i64, String> {
        match self.stack.pop()? {
            Value::Int(n) => Ok(n),
            v => Err(format!("expected int, got {}", v.type_name())),
        }
    }

    fn pop_float(&mut self) -> Result<f64, String> {
        match self.stack.pop()? {
            Value::Float(f) => Ok(f),
            v => Err(format!("expected float, got {}", v.type_name())),
        }
    }

    fn pop_bool(&mut self) -> Result<bool, String> {
        match self.stack.pop()? {
            Value::Bool(b) => Ok(b),
            v => Err(format!("expected bool, got {}", v.type_name())),
        }
    }

    fn pop_number(&mut self) -> Result<f64, String> {
        match self.stack.pop()? {
            Value::Int(n) => Ok(n as f64),
            Value::Float(f) => Ok(f),
            v => Err(format!("expected number, got {}", v.type_name())),
        }
    }

    fn value_to_number(&self, v: &Value) -> Result<f64, String> {
        match v {
            Value::Int(n) => Ok(*n as f64),
            Value::Float(f) => Ok(*f),
            _ => Err(format!("expected number, got {}", v.type_name())),
        }
    }

    fn push_number(&mut self, n: f64) {
        if n.fract() == 0.0 && n >= i64::MIN as f64 && n <= i64::MAX as f64 {
            self.stack.push(Value::Int(n as i64));
        } else {
            self.stack.push(Value::Float(n));
        }
    }

    fn values_equal(&self, a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Nil, Value::Nil) => true,
            (Value::Bool(x), Value::Bool(y)) => x == y,
            (Value::Int(x), Value::Int(y)) => x == y,
            (Value::Float(x), Value::Float(y)) => x == y,
            (Value::Int(x), Value::Float(y)) => (*x as f64) == *y,
            (Value::Float(x), Value::Int(y)) => *x == (*y as f64),
            (Value::Str(x), Value::Str(y)) => x == y,
            _ => false,
        }
    }

    /// Format a value for print/println output
    fn format_value(&self, v: &Value) -> String {
        match v {
            Value::Nil => "nil".into(),
            Value::Bool(b) => if *b { "true" } else { "false" }.into(),
            Value::Int(n) => n.to_string(),
            Value::Float(f) => format!("{}", f),
            Value::Str(s) => (**s).clone(),
            Value::Pair(p) => format!("({}, {})", self.format_value(&p.0), self.format_value(&p.1)),
            Value::Left(v) => format!("Left({})", self.format_value(v)),
            Value::Right(v) => format!("Right({})", self.format_value(v)),
            Value::Quote { offset, len } => format!("<quote@{}:{}>", offset, len),
            Value::List(items) => {
                let parts: Vec<String> = items.iter().map(|i| self.format_value(i)).collect();
                format!("({})", parts.join(" "))
            }
            Value::Fiber(f) => format!("<fiber@{} {}>", f.offset, if f.done { "done" } else { "running" }),
            Value::Channel(id) => format!("<channel:{}>", id),
            Value::Linear(v) => format!("linear({})", self.format_value(v)),
            Value::Affine(v) => format!("affine({})", self.format_value(v)),
            Value::Error(msg) => format!("Error({})", msg),
            Value::Map(m) => {
                let parts: Vec<String> = m.iter()
                    .map(|(k, v)| format!("{}: {}", k, self.format_value(v)))
                    .collect();
                format!("{{{}}}", parts.join(", "))
            }
            Value::Array(data) => {
                let parts: Vec<String> = data.iter().map(|n| n.to_string()).collect();
                format!("#({})", parts.join(" "))
            }
        }
    }

    /// Get the final result (top of stack)
    pub fn result(&self) -> Option<&Value> {
        self.stack.data.last()
    }

    /// Get the full stack (for debugging)
    pub fn stack(&self) -> &[Value] {
        &self.stack.data
    }
}

// ============================================================================
// WORD DICTIONARY (for describe introspection)
// ============================================================================

/// Return a description string for a built-in word.
fn describe_word(name: &str) -> String {
    match name {
        // Stack
        "drop" => "(a --)".into(),
        "dup" => "(a -- a a)".into(),
        "swap" => "(a b -- b a)".into(),
        "rot" => "(a b c -- b c a)".into(),
        "over" => "(a b -- a b a)".into(),
        // Arithmetic
        "add" | "+" => "(a b -- a+b)".into(),
        "sub" | "-" => "(a b -- a-b)".into(),
        "mul" | "*" => "(a b -- a*b)".into(),
        "div" | "/" => "(a b -- a/b)".into(),
        "mod" | "%" => "(a b -- a%b)".into(),
        "neg" => "(a -- -a)".into(),
        // Float arithmetic
        "fadd" => "(f f -- f)".into(),
        "fsub" => "(f f -- f)".into(),
        "fmul" => "(f f -- f)".into(),
        "fdiv" => "(f f -- f)".into(),
        "fneg" => "(f -- -f)".into(),
        "fsqrt" => "(f -- sqrt(f))".into(),
        "fabs" => "(f -- |f|)".into(),
        "fexp" => "(f -- e^f)".into(),
        "flog" => "(f -- ln(f))".into(),
        "fsin" => "(f -- sin(f))".into(),
        "fcos" => "(f -- cos(f))".into(),
        "fatan2" => "(f f -- atan2(y,x))".into(),
        "fpow" => "(f f -- f^f)".into(),
        "ffloor" => "(f -- floor(f))".into(),
        "fceil" => "(f -- ceil(f))".into(),
        "fround" => "(f -- round(f))".into(),
        "i2f" => "(int -- float)".into(),
        "f2i" => "(float -- int)".into(),
        // Comparison
        "eq" | "=" => "(a b -- bool)".into(),
        "ne" => "(a b -- bool)".into(),
        "lt" | "<" => "(a b -- bool)".into(),
        "gt" | ">" => "(a b -- bool)".into(),
        "le" => "(a b -- bool)".into(),
        "ge" => "(a b -- bool)".into(),
        // Logic (polymorphic: bool or int)
        "and" => "(a b -- a∧b) bool AND / int bitwise AND".into(),
        "or" => "(a b -- a∨b) bool OR / int bitwise OR".into(),
        "not" => "(a -- ¬a) bool NOT / int bitwise NOT".into(),
        "xor" => "(a b -- a⊕b) bool XOR / int bitwise XOR".into(),
        // Bitwise integer
        "band" => "(int int -- int) bitwise AND".into(),
        "bor" => "(int int -- int) bitwise OR".into(),
        "bxor" => "(int int -- int) bitwise XOR".into(),
        "bnot" => "(int -- int) bitwise NOT".into(),
        "shl" => "(int n -- int) shift left".into(),
        "shr" => "(int n -- int) arithmetic shift right".into(),
        // Data
        "pair" => "(a b -- (a,b))".into(),
        "unpair" => "((a,b) -- a b)".into(),
        "left" => "(a -- Left(a))".into(),
        "right" => "(a -- Right(a))".into(),
        // Lists
        "len" => "(list -- list n)".into(),
        "get" => "(list i -- elem)".into(),
        "set" => "(list i v -- list')".into(),
        "map" => "(list quote -- list')".into(),
        "fold" => "(list init quote -- result)".into(),
        "zip" => "(list list -- list-of-pairs)".into(),
        "append" => "(list value -- list')".into(),
        "reverse" => "(list -- list')".into(),
        // Array operations
        "array-new" => "(n -- array) create array of n zeros".into(),
        "array-get" => "(array i -- array elem) get element".into(),
        "array-set" => "(array i v -- array) set element".into(),
        "array-len" => "(array -- array n) array length".into(),
        "array-push" => "(array v -- array) append element".into(),
        "array-from" => "(list -- array) convert list of ints to array".into(),
        // Control
        "apply" => "(quote -- ...)".into(),
        "cond" => "(bool then_q else_q -- ...)".into(),
        "loop" => "(body_q -- ...)".into(),
        // Linear types
        "linear" => "(a -- Linear(a))".into(),
        "affine" => "(a -- Affine(a))".into(),
        "consume" => "(Linear(a)|Affine(a) -- a)".into(),
        "is-linear" => "(a -- a bool)".into(),
        "is-affine" => "(a -- a bool)".into(),
        // Fibers
        "fiber-new" => "(quote -- fiber)".into(),
        "fiber-step" => "(fiber -- fiber' bool)".into(),
        "fiber-push" => "(fiber value -- fiber')".into(),
        "fiber-stack" => "(fiber -- fiber list)".into(),
        "fiber-status" => "(fiber -- fiber bool)".into(),
        // Spawn / Channels
        "spawn" => "(quote caps -- result-list)".into(),
        "chan-new" => "(-- chan)".into(),
        "chan-send" => "(chan value --)".into(),
        "chan-recv" => "(chan -- value)".into(),
        // Introspection
        "type-of" => "(a -- a str)".into(),
        "depth" => "(-- n)".into(),
        "describe" => "(str -- str)".into(),
        // IO
        "print" => "(value --)".into(),
        "println" => "(value --)".into(),
        "rand" => "(-- n)".into(),
        // Reflection
        "fetch" => "(offset -- byte)".into(),
        "size" => "(-- len)".into(),
        // String operations
        "str-len" => "(str -- str n)".into(),
        "str-get" => "(str i -- str)".into(),
        "str-concat" => "(str str -- str)".into(),
        "str-slice" => "(str start end -- str)".into(),
        "to-str" => "(a -- str)".into(),
        "str-find" => "(str pattern -- int)".into(),
        "str-split" => "(str delim -- list)".into(),
        "str-replace" => "(str from to -- str)".into(),
        "str-upper" => "(str -- str)".into(),
        "str-lower" => "(str -- str)".into(),
        "str-trim" => "(str -- str)".into(),
        // Parsing
        "parse-float" => "(str -- float)".into(),
        "parse-int" => "(str -- int)".into(),
        // Error handling
        "try" => "(quote -- result|error)".into(),
        "fail" => "(str -- !)".into(),
        "is-error" => "(a -- a bool)".into(),
        "error" => "(str -- error) create Error value (P1: S→S)".into(),
        "?" | "propagate" => "(a -- a) if Error, re-fail; else pass through".into(),
        // HashMap
        "map-new" => "(-- map)".into(),
        "map-get" => "(map key -- map value)".into(),
        "map-set" => "(map key value -- map)".into(),
        "map-keys" => "(map -- map list)".into(),
        "map-has" => "(map key -- map bool)".into(),
        // Syscalls
        "file-read" => "(path -- str)".into(),
        "file-write" => "(path content --)".into(),
        "file-exists" => "(path -- bool)".into(),
        "time-now" => "(-- float)".into(),
        "env-get" => "(name -- str)".into(),
        "exec" => "(cmd -- (stdout, exit-code))".into(),
        // Additional syscalls
        "readline" => "(prompt -- str)".into(),
        "file-append" => "(path content --)".into(),
        "file-delete" => "(path -- bool)".into(),
        "file-list" => "(path -- list)".into(),
        "http-get" => "(url -- (body, status))".into(),
        "http-post" => "(url body content-type -- (response, status))".into(),
        "http-serve" => "(port handler-quote --)".into(),
        _ => format!("unknown word: {}", name),
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::*;

    /// Tool 5 — Compact Value representation.
    /// Verify that Value is exactly 16 bytes (3.5× smaller than the
    /// original 56-byte enum).  This is the foundational invariant
    /// for cache-friendly, GPU-mappable execution.
    #[test]
    fn test_value_size_is_16_bytes() {
        assert_eq!(std::mem::size_of::<Value>(), 16,
            "Value must be exactly 16 bytes (compact enum)");
        // For reference: i64=8, f64=8, Box<T>=8, bool=1
        // The 16-byte enum: 8 bytes discriminant+padding + 8 bytes payload
    }

    #[test]
    fn test_simple_add() {
        let mut asm = Assembler::new();
        asm.emit_i8(3);
        asm.emit_i8(4);
        asm.emit(Op::Add);
        asm.emit(Op::Halt);
        
        let module = asm.finalize().unwrap();
        let mut interp = Interpreter::new(&module.code);
        interp.run().unwrap();
        
        match interp.result() {
            Some(Value::Int(7)) => {}
            other => panic!("expected 7, got {:?}", other),
        }
    }

    #[test]
    fn test_factorial() {
        let mut asm = Assembler::new();

        // Main: 5 factorial halt
        asm.emit_i8(5);
        asm.emit_call_label("factorial");
        asm.emit(Op::Halt);

        // factorial function
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
            Some(Value::Int(120)) => {}
            other => panic!("expected 120, got {:?}", other),
        }
    }

    #[test]
    fn test_pair_unpair() {
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
            Some(Value::Int(3)) => {}
            other => panic!("expected 3, got {:?}", other),
        }
    }

    #[test]
    fn test_left_right_case() {
        // Test: Left(5) should return 5+1=6
        let mut asm = Assembler::new();
        asm.emit_i8(5);
        asm.emit(Op::Left);
        // CASE left_offset right_offset
        // We need to emit case manually with offsets
        asm.emit(Op::Case);
        // left branch: at current position, just add 1
        // right branch: at current position + 4, just add 2
        // For now, skip this complex test
        
        // Simpler test: just check Left/Right construction
        let mut asm2 = Assembler::new();
        asm2.emit_i8(42);
        asm2.emit(Op::Left);
        asm2.emit(Op::Halt);
        
        let module = asm2.finalize().unwrap();
        let mut interp = Interpreter::new(&module.code);
        interp.run().unwrap();
        
        match interp.result() {
            Some(Value::Left(v)) => match v.as_ref() {
                Value::Int(42) => {}
                other => panic!("expected Left(42), got Left({:?})", other),
            }
            other => panic!("expected Left(42), got {:?}", other),
        }
    }
}
