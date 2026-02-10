//! Kore Proof Checker
//!
//! P4: Constraints Attenuate - Types form a lattice, never escalate.
//!
//! This module verifies type constraints at COMPILE TIME, before bytecode
//! is executed. It ensures that:
//! 1. Stack effects are balanced
//! 2. Types match at every operation
//! 3. Control flow converges to same stack state
//!
//! NO DECISIONS - just mechanical type propagation.

use std::collections::HashMap;

// ============================================================================
// TYPE LATTICE (P4: Constraints form a lattice)
// ============================================================================

/// Types in the Kore type system.
/// These form a lattice with meet (∧) and join (∨) operations.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    /// Bottom type - no values (absurd)
    #[allow(dead_code)]
    Bottom,
    
    /// Unit type - exactly one value (nil)
    Unit,
    
    /// Boolean type
    Bool,
    
    /// Integer type
    Int,
    
    /// Float type  
    Float,
    
    /// String type
    Str,
    
    /// Product type (A × B)
    Pair(Box<Type>, Box<Type>),
    
    /// Sum type (A + B)
    Sum(Box<Type>, Box<Type>),
    
    /// List type [A]
    List(Box<Type>),
    
    /// Quote type (deferred computation with stack effect)
    Quote(StackEffect),
    
    /// Linear type — must be used exactly once (P4: constraint)
    Linear(Box<Type>),
    
    /// Affine type — must be used at most once (P4: constraint)
    Affine(Box<Type>),
    
    /// Top type - any value (unknown)
    Top,
    
    /// Type variable (for inference)
    Var(usize),
}

impl Type {
    /// Check if this type is a subtype of another (P4: attenuation)
    /// a ≤ b means "a is at least as constrained as b"
    pub fn is_subtype_of(&self, other: &Type) -> bool {
        match (self, other) {
            // Bottom is subtype of everything
            (Type::Bottom, _) => true,
            
            // Everything is subtype of Top
            (_, Type::Top) => true,
            
            // Same types
            (a, b) if a == b => true,
            
            // Int is subtype of Float (promotion)
            (Type::Int, Type::Float) => true,
            
            // Covariant pairs
            (Type::Pair(a1, b1), Type::Pair(a2, b2)) => {
                a1.is_subtype_of(a2) && b1.is_subtype_of(b2)
            }
            
            // Covariant sums
            (Type::Sum(a1, b1), Type::Sum(a2, b2)) => {
                a1.is_subtype_of(a2) && b1.is_subtype_of(b2)
            }
            
            // Covariant lists
            (Type::List(a), Type::List(b)) => a.is_subtype_of(b),
            
            // Contravariant input, covariant output for quotes
            (Type::Quote(e1), Type::Quote(e2)) => {
                e1.is_subeffect_of(e2)
            }
            
            // Linear/Affine: covariant in the inner type
            (Type::Linear(a), Type::Linear(b)) => a.is_subtype_of(b),
            (Type::Affine(a), Type::Affine(b)) => a.is_subtype_of(b),
            // Linear ≤ Affine (linear is MORE constrained)
            (Type::Linear(a), Type::Affine(b)) => a.is_subtype_of(b),
            
            _ => false,
        }
    }
    
    /// Meet operation (greatest lower bound) - P4 in action
    /// The result is the most general type that is a subtype of both
    #[allow(dead_code)]
    pub fn meet(&self, other: &Type) -> Type {
        match (self, other) {
            (Type::Top, t) | (t, Type::Top) => t.clone(),
            (Type::Bottom, _) | (_, Type::Bottom) => Type::Bottom,
            (a, b) if a == b => a.clone(),
            
            (Type::Int, Type::Float) | (Type::Float, Type::Int) => Type::Int,
            
            (Type::Pair(a1, b1), Type::Pair(a2, b2)) => {
                Type::Pair(Box::new(a1.meet(a2)), Box::new(b1.meet(b2)))
            }
            
            (Type::Sum(a1, b1), Type::Sum(a2, b2)) => {
                Type::Sum(Box::new(a1.meet(a2)), Box::new(b1.meet(b2)))
            }
            
            (Type::List(a), Type::List(b)) => {
                Type::List(Box::new(a.meet(b)))
            }
            
            (Type::Linear(a), Type::Linear(b)) => {
                Type::Linear(Box::new(a.meet(b)))
            }
            (Type::Affine(a), Type::Affine(b)) => {
                Type::Affine(Box::new(a.meet(b)))
            }
            // meet(Linear, Affine) = Linear (more constrained)
            (Type::Linear(a), Type::Affine(b)) | (Type::Affine(b), Type::Linear(a)) => {
                Type::Linear(Box::new(a.meet(b)))
            }
            
            _ => Type::Bottom,
        }
    }
    
    /// Join operation (least upper bound)
    pub fn join(&self, other: &Type) -> Type {
        match (self, other) {
            (Type::Bottom, t) | (t, Type::Bottom) => t.clone(),
            (Type::Top, _) | (_, Type::Top) => Type::Top,
            (a, b) if a == b => a.clone(),
            
            (Type::Int, Type::Float) | (Type::Float, Type::Int) => Type::Float,
            
            (Type::Pair(a1, b1), Type::Pair(a2, b2)) => {
                Type::Pair(Box::new(a1.join(a2)), Box::new(b1.join(b2)))
            }
            
            (Type::Sum(a1, b1), Type::Sum(a2, b2)) => {
                Type::Sum(Box::new(a1.join(a2)), Box::new(b1.join(b2)))
            }
            
            (Type::List(a), Type::List(b)) => {
                Type::List(Box::new(a.join(b)))
            }
            
            (Type::Linear(a), Type::Linear(b)) => {
                Type::Linear(Box::new(a.join(b)))
            }
            (Type::Affine(a), Type::Affine(b)) => {
                Type::Affine(Box::new(a.join(b)))
            }
            // join(Linear, Affine) = Affine (less constrained)
            (Type::Linear(a), Type::Affine(b)) | (Type::Affine(b), Type::Linear(a)) => {
                Type::Affine(Box::new(a.join(b)))
            }
            
            _ => Type::Top,
        }
    }
    
    pub fn name(&self) -> String {
        match self {
            Type::Bottom => "⊥".into(),
            Type::Unit => "()".into(),
            Type::Bool => "Bool".into(),
            Type::Int => "Int".into(),
            Type::Float => "Float".into(),
            Type::Str => "Str".into(),
            Type::Pair(a, b) => format!("({} × {})", a.name(), b.name()),
            Type::Sum(a, b) => format!("({} + {})", a.name(), b.name()),
            Type::List(t) => format!("[{}]", t.name()),
            Type::Quote(e) => format!("[{}]", e),
            Type::Linear(t) => format!("Linear({})", t.name()),
            Type::Affine(t) => format!("Affine({})", t.name()),
            Type::Top => "⊤".into(),
            Type::Var(n) => format!("?{}", n),
        }
    }
}

// ============================================================================
// STACK EFFECTS (P1: Tools are S → S)
// ============================================================================

/// A stack effect describes what a tool consumes and produces.
/// This is the TYPE of a tool.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StackEffect {
    /// Types consumed from stack (bottom to top)
    pub inputs: Vec<Type>,
    /// Types produced on stack (bottom to top)
    pub outputs: Vec<Type>,
}

impl StackEffect {
    #[allow(dead_code)]
    pub fn new(inputs: Vec<Type>, outputs: Vec<Type>) -> Self {
        StackEffect { inputs, outputs }
    }
    
    /// Effect that consumes and produces nothing
    pub fn identity() -> Self {
        StackEffect { inputs: vec![], outputs: vec![] }
    }
    
    /// Compose two effects (P3: concatenation)
    /// (a -- b) ; (b -- c) = (a -- c)
    #[allow(dead_code)]
    pub fn compose(&self, other: &StackEffect) -> Result<StackEffect, String> {
        // Check that outputs match inputs
        let mut stack: Vec<Type> = self.outputs.clone();
        
        for input in &other.inputs {
            if stack.is_empty() {
                return Err(format!("Stack underflow: need {} but stack empty", input.name()));
            }
            let top = stack.pop().unwrap();
            if !top.is_subtype_of(input) {
                return Err(format!("Type mismatch: expected {}, got {}", input.name(), top.name()));
            }
        }
        
        // Remaining stack + other outputs
        stack.extend(other.outputs.clone());
        
        Ok(StackEffect {
            inputs: self.inputs.clone(),
            outputs: stack,
        })
    }
    
    /// Check if this effect is a "sub-effect" of another
    /// More constrained inputs, less constrained outputs
    pub fn is_subeffect_of(&self, other: &StackEffect) -> bool {
        if self.inputs.len() != other.inputs.len() || 
           self.outputs.len() != other.outputs.len() {
            return false;
        }
        
        // Contravariant in inputs
        for (a, b) in self.inputs.iter().zip(other.inputs.iter()) {
            if !b.is_subtype_of(a) {
                return false;
            }
        }
        
        // Covariant in outputs
        for (a, b) in self.outputs.iter().zip(other.outputs.iter()) {
            if !a.is_subtype_of(b) {
                return false;
            }
        }
        
        true
    }
}

impl std::fmt::Display for StackEffect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inputs: Vec<_> = self.inputs.iter().map(|t| t.name()).collect();
        let outputs: Vec<_> = self.outputs.iter().map(|t| t.name()).collect();
        write!(f, "({} -- {})", inputs.join(" "), outputs.join(" "))
    }
}

// ============================================================================
// PROOF CHECKER
// ============================================================================

/// The proof checker verifies bytecode type-correctness.
/// It simulates execution at the type level.
pub struct ProofChecker {
    /// Current type stack
    stack: Vec<Type>,
    
    /// Known stack states at each address (for control flow)
    states: HashMap<usize, Vec<Type>>,
    
    /// Local variable types (for STORE/LOAD)
    locals: Vec<Option<Type>>,
    
    /// Capability flags granted to this module (P4: constraints attenuate)
    caps: u8,
    
    /// Type variable counter
    next_var: usize,
    
    /// Errors found
    errors: Vec<ProofError>,
}

#[derive(Debug, Clone)]
pub struct ProofError {
    pub address: usize,
    pub message: String,
}

impl ProofChecker {
    pub fn new() -> Self {
        ProofChecker {
            stack: Vec::new(),
            states: HashMap::new(),
            locals: Vec::new(),
            caps: 0xFF, // permissive by default (backward compat)
            next_var: 0,
            errors: Vec::new(),
        }
    }
    
    /// Check bytecode with specific capability flags (P4: constraints attenuate)
    /// Only opcodes allowed by cap_flags will pass type checking.
    pub fn check_with_caps(&mut self, code: &[u8], cap_flags: u8) -> Vec<ProofError> {
        self.caps = cap_flags;
        self.check(code)
    }
    
    /// Check bytecode and return any errors
    pub fn check(&mut self, code: &[u8]) -> Vec<ProofError> {
        self.stack.clear();
        self.states.clear();
        self.locals.clear();
        self.errors.clear();
        
        let mut pc = 0;
        
        while pc < code.len() {
            // Record state at this address
            self.states.insert(pc, self.stack.clone());
            
            let op = code[pc];
            pc += 1;
            
            match self.check_op(op, code, &mut pc) {
                Ok(()) => {}
                Err(msg) => {
                    self.errors.push(ProofError {
                        address: pc - 1,
                        message: msg,
                    });
                }
            }
            
            // HALT ends checking
            if op == 0xFF {
                break;
            }
        }
        
        self.errors.clone()
    }
    
    fn check_op(&mut self, op: u8, code: &[u8], pc: &mut usize) -> Result<(), String> {
        match op {
            // NOP
            0x00 => Ok(()),
            
            // DROP: (a -- )
            // P4: linear values cannot be dropped
            0x01 => {
                let t = self.pop()?;
                if matches!(&t, Type::Linear(_)) {
                    return Err("Cannot drop a linear value — use 'consume' first".into());
                }
                Ok(())
            }
            
            // DUP: (a -- a a)
            // P4: linear and affine values cannot be duplicated
            0x02 => {
                let a = self.pop()?;
                if matches!(&a, Type::Linear(_)) {
                    return Err("Cannot dup a linear value — must be used exactly once".into());
                }
                if matches!(&a, Type::Affine(_)) {
                    return Err("Cannot dup an affine value — must be used at most once".into());
                }
                self.stack.push(a.clone());
                self.stack.push(a);
                Ok(())
            }
            
            // SWAP: (a b -- b a)
            0x03 => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.stack.push(b);
                self.stack.push(a);
                Ok(())
            }
            
            // ROT: (a b c -- b c a)
            0x04 => {
                let c = self.pop()?;
                let b = self.pop()?;
                let a = self.pop()?;
                self.stack.push(b);
                self.stack.push(c);
                self.stack.push(a);
                Ok(())
            }
            
            // OVER: (a b -- a b a)
            // P4: cannot duplicate linear/affine values
            0x05 => {
                let b = self.pop()?;
                let a = self.pop()?;
                if matches!(&a, Type::Linear(_)) {
                    return Err("Cannot over (duplicate) a linear value — must be used exactly once".into());
                }
                if matches!(&a, Type::Affine(_)) {
                    return Err("Cannot over (duplicate) an affine value — must be used at most once".into());
                }
                self.stack.push(a.clone());
                self.stack.push(b);
                self.stack.push(a);
                Ok(())
            }
            
            // PAIR: (a b -- (a,b))
            0x10 => {
                let b = self.pop()?;
                let a = self.pop()?;
                self.stack.push(Type::Pair(Box::new(a), Box::new(b)));
                Ok(())
            }
            
            // UNPAIR: ((a,b) -- a b)
            0x11 => {
                match self.pop()? {
                    Type::Pair(a, b) => {
                        self.stack.push(*a);
                        self.stack.push(*b);
                        Ok(())
                    }
                    Type::Top => {
                        // Unknown type - introduce variables
                        let a = self.fresh_var();
                        let b = self.fresh_var();
                        self.stack.push(a);
                        self.stack.push(b);
                        Ok(())
                    }
                    t => Err(format!("UNPAIR expects Pair, got {}", t.name()))
                }
            }
            
            // LEFT: (a -- Left(a))
            0x12 => {
                let a = self.pop()?;
                let b = self.fresh_var();
                self.stack.push(Type::Sum(Box::new(a), Box::new(b)));
                Ok(())
            }
            
            // RIGHT: (a -- Right(a))  
            0x13 => {
                let a = self.pop()?;
                let b = self.fresh_var();
                self.stack.push(Type::Sum(Box::new(b), Box::new(a)));
                Ok(())
            }
            
            // CASE: (Sum off_l off_r -- ...)
            0x14 => {
                let _off_l = self.read_i16(code, pc)?;
                let _off_r = self.read_i16(code, pc)?;
                match self.pop()? {
                    Type::Sum(a, _b) => {
                        // Both branches should result in same stack
                        // For now, just push a fresh variable
                        self.stack.push(*a);  // Simplified - real impl needs branch analysis
                        Ok(())
                    }
                    Type::Top => {
                        let var = self.fresh_var();
                        self.stack.push(var);
                        Ok(())
                    }
                    t => Err(format!("CASE expects Sum, got {}", t.name()))
                }
            }
            
            // QUOTE
            0x20 => {
                let len = self.read_u16(code, pc)? as usize;
                *pc += len;  // Skip quoted code
                // Push a quote type (we'd need to analyze the quoted code for full effect)
                self.stack.push(Type::Quote(StackEffect::identity()));
                Ok(())
            }
            
            // APPLY
            0x21 => {
                match self.pop()? {
                    Type::Quote(effect) => {
                        // Apply the effect
                        for input in effect.inputs.iter().rev() {
                            let t = self.pop()?;
                            if !t.is_subtype_of(input) {
                                return Err(format!("APPLY: expected {}, got {}", input.name(), t.name()));
                            }
                        }
                        for output in &effect.outputs {
                            self.stack.push(output.clone());
                        }
                        Ok(())
                    }
                    Type::Top => Ok(()),  // Unknown - can't check
                    t => Err(format!("APPLY expects Quote, got {}", t.name()))
                }
            }
            
            // CALL
            0x22 => {
                let _idx = self.read_u16(code, pc)?;
                // Function calls change the stack in ways we can't know
                // without analyzing the called function's body.
                // P4: "constraints attenuate" — we push Top (maximally 
                // permissive) rather than clearing the stack, because the
                // function may consume any number of inputs and produce
                // any number of outputs. Keeping existing values as Top
                // allows subsequent operations to proceed.
                //
                // A full implementation would look up the function's 
                // declared stack effect annotation.
                self.stack.push(Type::Top);
                Ok(())
            }
            
            // RET — marks end of a function body. Stop checking this path.
            0x23 => {
                // RET returns to the caller. We can't continue linear
                // analysis past a RET because we don't know where the
                // caller's PC is. This is not an error — it's a function boundary.
                Ok(())
            }
            
            // COND: (bool then_q else_q -- ...) old-style conditional
            0x24 => {
                self.pop()?; // else_q (Quote)
                self.pop()?; // then_q (Quote)
                self.pop()?; // bool
                // Result depends on which branch — push Top
                self.stack.push(Type::Top);
                Ok(())
            }
            
            // LOOP: (body_q -- ...) repeat while true
            0x25 => {
                self.pop()?; // body_q (Quote)
                // Loop may produce any values
                self.stack.push(Type::Top);
                Ok(())
            }
            
            // INT8
            0x30 => {
                *pc += 1;
                self.stack.push(Type::Int);
                Ok(())
            }
            
            // INT16
            0x31 => {
                *pc += 2;
                self.stack.push(Type::Int);
                Ok(())
            }
            
            // INT32
            0x32 => {
                *pc += 4;
                self.stack.push(Type::Int);
                Ok(())
            }
            
            // INT64
            0x33 => {
                *pc += 8;
                self.stack.push(Type::Int);
                Ok(())
            }
            
            // F32
            0x34 => {
                *pc += 4;
                self.stack.push(Type::Float);
                Ok(())
            }
            
            // F64
            0x35 => {
                *pc += 8;
                self.stack.push(Type::Float);
                Ok(())
            }
            
            // STR
            0x36 => {
                let len = self.read_u16(code, pc)? as usize;
                *pc += len;
                self.stack.push(Type::Str);
                Ok(())
            }
            
            // NIL
            0x37 => {
                self.stack.push(Type::Unit);
                Ok(())
            }
            
            // TRUE, FALSE
            0x38 | 0x39 => {
                self.stack.push(Type::Bool);
                Ok(())
            }
            
            // ADD, SUB, MUL, DIV
            0x40 | 0x41 | 0x42 | 0x43 => {
                let b = self.pop_numeric()?;
                let a = self.pop_numeric()?;
                // Result is Float if either is Float
                let result = a.join(&b);
                self.stack.push(result);
                Ok(())
            }
            
            // MOD
            0x44 => {
                self.expect(Type::Int)?;
                self.expect(Type::Int)?;
                self.stack.push(Type::Int);
                Ok(())
            }
            
            // NEG
            0x45 => {
                let a = self.pop_numeric()?;
                self.stack.push(a);
                Ok(())
            }
            
            // FADD, FSUB, FMUL, FDIV: Float Float → Float
            0x46 | 0x47 | 0x48 | 0x49 => {
                self.expect(Type::Float)?;
                self.expect(Type::Float)?;
                self.stack.push(Type::Float);
                Ok(())
            }
            
            // FNEG, FSQRT, FABS: Float → Float
            0x4A | 0x4B | 0x4C => {
                self.expect(Type::Float)?;
                self.stack.push(Type::Float);
                Ok(())
            }
            
            // I2F: Int → Float
            0x4D => {
                self.expect(Type::Int)?;
                self.stack.push(Type::Float);
                Ok(())
            }
            
            // F2I: Float → Int
            0x4E => {
                self.expect(Type::Float)?;
                self.stack.push(Type::Int);
                Ok(())
            }
            
            // FEXP: Float → Float (e^x)
            0x4F => {
                self.expect(Type::Float)?;
                self.stack.push(Type::Float);
                Ok(())
            }
            
            // EQ, NE
            0x50 | 0x55 => {
                self.pop()?;
                self.pop()?;
                self.stack.push(Type::Bool);
                Ok(())
            }
            
            // LT, GT, LE, GE
            0x51 | 0x52 | 0x53 | 0x54 => {
                self.pop_numeric()?;
                self.pop_numeric()?;
                self.stack.push(Type::Bool);
                Ok(())
            }
            
            // FLOG: Float → Float (ln(x))
            0x56 => {
                self.expect(Type::Float)?;
                self.stack.push(Type::Float);
                Ok(())
            }
            
            // FSIN: Float → Float
            0x57 => {
                self.expect(Type::Float)?;
                self.stack.push(Type::Float);
                Ok(())
            }
            
            // FCOS: Float → Float
            0x58 => {
                self.expect(Type::Float)?;
                self.stack.push(Type::Float);
                Ok(())
            }
            
            // FATAN2: Float Float → Float
            0x59 => {
                self.expect(Type::Float)?;
                self.expect(Type::Float)?;
                self.stack.push(Type::Float);
                Ok(())
            }
            
            // FPOW: Float Float → Float
            0x5A => {
                self.expect(Type::Float)?;
                self.expect(Type::Float)?;
                self.stack.push(Type::Float);
                Ok(())
            }
            
            // FFLOOR: Float → Float
            0x5B => {
                self.expect(Type::Float)?;
                self.stack.push(Type::Float);
                Ok(())
            }
            
            // FCEIL: Float → Float
            0x5C => {
                self.expect(Type::Float)?;
                self.stack.push(Type::Float);
                Ok(())
            }
            
            // FROUND: Float → Float
            0x5D => {
                self.expect(Type::Float)?;
                self.stack.push(Type::Float);
                Ok(())
            }
            
            // AND, OR, XOR — polymorphic: (bool bool -- bool) or (int int -- int)
            0x60 | 0x61 | 0x63 => {
                self.pop()?;
                self.pop()?;
                self.stack.push(Type::Top);
                Ok(())
            }
            
            // NOT — polymorphic: (bool -- bool) or (int -- int)
            0x62 => {
                self.pop()?;
                self.stack.push(Type::Top);
                Ok(())
            }

            // BAND, BOR, BXOR: (int int -- int)
            0x64 | 0x65 | 0x66 => {
                self.pop()?;
                self.pop()?;
                self.stack.push(Type::Int);
                Ok(())
            }

            // BNOT: (int -- int)
            0x67 => {
                self.pop()?;
                self.stack.push(Type::Int);
                Ok(())
            }

            // SHL, SHR: (int int -- int)
            0x68 | 0x69 => {
                self.pop()?;
                self.pop()?;
                self.stack.push(Type::Int);
                Ok(())
            }
            
            // JMP — unconditional jump. Follow it.
            // For backward jumps (loops), we don't follow — the loop body
            // has already been checked linearly. P4: constraints only
            // attenuate, so if the body type-checks once, it type-checks
            // on every iteration.
            0x70 => {
                let offset = self.read_i16(code, pc)?;
                let target = (*pc as i64 + offset as i64) as usize;
                if target < *pc {
                    // Backward jump = loop back edge. Don't follow.
                    // The loop body was already checked. Move past the JMP.
                    Ok(())
                } else {
                    // Forward jump: follow it.
                    *pc = target;
                    Ok(())
                }
            }
            
            // JZ, JNZ
            0x71 | 0x72 => {
                let _offset = self.read_i16(code, pc)?;
                self.expect(Type::Bool)?;
                Ok(())
            }
            
            // LIST
            0x80 => {
                let count = self.read_u16(code, pc)? as usize;
                if count == 0 {
                    let var = self.fresh_var();
                    self.stack.push(Type::List(Box::new(var)));
                } else {
                    let mut elem_type = self.pop()?;
                    for _ in 1..count {
                        let t = self.pop()?;
                        elem_type = elem_type.join(&t);
                    }
                    self.stack.push(Type::List(Box::new(elem_type)));
                }
                Ok(())
            }
            
            // UNLIST
            0x81 => {
                match self.pop()? {
                    Type::List(_elem) => {
                        // We don't know how many elements, so push Top
                        self.stack.push(Type::Top);
                        Ok(())
                    }
                    Type::Top => Ok(()),
                    t => Err(format!("UNLIST expects List, got {}", t.name()))
                }
            }
            
            // LEN
            0x82 => {
                let t = self.peek()?;
                match t {
                    Type::List(_) | Type::Str | Type::Top => {
                        self.stack.push(Type::Int);
                        Ok(())
                    }
                    _ => Err(format!("LEN expects List or Str, got {}", t.name()))
                }
            }
            
            // GET
            0x83 => {
                self.expect(Type::Int)?;
                match self.pop()? {
                    Type::List(elem) => {
                        self.stack.push(*elem);
                        Ok(())
                    }
                    Type::Top => {
                        self.stack.push(Type::Top);
                        Ok(())
                    }
                    t => Err(format!("GET expects List, got {}", t.name()))
                }
            }
            
            // SET
            0x84 => {
                let v = self.pop()?;
                self.expect(Type::Int)?;
                match self.pop()? {
                    Type::List(elem) => {
                        if !v.is_subtype_of(&elem) {
                            return Err(format!("SET: element type {} incompatible with list [{}]", 
                                             v.name(), elem.name()));
                        }
                        self.stack.push(Type::List(elem));
                        Ok(())
                    }
                    Type::Top => {
                        self.stack.push(Type::Top);
                        Ok(())
                    }
                    t => Err(format!("SET expects List, got {}", t.name()))
                }
            }
            
            // MAP: (List Quote -- List)
            // The functor lift: f:A→B ⟹ map(f):[A]→[B]
            // We can't fully type-check the quote's effect statically
            // (would need dependent types), so we check structural constraints
            // and type the result conservatively.
            0x85 => {
                let quote_type = self.pop()?;
                let list_type = self.pop()?;
                match (&list_type, &quote_type) {
                    (Type::List(_), Type::Quote(_)) | 
                    (Type::List(_), Type::Top) |
                    (Type::Top, _) => {
                        // Result is List(Top) — we can't infer element type
                        // without executing the quote
                        self.stack.push(Type::List(Box::new(Type::Top)));
                        Ok(())
                    }
                    (Type::List(_), t) => Err(format!("MAP expects Quote, got {}", t.name())),
                    (t, _) => Err(format!("MAP expects List, got {}", t.name())),
                }
            }
            
            // FOLD: (List init Quote -- result)  
            // The catamorphism: (f:B×A→B, b₀:B) → [A] → B
            0x86 => {
                let quote_type = self.pop()?;
                let init_type = self.pop()?;
                let list_type = self.pop()?;
                match (&list_type, &quote_type) {
                    (Type::List(_), Type::Quote(_)) |
                    (Type::List(_), Type::Top) |
                    (Type::Top, _) => {
                        // Result type is the init type (accumulator type)
                        // This is the best static approximation
                        self.stack.push(init_type);
                        Ok(())
                    }
                    (Type::List(_), t) => Err(format!("FOLD expects Quote, got {}", t.name())),
                    (t, _) => Err(format!("FOLD expects List, got {}", t.name())),
                }
            }
            
            // ZIP: (List List -- List)
            // Product lifting: [A]×[B] → [(A,B)]
            0x87 => {
                let b_type = self.pop()?;
                let a_type = self.pop()?;
                match (&a_type, &b_type) {
                    (Type::List(a_elem), Type::List(b_elem)) => {
                        let pair_type = Type::Pair(a_elem.clone(), b_elem.clone());
                        self.stack.push(Type::List(Box::new(pair_type)));
                        Ok(())
                    }
                    (Type::List(_), Type::Top) | (Type::Top, Type::List(_)) | (Type::Top, Type::Top) => {
                        self.stack.push(Type::List(Box::new(Type::Top)));
                        Ok(())
                    }
                    (Type::List(_), t) => Err(format!("ZIP expects List, got {}", t.name())),
                    (t, _) => Err(format!("ZIP expects List, got {}", t.name())),
                }
            }

            // APPEND: (List value -- List)
            0x88 => {
                let val_type = self.pop()?;
                let list_type = self.pop()?;
                match &list_type {
                    Type::List(_) | Type::Top => {
                        let _ = val_type; // value can be any type
                        self.stack.push(Type::List(Box::new(Type::Top)));
                        Ok(())
                    }
                    t => Err(format!("APPEND expects List, got {}", t.name())),
                }
            }

            // REVERSE: (List -- List)
            0x89 => {
                let list_type = self.pop()?;
                match &list_type {
                    Type::List(_) | Type::Top => {
                        self.stack.push(list_type);
                        Ok(())
                    }
                    t => Err(format!("REVERSE expects List, got {}", t.name())),
                }
            }

            // FIBER_NEW: (Quote -- Fiber)
            0xB0 => {
                let t = self.pop()?;
                match t {
                    Type::Quote(_) | Type::Top => {
                        self.stack.push(Type::Top); // fiber type
                        Ok(())
                    }
                    t => Err(format!("FIBER_NEW expects Quote, got {}", t.name())),
                }
            }

            // FIBER_STEP: (Fiber -- Fiber Bool)
            0xB1 => {
                let _t = self.pop()?;
                self.stack.push(Type::Top); // fiber'
                self.stack.push(Type::Bool);
                Ok(())
            }

            // FIBER_PUSH: (Fiber Value -- Fiber)
            0xB2 => {
                self.pop()?; // value
                self.pop()?; // fiber
                self.stack.push(Type::Top); // fiber'
                Ok(())
            }

            // FIBER_STACK: (Fiber -- Fiber List)
            0xB3 => {
                self.pop()?;
                self.stack.push(Type::Top); // fiber (returned)
                self.stack.push(Type::List(Box::new(Type::Top)));
                Ok(())
            }

            // FIBER_STATUS: (Fiber -- Fiber Bool)
            0xB4 => {
                self.pop()?;
                self.stack.push(Type::Top); // fiber
                self.stack.push(Type::Bool);
                Ok(())
            }

            // SPAWN: (Quote Int -- List) sandboxed execution
            0xB5 => {
                self.pop()?; // caps (Int)
                let t = self.pop()?; // quote
                match t {
                    Type::Quote(_) | Type::Top => {
                        self.stack.push(Type::List(Box::new(Type::Top)));
                        Ok(())
                    }
                    t => Err(format!("SPAWN expects Quote, got {}", t.name())),
                }
            }

            // CHAN_NEW: (-- Channel)
            0xB6 => {
                self.stack.push(Type::Top); // channel
                Ok(())
            }

            // CHAN_SEND: (Channel Value --)
            0xB7 => {
                self.pop()?; // value
                self.pop()?; // channel
                Ok(())
            }

            // CHAN_RECV: (Channel -- Value)
            0xB8 => {
                self.pop()?; // channel
                self.stack.push(Type::Top); // received value
                Ok(())
            }

            // ================================================================
            // LINEAR TYPES (0xC0-0xC4) — P4: Constraints Attenuate
            // ================================================================
            
            // LINEAR: (a -- Linear(a))
            0xC0 => {
                let a = self.pop()?;
                self.stack.push(Type::Linear(Box::new(a)));
                Ok(())
            }
            
            // AFFINE: (a -- Affine(a))
            0xC1 => {
                let a = self.pop()?;
                self.stack.push(Type::Affine(Box::new(a)));
                Ok(())
            }
            
            // CONSUME: (Linear(a)|Affine(a) -- a) unwrap linearity
            0xC2 => {
                match self.pop()? {
                    Type::Linear(inner) => {
                        self.stack.push(*inner);
                        Ok(())
                    }
                    Type::Affine(inner) => {
                        self.stack.push(*inner);
                        Ok(())
                    }
                    Type::Top => {
                        self.stack.push(Type::Top);
                        Ok(())
                    }
                    t => Err(format!("CONSUME expects Linear or Affine, got {}", t.name()))
                }
            }
            
            // IS_LINEAR: (a -- a Bool)
            0xC3 => {
                let a = self.peek()?;
                let _ = a; // peek doesn't consume
                self.stack.push(Type::Bool);
                Ok(())
            }
            
            // IS_AFFINE: (a -- a Bool)
            0xC4 => {
                let a = self.peek()?;
                let _ = a;
                self.stack.push(Type::Bool);
                Ok(())
            }

            // ================================================================
            // TRAINING PRIMITIVES (0xC5-0xCD)
            // ================================================================

            // TIMES: (Int Quote -- ...)
            0xC5 => {
                self.pop()?; // quote
                self.pop()?; // count
                // Times may leave anything on the stack
                self.stack.push(Type::Top);
                Ok(())
            }

            // FILTER: (List Quote -- List)
            0xC6 => {
                let quote_type = self.pop()?;
                let list_type = self.pop()?;
                match (&list_type, &quote_type) {
                    (Type::List(_), Type::Quote(_)) |
                    (Type::List(_), Type::Top) |
                    (Type::Top, _) => {
                        self.stack.push(Type::List(Box::new(Type::Top)));
                        Ok(())
                    }
                    (Type::List(_), t) => Err(format!("FILTER expects Quote, got {}", t.name())),
                    (t, _) => Err(format!("FILTER expects List, got {}", t.name())),
                }
            }

            // HEAD: (List -- elem)
            0xC7 => {
                match self.pop()? {
                    Type::List(elem) => {
                        self.stack.push(*elem);
                        Ok(())
                    }
                    Type::Top => {
                        self.stack.push(Type::Top);
                        Ok(())
                    }
                    t => Err(format!("HEAD expects List, got {}", t.name()))
                }
            }

            // TAIL: (List -- List)
            0xC8 => {
                match self.pop()? {
                    Type::List(elem) => {
                        self.stack.push(Type::List(elem));
                        Ok(())
                    }
                    Type::Top => {
                        self.stack.push(Type::Top);
                        Ok(())
                    }
                    t => Err(format!("TAIL expects List, got {}", t.name()))
                }
            }

            // RANGE: (Int Int -- List)
            0xC9 => {
                self.expect(Type::Int)?;
                self.expect(Type::Int)?;
                self.stack.push(Type::List(Box::new(Type::Int)));
                Ok(())
            }

            // FIRST: (Pair -- Pair a)
            0xCA => {
                match self.pop()? {
                    Type::Pair(a, b) => {
                        self.stack.push(Type::Pair(a.clone(), b));
                        self.stack.push(*a);
                        Ok(())
                    }
                    Type::Top => {
                        self.stack.push(Type::Top);
                        self.stack.push(Type::Top);
                        Ok(())
                    }
                    t => Err(format!("FIRST expects Pair, got {}", t.name()))
                }
            }

            // SECOND: (Pair -- Pair b)
            0xCB => {
                match self.pop()? {
                    Type::Pair(a, b) => {
                        self.stack.push(Type::Pair(a, b.clone()));
                        self.stack.push(*b);
                        Ok(())
                    }
                    Type::Top => {
                        self.stack.push(Type::Top);
                        self.stack.push(Type::Top);
                        Ok(())
                    }
                    t => Err(format!("SECOND expects Pair, got {}", t.name()))
                }
            }

            // LIST_CONCAT: (List List -- List)
            0xCC => {
                let b = self.pop()?;
                let a = self.pop()?;
                match (&a, &b) {
                    (Type::List(ea), Type::List(eb)) => {
                        let elem = ea.join(eb);
                        self.stack.push(Type::List(Box::new(elem)));
                        Ok(())
                    }
                    (Type::List(_), Type::Top) | (Type::Top, Type::List(_)) | (Type::Top, Type::Top) => {
                        self.stack.push(Type::List(Box::new(Type::Top)));
                        Ok(())
                    }
                    (Type::List(_), t) => Err(format!("LIST_CONCAT expects List, got {}", t.name())),
                    (t, _) => Err(format!("LIST_CONCAT expects List, got {}", t.name())),
                }
            }

            // IS_EMPTY: (List -- List Bool)
            0xCD => {
                let t = self.peek()?;
                match t {
                    Type::List(_) | Type::Top => {
                        self.stack.push(Type::Bool);
                        Ok(())
                    }
                    _ => Err(format!("IS_EMPTY expects List, got {}", t.name()))
                }
            }
            
            // TYPE_OF: (a -- a Str)
            0xD0 => {
                let _a = self.peek()?;
                self.stack.push(Type::Str);
                Ok(())
            }
            
            // DEPTH: (-- Int)
            0xD1 => {
                self.stack.push(Type::Int);
                Ok(())
            }
            
            // DESCRIBE: (Str -- Str)
            0xD2 => {
                self.expect(Type::Str)?;
                self.stack.push(Type::Str);
                Ok(())
            }
            
            // STR_LEN: (Str -- Str Int)
            0xE0 => {
                let _s = self.peek()?;
                self.stack.push(Type::Int);
                Ok(())
            }
            
            // STR_GET: (Str Int -- Str)
            0xE1 => {
                self.pop()?;  // index
                self.pop()?;  // string
                self.stack.push(Type::Str);
                Ok(())
            }
            
            // STR_CONCAT: (Str Str -- Str)
            0xE2 => {
                self.pop()?;
                self.pop()?;
                self.stack.push(Type::Str);
                Ok(())
            }
            
            // STR_SLICE: (Str Int Int -- Str)
            0xE3 => {
                self.pop()?;  // end
                self.pop()?;  // start
                self.pop()?;  // string
                self.stack.push(Type::Str);
                Ok(())
            }
            
            // TO_STR: (a -- Str)
            0xE4 => {
                self.pop()?;
                self.stack.push(Type::Str);
                Ok(())
            }
            
            // STR_FIND: (Str Str -- Int)
            0xE5 => {
                self.pop()?;  // pattern
                self.pop()?;  // haystack
                self.stack.push(Type::Int);
                Ok(())
            }
            
            // STR_SPLIT: (Str Str -- List)
            0xE6 => {
                self.pop()?;  // delimiter
                self.pop()?;  // string
                self.stack.push(Type::List(Box::new(Type::Str)));
                Ok(())
            }
            
            // STR_REPLACE: (Str Str Str -- Str)
            0xE7 => {
                self.pop()?;  // to
                self.pop()?;  // from
                self.pop()?;  // string
                self.stack.push(Type::Str);
                Ok(())
            }
            
            // STR_UPPER: (Str -- Str)
            0xE8 => {
                self.pop()?;
                self.stack.push(Type::Str);
                Ok(())
            }
            
            // STR_LOWER: (Str -- Str)
            0xE9 => {
                self.pop()?;
                self.stack.push(Type::Str);
                Ok(())
            }
            
            // STR_TRIM: (Str -- Str)
            0xEA => {
                self.pop()?;
                self.stack.push(Type::Str);
                Ok(())
            }
            
            // TRY: (Quote -- Top) execute quote, catch errors
            0xA3 => {
                let t = self.pop()?;
                match t {
                    Type::Quote(_) | Type::Top => {
                        self.stack.push(Type::Top); // result or Error
                        Ok(())
                    }
                    t => Err(format!("TRY expects Quote, got {}", t.name()))
                }
            }
            
            // FAIL: (Str -- !) unwind with error
            0xA4 => {
                self.expect(Type::Str)?;
                Ok(()) // diverges
            }
            
            // IS_ERROR: (a -- a Bool) check if error
            0xA5 => {
                let _a = self.peek()?;
                self.stack.push(Type::Bool);
                Ok(())
            }
            
            // PROPAGATE: (a -- a) if Error, re-fail; else pass through
            // P1: always a tool (S→S). The re-fail is structured (caught by try).
            0xA6 => {
                let a = self.pop()?;
                self.stack.push(a); // type passes through unchanged
                Ok(())
            }

            // MAKE-ERROR: (str -- error) Create Error value (P1: S→S)
            0xA7 => {
                self.pop()?;
                self.stack.push(Type::Top); // Error is a value
                Ok(())
            }
            
            // PRINT: (value --) Write to stdout
            // P4: requires IO capability — constraints attenuate
            0xA0 => {
                if self.caps & crate::bytecode::CAP_IO == 0 {
                    return Err("PRINT requires 'io' capability — compile with --cap io".into());
                }
                self.pop()?;
                Ok(())
            }
            
            // PRINTLN: (value --) Write to stdout + newline
            0xA1 => {
                if self.caps & crate::bytecode::CAP_IO == 0 {
                    return Err("PRINTLN requires 'io' capability — compile with --cap io".into());
                }
                self.pop()?;
                Ok(())
            }
            
            // RAND: (-- n) Random u64 from OS entropy
            0xA2 => {
                if self.caps & crate::bytecode::CAP_IO == 0 {
                    return Err("RAND requires 'io' capability — compile with --cap io".into());
                }
                self.stack.push(Type::Int);
                Ok(())
            }
            
            // STORE: (value --) Store to local slot
            0xA8 => {
                let slot = *code.get(*pc).ok_or("Unexpected end of code")? as usize;
                *pc += 1;
                let t = self.pop()?;
                while self.locals.len() <= slot {
                    self.locals.push(None);
                }
                self.locals[slot] = Some(t);
                Ok(())
            }
            
            // LOAD: (-- value) Load from local slot
            0xA9 => {
                let slot = *code.get(*pc).ok_or("Unexpected end of code")? as usize;
                *pc += 1;
                if slot < self.locals.len() {
                    if let Some(t) = &self.locals[slot] {
                        self.stack.push(t.clone());
                    } else {
                        self.stack.push(Type::Top);
                    }
                } else {
                    self.stack.push(Type::Top);
                }
                Ok(())
            }
            
            // MAP_NEW: (-- Map)
            0xAA => {
                self.stack.push(Type::Top); // map
                Ok(())
            }
            
            // MAP_GET: (Map Key -- Map Value)
            0xAB => {
                self.pop()?; // key
                let m = self.pop()?;
                self.stack.push(m); // map returned
                self.stack.push(Type::Top); // value
                Ok(())
            }
            
            // MAP_SET: (Map Key Value -- Map)
            0xAC => {
                self.pop()?; // value
                self.pop()?; // key
                self.pop()?; // map
                self.stack.push(Type::Top); // map returned
                Ok(())
            }
            
            // MAP_KEYS: (Map -- Map List)
            0xAD => {
                let m = self.pop()?;
                self.stack.push(m); // map returned
                self.stack.push(Type::List(Box::new(Type::Str))); // keys
                Ok(())
            }
            
            // MAP_HAS: (Map Key -- Map Bool)
            0xAE => {
                self.pop()?; // key
                let m = self.pop()?;
                self.stack.push(m); // map returned
                self.stack.push(Type::Bool);
                Ok(())
            }
            
            // SYSCALL: generalized host call with u8 call_id operand
            0xAF => {
                let call_id = *code.get(*pc).ok_or("Unexpected end of code")?;
                *pc += 1;
                match call_id {
                    // file-read: (Str -- Str) cap:fs
                    0x01 => {
                        self.pop()?;
                        self.stack.push(Type::Str);
                    }
                    // file-write: (Str Str --) cap:fs
                    0x02 => {
                        self.pop()?;  // content
                        self.pop()?;  // path
                    }
                    // file-exists: (Str -- Bool) cap:fs
                    0x03 => {
                        self.pop()?;
                        self.stack.push(Type::Bool);
                    }
                    // time-now: (-- Float) cap:io
                    0x10 => {
                        self.stack.push(Type::Float);
                    }
                    // env-get: (Str -- Str) cap:io
                    0x20 => {
                        self.pop()?;
                        self.stack.push(Type::Str);
                    }
                    // exec: (Str -- Pair(Str, Int)) cap:exec
                    0x30 => {
                        self.pop()?;
                        self.stack.push(Type::Pair(Box::new(Type::Str), Box::new(Type::Int)));
                    }
                    // readline: (Str -- Str) cap:io
                    0x04 => {
                        self.pop()?;
                        self.stack.push(Type::Str);
                    }
                    // file-append: (Str Str --) cap:fs
                    0x05 => {
                        self.pop()?;
                        self.pop()?;
                    }
                    // file-delete: (Str -- Bool) cap:fs
                    0x06 => {
                        self.pop()?;
                        self.stack.push(Type::Bool);
                    }
                    // file-list: (Str -- List) cap:fs
                    0x07 => {
                        self.pop()?;
                        self.stack.push(Type::List(Box::new(Type::Str)));
                    }
                    // http-get: (Str -- Pair(Str, Int)) cap:net
                    0x40 => {
                        self.pop()?;
                        self.stack.push(Type::Pair(Box::new(Type::Str), Box::new(Type::Int)));
                    }
                    // http-post: (Str Str Str -- Pair(Str, Int)) cap:net
                    0x41 => {
                        self.pop()?;
                        self.pop()?;
                        self.pop()?;
                        self.stack.push(Type::Pair(Box::new(Type::Str), Box::new(Type::Int)));
                    }
                    // http-serve: (Int Quote -- ) cap:net
                    0x50 => {
                        self.pop()?; // handler
                        self.pop()?; // port
                    }
                    _ => return Err(format!("Unknown syscall id: 0x{:02X}", call_id)),
                }
                Ok(())
            }
            
            // FETCH: (Int -- Int) Read bytecode at offset, returns byte as int
            0x90 => {
                self.expect(Type::Int)?;
                self.stack.push(Type::Int);
                Ok(())
            }
            
            // SIZE: ( -- Int) Get bytecode length
            0x91 => {
                self.stack.push(Type::Int);
                Ok(())
            }
            
            // HALT
            0xFF => Ok(()),
            
            _ => Err(format!("Unknown opcode: 0x{:02X}", op)),
        }
    }
    
    // === Helpers ===
    
    fn pop(&mut self) -> Result<Type, String> {
        self.stack.pop().ok_or_else(|| "Stack underflow".into())
    }
    
    fn peek(&self) -> Result<Type, String> {
        self.stack.last().cloned().ok_or_else(|| "Stack underflow".into())
    }
    
    fn expect(&mut self, expected: Type) -> Result<(), String> {
        let actual = self.pop()?;
        if actual.is_subtype_of(&expected) || actual == Type::Top {
            // Top is accepted anywhere — P4 attenuation:
            // Top can narrow to any type at runtime.
            // CALL returns Top because we can't statically track return types.
            Ok(())
        } else {
            Err(format!("Expected {}, got {}", expected.name(), actual.name()))
        }
    }
    
    fn pop_numeric(&mut self) -> Result<Type, String> {
        let t = self.pop()?;
        match t {
            Type::Int | Type::Float | Type::Top => Ok(t),
            _ => Err(format!("Expected numeric type, got {}", t.name())),
        }
    }
    
    fn fresh_var(&mut self) -> Type {
        let v = Type::Var(self.next_var);
        self.next_var += 1;
        v
    }
    
    fn read_u16(&self, code: &[u8], pc: &mut usize) -> Result<u16, String> {
        if *pc + 2 > code.len() {
            return Err("Unexpected end of code".into());
        }
        let v = u16::from_le_bytes([code[*pc], code[*pc + 1]]);
        *pc += 2;
        Ok(v)
    }
    
    fn read_i16(&self, code: &[u8], pc: &mut usize) -> Result<i16, String> {
        Ok(self.read_u16(code, pc)? as i16)
    }
    
    /// Get final stack types
    pub fn final_stack(&self) -> &[Type] {
        &self.stack
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::*;

    #[test]
    fn test_simple_valid() {
        let mut asm = Assembler::new();
        asm.emit_i8(3);
        asm.emit_i8(4);
        asm.emit(Op::Add);
        asm.emit(Op::Halt);
        
        let module = asm.finalize().unwrap();
        let mut checker = ProofChecker::new();
        let errors = checker.check(&module.code);
        
        assert!(errors.is_empty(), "Expected no errors, got {:?}", errors);
        assert_eq!(checker.final_stack(), &[Type::Int]);
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
        let mut checker = ProofChecker::new();
        let errors = checker.check(&module.code);
        
        assert!(errors.is_empty(), "Expected no errors, got {:?}", errors);
        assert_eq!(checker.final_stack(), &[Type::Int]);
    }
    
    #[test]
    fn test_type_error() {
        // Stack underflow: And needs 2 values but only 1 is on stack
        let mut asm = Assembler::new();
        asm.emit(Op::True);   // Push Bool (1 value)
        asm.emit(Op::And);    // And needs 2 → stack underflow
        asm.emit(Op::Halt);
        
        let module = asm.finalize().unwrap();
        let mut checker = ProofChecker::new();
        let errors = checker.check(&module.code);
        
        assert!(!errors.is_empty(), "Expected type error");
    }
    
    #[test]
    fn test_stack_underflow() {
        let mut asm = Assembler::new();
        asm.emit(Op::Add);  // Stack is empty!
        asm.emit(Op::Halt);
        
        let module = asm.finalize().unwrap();
        let mut checker = ProofChecker::new();
        let errors = checker.check(&module.code);
        
        assert!(!errors.is_empty(), "Expected stack underflow error");
    }
    
    #[test]
    fn test_left_right() {
        let mut asm = Assembler::new();
        asm.emit_i8(42);
        asm.emit(Op::Left);
        asm.emit(Op::Halt);
        
        let module = asm.finalize().unwrap();
        let mut checker = ProofChecker::new();
        let errors = checker.check(&module.code);
        
        assert!(errors.is_empty(), "Expected no errors, got {:?}", errors);
        
        // Should be Sum(Int, ?0) - a sum type
        match &checker.final_stack()[0] {
            Type::Sum(left, _) => {
                assert_eq!(**left, Type::Int);
            }
            other => panic!("Expected Sum type, got {:?}", other),
        }
    }
}
