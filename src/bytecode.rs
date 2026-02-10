//! Kore Bytecode Assembler/Disassembler
//! 
//! This is the reference implementation for Kore bytecode.
//! All backends (WASM, LLVM, GPU) consume this format.

use std::collections::HashMap;

// ============================================================================
// OPCODES
// ============================================================================

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    // Stack (0x00-0x0F)
    Nop = 0x00,
    Drop = 0x01,
    Dup = 0x02,
    Swap = 0x03,
    Rot = 0x04,
    Over = 0x05,

    // Data (0x10-0x1F)
    Pair = 0x10,
    Unpair = 0x11,
    Left = 0x12,
    Right = 0x13,
    Case = 0x14,

    // Control (0x20-0x2F)
    Quote = 0x20,
    Apply = 0x21,
    Call = 0x22,
    Ret = 0x23,
    Cond = 0x24,    // (bool then_q else_q -- ...) old-style conditional
    Loop = 0x25,    // (body_q -- ...) apply quote, if true on top repeat

    // Literals (0x30-0x3F)
    Int8 = 0x30,
    Int16 = 0x31,
    Int32 = 0x32,
    Int64 = 0x33,
    F32 = 0x34,
    F64 = 0x35,
    Str = 0x36,
    Nil = 0x37,
    True = 0x38,
    False = 0x39,

    // Arithmetic (0x40-0x4F)
    Add = 0x40,
    Sub = 0x41,
    Mul = 0x42,
    Div = 0x43,
    Mod = 0x44,
    Neg = 0x45,
    Fadd = 0x46,    // Float add (f64)
    Fsub = 0x47,    // Float sub (f64)
    Fmul = 0x48,    // Float mul (f64)
    Fdiv = 0x49,    // Float div (f64)
    Fneg = 0x4A,    // Float negate (f64)
    Fsqrt = 0x4B,   // Float square root (f64)
    Fabs = 0x4C,    // Float absolute value (f64)
    I2f = 0x4D,     // Int to Float conversion
    F2i = 0x4E,     // Float to Int conversion (truncate)
    Fexp = 0x4F,    // Float exp (e^x)

    // Comparison (0x50-0x5F)
    Eq = 0x50,
    Lt = 0x51,
    Gt = 0x52,
    Le = 0x53,
    Ge = 0x54,
    Ne = 0x55,
    Flog = 0x56,    // Float natural log (ln(x))

    Fsin = 0x57,    // Float sin(x)
    Fcos = 0x58,    // Float cos(x)
    Fatan2 = 0x59,  // Float atan2(y, x)
    Fpow = 0x5A,    // Float pow(base, exp)
    Ffloor = 0x5B,  // Float floor(x)
    Fceil = 0x5C,   // Float ceil(x)
    Fround = 0x5D,  // Float round(x)

    // Logic (0x60-0x6F)
    And = 0x60,
    Or = 0x61,
    Not = 0x62,
    Xor = 0x63,
    Band = 0x64,    // (int int -- int) bitwise AND
    Bor = 0x65,     // (int int -- int) bitwise OR
    Bxor = 0x66,    // (int int -- int) bitwise XOR
    Bnot = 0x67,    // (int -- int) bitwise NOT
    Shl = 0x68,     // (int n -- int) shift left
    Shr = 0x69,     // (int n -- int) shift right (arithmetic)

    // Jumps (0x70-0x7F)
    Jmp = 0x70,
    Jz = 0x71,
    Jnz = 0x72,

    // Lists (0x80-0x8F)
    List = 0x80,
    Unlist = 0x81,
    Len = 0x82,
    Get = 0x83,
    Set = 0x84,
    
    // Higher-order list ops (0x85-0x87)
    // These are the mathematical foundations:
    // MAP  = functor lift: f:A→B ⟹ map(f):[A]→[B]
    // FOLD = catamorphism: (f:B×A→B, b₀:B) → [A] → B
    // ZIP  = product lift: [A]×[B] → [(A,B)]
    Map = 0x85,     // (list quote -- list')    apply quote to each element
    Fold = 0x86,    // (list init quote -- result)  reduce list
    Zip = 0x87,     // (list list -- list-of-pairs)
    Append = 0x88,  // (list value -- list')  append value to end of list
    Reverse = 0x89, // (list -- list')  reverse a list

    // Array operations (0x8A-0x8F) — P1: contiguous homogeneous i64 storage
    // Arrays are tools (S → S). GPU-friendly, cache-friendly.
    ArrayNew  = 0x8A, // (n -- array)       create array of n zeros
    ArrayGet  = 0x8B, // (array i -- array elem)  get element (non-destructive on array)
    ArraySet  = 0x8C, // (array i v -- array)     set element
    ArrayLen  = 0x8D, // (array -- array n)       length (non-destructive)
    ArrayPush = 0x8E, // (array v -- array)       append element
    ArrayFrom = 0x8F, // (list -- array)          convert list of ints to array

    // Fibers (0xB0-0xB5) — cooperative multitasking
    FiberNew = 0xB0,    // (quote -- fiber)  create fiber from quote
    FiberStep = 0xB1,   // (fiber -- fiber' bool)  execute one instruction, push done?
    FiberPush = 0xB2,   // (fiber value -- fiber')  push value onto fiber's stack
    FiberStack = 0xB3,  // (fiber -- fiber list)  get fiber's stack as list
    FiberStatus = 0xB4, // (fiber -- fiber bool)  check if fiber is done
    Spawn = 0xB5,       // (quote caps -- list)   sandboxed execution with attenuated caps (P4)
    ChanNew = 0xB6,     // (-- chan)               create new channel
    ChanSend = 0xB7,    // (chan value --)          send value to channel
    ChanRecv = 0xB8,    // (chan -- value)          receive value from channel

    // Error handling (0xA3-0xA5) — P4: errors are values, not exceptions
    Try = 0xA3,     // (quote -- result|error) execute quote, catch errors
    Fail = 0xA4,    // (str -- ⊥) unwind to nearest try (escape hatch)
    IsError = 0xA5, // (a -- a bool) check if value is an error
    Propagate = 0xA6, // (a -- a) if Error, re-fail; else pass through (Rust's ?)
    MakeError = 0xA7, // (str -- error) create Error value without unwinding (P1: S→S)

    // IO (0xA0-0xA7) — require capabilities
    Print = 0xA0,   // (value --) Write to stdout (requires cap:io)
    Println = 0xA1, // (value --) Write to stdout + newline (requires cap:io)
    Rand = 0xA2,    // (-- n) Random u64 from OS entropy (requires cap:io)

    // Locals (0xA8-0xA9)
    Store = 0xA8,   // (value --) Store to local slot N
    Load = 0xA9,    // (-- value) Load from local slot N

    // HashMap (0xAA-0xAE) — associative data structure
    MapNew = 0xAA,    // (-- map)               create empty map
    MapGet = 0xAB,    // (map key -- map value)  get value by key
    MapSet = 0xAC,    // (map key value -- map)  set key-value pair
    MapKeys = 0xAD,   // (map -- map list)       get all keys as list
    MapHas = 0xAE,    // (map key -- map bool)   check if key exists

    // SYSCALL (0xAF) — generalized host calls, capability-gated
    Syscall = 0xAF, // varies by call_id (u8 operand)

    // Reflection (0x90-0x9F) - Self-hosting primitives
    Fetch = 0x90,   // (offset -- byte) Read own bytecode
    Size = 0x91,    // ( -- len) Length of own bytecode

    // Linear types (0xC0-0xC4) — P4: constraints attenuate
    Linear = 0xC0,    // (a -- Linear(a))  mark value as linear (must use exactly once)
    Affine = 0xC1,    // (a -- Affine(a))  mark value as affine (must use at most once)
    Consume = 0xC2,   // (Linear(a) -- a)  unwrap linearity tag
    IsLinear = 0xC3,  // (a -- a bool)     check if value is linear-tagged
    IsAffine = 0xC4,  // (a -- a bool)     check if value is affine-tagged

    // Training primitives (0xC5-0xCD) — reduce token count for RL agent
    // P1: all are tools (S→S). No hidden state.
    Times     = 0xC5,  // (n quote -- ...)     execute quote N times
    Filter    = 0xC6,  // (list quote -- list') keep elements where quote returns true
    Head      = 0xC7,  // (list -- elem)        first element
    Tail      = 0xC8,  // (list -- list')       all but first element
    Range     = 0xC9,  // (start end -- list)   build list of integers [start..end)
    First     = 0xCA,  // (pair -- pair a)      non-destructive first (dup unpair drop)
    Second    = 0xCB,  // (pair -- pair b)      non-destructive second (dup unpair swap drop)
    ListConcat = 0xCC, // (list list -- list')   concatenate two lists
    IsEmpty   = 0xCD,  // (list -- list bool)    non-destructive empty check

    // Introspection (0xD0-0xD4)
    TypeOf = 0xD0,    // (a -- a str)      push type name as string
    Depth = 0xD1,     // (-- n)            push current stack depth
    Describe = 0xD2,  // (str -- str)      look up word description from dictionary

    // String operations (0xE0-0xE5) — P1: strings are tools too
    StrLen = 0xE0,    // (str -- str n)    length without consuming
    StrGet = 0xE1,    // (str i -- str)    get char at index (as 1-char string)
    StrConcat = 0xE2, // (str str -- str)  concatenate two strings
    StrSlice = 0xE3,  // (str start end -- str)  substring [start..end)
    ToStr = 0xE4,     // (a -- str)        convert any value to string
    StrFind = 0xE5,   // (str pattern -- int)  find substring, -1 if not found

    StrSplit = 0xE6,   // (str delim -- list) split string by delimiter
    StrReplace = 0xE7, // (str from to -- str) replace all occurrences
    StrUpper = 0xE8,   // (str -- str) convert to uppercase
    StrLower = 0xE9,   // (str -- str) convert to lowercase
    StrTrim = 0xEA,    // (str -- str) trim whitespace from both ends

    // Extension (0xF0)
    Ext = 0xF0,

    // Halt (0xFF)
    Halt = 0xFF,
}

impl Op {
    /// All valid opcodes in the Kore instruction set.
    /// This is the single source of truth — every other list derives from this.
    pub fn all_opcodes() -> &'static [Op] {
        &[
            // Stack
            Op::Nop, Op::Drop, Op::Dup, Op::Swap, Op::Rot, Op::Over,
            // Data
            Op::Pair, Op::Unpair, Op::Left, Op::Right, Op::Case,
            // Control
            Op::Quote, Op::Apply, Op::Call, Op::Ret, Op::Cond, Op::Loop,
            // Literals
            Op::Int8, Op::Int16, Op::Int32, Op::Int64, Op::F32, Op::F64,
            Op::Str, Op::Nil, Op::True, Op::False,
            // Arithmetic
            Op::Add, Op::Sub, Op::Mul, Op::Div, Op::Mod, Op::Neg,
            Op::Fadd, Op::Fsub, Op::Fmul, Op::Fdiv, Op::Fneg, Op::Fsqrt, Op::Fabs,
            Op::I2f, Op::F2i, Op::Fexp,
            // Comparison
            Op::Eq, Op::Lt, Op::Gt, Op::Le, Op::Ge, Op::Ne,
            Op::Flog, Op::Fsin, Op::Fcos, Op::Fatan2, Op::Fpow, Op::Ffloor, Op::Fceil, Op::Fround,
            // Logic
            Op::And, Op::Or, Op::Not, Op::Xor,
            Op::Band, Op::Bor, Op::Bxor, Op::Bnot, Op::Shl, Op::Shr,
            // Jumps
            Op::Jmp, Op::Jz, Op::Jnz,
            // Lists
            Op::List, Op::Unlist, Op::Len, Op::Get, Op::Set,
            Op::Map, Op::Fold, Op::Zip, Op::Append, Op::Reverse,
            // Arrays
            Op::ArrayNew, Op::ArrayGet, Op::ArraySet, Op::ArrayLen, Op::ArrayPush, Op::ArrayFrom,
            // Fibers
            Op::FiberNew, Op::FiberStep, Op::FiberPush, Op::FiberStack, Op::FiberStatus,
            Op::Spawn, Op::ChanNew, Op::ChanSend, Op::ChanRecv,
            // Error handling
            Op::Try, Op::Fail, Op::IsError, Op::Propagate, Op::MakeError,
            // IO
            Op::Print, Op::Println, Op::Rand,
            // Locals
            Op::Store, Op::Load,
            // HashMap
            Op::MapNew, Op::MapGet, Op::MapSet, Op::MapKeys, Op::MapHas,
            // Syscall
            Op::Syscall,
            // Reflection
            Op::Fetch, Op::Size,
            // Linear types
            Op::Linear, Op::Affine, Op::Consume, Op::IsLinear, Op::IsAffine,
            // Training primitives
            Op::Times, Op::Filter, Op::Head, Op::Tail, Op::Range,
            Op::First, Op::Second, Op::ListConcat, Op::IsEmpty,
            // Introspection
            Op::TypeOf, Op::Depth, Op::Describe,
            // String ops
            Op::StrLen, Op::StrGet, Op::StrConcat, Op::StrSlice, Op::ToStr, Op::StrFind,
            Op::StrSplit, Op::StrReplace, Op::StrUpper, Op::StrLower, Op::StrTrim,
            // Extension
            Op::Ext,
            // Halt
            Op::Halt,
        ]
    }

    /// Opcodes that the JIT native backend supports (integer-only, no heap types).
    /// Both compile() and compile_module() MUST handle exactly this set.
    /// This is the contract — add to it when adding JIT support for a new opcode.
    pub fn jit_opcodes() -> &'static [Op] {
        &[
            // Stack
            Op::Nop, Op::Drop, Op::Dup, Op::Swap, Op::Rot, Op::Over,
            // Literals (integer + float only, no Str/Quote)
            Op::Int8, Op::Int16, Op::Int32, Op::Int64, Op::F32, Op::F64,
            Op::Nil, Op::True, Op::False,
            // Integer arithmetic
            Op::Add, Op::Sub, Op::Mul, Op::Div, Op::Mod, Op::Neg,
            // Float arithmetic
            Op::Fadd, Op::Fsub, Op::Fmul, Op::Fdiv, Op::Fneg, Op::Fsqrt, Op::Fabs,
            Op::I2f, Op::F2i,
            // Comparison
            Op::Eq, Op::Lt, Op::Gt, Op::Le, Op::Ge, Op::Ne,
            // Logic + bitwise
            Op::And, Op::Or, Op::Not, Op::Xor,
            Op::Band, Op::Bor, Op::Bxor, Op::Bnot, Op::Shl, Op::Shr,
            // Jumps
            Op::Jmp, Op::Jz, Op::Jnz,
            // Call/Ret (compile_module only, compile() errors on these)
            Op::Call, Op::Ret,
            // Locals
            Op::Store, Op::Load,
            // Reflection
            Op::Fetch, Op::Size, Op::Depth,
            // Halt
            Op::Halt,
        ]
    }

    pub fn from_byte(b: u8) -> Option<Op> {
        match b {
            0x00 => Some(Op::Nop),
            0x01 => Some(Op::Drop),
            0x02 => Some(Op::Dup),
            0x03 => Some(Op::Swap),
            0x04 => Some(Op::Rot),
            0x05 => Some(Op::Over),
            0x10 => Some(Op::Pair),
            0x11 => Some(Op::Unpair),
            0x12 => Some(Op::Left),
            0x13 => Some(Op::Right),
            0x14 => Some(Op::Case),
            0x20 => Some(Op::Quote),
            0x21 => Some(Op::Apply),
            0x22 => Some(Op::Call),
            0x23 => Some(Op::Ret),
            0x24 => Some(Op::Cond),
            0x25 => Some(Op::Loop),
            0x30 => Some(Op::Int8),
            0x31 => Some(Op::Int16),
            0x32 => Some(Op::Int32),
            0x33 => Some(Op::Int64),
            0x34 => Some(Op::F32),
            0x35 => Some(Op::F64),
            0x36 => Some(Op::Str),
            0x37 => Some(Op::Nil),
            0x38 => Some(Op::True),
            0x39 => Some(Op::False),
            0x40 => Some(Op::Add),
            0x41 => Some(Op::Sub),
            0x42 => Some(Op::Mul),
            0x43 => Some(Op::Div),
            0x44 => Some(Op::Mod),
            0x45 => Some(Op::Neg),
            0x46 => Some(Op::Fadd),
            0x47 => Some(Op::Fsub),
            0x48 => Some(Op::Fmul),
            0x49 => Some(Op::Fdiv),
            0x4A => Some(Op::Fneg),
            0x4B => Some(Op::Fsqrt),
            0x4C => Some(Op::Fabs),
            0x4D => Some(Op::I2f),
            0x4E => Some(Op::F2i),
            0x4F => Some(Op::Fexp),
            0x50 => Some(Op::Eq),
            0x51 => Some(Op::Lt),
            0x52 => Some(Op::Gt),
            0x53 => Some(Op::Le),
            0x54 => Some(Op::Ge),
            0x55 => Some(Op::Ne),
            0x56 => Some(Op::Flog),
            0x57 => Some(Op::Fsin),
            0x58 => Some(Op::Fcos),
            0x59 => Some(Op::Fatan2),
            0x5A => Some(Op::Fpow),
            0x5B => Some(Op::Ffloor),
            0x5C => Some(Op::Fceil),
            0x5D => Some(Op::Fround),
            0x60 => Some(Op::And),
            0x61 => Some(Op::Or),
            0x62 => Some(Op::Not),
            0x63 => Some(Op::Xor),
            0x64 => Some(Op::Band),
            0x65 => Some(Op::Bor),
            0x66 => Some(Op::Bxor),
            0x67 => Some(Op::Bnot),
            0x68 => Some(Op::Shl),
            0x69 => Some(Op::Shr),
            0x70 => Some(Op::Jmp),
            0x71 => Some(Op::Jz),
            0x72 => Some(Op::Jnz),
            0x80 => Some(Op::List),
            0x81 => Some(Op::Unlist),
            0x82 => Some(Op::Len),
            0x83 => Some(Op::Get),
            0x84 => Some(Op::Set),
            0x85 => Some(Op::Map),
            0x86 => Some(Op::Fold),
            0x87 => Some(Op::Zip),
            0x88 => Some(Op::Append),
            0x89 => Some(Op::Reverse),
            0x8A => Some(Op::ArrayNew),
            0x8B => Some(Op::ArrayGet),
            0x8C => Some(Op::ArraySet),
            0x8D => Some(Op::ArrayLen),
            0x8E => Some(Op::ArrayPush),
            0x8F => Some(Op::ArrayFrom),
            0xB0 => Some(Op::FiberNew),
            0xB1 => Some(Op::FiberStep),
            0xB2 => Some(Op::FiberPush),
            0xB3 => Some(Op::FiberStack),
            0xB4 => Some(Op::FiberStatus),
            0xB5 => Some(Op::Spawn),
            0xB6 => Some(Op::ChanNew),
            0xB7 => Some(Op::ChanSend),
            0xB8 => Some(Op::ChanRecv),
            0xA0 => Some(Op::Print),
            0xA1 => Some(Op::Println),
            0xA2 => Some(Op::Rand),
            0xA3 => Some(Op::Try),
            0xA4 => Some(Op::Fail),
            0xA5 => Some(Op::IsError),
            0xA6 => Some(Op::Propagate),
            0xA7 => Some(Op::MakeError),
            0xA8 => Some(Op::Store),
            0xA9 => Some(Op::Load),
            0xAA => Some(Op::MapNew),
            0xAB => Some(Op::MapGet),
            0xAC => Some(Op::MapSet),
            0xAD => Some(Op::MapKeys),
            0xAE => Some(Op::MapHas),
            0xAF => Some(Op::Syscall),
            0x90 => Some(Op::Fetch),
            0x91 => Some(Op::Size),
            0xC0 => Some(Op::Linear),
            0xC1 => Some(Op::Affine),
            0xC2 => Some(Op::Consume),
            0xC3 => Some(Op::IsLinear),
            0xC4 => Some(Op::IsAffine),
            0xC5 => Some(Op::Times),
            0xC6 => Some(Op::Filter),
            0xC7 => Some(Op::Head),
            0xC8 => Some(Op::Tail),
            0xC9 => Some(Op::Range),
            0xCA => Some(Op::First),
            0xCB => Some(Op::Second),
            0xCC => Some(Op::ListConcat),
            0xCD => Some(Op::IsEmpty),
            0xD0 => Some(Op::TypeOf),
            0xD1 => Some(Op::Depth),
            0xD2 => Some(Op::Describe),
            0xE0 => Some(Op::StrLen),
            0xE1 => Some(Op::StrGet),
            0xE2 => Some(Op::StrConcat),
            0xE3 => Some(Op::StrSlice),
            0xE4 => Some(Op::ToStr),
            0xE5 => Some(Op::StrFind),
            0xE6 => Some(Op::StrSplit),
            0xE7 => Some(Op::StrReplace),
            0xE8 => Some(Op::StrUpper),
            0xE9 => Some(Op::StrLower),
            0xEA => Some(Op::StrTrim),
            0xF0 => Some(Op::Ext),
            0xFF => Some(Op::Halt),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Op::Nop => "nop",
            Op::Drop => "drop",
            Op::Dup => "dup",
            Op::Swap => "swap",
            Op::Rot => "rot",
            Op::Over => "over",
            Op::Pair => "pair",
            Op::Unpair => "unpair",
            Op::Left => "left",
            Op::Right => "right",
            Op::Case => "case",
            Op::Quote => "quote",
            Op::Apply => "apply",
            Op::Call => "call",
            Op::Ret => "ret",
            Op::Cond => "cond",
            Op::Loop => "loop",
            Op::Int8 => "i8",
            Op::Int16 => "i16",
            Op::Int32 => "i32",
            Op::Int64 => "i64",
            Op::F32 => "f32",
            Op::F64 => "f64",
            Op::Str => "str",
            Op::Nil => "nil",
            Op::True => "true",
            Op::False => "false",
            Op::Add => "add",
            Op::Sub => "sub",
            Op::Mul => "mul",
            Op::Div => "div",
            Op::Mod => "mod",
            Op::Neg => "neg",
            Op::Fadd => "fadd",
            Op::Fsub => "fsub",
            Op::Fmul => "fmul",
            Op::Fdiv => "fdiv",
            Op::Fneg => "fneg",
            Op::Fsqrt => "fsqrt",
            Op::Fabs => "fabs",
            Op::I2f => "i2f",
            Op::F2i => "f2i",
            Op::Fexp => "fexp",
            Op::Eq => "eq",
            Op::Lt => "lt",
            Op::Gt => "gt",
            Op::Le => "le",
            Op::Ge => "ge",
            Op::Ne => "ne",
            Op::Flog => "flog",
            Op::Fsin => "fsin",
            Op::Fcos => "fcos",
            Op::Fatan2 => "fatan2",
            Op::Fpow => "fpow",
            Op::Ffloor => "ffloor",
            Op::Fceil => "fceil",
            Op::Fround => "fround",
            Op::And => "and",
            Op::Or => "or",
            Op::Not => "not",
            Op::Xor => "xor",
            Op::Band => "band",
            Op::Bor => "bor",
            Op::Bxor => "bxor",
            Op::Bnot => "bnot",
            Op::Shl => "shl",
            Op::Shr => "shr",
            Op::Jmp => "jmp",
            Op::Jz => "jz",
            Op::Jnz => "jnz",
            Op::List => "list",
            Op::Unlist => "unlist",
            Op::Len => "len",
            Op::Get => "get",
            Op::Set => "set",
            Op::Map => "map",
            Op::Fold => "fold",
            Op::Zip => "zip",
            Op::Append => "append",
            Op::Reverse => "reverse",
            Op::ArrayNew => "array-new",
            Op::ArrayGet => "array-get",
            Op::ArraySet => "array-set",
            Op::ArrayLen => "array-len",
            Op::ArrayPush => "array-push",
            Op::ArrayFrom => "array-from",
            Op::FiberNew => "fiber-new",
            Op::FiberStep => "fiber-step",
            Op::FiberPush => "fiber-push",
            Op::FiberStack => "fiber-stack",
            Op::FiberStatus => "fiber-status",
            Op::Spawn => "spawn",
            Op::ChanNew => "chan-new",
            Op::ChanSend => "chan-send",
            Op::ChanRecv => "chan-recv",
            Op::Try => "try",
            Op::Fail => "fail",
            Op::IsError => "is-error",
            Op::Propagate => "?",
            Op::MakeError => "error",
            Op::Print => "print",
            Op::Println => "println",
            Op::Rand => "rand",
            Op::Store => "store",
            Op::Load => "load",
            Op::MapNew => "map-new",
            Op::MapGet => "map-get",
            Op::MapSet => "map-set",
            Op::MapKeys => "map-keys",
            Op::MapHas => "map-has",
            Op::Syscall => "syscall",
            Op::Fetch => "fetch",
            Op::Size => "size",
            Op::Linear => "linear",
            Op::Affine => "affine",
            Op::Consume => "consume",
            Op::IsLinear => "is-linear",
            Op::IsAffine => "is-affine",
            Op::Times => "times",
            Op::Filter => "filter",
            Op::Head => "head",
            Op::Tail => "tail",
            Op::Range => "range",
            Op::First => "first",
            Op::Second => "second",
            Op::ListConcat => "list-concat",
            Op::IsEmpty => "empty?",
            Op::TypeOf => "type-of",
            Op::Depth => "depth",
            Op::Describe => "describe",
            Op::StrLen => "str-len",
            Op::StrGet => "str-get",
            Op::StrConcat => "str-concat",
            Op::StrSlice => "str-slice",
            Op::ToStr => "to-str",
            Op::StrFind => "str-find",
            Op::StrSplit => "str-split",
            Op::StrReplace => "str-replace",
            Op::StrUpper => "str-upper",
            Op::StrLower => "str-lower",
            Op::StrTrim => "str-trim",
            Op::Ext => "ext",
            Op::Halt => "halt",
        }
    }
}

// ============================================================================
// BYTECODE STRUCTURE
// ============================================================================

pub const MAGIC: [u8; 4] = [0x4B, 0x4F, 0x52, 0x45]; // "KORE"
pub const VERSION_MAJOR: u8 = 0;
pub const VERSION_MINOR: u8 = 1;

/// Capability flag bits
pub const CAP_IO: u8  = 0x01;  // bit 0: print, println, rand
pub const CAP_FS: u8  = 0x02;  // bit 1: file read/write (future)
pub const CAP_NET: u8 = 0x04;  // bit 2: network (future)
pub const CAP_EXEC: u8 = 0x08; // bit 3: exec (shell commands)

#[derive(Debug, Clone)]
pub struct BytecodeModule {
    pub version: (u8, u8),
    pub cap_flags: u8,  // capability flags declared by module
    pub code: Vec<u8>,
    pub symbols: HashMap<String, usize>, // name -> code offset
    pub symbol_table: HashMap<u16, usize>, // index -> code offset (for CALL)
}

impl BytecodeModule {
    pub fn new() -> Self {
        BytecodeModule {
            version: (VERSION_MAJOR, VERSION_MINOR),
            cap_flags: 0,
            code: Vec::new(),
            symbols: HashMap::new(),
            symbol_table: HashMap::new(),
        }
    }

    /// Encode to binary format
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();

        // Magic
        out.extend_from_slice(&MAGIC);

        // Version
        out.push(self.version.0);
        out.push(self.version.1);

        // Flags: byte 6 = cap_flags, byte 7 = reserved
        out.push(self.cap_flags);
        out.push(0);

        // Encode symbol table
        let mut sym_data: Vec<u8> = Vec::new();
        let sym_count = self.symbol_table.len() as u16;
        sym_data.extend_from_slice(&sym_count.to_le_bytes());
        for (idx, offset) in &self.symbol_table {
            sym_data.extend_from_slice(&idx.to_le_bytes());
            sym_data.extend_from_slice(&(*offset as u32).to_le_bytes());
        }

        // Code starts at offset 32
        let code_offset: u32 = 32;
        let code_len: u32 = self.code.len() as u32;
        
        // Symbol table follows code
        let sym_offset: u32 = code_offset + code_len;
        let sym_len: u32 = sym_data.len() as u32;

        // Code section offset/length
        out.extend_from_slice(&code_offset.to_le_bytes());
        out.extend_from_slice(&code_len.to_le_bytes());

        // Data section (not used yet)
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());

        // Symbol table offset/length
        out.extend_from_slice(&sym_offset.to_le_bytes());
        out.extend_from_slice(&sym_len.to_le_bytes());

        // Pad to offset 32
        while out.len() < 32 {
            out.push(0);
        }

        // Code section
        out.extend_from_slice(&self.code);
        
        // Symbol table section
        out.extend_from_slice(&sym_data);

        out
    }

    /// Decode from binary format
    pub fn decode(data: &[u8]) -> Result<Self, String> {
        if data.len() < 32 {
            return Err("File too small".into());
        }

        if &data[0..4] != MAGIC {
            return Err("Invalid magic number".into());
        }

        let version = (data[4], data[5]);
        let cap_flags = data[6];

        let code_offset = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;
        let code_len = u32::from_le_bytes([data[12], data[13], data[14], data[15]]) as usize;
        
        let sym_offset = u32::from_le_bytes([data[24], data[25], data[26], data[27]]) as usize;
        let sym_len = u32::from_le_bytes([data[28], data[29], data[30], data[31]]) as usize;

        if data.len() < code_offset + code_len {
            return Err("Code section truncated".into());
        }

        let code = data[code_offset..code_offset + code_len].to_vec();
        
        // Decode symbol table
        let mut symbol_table = HashMap::new();
        if sym_len > 0 && data.len() >= sym_offset + sym_len {
            let sym_data = &data[sym_offset..sym_offset + sym_len];
            if sym_data.len() >= 2 {
                let count = u16::from_le_bytes([sym_data[0], sym_data[1]]) as usize;
                let mut pos = 2;
                for _ in 0..count {
                    if pos + 6 > sym_data.len() { break; }
                    let idx = u16::from_le_bytes([sym_data[pos], sym_data[pos + 1]]);
                    let offset = u32::from_le_bytes([
                        sym_data[pos + 2], sym_data[pos + 3], 
                        sym_data[pos + 4], sym_data[pos + 5]
                    ]) as usize;
                    symbol_table.insert(idx, offset);
                    pos += 6;
                }
            }
        }

        Ok(BytecodeModule {
            version,
            cap_flags,
            code,
            symbols: HashMap::new(),
            symbol_table,
        })
    }
}

// ============================================================================
// ASSEMBLER
// ============================================================================

pub struct Assembler {
    module: BytecodeModule,
    labels: HashMap<String, usize>,
    fixups: Vec<(usize, String)>, // (offset, label_name) for forward refs
    symbol_fixups: Vec<(u16, String)>, // (symbol_index, label_name)
    label_to_symbol: HashMap<String, u16>, // label -> symbol index
    next_symbol: u16,
}

impl Assembler {
    pub fn new() -> Self {
        Assembler {
            module: BytecodeModule::new(),
            labels: HashMap::new(),
            fixups: Vec::new(),
            symbol_fixups: Vec::new(),
            label_to_symbol: HashMap::new(),
            next_symbol: 0,
        }
    }

    pub fn emit(&mut self, op: Op) {
        self.module.code.push(op as u8);
    }

    /// Emit a raw byte (for syscall operands, etc.)
    pub fn emit_u8(&mut self, v: u8) {
        self.module.code.push(v);
    }

    pub fn emit_i8(&mut self, v: i8) {
        self.emit(Op::Int8);
        self.module.code.push(v as u8);
    }

    pub fn emit_i64(&mut self, v: i64) {
        self.emit(Op::Int64);
        self.module.code.extend_from_slice(&v.to_le_bytes());
    }

    pub fn emit_f64(&mut self, v: f64) {
        self.emit(Op::F64);
        self.module.code.extend_from_slice(&v.to_le_bytes());
    }

    pub fn emit_str(&mut self, s: &str) {
        self.emit(Op::Str);
        let bytes = s.as_bytes();
        self.module.code.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
        self.module.code.extend_from_slice(bytes);
    }

    pub fn emit_store(&mut self, slot: u8) {
        self.emit(Op::Store);
        self.module.code.push(slot);
    }

    pub fn emit_load(&mut self, slot: u8) {
        self.emit(Op::Load);
        self.module.code.push(slot);
    }

    pub fn emit_call(&mut self, symbol_idx: u16) {
        self.emit(Op::Call);
        self.module.code.extend_from_slice(&symbol_idx.to_le_bytes());
    }
    
    /// Emit a call to a labeled function
    pub fn emit_call_label(&mut self, label: &str) {
        // Reuse existing symbol index or assign new one
        let idx = if let Some(&existing) = self.label_to_symbol.get(label) {
            existing
        } else {
            let idx = self.next_symbol;
            self.next_symbol += 1;
            self.label_to_symbol.insert(label.to_string(), idx);
            self.symbol_fixups.push((idx, label.to_string()));
            idx
        };
        
        self.emit(Op::Call);
        self.module.code.extend_from_slice(&idx.to_le_bytes());
    }
    
    /// Get current code offset (for patching)
    pub fn current_offset(&self) -> usize {
        self.module.code.len()
    }
    
    /// Emit a direct jump (returns offset of the target field for patching)
    pub fn emit_jmp_direct(&mut self, target: i16) -> usize {
        self.emit(Op::Jmp);
        let offset = self.module.code.len();
        self.module.code.extend_from_slice(&target.to_le_bytes());
        offset
    }
    
    /// Patch a jump target (offset is from emit_jmp_direct)
    pub fn patch_jmp(&mut self, offset: usize, target: i32) {
        // Jump target is relative to the position after the jump instruction
        // jmp instruction: opcode (1 byte) + offset (2 bytes)
        // So target is relative to offset + 2
        let rel = (target as i64 - (offset as i64 + 2)) as i16;
        let bytes = rel.to_le_bytes();
        self.module.code[offset] = bytes[0];
        self.module.code[offset + 1] = bytes[1];
    }

    /// Emit a raw u16 value (for Quote length prefix, etc.)
    /// Returns the offset where the u16 was written (for patching).
    pub fn emit_raw_u16(&mut self, val: u16) -> usize {
        let offset = self.module.code.len();
        self.module.code.extend_from_slice(&val.to_le_bytes());
        offset
    }

    /// Patch a u16 value at a given offset.
    pub fn patch_u16(&mut self, offset: usize, val: u16) {
        let bytes = val.to_le_bytes();
        self.module.code[offset] = bytes[0];
        self.module.code[offset + 1] = bytes[1];
    }

    pub fn label(&mut self, name: &str) {
        self.labels.insert(name.to_string(), self.module.code.len());
    }

    #[allow(dead_code)]
    pub fn emit_jmp(&mut self, label: &str) {
        self.emit(Op::Jmp);
        self.fixups.push((self.module.code.len(), label.to_string()));
        self.module.code.extend_from_slice(&0i16.to_le_bytes()); // placeholder
    }

    pub fn emit_jz(&mut self, label: &str) {
        self.emit(Op::Jz);
        self.fixups.push((self.module.code.len(), label.to_string()));
        self.module.code.extend_from_slice(&0i16.to_le_bytes());
    }

    #[allow(dead_code)]
    pub fn emit_jnz(&mut self, label: &str) {
        self.emit(Op::Jnz);
        self.fixups.push((self.module.code.len(), label.to_string()));
        self.module.code.extend_from_slice(&0i16.to_le_bytes());
    }

    pub fn finalize(mut self) -> Result<BytecodeModule, String> {
        // Resolve label fixups (jumps)
        for (offset, label) in &self.fixups {
            let target = self.labels.get(label)
                .ok_or_else(|| format!("Unknown label: {}", label))?;
            let rel = (*target as i64 - *offset as i64 - 2) as i16;
            let bytes = rel.to_le_bytes();
            self.module.code[*offset] = bytes[0];
            self.module.code[*offset + 1] = bytes[1];
        }
        
        // Resolve symbol fixups (calls)
        for (idx, label) in &self.symbol_fixups {
            let target = self.labels.get(label)
                .ok_or_else(|| format!("Unknown function: {}", label))?;
            self.module.symbols.insert(label.clone(), *target);
            self.module.symbol_table.insert(*idx, *target);
        }
        
        Ok(self.module)
    }
}

// ============================================================================
// DISASSEMBLER
// ============================================================================

pub fn disassemble(code: &[u8]) -> String {
    let mut out = String::new();
    let mut i = 0;

    while i < code.len() {
        out.push_str(&format!("{:04X}: ", i));

        let op = match Op::from_byte(code[i]) {
            Some(op) => op,
            None => {
                out.push_str(&format!("??? ({:02X})\n", code[i]));
                i += 1;
                continue;
            }
        };

        out.push_str(op.name());

        match op {
            Op::Int8 if i + 1 < code.len() => {
                out.push_str(&format!(" {}", code[i + 1] as i8));
                i += 2;
            }
            Op::Int16 if i + 2 < code.len() => {
                let v = i16::from_le_bytes([code[i + 1], code[i + 2]]);
                out.push_str(&format!(" {}", v));
                i += 3;
            }
            Op::Int32 if i + 4 < code.len() => {
                let v = i32::from_le_bytes([code[i + 1], code[i + 2], code[i + 3], code[i + 4]]);
                out.push_str(&format!(" {}", v));
                i += 5;
            }
            Op::Int64 if i + 8 < code.len() => {
                let v = i64::from_le_bytes([
                    code[i + 1], code[i + 2], code[i + 3], code[i + 4],
                    code[i + 5], code[i + 6], code[i + 7], code[i + 8],
                ]);
                out.push_str(&format!(" {}", v));
                i += 9;
            }
            Op::F64 if i + 8 < code.len() => {
                let v = f64::from_le_bytes([
                    code[i + 1], code[i + 2], code[i + 3], code[i + 4],
                    code[i + 5], code[i + 6], code[i + 7], code[i + 8],
                ]);
                out.push_str(&format!(" {}", v));
                i += 9;
            }
            Op::Call if i + 2 < code.len() => {
                let idx = u16::from_le_bytes([code[i + 1], code[i + 2]]);
                out.push_str(&format!(" @{}", idx));
                i += 3;
            }
            Op::Jmp | Op::Jz | Op::Jnz if i + 2 < code.len() => {
                let rel = i16::from_le_bytes([code[i + 1], code[i + 2]]);
                let target = (i as i64 + 3 + rel as i64) as usize;
                out.push_str(&format!(" {:04X}", target));
                i += 3;
            }
            Op::Str if i + 2 < code.len() => {
                let len = u16::from_le_bytes([code[i + 1], code[i + 2]]) as usize;
                if i + 3 + len <= code.len() {
                    let s = String::from_utf8_lossy(&code[i + 3..i + 3 + len]);
                    out.push_str(&format!(" \"{}\"", s));
                }
                i += 3 + len;
            }
            Op::Quote if i + 2 < code.len() => {
                let len = u16::from_le_bytes([code[i + 1], code[i + 2]]) as usize;
                out.push_str(&format!(" [{}]", len));
                i += 3; // body follows inline, disassembled normally
            }
            Op::Print | Op::Println | Op::Rand => {
                i += 1;
            }
            Op::Store if i + 1 < code.len() => {
                out.push_str(&format!(" {}", code[i + 1]));
                i += 2;
            }
            Op::Load if i + 1 < code.len() => {
                out.push_str(&format!(" {}", code[i + 1]));
                i += 2;
            }
            Op::List if i + 2 < code.len() => {
                let count = u16::from_le_bytes([code[i + 1], code[i + 2]]);
                out.push_str(&format!(" ({})", count));
                i += 3;
            }
            Op::Syscall if i + 1 < code.len() => {
                let call_id = code[i + 1];
                let name = match call_id {
                    0x01 => "file-read",
                    0x02 => "file-write",
                    0x03 => "file-exists",
                    0x10 => "time-now",
                    0x20 => "env-get",
                    0x30 => "exec",
                    0x04 => "readline",
                    0x05 => "file-append",
                    0x06 => "file-delete",
                    0x07 => "file-list",
                    0x40 => "http-get",
                    0x41 => "http-post",
                    0x50 => "http-serve",
                    0x51 => "http-respond",
                    _ => "unknown",
                };
                out.push_str(&format!(" 0x{:02X} ({})", call_id, name));
                i += 2;
            }
            _ => {
                i += 1;
            }
        }

        out.push('\n');
    }

    out
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_factorial_bytecode() {
        let mut asm = Assembler::new();

        // 5 factorial
        asm.emit_i8(5);
        asm.emit_call(0); // factorial
        asm.emit(Op::Halt);

        // factorial function (symbol 0)
        asm.label("factorial");
        asm.emit(Op::Dup);
        asm.emit_i8(1);
        asm.emit(Op::Le);
        asm.emit_jz("recurse");
        // base case
        asm.emit(Op::Drop);
        asm.emit_i8(1);
        asm.emit(Op::Ret);
        // recursive case
        asm.label("recurse");
        asm.emit(Op::Dup);
        asm.emit_i8(1);
        asm.emit(Op::Sub);
        asm.emit_call(0); // recurse
        asm.emit(Op::Mul);
        asm.emit(Op::Ret);

        let module = asm.finalize().unwrap();
        let binary = module.encode();

        println!("Binary size: {} bytes", binary.len());
        println!("\nDisassembly:\n{}", disassemble(&module.code));

        // Verify round-trip
        let decoded = BytecodeModule::decode(&binary).unwrap();
        assert_eq!(decoded.code, module.code);
    }

    #[test]
    fn test_simple_math() {
        let mut asm = Assembler::new();
        asm.emit_i8(3);
        asm.emit_i8(4);
        asm.emit(Op::Add);
        asm.emit(Op::Halt);

        let module = asm.finalize().unwrap();
        println!("3 + 4:\n{}", disassemble(&module.code));
    }
}
