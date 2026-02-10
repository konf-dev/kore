//! SPIR-V Backend for Kore Bytecode
//!
//! Compiles Kore bytecode to SPIR-V for GPU execution.
//!
//! P1: Tools remain tools - SPIR-V functions are still S → S
//! P2: Apply is the only operation - shader invocation is our apply
//! P3: Composition = Concatenation - SPIR-V instructions concat
//! P4: Types checked by proof_checker before we get here
//!
//! Target: Vulkan compute shaders (no graphics, pure compute)

use std::collections::HashMap;

// ============================================================================
// SPIR-V CONSTANTS
// ============================================================================

/// SPIR-V magic number
const SPIRV_MAGIC: u32 = 0x07230203;

/// SPIR-V version (1.0)
const SPIRV_VERSION: u32 = 0x00010000;

/// Generator magic (Kore = 0x4B4F5245)
const GENERATOR_MAGIC: u32 = 0x4B4F5245;

// Capability constants
const CAPABILITY_SHADER: u32 = 1;

// Execution model
const EXECUTION_MODEL_GLCOMPUTE: u32 = 5;

// Addressing model  
const ADDRESSING_MODEL_LOGICAL: u32 = 0;

// Memory model
const MEMORY_MODEL_GLSL450: u32 = 1;

// Storage classes
#[allow(dead_code)]
const STORAGE_CLASS_UNIFORM_CONSTANT: u32 = 0;
#[allow(dead_code)]
const STORAGE_CLASS_INPUT: u32 = 1;
#[allow(dead_code)]
const STORAGE_CLASS_UNIFORM: u32 = 2;
#[allow(dead_code)]
const STORAGE_CLASS_WORKGROUP: u32 = 4;
#[allow(dead_code)]
const STORAGE_CLASS_PRIVATE: u32 = 6;
#[allow(dead_code)]
const STORAGE_CLASS_FUNCTION: u32 = 7;
#[allow(dead_code)]
const STORAGE_CLASS_STORAGE_BUFFER: u32 = 12;

// Decorations
const DECORATION_BUILTIN: u32 = 11;
const DECORATION_BINDING: u32 = 33;
const DECORATION_DESCRIPTOR_SET: u32 = 34;

// Built-ins
const BUILTIN_GLOBAL_INVOCATION_ID: u32 = 28;

// ============================================================================
// SPIR-V OPCODES
// ============================================================================

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
#[repr(u16)]
enum SpvOp {
    Nop = 0,
    Source = 3,
    Name = 5,
    MemberName = 6,
    Extension = 10,
    ExtInstImport = 11,
    MemoryModel = 14,
    EntryPoint = 15,
    ExecutionMode = 16,
    Capability = 17,
    TypeVoid = 19,
    TypeBool = 20,
    TypeInt = 21,
    TypeFloat = 22,
    TypeVector = 23,
    TypeArray = 28,
    TypeRuntimeArray = 29,
    TypeStruct = 30,
    TypePointer = 32,
    TypeFunction = 33,
    Constant = 43,
    ConstantComposite = 44,
    Function = 54,
    FunctionParameter = 55,
    FunctionEnd = 56,
    FunctionCall = 57,
    Variable = 59,
    Load = 61,
    Store = 62,
    AccessChain = 65,
    Decorate = 71,
    MemberDecorate = 72,
    CompositeExtract = 81,
    IAdd = 128,
    ISub = 130,
    IMul = 132,
    SDiv = 135,
    SMod = 139,
    IEqual = 170,
    INotEqual = 171,
    SLessThan = 177,
    SGreaterThan = 179,
    SLessThanEqual = 181,
    SGreaterThanEqual = 183,
    LogicalEqual = 164,
    LogicalNotEqual = 165,
    LogicalOr = 166,
    LogicalAnd = 167,
    LogicalNot = 168,
    Select = 169,
    FAdd = 129,
    FSub = 131,
    FMul = 133,
    FDiv = 136,
    FNegate = 127,
    Bitcast = 124,
    ConvertSToF = 111,
    ConvertFToS = 110,
    Label = 248,
    Branch = 249,
    BranchConditional = 250,
    Return = 253,
    ReturnValue = 254,
}

// ============================================================================
// SPIR-V MODULE BUILDER
// ============================================================================

/// A SPIR-V module under construction
pub struct SpirVModule {
    /// Bound (highest ID + 1)
    bound: u32,
    
    /// Capabilities
    capabilities: Vec<u32>,
    
    /// Extensions
    extensions: Vec<String>,
    
    /// Extended instruction sets
    ext_inst_imports: Vec<(u32, String)>,
    
    /// Memory model instruction
    memory_model: Option<(u32, u32)>,
    
    /// Entry points
    entry_points: Vec<EntryPoint>,
    
    /// Execution modes
    execution_modes: Vec<Vec<u32>>,
    
    /// Debug names
    names: Vec<(u32, String)>,
    
    /// Decorations
    decorations: Vec<Vec<u32>>,
    
    /// Type declarations (in order)
    types: Vec<Vec<u32>>,
    
    /// Constants
    constants: Vec<Vec<u32>>,
    
    /// Global variables
    globals: Vec<Vec<u32>>,
    
    /// Functions
    functions: Vec<SpirVFunction>,
    
    /// ID allocation
    id_map: HashMap<String, u32>,
}

#[derive(Debug, Clone)]
struct EntryPoint {
    execution_model: u32,
    function_id: u32,
    name: String,
    interface_ids: Vec<u32>,
}

#[derive(Debug, Clone)]
struct SpirVFunction {
    instructions: Vec<Vec<u32>>,
}

impl SpirVModule {
    pub fn new() -> Self {
        SpirVModule {
            bound: 1,
            capabilities: Vec::new(),
            extensions: Vec::new(),
            ext_inst_imports: Vec::new(),
            memory_model: None,
            entry_points: Vec::new(),
            execution_modes: Vec::new(),
            names: Vec::new(),
            decorations: Vec::new(),
            types: Vec::new(),
            constants: Vec::new(),
            globals: Vec::new(),
            functions: Vec::new(),
            id_map: HashMap::new(),
        }
    }
    
    /// Allocate a new ID
    pub fn alloc_id(&mut self) -> u32 {
        let id = self.bound;
        self.bound += 1;
        id
    }
    
    /// Allocate a named ID
    pub fn alloc_named_id(&mut self, name: &str) -> u32 {
        if let Some(&id) = self.id_map.get(name) {
            return id;
        }
        let id = self.alloc_id();
        self.id_map.insert(name.to_string(), id);
        id
    }
    
    /// Get ID by name
    pub fn get_id(&self, name: &str) -> Option<u32> {
        self.id_map.get(name).copied()
    }
    
    /// Add capability
    pub fn add_capability(&mut self, cap: u32) {
        if !self.capabilities.contains(&cap) {
            self.capabilities.push(cap);
        }
    }
    
    /// Add extension
    pub fn add_extension(&mut self, ext: &str) {
        if !self.extensions.iter().any(|e| e == ext) {
            self.extensions.push(ext.to_string());
        }
    }
    
    /// Set memory model
    pub fn set_memory_model(&mut self, addressing: u32, memory: u32) {
        self.memory_model = Some((addressing, memory));
    }
    
    /// Add entry point
    pub fn add_entry_point(&mut self, exec_model: u32, func_id: u32, name: &str, interfaces: Vec<u32>) {
        self.entry_points.push(EntryPoint {
            execution_model: exec_model,
            function_id: func_id,
            name: name.to_string(),
            interface_ids: interfaces,
        });
    }
    
    /// Add execution mode
    pub fn add_execution_mode(&mut self, entry_point: u32, mode: u32, operands: &[u32]) {
        let mut inst = vec![entry_point, mode];
        inst.extend_from_slice(operands);
        self.execution_modes.push(inst);
    }
    
    /// Add debug name
    pub fn add_name(&mut self, id: u32, name: &str) {
        self.names.push((id, name.to_string()));
    }
    
    /// Add decoration
    pub fn add_decoration(&mut self, target: u32, decoration: u32, operands: &[u32]) {
        let mut inst = vec![target, decoration];
        inst.extend_from_slice(operands);
        self.decorations.push(inst);
    }
    
    /// Add member decoration (for struct members)
    pub fn add_member_decoration(&mut self, struct_type: u32, member: u32, decoration: u32, operands: &[u32]) {
        // MemberDecorate uses a different format: struct_type, member, decoration, ...operands
        // We mark it with a special flag (MSB set) so encode knows it's MemberDecorate
        let mut inst = vec![struct_type | 0x80000000, member, decoration];
        inst.extend_from_slice(operands);
        self.decorations.push(inst);
    }
    
    /// Add type instruction
    pub fn add_type(&mut self, words: Vec<u32>) {
        self.types.push(words);
    }
    
    /// Add constant
    pub fn add_constant(&mut self, words: Vec<u32>) {
        self.constants.push(words);
    }
    
    /// Add global variable
    pub fn add_global(&mut self, words: Vec<u32>) {
        self.globals.push(words);
    }
    
    /// Start a function
    pub fn start_function(&mut self) -> usize {
        let idx = self.functions.len();
        self.functions.push(SpirVFunction { instructions: Vec::new() });
        idx
    }
    
    /// Add instruction to function
    pub fn add_instruction(&mut self, func_idx: usize, words: Vec<u32>) {
        self.functions[func_idx].instructions.push(words);
    }
    
    /// Encode to binary SPIR-V
    pub fn encode(&self) -> Vec<u8> {
        let mut words: Vec<u32> = Vec::new();
        
        // Header
        words.push(SPIRV_MAGIC);
        words.push(SPIRV_VERSION);
        words.push(GENERATOR_MAGIC);
        words.push(self.bound);
        words.push(0); // Reserved
        
        // Capabilities
        for &cap in &self.capabilities {
            words.push(encode_op(SpvOp::Capability, 2));
            words.push(cap);
        }
        
        // Extensions (OpExtension)
        for ext in &self.extensions {
            let str_words = encode_string(ext);
            words.push(encode_op(SpvOp::Extension, 1 + str_words.len() as u16));
            words.extend(str_words);
        }
        
        // ExtInstImport
        for (id, name) in &self.ext_inst_imports {
            let str_words = encode_string(name);
            words.push(encode_op(SpvOp::ExtInstImport, 2 + str_words.len() as u16));
            words.push(*id);
            words.extend(str_words);
        }
        
        // Memory model
        if let Some((addr, mem)) = self.memory_model {
            words.push(encode_op(SpvOp::MemoryModel, 3));
            words.push(addr);
            words.push(mem);
        }
        
        // Entry points
        for ep in &self.entry_points {
            let name_words = encode_string(&ep.name);
            let len = 3 + name_words.len() + ep.interface_ids.len();
            words.push(encode_op(SpvOp::EntryPoint, len as u16));
            words.push(ep.execution_model);
            words.push(ep.function_id);
            words.extend(name_words);
            words.extend(&ep.interface_ids);
        }
        
        // Execution modes
        for mode in &self.execution_modes {
            words.push(encode_op(SpvOp::ExecutionMode, 1 + mode.len() as u16));
            words.extend(mode);
        }
        
        // Debug names
        for (id, name) in &self.names {
            let name_words = encode_string(name);
            words.push(encode_op(SpvOp::Name, 2 + name_words.len() as u16));
            words.push(*id);
            words.extend(name_words);
        }
        
        // Decorations
        for dec in &self.decorations {
            // Check if MSB is set (indicates MemberDecorate)
            if dec[0] & 0x80000000 != 0 {
                // MemberDecorate: struct_type (with MSB cleared), member, decoration, ...operands
                let struct_type = dec[0] & 0x7FFFFFFF;
                words.push(encode_op(SpvOp::MemberDecorate, 1 + dec.len() as u16));
                words.push(struct_type);
                words.extend(&dec[1..]);
            } else {
                // Regular Decorate
                words.push(encode_op(SpvOp::Decorate, 1 + dec.len() as u16));
                words.extend(dec);
            }
        }
        
        // Types
        for ty in &self.types {
            words.extend(ty);
        }
        
        // Constants
        for c in &self.constants {
            words.extend(c);
        }
        
        // Globals
        for g in &self.globals {
            words.extend(g);
        }
        
        // Functions
        for func in &self.functions {
            for inst in &func.instructions {
                words.extend(inst);
            }
        }
        
        // Convert to bytes (little-endian)
        let mut bytes = Vec::with_capacity(words.len() * 4);
        for word in words {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes
    }
}

// ============================================================================
// SPIR-V ENCODING HELPERS
// ============================================================================

/// Encode opcode with word count
fn encode_op(op: SpvOp, word_count: u16) -> u32 {
    ((word_count as u32) << 16) | (op as u32)
}

/// Encode a string as SPIR-V words (null-terminated, padded to word boundary)
fn encode_string(s: &str) -> Vec<u32> {
    let bytes = s.as_bytes();
    let mut words = Vec::new();
    
    let mut i = 0;
    while i < bytes.len() {
        let mut word: u32 = 0;
        for j in 0..4 {
            if i + j < bytes.len() {
                word |= (bytes[i + j] as u32) << (j * 8);
            }
        }
        words.push(word);
        i += 4;
    }
    
    // Add null terminator if needed
    if bytes.len() % 4 == 0 {
        words.push(0);
    }
    
    words
}

// ============================================================================
// KORE → SPIR-V COMPILER
// ============================================================================

/// Compile Kore bytecode to SPIR-V compute shader
pub fn compile_to_spirv(kore_code: &[u8]) -> Result<Vec<u8>, String> {
    let mut module = SpirVModule::new();
    
    // Required capabilities
    module.add_capability(CAPABILITY_SHADER);
    
    // Required extension for StorageBuffer storage class
    module.add_extension("SPV_KHR_storage_buffer_storage_class");
    
    // Memory model: Logical + GLSL450
    module.set_memory_model(ADDRESSING_MODEL_LOGICAL, MEMORY_MODEL_GLSL450);
    
    // Allocate type IDs
    let void_type = module.alloc_named_id("void");
    let bool_type = module.alloc_named_id("bool");
    let int_type = module.alloc_named_id("int");
    let uint_type = module.alloc_named_id("uint");
    let uvec3_type = module.alloc_named_id("uvec3");
    let ptr_input_uvec3 = module.alloc_named_id("ptr_input_uvec3");
    let ptr_storage_int = module.alloc_named_id("ptr_storage_int");
    let ptr_function_int = module.alloc_named_id("ptr_func_int");
    let func_type = module.alloc_named_id("func_void");
    
    // Float types
    let float_type = module.alloc_named_id("float");
    let ptr_storage_float = module.alloc_named_id("ptr_storage_float");
    let ptr_function_float = module.alloc_named_id("ptr_func_float");
    
    // Buffer type for input/output
    let runtime_array_int = module.alloc_named_id("runtime_array_int");
    let buffer_type = module.alloc_named_id("buffer_type");
    let ptr_buffer = module.alloc_named_id("ptr_buffer");
    
    // Global variables
    let global_invocation_id = module.alloc_named_id("gl_GlobalInvocationID");
    let input_buffer = module.alloc_named_id("input_buffer");
    let output_buffer = module.alloc_named_id("output_buffer");
    
    // Main function
    let main_func = module.alloc_named_id("main");
    let main_label = module.alloc_named_id("main_label");
    
    // Constants
    let const_0 = module.alloc_named_id("const_0");
    let const_1 = module.alloc_named_id("const_1");
    
    // Add names for debugging
    module.add_name(main_func, "main");
    module.add_name(global_invocation_id, "gl_GlobalInvocationID");
    module.add_name(input_buffer, "input_buffer");
    module.add_name(output_buffer, "output_buffer");
    
    // Decorations
    module.add_decoration(global_invocation_id, DECORATION_BUILTIN, &[BUILTIN_GLOBAL_INVOCATION_ID]);
    module.add_decoration(input_buffer, DECORATION_DESCRIPTOR_SET, &[0]);
    module.add_decoration(input_buffer, DECORATION_BINDING, &[0]);
    module.add_decoration(output_buffer, DECORATION_DESCRIPTOR_SET, &[0]);
    module.add_decoration(output_buffer, DECORATION_BINDING, &[1]);
    
    // Block decoration for buffer struct
    module.add_decoration(buffer_type, 2, &[]); // Block decoration
    
    // Member Offset decoration (required for Block structs)
    module.add_member_decoration(buffer_type, 0, 35, &[0]); // Offset decoration = 0
    
    // ArrayStride decoration for RuntimeArray (required)
    module.add_decoration(runtime_array_int, 6, &[4]); // ArrayStride = 4 (sizeof int)
    
    // Type declarations
    // OpTypeVoid
    module.add_type(vec![encode_op(SpvOp::TypeVoid, 2), void_type]);
    
    // OpTypeBool
    module.add_type(vec![encode_op(SpvOp::TypeBool, 2), bool_type]);
    
    // OpTypeInt (32-bit signed)
    module.add_type(vec![encode_op(SpvOp::TypeInt, 4), int_type, 32, 1]);
    
    // OpTypeInt (32-bit unsigned)
    module.add_type(vec![encode_op(SpvOp::TypeInt, 4), uint_type, 32, 0]);
    
    // OpTypeVector (uvec3)
    module.add_type(vec![encode_op(SpvOp::TypeVector, 4), uvec3_type, uint_type, 3]);
    
    // OpTypeRuntimeArray (for buffer)
    module.add_type(vec![encode_op(SpvOp::TypeRuntimeArray, 3), runtime_array_int, int_type]);
    
    // OpTypeStruct (buffer wrapper)
    module.add_type(vec![encode_op(SpvOp::TypeStruct, 3), buffer_type, runtime_array_int]);
    
    // OpTypePointer (Input uvec3)
    module.add_type(vec![encode_op(SpvOp::TypePointer, 4), ptr_input_uvec3, STORAGE_CLASS_INPUT, uvec3_type]);
    
    // OpTypePointer (StorageBuffer buffer)
    module.add_type(vec![encode_op(SpvOp::TypePointer, 4), ptr_buffer, STORAGE_CLASS_STORAGE_BUFFER, buffer_type]);
    
    // OpTypePointer (StorageBuffer int)
    module.add_type(vec![encode_op(SpvOp::TypePointer, 4), ptr_storage_int, STORAGE_CLASS_STORAGE_BUFFER, int_type]);
    
    // OpTypePointer (Function int)
    module.add_type(vec![encode_op(SpvOp::TypePointer, 4), ptr_function_int, STORAGE_CLASS_FUNCTION, int_type]);
    
    // OpTypeFloat (32-bit)
    module.add_type(vec![encode_op(SpvOp::TypeFloat, 3), float_type, 32]);
    
    // OpTypePointer (StorageBuffer float)
    module.add_type(vec![encode_op(SpvOp::TypePointer, 4), ptr_storage_float, STORAGE_CLASS_STORAGE_BUFFER, float_type]);
    
    // OpTypePointer (Function float)
    module.add_type(vec![encode_op(SpvOp::TypePointer, 4), ptr_function_float, STORAGE_CLASS_FUNCTION, float_type]);
    
    // OpTypeFunction (void)
    module.add_type(vec![encode_op(SpvOp::TypeFunction, 3), func_type, void_type]);
    
    // Constants
    module.add_constant(vec![encode_op(SpvOp::Constant, 4), int_type, const_0, 0]);
    module.add_constant(vec![encode_op(SpvOp::Constant, 4), int_type, const_1, 1]);
    
    // Float constants
    let fconst_0 = module.alloc_named_id("fconst_0");
    let fconst_1 = module.alloc_named_id("fconst_1");
    module.add_constant(vec![encode_op(SpvOp::Constant, 4), float_type, fconst_0, 0f32.to_bits()]);
    module.add_constant(vec![encode_op(SpvOp::Constant, 4), float_type, fconst_1, 1f32.to_bits()]);
    
    // Global variables
    module.add_global(vec![encode_op(SpvOp::Variable, 4), ptr_input_uvec3, global_invocation_id, STORAGE_CLASS_INPUT]);
    module.add_global(vec![encode_op(SpvOp::Variable, 4), ptr_buffer, input_buffer, STORAGE_CLASS_STORAGE_BUFFER]);
    module.add_global(vec![encode_op(SpvOp::Variable, 4), ptr_buffer, output_buffer, STORAGE_CLASS_STORAGE_BUFFER]);
    
    // Entry point - only Input/Output variables in interface (not StorageBuffer)
    module.add_entry_point(
        EXECUTION_MODEL_GLCOMPUTE,
        main_func,
        "main",
        vec![global_invocation_id],  // Only gl_GlobalInvocationID
    );
    
    // Execution mode: LocalSize(1, 1, 1) for simple compute
    module.add_execution_mode(main_func, 17, &[1, 1, 1]); // LocalSize
    
    // Start function
    let func_idx = module.start_function();
    
    // OpFunction
    module.add_instruction(func_idx, vec![
        encode_op(SpvOp::Function, 5),
        void_type,
        main_func,
        0, // None function control
        func_type,
    ]);
    
    // OpLabel
    module.add_instruction(func_idx, vec![
        encode_op(SpvOp::Label, 2),
        main_label,
    ]);
    
    // Load global invocation ID
    let gid_val = module.alloc_id();
    module.add_instruction(func_idx, vec![
        encode_op(SpvOp::Load, 4),
        uvec3_type,
        gid_val,
        global_invocation_id,
    ]);
    
    // Extract x component (thread index)
    let thread_idx = module.alloc_id();
    module.add_instruction(func_idx, vec![
        encode_op(SpvOp::CompositeExtract, 5),
        uint_type,
        thread_idx,
        gid_val,
        0, // .x
    ]);
    
    // Convert to signed int for indexing
    // (In full implementation, would compile Kore ops here)
    
    // Access chain for input[0].data[0] (struct member 0, array element 0)
    // Buffer struct: { data: RuntimeArray<int> }
    // So we index: buffer -> member 0 (the array) -> element 0
    let input_ptr = module.alloc_id();
    module.add_instruction(func_idx, vec![
        encode_op(SpvOp::AccessChain, 6),  // 6 words: op, type, result, base, idx1, idx2
        ptr_storage_int,
        input_ptr,
        input_buffer,
        const_0,  // struct member 0 (the RuntimeArray)
        const_0,  // array element 0
    ]);
    
    // Load input value
    let input_val = module.alloc_id();
    module.add_instruction(func_idx, vec![
        encode_op(SpvOp::Load, 4),
        int_type,
        input_val,
        input_ptr,
    ]);
    
    // Compile Kore bytecode to SPIR-V operations
    // For now: just copy input to output (identity)
    // Full implementation would translate each Kore op
    
    let result = compile_kore_ops(&mut module, func_idx, kore_code, input_val, int_type, float_type)?;
    
    // Access chain for output[0].data[0]
    let output_ptr = module.alloc_id();
    module.add_instruction(func_idx, vec![
        encode_op(SpvOp::AccessChain, 6),
        ptr_storage_int,
        output_ptr,
        output_buffer,
        const_0,  // struct member 0
        const_0,  // array element 0
    ]);
    
    // Store result
    module.add_instruction(func_idx, vec![
        encode_op(SpvOp::Store, 3),
        output_ptr,
        result,
    ]);
    
    // OpReturn
    module.add_instruction(func_idx, vec![
        encode_op(SpvOp::Return, 1),
    ]);
    
    // OpFunctionEnd
    module.add_instruction(func_idx, vec![
        encode_op(SpvOp::FunctionEnd, 1),
    ]);
    
    Ok(module.encode())
}

/// Compile Kore operations to SPIR-V
fn compile_kore_ops(
    module: &mut SpirVModule,
    func_idx: usize,
    kore_code: &[u8],
    input_val: u32,
    int_type: u32,
    float_type: u32,
) -> Result<u32, String> {
    // Simulated stack of SPIR-V value IDs
    let mut stack: Vec<u32> = vec![input_val];
    
    let mut pc = 0;
    while pc < kore_code.len() {
        let op = kore_code[pc];
        pc += 1;
        
        match op {
            // NOP
            0x00 => {}
            
            // DROP
            0x01 => {
                if stack.is_empty() {
                    return Err("Stack underflow at DROP".into());
                }
                stack.pop();
            }
            
            // DUP
            0x02 => {
                if stack.is_empty() {
                    return Err("Stack underflow at DUP".into());
                }
                let top = *stack.last().unwrap();
                stack.push(top);
            }
            
            // SWAP
            0x03 => {
                if stack.len() < 2 {
                    return Err("Stack underflow at SWAP".into());
                }
                let len = stack.len();
                stack.swap(len - 1, len - 2);
            }
            
            // INT8
            0x30 => {
                let val = kore_code[pc] as i8 as i32;
                pc += 1;
                
                // Create constant
                let const_id = module.alloc_id();
                module.add_constant(vec![
                    encode_op(SpvOp::Constant, 4),
                    int_type,
                    const_id,
                    val as u32,
                ]);
                stack.push(const_id);
            }
            
            // ADD
            0x40 => {
                if stack.len() < 2 {
                    return Err("Stack underflow at ADD".into());
                }
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let result = module.alloc_id();
                
                module.add_instruction(func_idx, vec![
                    encode_op(SpvOp::IAdd, 5),
                    int_type,
                    result,
                    a,
                    b,
                ]);
                stack.push(result);
            }
            
            // SUB
            0x41 => {
                if stack.len() < 2 {
                    return Err("Stack underflow at SUB".into());
                }
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let result = module.alloc_id();
                
                module.add_instruction(func_idx, vec![
                    encode_op(SpvOp::ISub, 5),
                    int_type,
                    result,
                    a,
                    b,
                ]);
                stack.push(result);
            }
            
            // MUL
            0x42 => {
                if stack.len() < 2 {
                    return Err("Stack underflow at MUL".into());
                }
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let result = module.alloc_id();
                
                module.add_instruction(func_idx, vec![
                    encode_op(SpvOp::IMul, 5),
                    int_type,
                    result,
                    a,
                    b,
                ]);
                stack.push(result);
            }
            
            // DIV (integer)
            0x43 => {
                if stack.len() < 2 {
                    return Err("Stack underflow at DIV".into());
                }
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let result = module.alloc_id();
                
                module.add_instruction(func_idx, vec![
                    encode_op(SpvOp::SDiv, 5),
                    int_type,
                    result,
                    a,
                    b,
                ]);
                stack.push(result);
            }
            
            // NEG (integer)
            0x45 => {
                if stack.is_empty() {
                    return Err("Stack underflow at NEG".into());
                }
                let a = stack.pop().unwrap();
                let const_0 = module.get_id("const_0").unwrap();
                let result = module.alloc_id();
                
                module.add_instruction(func_idx, vec![
                    encode_op(SpvOp::ISub, 5),
                    int_type,
                    result,
                    const_0,
                    a,
                ]);
                stack.push(result);
            }
            
            // FADD: Float + Float → Float
            0x46 => {
                if stack.len() < 2 {
                    return Err("Stack underflow at FADD".into());
                }
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let result = module.alloc_id();
                
                module.add_instruction(func_idx, vec![
                    encode_op(SpvOp::FAdd, 5),
                    float_type,
                    result,
                    a,
                    b,
                ]);
                stack.push(result);
            }
            
            // FSUB: Float - Float → Float
            0x47 => {
                if stack.len() < 2 {
                    return Err("Stack underflow at FSUB".into());
                }
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let result = module.alloc_id();
                
                module.add_instruction(func_idx, vec![
                    encode_op(SpvOp::FSub, 5),
                    float_type,
                    result,
                    a,
                    b,
                ]);
                stack.push(result);
            }
            
            // FMUL: Float * Float → Float
            0x48 => {
                if stack.len() < 2 {
                    return Err("Stack underflow at FMUL".into());
                }
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let result = module.alloc_id();
                
                module.add_instruction(func_idx, vec![
                    encode_op(SpvOp::FMul, 5),
                    float_type,
                    result,
                    a,
                    b,
                ]);
                stack.push(result);
            }
            
            // FDIV: Float / Float → Float
            0x49 => {
                if stack.len() < 2 {
                    return Err("Stack underflow at FDIV".into());
                }
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                let result = module.alloc_id();
                
                module.add_instruction(func_idx, vec![
                    encode_op(SpvOp::FDiv, 5),
                    float_type,
                    result,
                    a,
                    b,
                ]);
                stack.push(result);
            }
            
            // FNEG: -Float → Float
            0x4A => {
                if stack.is_empty() {
                    return Err("Stack underflow at FNEG".into());
                }
                let a = stack.pop().unwrap();
                let result = module.alloc_id();
                
                module.add_instruction(func_idx, vec![
                    encode_op(SpvOp::FNegate, 4),
                    float_type,
                    result,
                    a,
                ]);
                stack.push(result);
            }
            
            // I2F: Int → Float (ConvertSToF)
            0x4D => {
                if stack.is_empty() {
                    return Err("Stack underflow at I2F".into());
                }
                let a = stack.pop().unwrap();
                let result = module.alloc_id();
                
                module.add_instruction(func_idx, vec![
                    encode_op(SpvOp::ConvertSToF, 4),
                    float_type,
                    result,
                    a,
                ]);
                stack.push(result);
            }
            
            // F2I: Float → Int (ConvertFToS)
            0x4E => {
                if stack.is_empty() {
                    return Err("Stack underflow at F2I".into());
                }
                let a = stack.pop().unwrap();
                let result = module.alloc_id();
                
                module.add_instruction(func_idx, vec![
                    encode_op(SpvOp::ConvertFToS, 4),
                    int_type,
                    result,
                    a,
                ]);
                stack.push(result);
            }
            
            // F64 literal → float constant
            0x35 => {
                if pc + 8 > kore_code.len() {
                    return Err("Unexpected end of code at F64".into());
                }
                let bits = u64::from_le_bytes([
                    kore_code[pc], kore_code[pc+1], kore_code[pc+2], kore_code[pc+3],
                    kore_code[pc+4], kore_code[pc+5], kore_code[pc+6], kore_code[pc+7],
                ]);
                pc += 8;
                let val = f64::from_bits(bits) as f32;
                
                let const_id = module.alloc_id();
                module.add_constant(vec![
                    encode_op(SpvOp::Constant, 4),
                    float_type,
                    const_id,
                    val.to_bits(),
                ]);
                stack.push(const_id);
            }
            
            // LE (less than or equal)
            0x53 => {
                if stack.len() < 2 {
                    return Err("Stack underflow at LE".into());
                }
                let b = stack.pop().unwrap();
                let a = stack.pop().unwrap();
                
                // Compare
                let cmp_result = module.alloc_id();
                let bool_type = module.get_id("bool").unwrap();
                module.add_instruction(func_idx, vec![
                    encode_op(SpvOp::SLessThanEqual, 5),
                    bool_type,
                    cmp_result,
                    a,
                    b,
                ]);
                
                // Convert bool to int (0 or 1)
                let const_0 = module.get_id("const_0").unwrap();
                let const_1 = module.get_id("const_1").unwrap();
                let result = module.alloc_id();
                module.add_instruction(func_idx, vec![
                    encode_op(SpvOp::Select, 6),
                    int_type,
                    result,
                    cmp_result,
                    const_1,
                    const_0,
                ]);
                stack.push(result);
            }
            
            // FETCH: Not yet supported in SPIR-V (reflection not available)
            0x90 => {
                // TODO: Would need to embed bytecode in SSBO
                // For now, pop offset and push 0
                if !stack.is_empty() {
                    stack.pop();
                }
                // Push constant 0
                let result = module.alloc_id();
                module.add_instruction(func_idx, vec![
                    encode_op(SpvOp::Constant, 4),
                    int_type,
                    result,
                    0,
                ]);
                stack.push(result);
            }
            
            // SIZE: Not yet supported in SPIR-V
            0x91 => {
                // TODO: Would need bytecode length as uniform
                // Push constant 0
                let result = module.alloc_id();
                module.add_instruction(func_idx, vec![
                    encode_op(SpvOp::Constant, 4),
                    int_type,
                    result,
                    0,
                ]);
                stack.push(result);
            }
            
            // HALT
            0xFF => {
                break;
            }
            
            _ => {
                // Skip unknown opcodes for now
            }
        }
    }
    
    // Return top of stack (or input if empty)
    Ok(*stack.last().unwrap_or(&input_val))
}

// ============================================================================
// KORE → SPIR-V PARALLEL MAP COMPILER
// ============================================================================

/// Compile a Kore function body to a SPIR-V compute shader that applies it
/// to every element of an input array in parallel.
///
/// Each GPU thread (gl_GlobalInvocationID.x) processes one element:
///   output[i] = f(input[i])
///
/// This is P2 (Apply) parallelized: the same tool S→S applied N times
/// simultaneously via GPU hardware.
///
/// # Arguments
/// * `kore_body` - Kore bytecode for the per-element function (e.g., "dup mul" for squaring)
///
/// # Returns
/// * SPIR-V binary for a compute shader. Dispatch with workgroup_count = N.
pub fn compile_to_spirv_parallel_map(kore_body: &[u8]) -> Result<Vec<u8>, String> {
    let mut module = SpirVModule::new();
    
    // Required capabilities
    module.add_capability(CAPABILITY_SHADER);
    module.add_extension("SPV_KHR_storage_buffer_storage_class");
    module.set_memory_model(ADDRESSING_MODEL_LOGICAL, MEMORY_MODEL_GLSL450);
    
    // Allocate type IDs
    let void_type = module.alloc_named_id("void");
    let bool_type = module.alloc_named_id("bool");
    let int_type = module.alloc_named_id("int");
    let uint_type = module.alloc_named_id("uint");
    let uvec3_type = module.alloc_named_id("uvec3");
    let ptr_input_uvec3 = module.alloc_named_id("ptr_input_uvec3");
    let ptr_storage_int = module.alloc_named_id("ptr_storage_int");
    let _ptr_function_int = module.alloc_named_id("ptr_func_int");
    let func_type = module.alloc_named_id("func_void");
    let float_type = module.alloc_named_id("float");
    let ptr_storage_float = module.alloc_named_id("ptr_storage_float");
    let _ptr_function_float = module.alloc_named_id("ptr_func_float");
    
    let runtime_array_int = module.alloc_named_id("runtime_array_int");
    let buffer_type = module.alloc_named_id("buffer_type");
    let ptr_buffer = module.alloc_named_id("ptr_buffer");
    
    let global_invocation_id = module.alloc_named_id("gl_GlobalInvocationID");
    let input_buffer = module.alloc_named_id("input_buffer");
    let output_buffer = module.alloc_named_id("output_buffer");
    
    let main_func = module.alloc_named_id("main");
    let main_label = module.alloc_named_id("main_label");
    
    let const_0 = module.alloc_named_id("const_0");
    let const_1 = module.alloc_named_id("const_1");
    
    // Debug names
    module.add_name(main_func, "main");
    module.add_name(global_invocation_id, "gl_GlobalInvocationID");
    module.add_name(input_buffer, "input_buffer");
    module.add_name(output_buffer, "output_buffer");
    
    // Decorations
    module.add_decoration(global_invocation_id, DECORATION_BUILTIN, &[BUILTIN_GLOBAL_INVOCATION_ID]);
    module.add_decoration(input_buffer, DECORATION_DESCRIPTOR_SET, &[0]);
    module.add_decoration(input_buffer, DECORATION_BINDING, &[0]);
    module.add_decoration(output_buffer, DECORATION_DESCRIPTOR_SET, &[0]);
    module.add_decoration(output_buffer, DECORATION_BINDING, &[1]);
    module.add_decoration(buffer_type, 2, &[]); // Block
    module.add_member_decoration(buffer_type, 0, 35, &[0]); // Offset = 0
    module.add_decoration(runtime_array_int, 6, &[4]); // ArrayStride = 4
    
    // Type declarations
    module.add_type(vec![encode_op(SpvOp::TypeVoid, 2), void_type]);
    module.add_type(vec![encode_op(SpvOp::TypeBool, 2), bool_type]);
    module.add_type(vec![encode_op(SpvOp::TypeInt, 4), int_type, 32, 1]);
    module.add_type(vec![encode_op(SpvOp::TypeInt, 4), uint_type, 32, 0]);
    module.add_type(vec![encode_op(SpvOp::TypeVector, 4), uvec3_type, uint_type, 3]);
    module.add_type(vec![encode_op(SpvOp::TypeRuntimeArray, 3), runtime_array_int, int_type]);
    module.add_type(vec![encode_op(SpvOp::TypeStruct, 3), buffer_type, runtime_array_int]);
    module.add_type(vec![encode_op(SpvOp::TypePointer, 4), ptr_input_uvec3, STORAGE_CLASS_INPUT, uvec3_type]);
    module.add_type(vec![encode_op(SpvOp::TypePointer, 4), ptr_buffer, STORAGE_CLASS_STORAGE_BUFFER, buffer_type]);
    module.add_type(vec![encode_op(SpvOp::TypePointer, 4), ptr_storage_int, STORAGE_CLASS_STORAGE_BUFFER, int_type]);
    module.add_type(vec![encode_op(SpvOp::TypePointer, 4), _ptr_function_int, STORAGE_CLASS_FUNCTION, int_type]);
    module.add_type(vec![encode_op(SpvOp::TypeFloat, 3), float_type, 32]);
    module.add_type(vec![encode_op(SpvOp::TypePointer, 4), ptr_storage_float, STORAGE_CLASS_STORAGE_BUFFER, float_type]);
    module.add_type(vec![encode_op(SpvOp::TypePointer, 4), _ptr_function_float, STORAGE_CLASS_FUNCTION, float_type]);
    module.add_type(vec![encode_op(SpvOp::TypeFunction, 3), func_type, void_type]);
    
    // Constants
    module.add_constant(vec![encode_op(SpvOp::Constant, 4), int_type, const_0, 0]);
    module.add_constant(vec![encode_op(SpvOp::Constant, 4), int_type, const_1, 1]);
    let fconst_0 = module.alloc_named_id("fconst_0");
    let fconst_1 = module.alloc_named_id("fconst_1");
    module.add_constant(vec![encode_op(SpvOp::Constant, 4), float_type, fconst_0, 0f32.to_bits()]);
    module.add_constant(vec![encode_op(SpvOp::Constant, 4), float_type, fconst_1, 1f32.to_bits()]);
    
    // Global variables
    module.add_global(vec![encode_op(SpvOp::Variable, 4), ptr_input_uvec3, global_invocation_id, STORAGE_CLASS_INPUT]);
    module.add_global(vec![encode_op(SpvOp::Variable, 4), ptr_buffer, input_buffer, STORAGE_CLASS_STORAGE_BUFFER]);
    module.add_global(vec![encode_op(SpvOp::Variable, 4), ptr_buffer, output_buffer, STORAGE_CLASS_STORAGE_BUFFER]);
    
    // Entry point
    module.add_entry_point(
        EXECUTION_MODEL_GLCOMPUTE,
        main_func,
        "main",
        vec![global_invocation_id],
    );
    
    // Execution mode: LocalSize(1, 1, 1) — one invocation per workgroup
    // We dispatch N workgroups for N elements
    module.add_execution_mode(main_func, 17, &[1, 1, 1]);
    
    // Function body
    let func_idx = module.start_function();
    
    // OpFunction
    module.add_instruction(func_idx, vec![
        encode_op(SpvOp::Function, 5), void_type, main_func, 0, func_type,
    ]);
    
    // OpLabel
    module.add_instruction(func_idx, vec![
        encode_op(SpvOp::Label, 2), main_label,
    ]);
    
    // Load gl_GlobalInvocationID
    let gid_val = module.alloc_id();
    module.add_instruction(func_idx, vec![
        encode_op(SpvOp::Load, 4), uvec3_type, gid_val, global_invocation_id,
    ]);
    
    // Extract .x = thread index
    let thread_idx = module.alloc_id();
    module.add_instruction(func_idx, vec![
        encode_op(SpvOp::CompositeExtract, 5), uint_type, thread_idx, gid_val, 0,
    ]);
    
    // Bitcast uint → int for signed indexing
    let thread_idx_int = module.alloc_id();
    module.add_instruction(func_idx, vec![
        encode_op(SpvOp::Bitcast, 4), int_type, thread_idx_int, thread_idx,
    ]);
    
    // AccessChain: &input.data[gl_GlobalInvocationID.x]
    let input_ptr = module.alloc_id();
    module.add_instruction(func_idx, vec![
        encode_op(SpvOp::AccessChain, 6),
        ptr_storage_int, input_ptr, input_buffer,
        const_0,          // struct member 0 (the RuntimeArray)
        thread_idx_int,   // array index = thread ID
    ]);
    
    // Load input element
    let input_val = module.alloc_id();
    module.add_instruction(func_idx, vec![
        encode_op(SpvOp::Load, 4), int_type, input_val, input_ptr,
    ]);
    
    // Apply Kore operation body to this element
    let result = compile_kore_ops(&mut module, func_idx, kore_body, input_val, int_type, float_type)?;
    
    // AccessChain: &output.data[gl_GlobalInvocationID.x]
    let output_ptr = module.alloc_id();
    module.add_instruction(func_idx, vec![
        encode_op(SpvOp::AccessChain, 6),
        ptr_storage_int, output_ptr, output_buffer,
        const_0,          // struct member 0
        thread_idx_int,   // array index = thread ID
    ]);
    
    // Store result
    module.add_instruction(func_idx, vec![
        encode_op(SpvOp::Store, 3), output_ptr, result,
    ]);
    
    // Return
    module.add_instruction(func_idx, vec![encode_op(SpvOp::Return, 1)]);
    module.add_instruction(func_idx, vec![encode_op(SpvOp::FunctionEnd, 1)]);
    
    Ok(module.encode())
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::Assembler;
    use crate::bytecode::Op;
    
    #[test]
    fn test_spirv_magic() {
        let mut module = SpirVModule::new();
        module.add_capability(CAPABILITY_SHADER);
        let bytes = module.encode();
        
        // Check magic number (little-endian)
        assert_eq!(bytes[0], 0x03);
        assert_eq!(bytes[1], 0x02);
        assert_eq!(bytes[2], 0x23);
        assert_eq!(bytes[3], 0x07);
    }
    
    #[test]
    fn test_compile_simple() {
        let mut asm = Assembler::new();
        asm.emit_i8(5);
        asm.emit_i8(3);
        asm.emit(Op::Add);
        asm.emit(Op::Halt);
        
        let module = asm.finalize().unwrap();
        let spirv = compile_to_spirv(&module.code).unwrap();
        
        // Basic sanity check
        assert!(spirv.len() > 20);
        
        // Check magic
        assert_eq!(&spirv[0..4], &[0x03, 0x02, 0x23, 0x07]);
    }
    
    #[test]
    fn test_encode_string() {
        let words = encode_string("main");
        assert_eq!(words.len(), 2); // "main\0" padded to 8 bytes
        
        // "main" = 0x6D 0x61 0x69 0x6E
        assert_eq!(words[0], 0x6E69616D);
    }
    
    #[test]
    fn test_spirv_float_add() {
        // Test that float ops compile to valid SPIR-V
        let mut asm = Assembler::new();
        asm.emit_f64(3.14);
        asm.emit_f64(2.72);
        asm.emit(Op::Fadd);
        asm.emit(Op::Halt);
        
        let module = asm.finalize().unwrap();
        let spirv = compile_to_spirv(&module.code).unwrap();
        
        // Check magic + size
        assert_eq!(&spirv[0..4], &[0x03, 0x02, 0x23, 0x07]);
        assert!(spirv.len() > 20);
    }
    
    #[test]
    fn test_spirv_float_mul() {
        let mut asm = Assembler::new();
        asm.emit_f64(2.0);
        asm.emit_f64(3.0);
        asm.emit(Op::Fmul);
        asm.emit(Op::Halt);
        
        let module = asm.finalize().unwrap();
        let spirv = compile_to_spirv(&module.code).unwrap();
        assert_eq!(&spirv[0..4], &[0x03, 0x02, 0x23, 0x07]);
    }
    
    #[test]
    fn test_spirv_i2f() {
        let mut asm = Assembler::new();
        asm.emit_i8(42);
        asm.emit(Op::I2f);
        asm.emit(Op::Halt);
        
        let module = asm.finalize().unwrap();
        let spirv = compile_to_spirv(&module.code).unwrap();
        assert_eq!(&spirv[0..4], &[0x03, 0x02, 0x23, 0x07]);
    }
    
    #[test]
    fn test_spirv_fneg() {
        let mut asm = Assembler::new();
        asm.emit_f64(5.0);
        asm.emit(Op::Fneg);
        asm.emit(Op::Halt);
        
        let module = asm.finalize().unwrap();
        let spirv = compile_to_spirv(&module.code).unwrap();
        assert_eq!(&spirv[0..4], &[0x03, 0x02, 0x23, 0x07]);
    }
    
    #[test]
    fn test_spirv_mixed_int_float() {
        // Integer arithmetic + conversion + float arithmetic
        let mut asm = Assembler::new();
        asm.emit_i8(10);
        asm.emit_i8(3);
        asm.emit(Op::Add);
        asm.emit(Op::I2f);  // 13 → 13.0
        asm.emit_f64(2.0);
        asm.emit(Op::Fmul); // 13.0 * 2.0 = 26.0
        asm.emit(Op::Halt);
        
        let module = asm.finalize().unwrap();
        let spirv = compile_to_spirv(&module.code).unwrap();
        assert_eq!(&spirv[0..4], &[0x03, 0x02, 0x23, 0x07]);
    }
    
    // ========================================================================
    // PARALLEL MAP TESTS
    // ========================================================================
    
    #[test]
    fn test_spirv_parallel_map_identity() {
        // Identity: each element maps to itself
        // Body = HALT (just returns input_val)
        let body = vec![0xFF]; // HALT
        let spirv = compile_to_spirv_parallel_map(&body).unwrap();
        
        // Check valid SPIR-V
        assert_eq!(&spirv[0..4], &[0x03, 0x02, 0x23, 0x07]);
        assert!(spirv.len() > 20);
    }
    
    #[test]
    fn test_spirv_parallel_map_double() {
        // Double each element: dup add halt
        let mut asm = Assembler::new();
        asm.emit(Op::Dup);
        asm.emit(Op::Add);
        asm.emit(Op::Halt);
        
        let module = asm.finalize().unwrap();
        let spirv = compile_to_spirv_parallel_map(&module.code).unwrap();
        assert_eq!(&spirv[0..4], &[0x03, 0x02, 0x23, 0x07]);
    }
    
    #[test]
    fn test_spirv_parallel_map_square() {
        // Square each element: dup mul halt
        let mut asm = Assembler::new();
        asm.emit(Op::Dup);
        asm.emit(Op::Mul);
        asm.emit(Op::Halt);
        
        let module = asm.finalize().unwrap();
        let spirv = compile_to_spirv_parallel_map(&module.code).unwrap();
        assert_eq!(&spirv[0..4], &[0x03, 0x02, 0x23, 0x07]);
    }
    
    #[test]
    fn test_spirv_parallel_map_add_constant() {
        // Add 10 to each element: 10 add halt
        let mut asm = Assembler::new();
        asm.emit_i8(10);
        asm.emit(Op::Add);
        asm.emit(Op::Halt);
        
        let module = asm.finalize().unwrap();
        let spirv = compile_to_spirv_parallel_map(&module.code).unwrap();
        assert_eq!(&spirv[0..4], &[0x03, 0x02, 0x23, 0x07]);
    }
    
    #[test]
    fn test_spirv_parallel_map_neg() {
        // Negate each element: neg halt
        let mut asm = Assembler::new();
        asm.emit(Op::Neg);
        asm.emit(Op::Halt);
        
        let module = asm.finalize().unwrap();
        let spirv = compile_to_spirv_parallel_map(&module.code).unwrap();
        assert_eq!(&spirv[0..4], &[0x03, 0x02, 0x23, 0x07]);
    }
}
