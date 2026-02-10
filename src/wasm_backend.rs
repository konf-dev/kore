//! WASM Backend for Kore Bytecode
//!
//! This module compiles Kore bytecode to WebAssembly.
//! 
//! P1: Tools remain tools - WASM functions are still S → S
//! P2: Apply is the only operation - WASM call is our apply
//! P3: Composition = Concatenation - WASM instructions concat
//! P4: Types checked by proof_checker before we get here
//!
//! Target: wasm32-unknown-unknown (no WASI, pure compute)

/// WASM value types
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WasmType {
    I32,
    I64,
    F32,
    F64,
}

/// WASM instruction opcodes (subset we need)
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum WasmOp {
    // Control
    Unreachable,        // 0x00
    Nop,                // 0x01
    Block(i32),         // 0x02 (block type)
    Loop(i32),          // 0x03
    If(i32),            // 0x04
    Else,               // 0x05
    End,                // 0x0B
    Br(u32),            // 0x0C (label index)
    BrIf(u32),          // 0x0D
    Return,             // 0x0F
    Call(u32),          // 0x10 (function index)
    
    // Locals
    LocalGet(u32),      // 0x20
    LocalSet(u32),      // 0x21
    LocalTee(u32),      // 0x22
    
    // Memory
    I32Load(u32, u32),  // 0x28 (align, offset)
    I64Load(u32, u32),  // 0x29
    F64Load(u32, u32),  // 0x2B
    I32Store(u32, u32), // 0x36
    I64Store(u32, u32), // 0x37
    F64Store(u32, u32), // 0x39
    
    // Constants
    I32Const(i32),      // 0x41
    I64Const(i64),      // 0x42
    F64Const(f64),      // 0x44
    
    // Numeric i32
    I32Eqz,             // 0x45
    I32Eq,              // 0x46
    I32Ne,              // 0x47
    I32LtS,             // 0x48
    I32GtS,             // 0x4A
    I32LeS,             // 0x4C
    I32GeS,             // 0x4E
    I32Add,             // 0x6A
    I32Sub,             // 0x6B
    I32Mul,             // 0x6C
    I32DivS,            // 0x6D
    I32RemS,            // 0x6F
    I32And,             // 0x71
    I32Or,              // 0x72
    I32Xor,             // 0x73
    
    // Numeric i64
    I64Eqz,             // 0x50
    I64Eq,              // 0x51
    I64LtS,             // 0x53
    I64GtS,             // 0x55
    I64LeS,             // 0x57
    I64GeS,             // 0x59
    I64Add,             // 0x7C
    I64Sub,             // 0x7D
    I64Mul,             // 0x7E
    I64DivS,            // 0x7F
    I64RemS,            // 0x81
    
    // Numeric f64
    F64Eq,              // 0x61
    F64Lt,              // 0x63
    F64Gt,              // 0x64
    F64Le,              // 0x65
    F64Ge,              // 0x66
    F64Add,             // 0xA0
    F64Sub,             // 0xA1
    F64Mul,             // 0xA2
    F64Div,             // 0xA3
    F64Neg,             // 0x9A
    
    // Conversions
    I64ExtendI32S,      // 0xAC
    I32WrapI64,         // 0xA7
    F64ConvertI64S,     // 0xB9
    I64TruncF64S,       // 0xB0
}

/// WASM module builder
pub struct WasmModule {
    /// Type section entries
    types: Vec<FuncType>,
    
    /// Function section (type indices)
    functions: Vec<u32>,
    
    /// Memory section
    memories: Vec<MemoryType>,
    
    /// Export section
    exports: Vec<Export>,
    
    /// Code section
    codes: Vec<FunctionBody>,
    
    /// Data section
    data: Vec<DataSegment>,
}

#[derive(Debug, Clone)]
pub struct FuncType {
    pub params: Vec<WasmType>,
    pub results: Vec<WasmType>,
}

#[derive(Debug, Clone)]
pub struct MemoryType {
    pub min_pages: u32,
    pub max_pages: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct Export {
    pub name: String,
    pub kind: ExportKind,
    pub index: u32,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum ExportKind {
    Func = 0,
    Table = 1,
    Memory = 2,
    Global = 3,
}

#[derive(Debug, Clone)]
pub struct FunctionBody {
    pub locals: Vec<(u32, WasmType)>,  // count, type
    pub code: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct DataSegment {
    pub offset: u32,
    pub data: Vec<u8>,
}

impl WasmModule {
    pub fn new() -> Self {
        WasmModule {
            types: Vec::new(),
            functions: Vec::new(),
            memories: Vec::new(),
            exports: Vec::new(),
            codes: Vec::new(),
            data: Vec::new(),
        }
    }
    
    /// Add a function type, return its index
    pub fn add_type(&mut self, func_type: FuncType) -> u32 {
        let idx = self.types.len() as u32;
        self.types.push(func_type);
        idx
    }
    
    /// Add a function, return its index
    pub fn add_function(&mut self, type_idx: u32, body: FunctionBody) -> u32 {
        let idx = self.functions.len() as u32;
        self.functions.push(type_idx);
        self.codes.push(body);
        idx
    }
    
    /// Add memory
    pub fn add_memory(&mut self, mem: MemoryType) -> u32 {
        let idx = self.memories.len() as u32;
        self.memories.push(mem);
        idx
    }
    
    /// Add export
    pub fn add_export(&mut self, export: Export) {
        self.exports.push(export);
    }
    
    /// Encode to binary WASM format
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        
        // Magic number
        out.extend_from_slice(b"\0asm");
        
        // Version
        out.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);
        
        // Type section (1)
        if !self.types.is_empty() {
            let mut section = Vec::new();
            write_leb128_u32(&mut section, self.types.len() as u32);
            for ft in &self.types {
                section.push(0x60);  // functype
                write_leb128_u32(&mut section, ft.params.len() as u32);
                for p in &ft.params {
                    section.push(wasm_type_byte(*p));
                }
                write_leb128_u32(&mut section, ft.results.len() as u32);
                for r in &ft.results {
                    section.push(wasm_type_byte(*r));
                }
            }
            write_section(&mut out, 1, &section);
        }
        
        // Function section (3)
        if !self.functions.is_empty() {
            let mut section = Vec::new();
            write_leb128_u32(&mut section, self.functions.len() as u32);
            for idx in &self.functions {
                write_leb128_u32(&mut section, *idx);
            }
            write_section(&mut out, 3, &section);
        }
        
        // Memory section (5)
        if !self.memories.is_empty() {
            let mut section = Vec::new();
            write_leb128_u32(&mut section, self.memories.len() as u32);
            for mem in &self.memories {
                if let Some(max) = mem.max_pages {
                    section.push(0x01);  // has max
                    write_leb128_u32(&mut section, mem.min_pages);
                    write_leb128_u32(&mut section, max);
                } else {
                    section.push(0x00);  // no max
                    write_leb128_u32(&mut section, mem.min_pages);
                }
            }
            write_section(&mut out, 5, &section);
        }
        
        // Export section (7)
        if !self.exports.is_empty() {
            let mut section = Vec::new();
            write_leb128_u32(&mut section, self.exports.len() as u32);
            for exp in &self.exports {
                write_leb128_u32(&mut section, exp.name.len() as u32);
                section.extend_from_slice(exp.name.as_bytes());
                section.push(exp.kind as u8);
                write_leb128_u32(&mut section, exp.index);
            }
            write_section(&mut out, 7, &section);
        }
        
        // Code section (10)
        if !self.codes.is_empty() {
            let mut section = Vec::new();
            write_leb128_u32(&mut section, self.codes.len() as u32);
            for body in &self.codes {
                let mut func = Vec::new();
                write_leb128_u32(&mut func, body.locals.len() as u32);
                for (count, ty) in &body.locals {
                    write_leb128_u32(&mut func, *count);
                    func.push(wasm_type_byte(*ty));
                }
                func.extend_from_slice(&body.code);
                
                write_leb128_u32(&mut section, func.len() as u32);
                section.extend_from_slice(&func);
            }
            write_section(&mut out, 10, &section);
        }
        
        // Data section (11)
        if !self.data.is_empty() {
            let mut section = Vec::new();
            write_leb128_u32(&mut section, self.data.len() as u32);
            for seg in &self.data {
                section.push(0x00);  // active segment, memory 0
                // i32.const offset, end
                section.push(0x41);
                write_leb128_i32(&mut section, seg.offset as i32);
                section.push(0x0B);  // end
                write_leb128_u32(&mut section, seg.data.len() as u32);
                section.extend_from_slice(&seg.data);
            }
            write_section(&mut out, 11, &section);
        }
        
        out
    }
}

// === WASM Encoding Helpers ===

fn wasm_type_byte(t: WasmType) -> u8 {
    match t {
        WasmType::I32 => 0x7F,
        WasmType::I64 => 0x7E,
        WasmType::F32 => 0x7D,
        WasmType::F64 => 0x7C,
    }
}

fn write_section(out: &mut Vec<u8>, id: u8, content: &[u8]) {
    out.push(id);
    write_leb128_u32(out, content.len() as u32);
    out.extend_from_slice(content);
}

fn write_leb128_u32(out: &mut Vec<u8>, mut value: u32) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
}

fn write_leb128_i32(out: &mut Vec<u8>, mut value: i32) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        let done = (value == 0 && (byte & 0x40) == 0) 
                || (value == -1 && (byte & 0x40) != 0);
        if !done {
            byte |= 0x80;
        }
        out.push(byte);
        if done {
            break;
        }
    }
}

fn write_leb128_i64(out: &mut Vec<u8>, mut value: i64) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        let done = (value == 0 && (byte & 0x40) == 0) 
                || (value == -1 && (byte & 0x40) != 0);
        if !done {
            byte |= 0x80;
        }
        out.push(byte);
        if done {
            break;
        }
    }
}

/// WASM instruction encoder
pub struct WasmEncoder {
    code: Vec<u8>,
}

impl WasmEncoder {
    pub fn new() -> Self {
        WasmEncoder { code: Vec::new() }
    }
    
    pub fn emit(&mut self, op: WasmOp) {
        match op {
            WasmOp::Unreachable => self.code.push(0x00),
            WasmOp::Nop => self.code.push(0x01),
            WasmOp::Block(t) => { self.code.push(0x02); write_leb128_i32(&mut self.code, t); }
            WasmOp::Loop(t) => { self.code.push(0x03); write_leb128_i32(&mut self.code, t); }
            WasmOp::If(t) => { self.code.push(0x04); write_leb128_i32(&mut self.code, t); }
            WasmOp::Else => self.code.push(0x05),
            WasmOp::End => self.code.push(0x0B),
            WasmOp::Br(l) => { self.code.push(0x0C); write_leb128_u32(&mut self.code, l); }
            WasmOp::BrIf(l) => { self.code.push(0x0D); write_leb128_u32(&mut self.code, l); }
            WasmOp::Return => self.code.push(0x0F),
            WasmOp::Call(f) => { self.code.push(0x10); write_leb128_u32(&mut self.code, f); }
            
            WasmOp::LocalGet(i) => { self.code.push(0x20); write_leb128_u32(&mut self.code, i); }
            WasmOp::LocalSet(i) => { self.code.push(0x21); write_leb128_u32(&mut self.code, i); }
            WasmOp::LocalTee(i) => { self.code.push(0x22); write_leb128_u32(&mut self.code, i); }
            
            WasmOp::I32Load(a, o) => { 
                self.code.push(0x28); 
                write_leb128_u32(&mut self.code, a);
                write_leb128_u32(&mut self.code, o);
            }
            WasmOp::I64Load(a, o) => {
                self.code.push(0x29);
                write_leb128_u32(&mut self.code, a);
                write_leb128_u32(&mut self.code, o);
            }
            WasmOp::F64Load(a, o) => {
                self.code.push(0x2B);
                write_leb128_u32(&mut self.code, a);
                write_leb128_u32(&mut self.code, o);
            }
            WasmOp::I32Store(a, o) => {
                self.code.push(0x36);
                write_leb128_u32(&mut self.code, a);
                write_leb128_u32(&mut self.code, o);
            }
            WasmOp::I64Store(a, o) => {
                self.code.push(0x37);
                write_leb128_u32(&mut self.code, a);
                write_leb128_u32(&mut self.code, o);
            }
            WasmOp::F64Store(a, o) => {
                self.code.push(0x39);
                write_leb128_u32(&mut self.code, a);
                write_leb128_u32(&mut self.code, o);
            }
            
            WasmOp::I32Const(v) => { self.code.push(0x41); write_leb128_i32(&mut self.code, v); }
            WasmOp::I64Const(v) => { self.code.push(0x42); write_leb128_i64(&mut self.code, v); }
            WasmOp::F64Const(v) => { 
                self.code.push(0x44); 
                self.code.extend_from_slice(&v.to_le_bytes());
            }
            
            WasmOp::I32Eqz => self.code.push(0x45),
            WasmOp::I32Eq => self.code.push(0x46),
            WasmOp::I32Ne => self.code.push(0x47),
            WasmOp::I32LtS => self.code.push(0x48),
            WasmOp::I32GtS => self.code.push(0x4A),
            WasmOp::I32LeS => self.code.push(0x4C),
            WasmOp::I32GeS => self.code.push(0x4E),
            WasmOp::I32Add => self.code.push(0x6A),
            WasmOp::I32Sub => self.code.push(0x6B),
            WasmOp::I32Mul => self.code.push(0x6C),
            WasmOp::I32DivS => self.code.push(0x6D),
            WasmOp::I32RemS => self.code.push(0x6F),
            WasmOp::I32And => self.code.push(0x71),
            WasmOp::I32Or => self.code.push(0x72),
            WasmOp::I32Xor => self.code.push(0x73),
            
            WasmOp::I64Eqz => self.code.push(0x50),
            WasmOp::I64Eq => self.code.push(0x51),
            WasmOp::I64LtS => self.code.push(0x53),
            WasmOp::I64GtS => self.code.push(0x55),
            WasmOp::I64LeS => self.code.push(0x57),
            WasmOp::I64GeS => self.code.push(0x59),
            WasmOp::I64Add => self.code.push(0x7C),
            WasmOp::I64Sub => self.code.push(0x7D),
            WasmOp::I64Mul => self.code.push(0x7E),
            WasmOp::I64DivS => self.code.push(0x7F),
            WasmOp::I64RemS => self.code.push(0x81),
            
            WasmOp::F64Eq => self.code.push(0x61),
            WasmOp::F64Lt => self.code.push(0x63),
            WasmOp::F64Gt => self.code.push(0x64),
            WasmOp::F64Le => self.code.push(0x65),
            WasmOp::F64Ge => self.code.push(0x66),
            WasmOp::F64Add => self.code.push(0xA0),
            WasmOp::F64Sub => self.code.push(0xA1),
            WasmOp::F64Mul => self.code.push(0xA2),
            WasmOp::F64Div => self.code.push(0xA3),
            WasmOp::F64Neg => self.code.push(0x9A),
            
            WasmOp::I64ExtendI32S => self.code.push(0xAC),
            WasmOp::I32WrapI64 => self.code.push(0xA7),
            WasmOp::F64ConvertI64S => self.code.push(0xB9),
            WasmOp::I64TruncF64S => self.code.push(0xB0),
        }
    }
    
    pub fn finish(mut self) -> Vec<u8> {
        self.code.push(0x0B);  // end
        self.code
    }
}

// ============================================================================
// KORE → WASM COMPILER
// ============================================================================

/// Compile Kore bytecode to WASM
pub fn compile_to_wasm(kore_code: &[u8]) -> Result<Vec<u8>, String> {
    let mut module = WasmModule::new();
    
    // Add memory (1 page = 64KB for stack)
    module.add_memory(MemoryType { min_pages: 1, max_pages: Some(16) });
    
    // Export memory
    module.add_export(Export {
        name: "memory".into(),
        kind: ExportKind::Memory,
        index: 0,
    });
    
    // Main function type: () -> i64 (returns top of stack)
    let main_type = module.add_type(FuncType {
        params: vec![],
        results: vec![WasmType::I64],
    });
    
    // Compile the main function
    let body = compile_function(kore_code)?;
    let main_idx = module.add_function(main_type, body);
    
    // Export main
    module.add_export(Export {
        name: "main".into(),
        kind: ExportKind::Func,
        index: main_idx,
    });
    
    Ok(module.encode())
}

/// Compile Kore bytecode to a WASM function body
fn compile_function(kore_code: &[u8]) -> Result<FunctionBody, String> {
    let mut enc = WasmEncoder::new();
    
    // Local 0: stack pointer (i32)
    // Stack grows up from address 0
    // Each value is 8 bytes (i64)
    
    // Initialize stack pointer to 0
    enc.emit(WasmOp::I32Const(0));
    enc.emit(WasmOp::LocalSet(0));
    
    let mut pc = 0;
    while pc < kore_code.len() {
        let op = kore_code[pc];
        pc += 1;
        
        match op {
            // NOP
            0x00 => enc.emit(WasmOp::Nop),
            
            // DROP: pop from stack
            0x01 => {
                // sp -= 8
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::I32Const(8));
                enc.emit(WasmOp::I32Sub);
                enc.emit(WasmOp::LocalSet(0));
            }
            
            // DUP: duplicate top
            0x02 => {
                // val = mem[sp-8]; mem[sp] = val; sp += 8
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::I32Const(8));
                enc.emit(WasmOp::I32Sub);
                enc.emit(WasmOp::I64Load(3, 0));  // load top value
                enc.emit(WasmOp::LocalSet(1));    // save to local 1
                
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::LocalGet(1));
                enc.emit(WasmOp::I64Store(3, 0)); // store at sp
                
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::I32Const(8));
                enc.emit(WasmOp::I32Add);
                enc.emit(WasmOp::LocalSet(0));    // sp += 8
            }
            
            // SWAP: swap top two
            0x03 => {
                // a = mem[sp-16]; b = mem[sp-8]
                // mem[sp-16] = b; mem[sp-8] = a
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::I32Const(16));
                enc.emit(WasmOp::I32Sub);
                enc.emit(WasmOp::I64Load(3, 0));
                enc.emit(WasmOp::LocalSet(1));    // a = local 1
                
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::I32Const(8));
                enc.emit(WasmOp::I32Sub);
                enc.emit(WasmOp::I64Load(3, 0));
                enc.emit(WasmOp::LocalSet(2));    // b = local 2
                
                // store b at sp-16
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::I32Const(16));
                enc.emit(WasmOp::I32Sub);
                enc.emit(WasmOp::LocalGet(2));
                enc.emit(WasmOp::I64Store(3, 0));
                
                // store a at sp-8
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::I32Const(8));
                enc.emit(WasmOp::I32Sub);
                enc.emit(WasmOp::LocalGet(1));
                enc.emit(WasmOp::I64Store(3, 0));
            }
            
            // INT8: push small constant
            0x30 => {
                let val = kore_code[pc] as i8 as i64;
                pc += 1;
                
                // mem[sp] = val; sp += 8
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::I64Const(val));
                enc.emit(WasmOp::I64Store(3, 0));
                
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::I32Const(8));
                enc.emit(WasmOp::I32Add);
                enc.emit(WasmOp::LocalSet(0));
            }
            
            // ADD
            0x40 => {
                // b = pop; a = pop; push(a+b)
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::I32Const(8));
                enc.emit(WasmOp::I32Sub);
                enc.emit(WasmOp::I64Load(3, 0));
                enc.emit(WasmOp::LocalSet(2));    // b
                
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::I32Const(16));
                enc.emit(WasmOp::I32Sub);
                enc.emit(WasmOp::I64Load(3, 0));
                enc.emit(WasmOp::LocalSet(1));    // a
                
                // sp -= 8 (one value consumed)
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::I32Const(8));
                enc.emit(WasmOp::I32Sub);
                enc.emit(WasmOp::LocalSet(0));
                
                // store a+b at sp-8
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::I32Const(8));
                enc.emit(WasmOp::I32Sub);
                enc.emit(WasmOp::LocalGet(1));
                enc.emit(WasmOp::LocalGet(2));
                enc.emit(WasmOp::I64Add);
                enc.emit(WasmOp::I64Store(3, 0));
            }
            
            // SUB
            0x41 => {
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::I32Const(8));
                enc.emit(WasmOp::I32Sub);
                enc.emit(WasmOp::I64Load(3, 0));
                enc.emit(WasmOp::LocalSet(2));
                
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::I32Const(16));
                enc.emit(WasmOp::I32Sub);
                enc.emit(WasmOp::I64Load(3, 0));
                enc.emit(WasmOp::LocalSet(1));
                
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::I32Const(8));
                enc.emit(WasmOp::I32Sub);
                enc.emit(WasmOp::LocalSet(0));
                
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::I32Const(8));
                enc.emit(WasmOp::I32Sub);
                enc.emit(WasmOp::LocalGet(1));
                enc.emit(WasmOp::LocalGet(2));
                enc.emit(WasmOp::I64Sub);
                enc.emit(WasmOp::I64Store(3, 0));
            }
            
            // MUL
            0x42 => {
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::I32Const(8));
                enc.emit(WasmOp::I32Sub);
                enc.emit(WasmOp::I64Load(3, 0));
                enc.emit(WasmOp::LocalSet(2));
                
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::I32Const(16));
                enc.emit(WasmOp::I32Sub);
                enc.emit(WasmOp::I64Load(3, 0));
                enc.emit(WasmOp::LocalSet(1));
                
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::I32Const(8));
                enc.emit(WasmOp::I32Sub);
                enc.emit(WasmOp::LocalSet(0));
                
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::I32Const(8));
                enc.emit(WasmOp::I32Sub);
                enc.emit(WasmOp::LocalGet(1));
                enc.emit(WasmOp::LocalGet(2));
                enc.emit(WasmOp::I64Mul);
                enc.emit(WasmOp::I64Store(3, 0));
            }
            
            // LE (less than or equal)
            0x53 => {
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::I32Const(8));
                enc.emit(WasmOp::I32Sub);
                enc.emit(WasmOp::I64Load(3, 0));
                enc.emit(WasmOp::LocalSet(2));
                
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::I32Const(16));
                enc.emit(WasmOp::I32Sub);
                enc.emit(WasmOp::I64Load(3, 0));
                enc.emit(WasmOp::LocalSet(1));
                
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::I32Const(8));
                enc.emit(WasmOp::I32Sub);
                enc.emit(WasmOp::LocalSet(0));
                
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::I32Const(8));
                enc.emit(WasmOp::I32Sub);
                enc.emit(WasmOp::LocalGet(1));
                enc.emit(WasmOp::LocalGet(2));
                enc.emit(WasmOp::I64LeS);
                enc.emit(WasmOp::I64ExtendI32S);
                enc.emit(WasmOp::I64Store(3, 0));
            }
            
            // FETCH: Not yet supported in WASM (needs bytecode in memory)
            0x90 => {
                // TODO: Implement by embedding bytecode in WASM memory
                // For now, pop offset and push 0
                // Stack: read offset (already there), replace with 0
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::I32Const(8));
                enc.emit(WasmOp::I32Sub);
                enc.emit(WasmOp::I64Const(0));   // push 0 (placeholder) - overwrites offset
                enc.emit(WasmOp::I64Store(3, 0));
            }
            
            // SIZE: Not yet supported in WASM (needs bytecode in memory)
            0x91 => {
                // TODO: Implement by storing bytecode length
                // Stack: push 0
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::I64Const(0));   // push 0 (placeholder)
                enc.emit(WasmOp::I64Store(3, 0));
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::I32Const(8));
                enc.emit(WasmOp::I32Add);
                enc.emit(WasmOp::LocalSet(0));
            }
            
            // HALT: return top of stack
            0xFF => {
                enc.emit(WasmOp::LocalGet(0));
                enc.emit(WasmOp::I32Const(8));
                enc.emit(WasmOp::I32Sub);
                enc.emit(WasmOp::I64Load(3, 0));
                enc.emit(WasmOp::Return);
            }
            
            _ => {
                // For now, skip unimplemented opcodes
                // In full implementation, each opcode would be translated
            }
        }
    }
    
    // Return 0 if we fall through
    enc.emit(WasmOp::I64Const(0));
    
    Ok(FunctionBody {
        locals: vec![
            (1, WasmType::I32),  // local 0: stack pointer
            (2, WasmType::I64),  // local 1-2: temp values
        ],
        code: enc.finish(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::{Assembler, Op};
    
    #[test]
    fn test_wasm_encode_simple() {
        let mut module = WasmModule::new();
        
        // Add type: () -> i64
        let type_idx = module.add_type(FuncType {
            params: vec![],
            results: vec![WasmType::I64],
        });
        
        // Add function that returns 42
        let mut enc = WasmEncoder::new();
        enc.emit(WasmOp::I64Const(42));
        
        module.add_function(type_idx, FunctionBody {
            locals: vec![],
            code: enc.finish(),
        });
        
        let wasm = module.encode();
        
        // Check magic number
        assert_eq!(&wasm[0..4], b"\0asm");
        // Check version
        assert_eq!(&wasm[4..8], &[0x01, 0x00, 0x00, 0x00]);
    }
    
    #[test]
    fn test_compile_simple_add() {
        let mut asm = Assembler::new();
        asm.emit_i8(3);
        asm.emit_i8(4);
        asm.emit(Op::Add);
        asm.emit(Op::Halt);
        
        let module = asm.finalize().unwrap();
        let wasm = compile_to_wasm(&module.code).unwrap();
        
        // Basic sanity check
        assert!(wasm.len() > 8);
        assert_eq!(&wasm[0..4], b"\0asm");
    }
}
