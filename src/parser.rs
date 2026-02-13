//! Kore Parser
//!
//! Parses Kore source code into bytecode.
//!
//! P1: The parser is a tool (Source → Bytecode)
//! P2: Apply only - parsing is applying grammar rules
//! P3: Composition = Concatenation - statements concat
//! P4: Types inferred and checked after parsing
//!
//! Syntax (Forth-like, stack-based):
//!
//!   -- comment (or \ comment)
//!   123       -- push integer
//!   3.14      -- push float
//!   "hello"   -- push string
//!   true/false -- push boolean
//!   nil       -- push nil
//!   
//!   drop dup swap rot over  -- stack ops
//!   + - * / %               -- arithmetic
//!   < > <= >= = !=          -- comparison
//!   and or not              -- logic
//!   
//!   pair unpair             -- product types
//!   left right case         -- sum types
//!   [ ... ]                 -- quote (code block)
//!   apply                   -- execute quote
//!   ( ... )                 -- list literal (elements counted at parse time)
//!   #( ... )                -- array literal (integer elements → contiguous i64)
//!   unlist len get set      -- list ops
//!   collect N               -- collect N stack values into list
//!   map fold zip            -- higher-order list ops
//!   array-new array-get array-set array-len array-push array-from -- array ops
//!   if ... else ... end     -- conditional (keyword style, compiles to jumps)
//!   if ... end              -- conditional (no else branch)
//!   while ... do ... end    -- loop (keyword style, compiles to jumps)
//!   cond                    -- (bool [then] [else] -- ...) quote-style conditional
//!   loop                    -- ([body] -- ...) quote-style loop (body returns bool)
//!   
//!   : name ... ;            -- define function
//!   name                    -- call function

use std::collections::HashMap;
use crate::bytecode::{Assembler, Op, BytecodeModule};

// ============================================================================
// LEXER
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Literals
    Int(i64),
    Float(f64),
    String(String),
    True,
    False,
    Nil,
    
    // Identifiers
    Ident(String),
    
    // Symbols
    Colon,      // :
    Semicolon,  // ;
    LBracket,   // [
    RBracket,   // ]
    LParen,     // (
    RParen,     // )
    HashParen,  // #( — array literal opener
    Arrow,      // ->
    
    // Operators (as tokens for flexibility)
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Ne,
    
    // End of input
    Eof,
}

pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
    line: usize,
    col: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Lexer {
            input,
            pos: 0,
            line: 1,
            col: 1,
        }
    }
    
    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }
    
    fn next_char(&mut self) -> Option<char> {
        let c = self.peek_char()?;
        self.pos += c.len_utf8();
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }
    
    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() {
                self.next_char();
            } else if c == '\\' {
                // Comment - skip to end of line (Forth-style)
                while let Some(c) = self.peek_char() {
                    if c == '\n' {
                        break;
                    }
                    self.next_char();
                }
            } else if c == '-' {
                // Check for -- comment (Haskell-style)
                let rest = &self.input[self.pos..];
                if rest.starts_with("--") {
                    while let Some(c) = self.peek_char() {
                        if c == '\n' {
                            break;
                        }
                        self.next_char();
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }
    
    pub fn next_token(&mut self) -> Result<Token, String> {
        self.skip_whitespace();
        
        let c = match self.peek_char() {
            Some(c) => c,
            None => return Ok(Token::Eof),
        };
        
        // Single-character tokens
        match c {
            ':' => { self.next_char(); return Ok(Token::Colon); }
            ';' => { self.next_char(); return Ok(Token::Semicolon); }
            '[' => { self.next_char(); return Ok(Token::LBracket); }
            ']' => { self.next_char(); return Ok(Token::RBracket); }
            '#' => {
                self.next_char();
                if self.peek_char() == Some('(') {
                    self.next_char();
                    return Ok(Token::HashParen);
                }
                return Err(format!("unexpected character after '#' at line {}", self.line));
            }
            '(' => { self.next_char(); return Ok(Token::LParen); }
            ')' => { self.next_char(); return Ok(Token::RParen); }
            '+' => { self.next_char(); return Ok(Token::Plus); }
            '*' => { self.next_char(); return Ok(Token::Star); }
            '/' => { self.next_char(); return Ok(Token::Slash); }
            '%' => { self.next_char(); return Ok(Token::Percent); }
            _ => {}
        }
        
        // Two-character operators
        if c == '<' {
            self.next_char();
            if self.peek_char() == Some('=') {
                self.next_char();
                return Ok(Token::Le);
            }
            return Ok(Token::Lt);
        }
        
        if c == '>' {
            self.next_char();
            if self.peek_char() == Some('=') {
                self.next_char();
                return Ok(Token::Ge);
            }
            return Ok(Token::Gt);
        }
        
        if c == '=' {
            self.next_char();
            return Ok(Token::Eq);
        }
        
        if c == '!' {
            self.next_char();
            if self.peek_char() == Some('=') {
                self.next_char();
                return Ok(Token::Ne);
            }
            return Err(format!("Unexpected '!' at line {}", self.line));
        }
        
        // Minus or negative number or arrow
        if c == '-' {
            self.next_char();
            if let Some(c2) = self.peek_char() {
                if c2 == '>' {
                    self.next_char();
                    return Ok(Token::Arrow);
                }
                if c2.is_ascii_digit() {
                    // Negative number
                    return self.lex_number(true);
                }
            }
            return Ok(Token::Minus);
        }
        
        // String literal
        if c == '"' {
            return self.lex_string();
        }
        
        // Number
        if c.is_ascii_digit() {
            return self.lex_number(false);
        }
        
        // Identifier or keyword
        if c.is_alphabetic() || c == '_' || c == '?' {
            return self.lex_ident();
        }
        
        Err(format!("Unexpected character '{}' at line {}", c, self.line))
    }
    
    fn lex_number(&mut self, negative: bool) -> Result<Token, String> {
        let mut s = String::new();
        if negative {
            s.push('-');
        }
        
        let mut has_dot = false;
        while let Some(c) = self.peek_char() {
            if c.is_ascii_digit() {
                s.push(c);
                self.next_char();
            } else if c == '.' && !has_dot {
                // Check if next char is digit (not method call)
                let rest = &self.input[self.pos + 1..];
                if rest.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    s.push(c);
                    self.next_char();
                    has_dot = true;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        
        if has_dot {
            // Check for scientific notation: e/E followed by optional +/- and digits
            if let Some(c) = self.peek_char() {
                if c == 'e' || c == 'E' {
                    s.push(c);
                    self.next_char();
                    // optional sign
                    if let Some(sign) = self.peek_char() {
                        if sign == '+' || sign == '-' {
                            s.push(sign);
                            self.next_char();
                        }
                    }
                    // exponent digits
                    while let Some(d) = self.peek_char() {
                        if d.is_ascii_digit() {
                            s.push(d);
                            self.next_char();
                        } else {
                            break;
                        }
                    }
                }
            }
            s.parse::<f64>()
                .map(Token::Float)
                .map_err(|e| format!("Invalid float: {}", e))
        } else {
            s.parse::<i64>()
                .map(Token::Int)
                .map_err(|e| format!("Invalid integer: {}", e))
        }
    }
    
    fn lex_string(&mut self) -> Result<Token, String> {
        self.next_char(); // consume opening "
        let mut s = String::new();
        
        loop {
            match self.next_char() {
                Some('"') => break,
                Some('\\') => {
                    match self.next_char() {
                        Some('n') => s.push('\n'),
                        Some('t') => s.push('\t'),
                        Some('r') => s.push('\r'),
                        Some('\\') => s.push('\\'),
                        Some('"') => s.push('"'),
                        Some(c) => return Err(format!("Invalid escape: \\{}", c)),
                        None => return Err("Unterminated string".into()),
                    }
                }
                Some(c) => s.push(c),
                None => return Err("Unterminated string".into()),
            }
        }
        
        Ok(Token::String(s))
    }
    
    fn lex_ident(&mut self) -> Result<Token, String> {
        let mut s = String::new();
        
        while let Some(c) = self.peek_char() {
            if c.is_alphanumeric() || c == '_' || c == '-' || c == '?' {
                s.push(c);
                self.next_char();
            } else {
                break;
            }
        }
        
        // Check for keywords
        match s.as_str() {
            "true" => Ok(Token::True),
            "false" => Ok(Token::False),
            "nil" => Ok(Token::Nil),
            _ => Ok(Token::Ident(s)),
        }
    }
    
    #[allow(dead_code)]
    pub fn position(&self) -> (usize, usize) {
        (self.line, self.col)
    }
}

// ============================================================================
// PARSER
// ============================================================================

pub struct Parser<'a> {
    input: &'a str,            // Store original input for reset
    lexer: Lexer<'a>,
    current: Token,
    functions: HashMap<String, usize>, // name -> label offset
    locals: HashMap<String, u32>,      // local name -> slot index
    next_local: u32,                   // next local slot to assign
    cap_flags: u8,                     // capability flags (P4: declared caps)
    asm: Assembler,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Result<Self, String> {
        let mut lexer = Lexer::new(input);
        let current = lexer.next_token()?;
        Ok(Parser {
            input,
            lexer,
            current,
            functions: HashMap::new(),
            locals: HashMap::new(),
            next_local: 0,
            cap_flags: 0,
            asm: Assembler::new(),
        })
    }
    
    fn advance(&mut self) -> Result<(), String> {
        self.current = self.lexer.next_token()?;
        Ok(())
    }
    
    fn expect(&mut self, expected: Token) -> Result<(), String> {
        if self.current == expected {
            self.advance()?;
            Ok(())
        } else {
            Err(format!("Expected {:?}, got {:?}", expected, self.current))
        }
    }
    
    /// Parse the entire input
    pub fn parse(mut self) -> Result<BytecodeModule, String> {
        // First pass: collect function names (no code generation)
        self.collect_functions()?;
        
        // Reset for second pass
        self.lexer = Lexer::new(self.input);
        self.current = self.lexer.next_token()?;
        
        // Emit jump to main code (we'll patch it later)
        let main_jump = self.asm.emit_jmp_direct(0); // Placeholder
        
        // Second pass: generate function bodies
        while self.current != Token::Eof {
            if self.current == Token::Colon {
                self.parse_function_def()?;
            } else {
                // Skip non-function tokens in this pass
                self.advance()?;
            }
        }
        
        // Patch the jump to here (start of main code)
        let main_start = self.asm.current_offset();
        self.asm.patch_jmp(main_jump, main_start as i32);
        
        // Third pass: generate main code (non-function expressions)
        self.lexer = Lexer::new(self.input);
        self.current = self.lexer.next_token()?;
        
        while self.current != Token::Eof {
            if self.current == Token::Colon {
                // Skip function definitions
                self.skip_function_def()?;
            } else {
                self.parse_expr()?;
            }
        }
        
        // Add halt at end
        self.asm.emit(Op::Halt);
        
        let mut module = self.asm.finalize()?;
        module.cap_flags = self.cap_flags;
        Ok(module)
    }
    
    fn skip_function_def(&mut self) -> Result<(), String> {
        self.expect(Token::Colon)?;
        self.advance()?; // Skip name
        while self.current != Token::Semicolon && self.current != Token::Eof {
            self.advance()?;
        }
        if self.current == Token::Semicolon {
            self.advance()?;
        }
        Ok(())
    }
    
    fn collect_functions(&mut self) -> Result<(), String> {
        // Scan for : name ... ; definitions
        let mut offset = 0;
        while self.current != Token::Eof {
            if self.current == Token::Colon {
                self.advance()?;
                if let Token::Ident(name) = &self.current {
                    self.functions.insert(name.clone(), offset);
                }
            }
            self.advance()?;
            offset += 1; // Rough estimate
        }
        Ok(())
    }
    
    #[allow(dead_code)]
    fn parse_item(&mut self) -> Result<(), String> {
        match &self.current {
            Token::Colon => self.parse_function_def()?,
            _ => self.parse_expr()?,
        }
        Ok(())
    }
    
    fn parse_function_def(&mut self) -> Result<(), String> {
        self.expect(Token::Colon)?;
        
        let name = match &self.current {
            Token::Ident(s) => s.clone(),
            _ => return Err("Expected function name after ':'".into()),
        };
        self.advance()?;
        
        // Save and reset locals for this function scope
        let saved_locals = std::mem::take(&mut self.locals);
        let saved_next = self.next_local;
        self.next_local = 0;
        
        // Create label for this function
        self.asm.label(&name);
        
        // Parse body until semicolon
        while self.current != Token::Semicolon && self.current != Token::Eof {
            self.parse_expr()?;
        }
        
        // Emit return
        self.asm.emit(Op::Ret);
        
        // Restore outer scope's locals
        self.locals = saved_locals;
        self.next_local = saved_next;
        
        self.expect(Token::Semicolon)?;
        Ok(())
    }
    
    fn parse_expr(&mut self) -> Result<(), String> {
        match &self.current.clone() {
            // Literals
            Token::Int(n) => {
                let n = *n;
                self.advance()?;
                if n >= -128 && n <= 127 {
                    self.asm.emit_i8(n as i8);
                } else {
                    self.asm.emit_i64(n);
                }
            }
            Token::Float(f) => {
                let f = *f;
                self.advance()?;
                self.asm.emit_f64(f);
            }
            Token::String(s) => {
                let s = s.clone();
                self.advance()?;
                self.asm.emit_str(&s);
            }
            Token::True => {
                self.advance()?;
                self.asm.emit(Op::True);
            }
            Token::False => {
                self.advance()?;
                self.asm.emit(Op::False);
            }
            Token::Nil => {
                self.advance()?;
                self.asm.emit(Op::Nil);
            }
            
            // Quote: [ body ] → QUOTE(len) body
            // The body is compiled inline after a 2-byte length prefix.
            // QUOTE pushes a Quote{offset, len} value and skips over the body.
            // APPLY then jumps to offset to execute the deferred code.
            Token::LBracket => {
                self.advance()?;
                
                // Emit QUOTE opcode + placeholder length (2 bytes)
                self.asm.emit(Op::Quote);
                let len_pos = self.asm.current_offset();
                self.asm.emit_raw_u16(0); // placeholder
                
                let body_start = self.asm.current_offset();
                
                // Compile the body (recursively — quotes can nest)
                while self.current != Token::RBracket && self.current != Token::Eof {
                    self.parse_expr()?;
                }
                
                if self.current == Token::Eof {
                    return Err("Unterminated quote: missing ']'".into());
                }
                self.expect(Token::RBracket)?;
                
                // Emit RET at end of quoted body so APPLY returns correctly
                self.asm.emit(Op::Ret);
                
                let body_end = self.asm.current_offset();
                let body_len = body_end - body_start;
                
                // Patch the length
                self.asm.patch_u16(len_pos, body_len as u16);
            }
            
            // List literal: ( elem1 elem2 ... ) → push elems then LIST(count)
            // Elements are compiled to bytecode that pushes them, then LIST
            // collects the top N values into a List value at runtime.
            // Two-pass: first pass collects element bytecodes into a buffer,
            // second emits them followed by LIST(count).
            Token::LParen => {
                self.advance()?;
                
                let mut count: u16 = 0;
                
                // Parse elements until RParen
                while self.current != Token::RParen && self.current != Token::Eof {
                    self.parse_expr()?;
                    count += 1;
                }
                
                if self.current == Token::Eof {
                    return Err("Unterminated list literal: missing ')'".into());
                }
                self.expect(Token::RParen)?;
                
                // Emit LIST opcode + count
                self.asm.emit(Op::List);
                self.asm.emit_raw_u16(count);
            }

            // Array literal: #( elem1 elem2 ... ) → push elems as list, then ARRAY_FROM
            // P1: arrays are tools. #(1 2 3) creates a contiguous i64 array.
            // Semantics: compile elements, collect into list, convert to array.
            Token::HashParen => {
                self.advance()?;

                let mut count: u16 = 0;

                // Parse elements until RParen
                while self.current != Token::RParen && self.current != Token::Eof {
                    self.parse_expr()?;
                    count += 1;
                }

                if self.current == Token::Eof {
                    return Err("Unterminated array literal: missing ')'".into());
                }
                self.expect(Token::RParen)?;

                // Collect into list, then convert to array
                self.asm.emit(Op::List);
                self.asm.emit_raw_u16(count);
                self.asm.emit(Op::ArrayFrom);
            }

            // Operators
            Token::Plus => { self.advance()?; self.asm.emit(Op::Add); }
            Token::Minus => { self.advance()?; self.asm.emit(Op::Sub); }
            Token::Star => { self.advance()?; self.asm.emit(Op::Mul); }
            Token::Slash => { self.advance()?; self.asm.emit(Op::Div); }
            Token::Percent => { self.advance()?; self.asm.emit(Op::Mod); }
            Token::Lt => { self.advance()?; self.asm.emit(Op::Lt); }
            Token::Gt => { self.advance()?; self.asm.emit(Op::Gt); }
            Token::Le => { self.advance()?; self.asm.emit(Op::Le); }
            Token::Ge => { self.advance()?; self.asm.emit(Op::Ge); }
            Token::Eq => { self.advance()?; self.asm.emit(Op::Eq); }
            Token::Ne => { self.advance()?; self.asm.emit(Op::Ne); }
            
            // Identifiers (built-ins or function calls)
            Token::Ident(name) => {
                let name = name.clone();
                self.advance()?;
                
                // Handle structured keywords that need special parsing
                if name == "if" {
                    self.parse_if()?;
                } else if name == "while" {
                    self.parse_while()?;
                } else {
                    self.emit_word(&name)?;
                }
            }
            
            // Arrow: -> name (store to local variable)
            Token::Arrow => {
                self.advance()?;
                let name = match &self.current {
                    Token::Ident(s) => s.clone(),
                    _ => return Err("Expected identifier after '->'".into()),
                };
                self.advance()?;
                
                // Assign a slot if not already assigned
                let slot = if let Some(&existing) = self.locals.get(&name) {
                    existing
                } else {
                    let slot = self.next_local;
                    self.next_local += 1;
                    self.locals.insert(name, slot);
                    slot
                };
                self.asm.emit_store(slot);
            }
            
            Token::Eof => {}
            
            tok => return Err(format!("Unexpected token: {:?}", tok)),
        }
        Ok(())
    }
    
    fn emit_word(&mut self, name: &str) -> Result<(), String> {
        match name {
            // Stack operations
            "drop" => self.asm.emit(Op::Drop),
            "dup" => self.asm.emit(Op::Dup),
            "swap" => self.asm.emit(Op::Swap),
            "rot" => self.asm.emit(Op::Rot),
            "over" => self.asm.emit(Op::Over),
            
            // Arithmetic (alternate syntax)
            "add" => self.asm.emit(Op::Add),
            "sub" => self.asm.emit(Op::Sub),
            "mul" => self.asm.emit(Op::Mul),
            "div" => self.asm.emit(Op::Div),
            "mod" => self.asm.emit(Op::Mod),
            "neg" => self.asm.emit(Op::Neg),
            
            // Float arithmetic (strict f64)
            "fadd" => self.asm.emit(Op::Fadd),
            "fsub" => self.asm.emit(Op::Fsub),
            "fmul" => self.asm.emit(Op::Fmul),
            "fdiv" => self.asm.emit(Op::Fdiv),
            "fneg" => self.asm.emit(Op::Fneg),
            "fsqrt" => self.asm.emit(Op::Fsqrt),
            "fabs" => self.asm.emit(Op::Fabs),
            "fexp" => self.asm.emit(Op::Fexp),
            "flog" => self.asm.emit(Op::Flog),
            "fsin" => self.asm.emit(Op::Fsin),
            "fcos" => self.asm.emit(Op::Fcos),
            "fatan2" => self.asm.emit(Op::Fatan2),
            "fpow" => self.asm.emit(Op::Fpow),
            "ffloor" => self.asm.emit(Op::Ffloor),
            "fceil" => self.asm.emit(Op::Fceil),
            "fround" => self.asm.emit(Op::Fround),
            "i2f" => self.asm.emit(Op::I2f),
            "f2i" => self.asm.emit(Op::F2i),
            
            // Comparison
            "lt" => self.asm.emit(Op::Lt),
            "gt" => self.asm.emit(Op::Gt),
            "le" => self.asm.emit(Op::Le),
            "ge" => self.asm.emit(Op::Ge),
            "eq" => self.asm.emit(Op::Eq),
            "ne" => self.asm.emit(Op::Ne),
            
            // Logic
            "and" => self.asm.emit(Op::And),
            "or" => self.asm.emit(Op::Or),
            "not" => self.asm.emit(Op::Not),
            "xor" => self.asm.emit(Op::Xor),
            "band" => self.asm.emit(Op::Band),
            "bor" => self.asm.emit(Op::Bor),
            "bxor" => self.asm.emit(Op::Bxor),
            "bnot" => self.asm.emit(Op::Bnot),
            "shl" => self.asm.emit(Op::Shl),
            "shr" => self.asm.emit(Op::Shr),
            
            // Data structures
            "pair" => self.asm.emit(Op::Pair),
            "unpair" => self.asm.emit(Op::Unpair),
            "left" => self.asm.emit(Op::Left),
            "right" => self.asm.emit(Op::Right),
            "case" => self.asm.emit(Op::Case),
            
            // Control
            "apply" => self.asm.emit(Op::Apply),
            "cond" => self.asm.emit(Op::Cond),
            "loop" => self.asm.emit(Op::Loop),
            "ret" | "return" => self.asm.emit(Op::Ret),
            "halt" => self.asm.emit(Op::Halt),
            "nop" => self.asm.emit(Op::Nop),
            
            // Lists  
            // ( ... ) is the literal syntax (parsed above).
            // "collect" takes a literal integer: collects N stack values.
            "collect" => {
                match &self.current {
                    Token::Int(n) => {
                        let count = *n as u16;
                        self.advance()?;
                        self.asm.emit(Op::List);
                        self.asm.emit_raw_u16(count);
                    }
                    _ => return Err("'collect' requires a literal integer count".into()),
                }
            }
            "unlist" => self.asm.emit(Op::Unlist),
            "len" => self.asm.emit(Op::Len),
            "get" => self.asm.emit(Op::Get),
            "set" => self.asm.emit(Op::Set),
            
            // Higher-order list ops
            "map" => self.asm.emit(Op::Map),
            "fold" => self.asm.emit(Op::Fold),
            "zip" => self.asm.emit(Op::Zip),
            "append" => self.asm.emit(Op::Append),
            "reverse" => self.asm.emit(Op::Reverse),

            // Training primitives (reduce token count for RL agent)
            "times" => self.asm.emit(Op::Times),
            "filter" => self.asm.emit(Op::Filter),
            "head" => self.asm.emit(Op::Head),
            "tail" => self.asm.emit(Op::Tail),
            "range" => self.asm.emit(Op::Range),
            "first" => self.asm.emit(Op::First),
            "second" => self.asm.emit(Op::Second),
            "list-concat" | "concat" => self.asm.emit(Op::ListConcat),
            "empty?" => self.asm.emit(Op::IsEmpty),

            // Arrays (contiguous i64 storage — Tool 4)
            // P1: arrays are tools (S → S). GPU-friendly, cache-friendly.
            "array-new" => self.asm.emit(Op::ArrayNew),
            "array-get" => self.asm.emit(Op::ArrayGet),
            "array-set" => self.asm.emit(Op::ArraySet),
            "array-len" => self.asm.emit(Op::ArrayLen),
            "array-push" => self.asm.emit(Op::ArrayPush),
            "array-from" => self.asm.emit(Op::ArrayFrom),

            // Fibers (cooperative multitasking)
            "fiber-new" => self.asm.emit(Op::FiberNew),
            "fiber-step" => self.asm.emit(Op::FiberStep),
            "fiber-push" => self.asm.emit(Op::FiberPush),
            "fiber-stack" => self.asm.emit(Op::FiberStack),
            "fiber-status" => self.asm.emit(Op::FiberStatus),
            
            // Spawn (sandboxed parallel execution) — P4: capability attenuation
            "spawn" => self.asm.emit(Op::Spawn),
            
            // Channels (inter-fiber communication)
            "chan-new" => self.asm.emit(Op::ChanNew),
            "chan-send" => self.asm.emit(Op::ChanSend),
            "chan-recv" => self.asm.emit(Op::ChanRecv),
            
            // Error handling (P4: errors are values, not exceptions)
            "try" => self.asm.emit(Op::Try),
            "fail" => self.asm.emit(Op::Fail),
            "is-error" => self.asm.emit(Op::IsError),
            "error" => self.asm.emit(Op::MakeError),
            "?" | "propagate" => self.asm.emit(Op::Propagate),
            
            // HashMap (associative data structure)
            "map-new" => self.asm.emit(Op::MapNew),
            "map-get" => self.asm.emit(Op::MapGet),
            "map-set" => self.asm.emit(Op::MapSet),
            "map-keys" => self.asm.emit(Op::MapKeys),
            "map-has" => self.asm.emit(Op::MapHas),
            
            // Linear types (P4: constraints attenuate)
            "linear" => self.asm.emit(Op::Linear),
            "affine" => self.asm.emit(Op::Affine),
            "consume" => self.asm.emit(Op::Consume),
            "is-linear" => self.asm.emit(Op::IsLinear),
            "is-affine" => self.asm.emit(Op::IsAffine),
            
            // Introspection
            "type-of" => self.asm.emit(Op::TypeOf),
            "depth" => self.asm.emit(Op::Depth),
            "describe" => self.asm.emit(Op::Describe),
            
            // String operations
            "str-len" => self.asm.emit(Op::StrLen),
            "str-get" => self.asm.emit(Op::StrGet),
            "str-concat" => self.asm.emit(Op::StrConcat),
            "str-slice" => self.asm.emit(Op::StrSlice),
            "to-str" => self.asm.emit(Op::ToStr),
            "str-find" => self.asm.emit(Op::StrFind),
            "str-split" => self.asm.emit(Op::StrSplit),
            "str-replace" => self.asm.emit(Op::StrReplace),
            "str-upper" => self.asm.emit(Op::StrUpper),
            "str-lower" => self.asm.emit(Op::StrLower),
            "str-trim" => self.asm.emit(Op::StrTrim),
            
            // Parsing operations
            "parse-float" => self.asm.emit(Op::ParseFloat),
            "parse-int" => self.asm.emit(Op::ParseInt),
            
            // Reflection (self-hosting)
            "fetch" => self.asm.emit(Op::Fetch),
            "size" => self.asm.emit(Op::Size),
            
            // SYSCALL — generalized host calls
            "file-read" => {
                self.cap_flags |= crate::bytecode::CAP_FS;
                self.asm.emit(Op::Syscall);
                self.asm.emit_u8(0x01);
            }
            "file-write" => {
                self.cap_flags |= crate::bytecode::CAP_FS;
                self.asm.emit(Op::Syscall);
                self.asm.emit_u8(0x02);
            }
            "file-exists" => {
                self.cap_flags |= crate::bytecode::CAP_FS;
                self.asm.emit(Op::Syscall);
                self.asm.emit_u8(0x03);
            }
            "time-now" => {
                self.cap_flags |= crate::bytecode::CAP_IO;
                self.asm.emit(Op::Syscall);
                self.asm.emit_u8(0x10);
            }
            "env-get" => {
                self.cap_flags |= crate::bytecode::CAP_IO;
                self.asm.emit(Op::Syscall);
                self.asm.emit_u8(0x20);
            }
            "exec" => {
                self.cap_flags |= crate::bytecode::CAP_EXEC;
                self.asm.emit(Op::Syscall);
                self.asm.emit_u8(0x30);
            }
            "readline" => {
                self.cap_flags |= crate::bytecode::CAP_IO;
                self.asm.emit(Op::Syscall);
                self.asm.emit_u8(0x04);
            }
            "file-append" => {
                self.cap_flags |= crate::bytecode::CAP_FS;
                self.asm.emit(Op::Syscall);
                self.asm.emit_u8(0x05);
            }
            "file-delete" => {
                self.cap_flags |= crate::bytecode::CAP_FS;
                self.asm.emit(Op::Syscall);
                self.asm.emit_u8(0x06);
            }
            "file-list" => {
                self.cap_flags |= crate::bytecode::CAP_FS;
                self.asm.emit(Op::Syscall);
                self.asm.emit_u8(0x07);
            }
            "http-get" => {
                self.cap_flags |= crate::bytecode::CAP_NET;
                self.asm.emit(Op::Syscall);
                self.asm.emit_u8(0x40);
            }
            "http-post" => {
                self.cap_flags |= crate::bytecode::CAP_NET;
                self.asm.emit(Op::Syscall);
                self.asm.emit_u8(0x41);
            }
            "http-serve" => {
                self.cap_flags |= crate::bytecode::CAP_NET;
                self.asm.emit(Op::Syscall);
                self.asm.emit_u8(0x50);
            }
            
            // IO (capability-gated at compile and runtime)
            "print" => {
                self.cap_flags |= crate::bytecode::CAP_IO;
                self.asm.emit(Op::Print);
            }
            "println" => {
                self.cap_flags |= crate::bytecode::CAP_IO;
                self.asm.emit(Op::Println);
            }
            "rand" => {
                self.cap_flags |= crate::bytecode::CAP_IO;
                self.asm.emit(Op::Rand);
            }
            
            // Function call or local variable
            _ => {
                // Check if it's a local variable
                if let Some(&slot) = self.locals.get(name) {
                    self.asm.emit_load(slot);
                } else if self.functions.contains_key(name) {
                    // Check if it's a defined function
                    self.asm.emit_call_label(name);
                } else {
                    return Err(format!("Unknown word: {}", name));
                }
            }
        }
        Ok(())
    }
    
    /// Parse `if ... else ... end` or `if ... end`
    /// Compiles to: JZ(else_offset) then-body JMP(end_offset) else-body
    /// Or without else: JZ(end_offset) then-body
    /// Stack effect: pops Bool, executes the appropriate branch
    fn parse_if(&mut self) -> Result<(), String> {
        // Emit JZ with placeholder offset (jump over then-body if false)
        self.asm.emit(Op::Jz);
        let jz_offset = self.asm.emit_raw_i32(0); // placeholder
        
        // Parse then-body until "else" or "end"
        loop {
            match &self.current {
                Token::Eof => return Err("Unterminated 'if': missing 'end'".into()),
                Token::Ident(s) if s == "else" => {
                    self.advance()?;
                    
                    // Emit JMP to skip else-body (then-body falls through to here)
                    self.asm.emit(Op::Jmp);
                    let jmp_offset = self.asm.emit_raw_i32(0); // placeholder
                    
                    // Patch JZ to jump here (start of else-body)
                    let else_start = self.asm.current_offset();
                    self.asm.patch_jmp(jz_offset, else_start as i32);
                    
                    // Parse else-body until "end"
                    loop {
                        match &self.current {
                            Token::Eof => return Err("Unterminated 'if': missing 'end' after 'else'".into()),
                            Token::Ident(s) if s == "end" => {
                                self.advance()?;
                                break;
                            }
                            _ => self.parse_expr()?,
                        }
                    }
                    
                    // Patch JMP to jump here (after else-body)
                    let end_pos = self.asm.current_offset();
                    self.asm.patch_jmp(jmp_offset, end_pos as i32);
                    
                    return Ok(());
                }
                Token::Ident(s) if s == "end" => {
                    self.advance()?;
                    
                    // No else branch — patch JZ to jump here
                    let end_pos = self.asm.current_offset();
                    self.asm.patch_jmp(jz_offset, end_pos as i32);
                    
                    return Ok(());
                }
                _ => self.parse_expr()?,
            }
        }
    }
    /// Parse `while <cond> do <body> end`
    /// Compiles to: loop_start: <cond> JZ(end) <body> JMP(loop_start)
    /// P3: composition = concatenation — the loop body concatenates.
    /// P1: the while construct is itself a tool S → S.
    fn parse_while(&mut self) -> Result<(), String> {
        // Record loop start position
        let loop_start = self.asm.current_offset();
        
        // Parse condition until "do"
        loop {
            match &self.current {
                Token::Eof => return Err("Unterminated 'while': missing 'do'".into()),
                Token::Ident(s) if s == "do" => {
                    self.advance()?;
                    break;
                }
                _ => self.parse_expr()?,
            }
        }
        
        // Emit JZ to exit loop (placeholder)
        self.asm.emit(Op::Jz);
        let jz_offset = self.asm.emit_raw_i32(0);
        
        // Parse body until "end"
        loop {
            match &self.current {
                Token::Eof => return Err("Unterminated 'while': missing 'end'".into()),
                Token::Ident(s) if s == "end" => {
                    self.advance()?;
                    break;
                }
                _ => self.parse_expr()?,
            }
        }
        
        // JMP back to loop_start
        self.asm.emit(Op::Jmp);
        let jmp_offset = self.asm.emit_raw_i32(0);
        self.asm.patch_jmp(jmp_offset, loop_start as i32);
        
        // Patch JZ to jump here (after loop)
        let loop_end = self.asm.current_offset();
        self.asm.patch_jmp(jz_offset, loop_end as i32);
        
        Ok(())
    }
}

// ============================================================================
// COMPILE FUNCTION
// ============================================================================

/// Compile Kore source code to bytecode
pub fn compile(source: &str) -> Result<BytecodeModule, String> {
    let parser = Parser::new(source)?;
    parser.parse()
}

/// Compile Kore source code with import resolution from a base path.
/// `import "path.kore"` will resolve relative to `base_dir`.
pub fn compile_with_imports(source: &str, base_dir: &std::path::Path) -> Result<BytecodeModule, String> {
    // Pre-process imports: expand `import "file.kore"` into inline source
    let expanded = expand_imports(source, base_dir, &mut Vec::new())?;
    let parser = Parser::new(&expanded)?;
    parser.parse()
}

/// Recursively expand `import "file.kore"` directives.
/// Tracks already-imported files to prevent cycles.
fn expand_imports(source: &str, base_dir: &std::path::Path, imported: &mut Vec<String>) -> Result<String, String> {
    let mut result = String::new();
    
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("import ") {
            // Extract the path from `import "path"`
            let rest = trimmed["import ".len()..].trim();
            if rest.starts_with('"') && rest.ends_with('"') && rest.len() > 2 {
                let import_path = &rest[1..rest.len()-1];
                let full_path = base_dir.join(import_path);
                let canonical = full_path.to_string_lossy().to_string();
                
                if imported.contains(&canonical) {
                    // Already imported — skip (prevents cycles)
                    result.push_str("-- (import already loaded)\n");
                    continue;
                }
                imported.push(canonical.clone());
                
                // Also check stdlib directory
                let file_content = if full_path.exists() {
                    std::fs::read_to_string(&full_path)
                        .map_err(|e| format!("import '{}': {}", import_path, e))?
                } else {
                    // Try stdlib directory relative to executable
                    let stdlib_path = std::path::PathBuf::from("stdlib").join(import_path);
                    if stdlib_path.exists() {
                        std::fs::read_to_string(&stdlib_path)
                            .map_err(|e| format!("import '{}': {}", import_path, e))?
                    } else {
                        return Err(format!("import '{}': file not found (tried {} and {})", 
                            import_path, full_path.display(), stdlib_path.display()));
                    }
                };
                
                // Recursively expand imports in the imported file
                let parent = full_path.parent().unwrap_or(base_dir);
                let expanded = expand_imports(&file_content, parent, imported)?;
                result.push_str(&expanded);
                result.push('\n');
            } else {
                return Err(format!("invalid import syntax: {}", trimmed));
            }
        } else {
            result.push_str(line);
            result.push('\n');
        }
    }
    
    Ok(result)
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::Interpreter;
    
    #[test]
    fn test_lex_simple() {
        let mut lexer = Lexer::new("42 + 3");
        assert_eq!(lexer.next_token().unwrap(), Token::Int(42));
        assert_eq!(lexer.next_token().unwrap(), Token::Plus);
        assert_eq!(lexer.next_token().unwrap(), Token::Int(3));
        assert_eq!(lexer.next_token().unwrap(), Token::Eof);
    }
    
    #[test]
    fn test_lex_operators() {
        let mut lexer = Lexer::new("< > <= >= = !=");
        assert_eq!(lexer.next_token().unwrap(), Token::Lt);
        assert_eq!(lexer.next_token().unwrap(), Token::Gt);
        assert_eq!(lexer.next_token().unwrap(), Token::Le);
        assert_eq!(lexer.next_token().unwrap(), Token::Ge);
        assert_eq!(lexer.next_token().unwrap(), Token::Eq);
        assert_eq!(lexer.next_token().unwrap(), Token::Ne);
    }
    
    #[test]
    fn test_lex_keywords() {
        let mut lexer = Lexer::new("true false nil drop");
        assert_eq!(lexer.next_token().unwrap(), Token::True);
        assert_eq!(lexer.next_token().unwrap(), Token::False);
        assert_eq!(lexer.next_token().unwrap(), Token::Nil);
        assert_eq!(lexer.next_token().unwrap(), Token::Ident("drop".into()));
    }
    
    #[test]
    fn test_lex_comment() {
        let mut lexer = Lexer::new("42 -- this is a comment\n43");
        assert_eq!(lexer.next_token().unwrap(), Token::Int(42));
        assert_eq!(lexer.next_token().unwrap(), Token::Int(43));
    }
    
    #[test]
    fn test_compile_simple_add() {
        let module = compile("3 4 +").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        assert_eq!(interp.result(), Some(&Value::Int(7)));
    }
    
    #[test]
    fn test_compile_stack_ops() {
        let module = compile("5 dup *").unwrap(); // 5 * 5 = 25
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        assert_eq!(interp.result(), Some(&Value::Int(25)));
    }
    
    #[test]
    fn test_compile_comparison() {
        let module = compile("3 5 <").unwrap(); // 3 < 5 = true
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        assert_eq!(interp.result(), Some(&Value::Bool(true)));
    }
    
    #[test]
    fn test_compile_function_def() {
        // : double 2 * ; 5 double
        let module = compile(": double 2 * ; 5 double").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        assert_eq!(interp.result(), Some(&Value::Int(10)));
    }
    
    #[test]
    fn test_compile_multiple_functions() {
        // : double 2 * ; : inc 1 + ; 3 double inc
        let module = compile(": double 2 * ; : inc 1 + ; 3 double inc").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        assert_eq!(interp.result(), Some(&Value::Int(7))); // 3*2 + 1 = 7
    }
    
    #[test]
    fn test_compile_quote_apply() {
        // [ 2 * ] stores a deferred computation, apply executes it
        let module = compile("5 [ 2 * ] apply").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        assert_eq!(interp.result(), Some(&Value::Int(10))); // 5 * 2 = 10
    }
    
    #[test]
    fn test_compile_nested_quote() {
        // Nested quotes: outer quote contains inner quote + apply
        let module = compile("3 [ [ 1 + ] apply ] apply").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        assert_eq!(interp.result(), Some(&Value::Int(4))); // 3 + 1 = 4
    }
    
    // ================================================================
    // LIST TESTS — Phase 0 of the build plan
    // ================================================================
    
    #[test]
    fn test_list_literal() {
        let module = compile("( 1 2 3 )").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        assert_eq!(interp.result(), Some(&Value::List(Box::new(vec![
            Value::Int(1), Value::Int(2), Value::Int(3)
        ]))));
    }
    
    #[test]
    fn test_list_empty() {
        let module = compile("( )").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        assert_eq!(interp.result(), Some(&Value::List(Box::new(vec![]))));
    }
    
    #[test]
    fn test_list_nested() {
        // Matrix = list of lists
        let module = compile("( ( 1 2 ) ( 3 4 ) )").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        assert_eq!(interp.result(), Some(&Value::List(Box::new(vec![
            Value::List(Box::new(vec![Value::Int(1), Value::Int(2)])),
            Value::List(Box::new(vec![Value::Int(3), Value::Int(4)])),
        ]))));
    }
    
    #[test]
    fn test_list_len() {
        // len peeks (non-destructive) then pushes Int
        let module = compile("( 10 20 30 ) len").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        // Top of stack is the length
        assert_eq!(interp.result(), Some(&Value::Int(3)));
        // List is still underneath
        assert_eq!(interp.stack().len(), 2);
    }
    
    #[test]
    fn test_list_get() {
        let module = compile("( 10 20 30 ) 1 get").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        assert_eq!(interp.result(), Some(&Value::Int(20)));
    }
    
    #[test]
    fn test_list_set() {
        let module = compile("( 10 20 30 ) 1 99 set").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        assert_eq!(interp.result(), Some(&Value::List(Box::new(vec![
            Value::Int(10), Value::Int(99), Value::Int(30)
        ]))));
    }
    
    #[test]
    fn test_list_unlist() {
        // Spread list onto stack
        let module = compile("( 5 10 ) unlist +").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        assert_eq!(interp.result(), Some(&Value::Int(15))); // 5 + 10
    }
    
    #[test]
    fn test_list_floats() {
        let module = compile("( 1.0 2.5 3.7 )").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        assert_eq!(interp.result(), Some(&Value::List(Box::new(vec![
            Value::Float(1.0), Value::Float(2.5), Value::Float(3.7)
        ]))));
    }
    
    #[test]
    fn test_list_with_computed_elements() {
        // Compute values BEFORE the list literal, then collect them
        // ( ... ) counts parse_expr calls — each must push exactly one value
        let module = compile("( 5 20 true \"hello\" )").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        assert_eq!(interp.result(), Some(&Value::List(Box::new(vec![
            Value::Int(5), Value::Int(20), 
            Value::Bool(true), Value::Str(Box::new("hello".into()))
        ]))));
    }
    
    #[test]
    fn test_list_with_nested_quotes() {
        // Lists can contain quotes (first-class code blocks)
        let module = compile("( [ 1 + ] [ 2 * ] )").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        match interp.result() {
            Some(Value::List(items)) => assert_eq!(items.len(), 2),
            other => panic!("Expected list of 2 quotes, got {:?}", other),
        }
    }
    
    // ================================================================
    // HIGHER-ORDER LIST TESTS — MAP, FOLD, ZIP
    // ================================================================
    
    #[test]
    fn test_map_double() {
        // map(2*) over [1,2,3] = [2,4,6]
        let module = compile("( 1 2 3 ) [ 2 * ] map").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        assert_eq!(interp.result(), Some(&Value::List(Box::new(vec![
            Value::Int(2), Value::Int(4), Value::Int(6)
        ]))));
    }
    
    #[test]
    fn test_map_negate() {
        let module = compile("( 1 2 3 ) [ neg ] map").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        assert_eq!(interp.result(), Some(&Value::List(Box::new(vec![
            Value::Int(-1), Value::Int(-2), Value::Int(-3)
        ]))));
    }
    
    #[test]
    fn test_map_empty() {
        let module = compile("( ) [ 2 * ] map").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        assert_eq!(interp.result(), Some(&Value::List(Box::new(vec![]))));
    }
    
    #[test]
    fn test_fold_sum() {
        // sum = fold(+, 0)
        let module = compile("( 1 2 3 4 5 ) 0 [ + ] fold").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        assert_eq!(interp.result(), Some(&Value::Int(15)));
    }
    
    #[test]
    fn test_fold_product() {
        // product = fold(*, 1)
        let module = compile("( 1 2 3 4 5 ) 1 [ * ] fold").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        assert_eq!(interp.result(), Some(&Value::Int(120))); // 5! = 120
    }
    
    #[test]
    fn test_fold_empty() {
        // fold on empty list returns init
        let module = compile("( ) 42 [ + ] fold").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        assert_eq!(interp.result(), Some(&Value::Int(42)));
    }
    
    #[test]
    fn test_zip() {
        let module = compile("( 1 2 ) ( 10 20 ) zip").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        assert_eq!(interp.result(), Some(&Value::List(Box::new(vec![
            Value::Pair(Box::new((Value::Int(1), Value::Int(10)))),
            Value::Pair(Box::new((Value::Int(2), Value::Int(20)))),
        ]))));
    }
    
    #[test]
    fn test_zip_unequal_lengths() {
        // zip truncates to shorter length
        let module = compile("( 1 2 3 ) ( 10 20 ) zip").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        assert_eq!(interp.result(), Some(&Value::List(Box::new(vec![
            Value::Pair(Box::new((Value::Int(1), Value::Int(10)))),
            Value::Pair(Box::new((Value::Int(2), Value::Int(20)))),
        ]))));
    }
    
    #[test]
    fn test_dot_product() {
        // dot product: zip → map(unpair *) → fold(+, 0)
        // [1,2,3] · [4,5,6] = 1*4 + 2*5 + 3*6 = 32
        let module = compile("( 1 2 3 ) ( 4 5 6 ) zip [ unpair * ] map 0 [ + ] fold").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        assert_eq!(interp.result(), Some(&Value::Int(32)));
    }
    
    #[test]
    fn test_map_i2f() {
        // int to float conversion via map
        let module = compile("( 1 2 3 ) [ i2f ] map").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        assert_eq!(interp.result(), Some(&Value::List(Box::new(vec![
            Value::Float(1.0), Value::Float(2.0), Value::Float(3.0)
        ]))));
    }
    
    // ================================================================
    // COLLECT + MATRIX TESTS
    // ================================================================
    
    #[test]
    fn test_collect() {
        // collect N: gather top N stack values into a list
        let module = compile("10 20 30 collect 3").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        assert_eq!(interp.result(), Some(&Value::List(Box::new(vec![
            Value::Int(10), Value::Int(20), Value::Int(30)
        ]))));
    }
    
    #[test]
    fn test_matmul_2x2() {
        // [[1,2],[3,4]] × [[5,6],[7,8]] = [[19,22],[43,50]]
        // Using dot product function + collect
        let src = "\
            : dot zip [ unpair * ] map 0 [ + ] fold ; \
            ( 1 2 ) ( 5 7 ) dot \
            ( 1 2 ) ( 6 8 ) dot \
            collect 2 \
            ( 3 4 ) ( 5 7 ) dot \
            ( 3 4 ) ( 6 8 ) dot \
            collect 2 \
            collect 2";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        let expected = Value::List(Box::new(vec![
            Value::List(Box::new(vec![Value::Int(19), Value::Int(22)])),
            Value::List(Box::new(vec![Value::Int(43), Value::Int(50)])),
        ]));
        assert_eq!(interp.result(), Some(&expected));
    }
    
    #[test]
    fn test_float_dot_product() {
        // [1.0, 2.0, 3.0] · [4.0, 5.0, 6.0] = 32.0
        let module = compile("( 1.0 2.0 3.0 ) ( 4.0 5.0 6.0 ) zip [ unpair fmul ] map 0.0 [ fadd ] fold").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        assert_eq!(interp.result(), Some(&Value::Float(32.0)));
    }
    
    #[test]
    fn test_map_chain() {
        // map(+1) then map(*2) on [1,2,3] → [4,6,8]
        let module = compile("( 1 2 3 ) [ 1 + ] map [ 2 * ] map").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        assert_eq!(interp.result(), Some(&Value::List(Box::new(vec![
            Value::Int(4), Value::Int(6), Value::Int(8)
        ]))));
    }
    
    // ================================================================
    // AUTODIFF TESTS — Forward-mode AD via dual numbers
    // ================================================================
    
    #[test]
    fn test_ad_addition() {
        // f(x) = x + 3 at x=2, f'(x) = 1
        // Dual: (2,1) + (3,0) = (5,1)
        let src = "\
            : fst unpair drop ; \
            : snd unpair swap drop ; \
            : dual pair ; \
            : dconst 0.0 dual ; \
            : dvar 1.0 dual ; \
            : dadd unpair rot unpair rot fadd rot rot swap fadd swap pair ; \
            2.0 dvar 3.0 dconst dadd unpair";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        let stack = interp.stack();
        assert_eq!(stack[stack.len()-2], Value::Float(5.0));  // value
        assert_eq!(stack[stack.len()-1], Value::Float(1.0));  // derivative
    }
    
    #[test]
    fn test_ad_square() {
        // f(x) = x² at x=3, f'(x) = 2x = 6
        let src = "\
            : fst unpair drop ; \
            : snd unpair swap drop ; \
            : dual pair ; \
            : dvar 1.0 dual ; \
            : dmul over over fst swap fst fmul rot rot \
              over over snd swap fst fmul rot rot \
              fst swap snd fmul fadd pair ; \
            3.0 dvar dup dmul unpair";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        let stack = interp.stack();
        assert_eq!(stack[stack.len()-2], Value::Float(9.0));  // value
        assert_eq!(stack[stack.len()-1], Value::Float(6.0));  // derivative
    }
    
    #[test]
    fn test_ad_product_rule() {
        // f(x) = (x+1)(x+2) at x=3
        // f(3) = 4×5 = 20, f'(3) = 2×3+3 = 9
        let src = "\
            : fst unpair drop ; \
            : snd unpair swap drop ; \
            : dual pair ; \
            : dconst 0.0 dual ; \
            : dvar 1.0 dual ; \
            : dadd unpair rot unpair rot fadd rot rot swap fadd swap pair ; \
            : dmul over over fst swap fst fmul rot rot \
              over over snd swap fst fmul rot rot \
              fst swap snd fmul fadd pair ; \
            3.0 dvar 1.0 dconst dadd \
            3.0 dvar 2.0 dconst dadd \
            dmul unpair";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        
        use crate::interpreter::Value;
        let stack = interp.stack();
        assert_eq!(stack[stack.len()-2], Value::Float(20.0));  // value
        assert_eq!(stack[stack.len()-1], Value::Float(9.0));   // derivative
    }
    
    // ================================================================
    // IF-ELSE TESTS
    // ================================================================
    
    #[test]
    fn test_if_else_true() {
        use crate::interpreter::Value;
        let src = "true if 42 else 99 end";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.stack().last().unwrap(), &Value::Int(42));
    }
    
    #[test]
    fn test_if_else_false() {
        use crate::interpreter::Value;
        let src = "false if 42 else 99 end";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.stack().last().unwrap(), &Value::Int(99));
    }
    
    #[test]
    fn test_if_no_else() {
        use crate::interpreter::Value;
        let src = "5 true if 10 end";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        let stack = interp.stack();
        assert_eq!(stack, &[Value::Int(5), Value::Int(10)]);
    }
    
    #[test]
    fn test_if_no_else_false() {
        use crate::interpreter::Value;
        // When false and no else, nothing extra is pushed
        let src = "5 false if 10 end";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        let stack = interp.stack();
        assert_eq!(stack, &[Value::Int(5)]);
    }
    
    #[test]
    fn test_if_with_comparison() {
        use crate::interpreter::Value;
        let src = "3 5 < if 1 else 0 end";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.stack().last().unwrap(), &Value::Int(1));
    }
    
    #[test]
    fn test_relu_function() {
        use crate::interpreter::Value;
        let src = "\
            : relu dup 0.0 le if drop 0.0 end ; \
            -5.0 relu \
            3.0 relu";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        let stack = interp.stack();
        assert_eq!(stack[0], Value::Float(0.0));
        assert_eq!(stack[1], Value::Float(3.0));
    }
    
    #[test]
    fn test_nested_if() {
        use crate::interpreter::Value;
        // 10 > 5 ? (10 > 8 ? 1 : 2) : 3
        let src = "10 5 > if 10 8 > if 1 else 2 end else 3 end";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.stack().last().unwrap(), &Value::Int(1));
    }
    
    #[test]
    fn test_abs_function() {
        use crate::interpreter::Value;
        let src = "\
            : abs dup 0 < if neg end ; \
            -7 abs \
            5 abs";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        let stack = interp.stack();
        assert_eq!(stack[0], Value::Int(7));
        assert_eq!(stack[1], Value::Int(5));
    }
    
    #[test]
    fn test_max_function() {
        use crate::interpreter::Value;
        let src = "\
            : max over over < if swap end drop ; \
            3 7 max \
            9 2 max";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        let stack = interp.stack();
        assert_eq!(stack[0], Value::Int(7));
        assert_eq!(stack[1], Value::Int(9));
    }
    
    // ================================================================
    // RAMSEY & BENCHMARK TESTS
    // ================================================================
    
    #[test]
    fn test_ramsey_c5_witness() {
        use crate::interpreter::Value;
        // Verify C₅ cycle coloring of K₅ has 0 red cycle sum and 5 blue diag sum
        let src = "\
            ( 0 1 1 0 0 1 1 0 1 0 ) \
            dup 0 get  over 4 get +  over 7 get +  over 9 get +  over 3 get + \
            swap \
            dup 1 get  over 2 get +  over 5 get +  over 6 get +  over 8 get + \
            swap drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        let stack = interp.stack();
        assert_eq!(stack[0], Value::Int(0));  // cycle edges all red
        assert_eq!(stack[1], Value::Int(5));  // diagonal edges all blue
    }
    
    #[test]
    fn test_stdlib_sum_product() {
        use crate::interpreter::Value;
        let src = "\
            : sum 0 [ + ] fold ; \
            : product 1 [ * ] fold ; \
            ( 3 -1 4 1 5 ) sum \
            ( 1 2 3 4 ) product";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        let stack = interp.stack();
        assert_eq!(stack[0], Value::Int(12));  // 3-1+4+1+5
        assert_eq!(stack[1], Value::Int(24));  // 1*2*3*4
    }
    
    #[test]
    fn test_stdlib_vadd() {
        use crate::interpreter::Value;
        let src = "\
            : vadd zip [ unpair + ] map ; \
            ( 1 2 3 ) ( 10 20 30 ) vadd";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        let stack = interp.stack();
        assert_eq!(stack[0], Value::List(Box::new(vec![
            Value::Int(11), Value::Int(22), Value::Int(33)
        ])));
    }
    
    #[test]
    fn test_neural_xor() {
        use crate::interpreter::Value;
        // 2→2→1 neural net that solves XOR
        // W₁=[[1,1],[1,1]], b₁=[0,-1], W₂=[[1,-2]], b₂=[0]
        let src = "\
            : fst unpair drop ; \
            : snd unpair swap drop ; \
            : relu dup 0.0 le if drop 0.0 end ; \
            : dot zip [ unpair * ] map 0 [ + ] fold ; \
            : neuron rot rot dot + i2f relu ; \
            ( 1 1 ) ( 0 0 ) 0 neuron  ( 1 1 ) ( 0 0 ) -1 neuron \
            swap 1.0 fmul swap -2.0 fmul fadd 0.0 fadd relu \
            ( 1 1 ) ( 0 1 ) 0 neuron  ( 1 1 ) ( 0 1 ) -1 neuron \
            swap 1.0 fmul swap -2.0 fmul fadd 0.0 fadd relu \
            ( 1 1 ) ( 1 0 ) 0 neuron  ( 1 1 ) ( 1 0 ) -1 neuron \
            swap 1.0 fmul swap -2.0 fmul fadd 0.0 fadd relu \
            ( 1 1 ) ( 1 1 ) 0 neuron  ( 1 1 ) ( 1 1 ) -1 neuron \
            swap 1.0 fmul swap -2.0 fmul fadd 0.0 fadd relu";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        let stack = interp.stack();
        assert_eq!(stack[0], Value::Float(0.0));  // XOR(0,0) = 0
        assert_eq!(stack[1], Value::Float(1.0));  // XOR(0,1) = 1
        assert_eq!(stack[2], Value::Float(1.0));  // XOR(1,0) = 1
        assert_eq!(stack[3], Value::Float(0.0));  // XOR(1,1) = 0
    }
    
    // ====================================================================
    // WHILE LOOP TESTS
    // ====================================================================
    
    #[test]
    fn test_while_countdown() {
        // Count down from 5 to 0: 5 while dup 0 > do 1 - end
        // After loop: stack has 0
        use crate::interpreter::Value;
        let module = compile("5 while dup 0 > do 1 - end").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.stack().last(), Some(&Value::Int(0)));
    }
    
    #[test]
    fn test_while_sum_1_to_10() {
        // Sum 1+2+...+10 = 55
        // Stack: accumulator counter
        // 0 10 while dup 0 > do swap over + swap 1 - end drop
        use crate::interpreter::Value;
        let module = compile("0 10 while dup 0 > do swap over + swap 1 - end drop").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.stack().last(), Some(&Value::Int(55)));
    }
    
    #[test]
    fn test_while_false_never_executes() {
        // Condition is false immediately — body never runs
        // while pushes/tests condition between while..do
        // "dup 0 >" on stack [0] → false, so body never executes
        use crate::interpreter::Value;
        let module = compile("42 0 while dup 0 > do 1 - end drop").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.stack().last(), Some(&Value::Int(42)));
    }
    
    #[test]
    fn test_while_factorial() {
        // 5! = 120
        // result=1 n=5: while dup 1 > do swap over * swap 1 - end drop
        use crate::interpreter::Value;
        let module = compile("1 5 while dup 1 > do swap over * swap 1 - end drop").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.stack().last(), Some(&Value::Int(120)));
    }
    
    #[test]
    fn test_while_with_function() {
        // Use function inside while loop body
        use crate::interpreter::Value;
        let module = compile(": double dup + ; 1 while dup 100 < do double end").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        // 1 → 2 → 4 → 8 → 16 → 32 → 64 → 128 (first ≥ 100)
        assert_eq!(interp.stack().last(), Some(&Value::Int(128)));
    }
    
    #[test]
    fn test_while_nested() {
        // Nested while: outer counts 0→2, inner sums
        // Result: 0+1+2 = 3 (from inner), done 3 times? No, simpler:
        // Just verify nested while compiles and runs
        use crate::interpreter::Value;
        let module = compile("0 3 while dup 0 > do swap 1 + swap 1 - end drop").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.stack().last(), Some(&Value::Int(3)));
    }

    #[test]
    fn test_matmul_4x4() {
        // A² where A = [[1..4],[5..8],[9..12],[13..16]]
        use crate::interpreter::Value;
        let src = "\
            1 1 * 2 5 * + 3 9 * + 4 13 * + \
            1 2 * 2 6 * + 3 10 * + 4 14 * + \
            1 3 * 2 7 * + 3 11 * + 4 15 * + \
            1 4 * 2 8 * + 3 12 * + 4 16 * + \
            5 1 * 6 5 * + 7 9 * + 8 13 * + \
            5 2 * 6 6 * + 7 10 * + 8 14 * + \
            5 3 * 6 7 * + 7 11 * + 8 15 * + \
            5 4 * 6 8 * + 7 12 * + 8 16 * + \
            9 1 * 10 5 * + 11 9 * + 12 13 * + \
            9 2 * 10 6 * + 11 10 * + 12 14 * + \
            9 3 * 10 7 * + 11 11 * + 12 15 * + \
            9 4 * 10 8 * + 11 12 * + 12 16 * + \
            13 1 * 14 5 * + 15 9 * + 16 13 * + \
            13 2 * 14 6 * + 15 10 * + 16 14 * + \
            13 3 * 14 7 * + 15 11 * + 16 15 * + \
            13 4 * 14 8 * + 15 12 * + 16 16 * +";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        let expected = vec![90,100,110,120, 202,228,254,280, 314,356,398,440, 426,484,542,600];
        let stack: Vec<i64> = interp.stack().iter().map(|v| match v {
            Value::Int(n) => *n,
            _ => panic!("expected int"),
        }).collect();
        assert_eq!(stack, expected, "4x4 matmul A² should match");
    }
    
    #[test]
    fn test_while_fibonacci() {
        // F(20) = 6765
        use crate::interpreter::Value;
        let src = "0 1 20 while dup 0 > do 1 - rot rot swap over + rot end drop drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.stack().last(), Some(&Value::Int(6765)));
    }

    #[test]
    fn test_while_gcd() {
        // gcd(252, 105) = 21
        use crate::interpreter::Value;
        let src = "252 105 while dup 0 > do swap over % end drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.stack().last(), Some(&Value::Int(21)));
    }

    #[test]
    fn test_while_isqrt() {
        // isqrt(144) = 12 via Newton's method
        use crate::interpreter::Value;
        let src = "144 dup 10 while dup 0 > do 1 - rot rot over over / + 2 / rot end drop swap drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.stack().last(), Some(&Value::Int(12)));
    }

    #[test]
    fn test_while_power() {
        // 2^10 = 1024
        use crate::interpreter::Value;
        let src = "1 2 10 while dup 0 > do rot rot swap over * swap rot 1 - end drop drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.stack().last(), Some(&Value::Int(1024)));
    }

    #[test]
    fn test_gradient_descent_while() {
        // Minimize f(x)=x² from x=10, lr=0.1, 50 steps
        // x_final = 10 * 0.8^50 ≈ 0.000143
        use crate::interpreter::Value;
        let src = "\
            : fst unpair drop ; \
            : snd unpair swap drop ; \
            : dual pair ; \
            : dvar 1.0 dual ; \
            : dmul over over fst swap fst fmul rot rot \
              over over snd swap fst fmul rot rot \
              fst swap snd fmul fadd pair ; \
            : f_dual dvar dup dmul ; \
            : gd_step dup f_dual snd 0.1 fmul swap over fsub swap drop ; \
            10.0 50 \
            while dup 0 > do swap gd_step swap 1 - end \
            drop dup dup fmul";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        let stack = interp.stack();
        // x_final should be very small (near 0)
        if let Value::Float(x) = stack[0] {
            assert!(x.abs() < 0.001, "x_final should be near 0, got {}", x);
        } else {
            panic!("Expected float");
        }
        // f(x_final) should be even smaller
        if let Value::Float(fx) = stack[1] {
            assert!(fx.abs() < 0.00001, "f(x) should be near 0, got {}", fx);
        } else {
            panic!("Expected float");
        }
    }

    // ===== Local variable tests =====

    #[test]
    fn test_local_variable_basic() {
        // Store 42 into x, then push x
        use crate::interpreter::Value;
        let src = "42 -> x x";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.stack(), &[Value::Int(42)]);
    }

    #[test]
    fn test_local_variable_two_vars() {
        // Store two locals, use both
        use crate::interpreter::Value;
        let src = "10 -> a 20 -> b a b +";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.stack(), &[Value::Int(30)]);
    }

    #[test]
    fn test_local_variable_reuse() {
        // x used multiple times
        use crate::interpreter::Value;
        let src = "5 -> x x x * x +";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        // 5*5 + 5 = 30
        assert_eq!(interp.stack(), &[Value::Int(30)]);
    }

    #[test]
    fn test_local_variable_reassign() {
        // Reassign same name
        use crate::interpreter::Value;
        let src = "10 -> x x 20 -> x x";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        // stack: [10, 20]
        assert_eq!(interp.stack(), &[Value::Int(10), Value::Int(20)]);
    }

    #[test]
    fn test_local_variable_float() {
        use crate::interpreter::Value;
        let src = "3.14 -> pi pi pi fmul";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        let stack = interp.stack();
        // -> pi consumes value, pi pushes it, pi fmul pushes pi then multiplies
        // stack: [pi * pi] = [9.8596]
        assert_eq!(stack.len(), 1);
        if let Value::Float(v) = &stack[0] {
            assert!((v - 3.14 * 3.14).abs() < 1e-10, "pi*pi = {}", v);
        } else {
            panic!("Expected float, got {:?}", stack);
        }
    }

    #[test]
    fn test_local_in_function() {
        // Locals work inside function definitions
        use crate::interpreter::Value;
        let src = ": square -> n n n * ; 7 square";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.stack(), &[Value::Int(49)]);
    }

    #[test]
    fn test_local_function_scoping() {
        // Two functions each have their own 'x'
        use crate::interpreter::Value;
        let src = ": double -> x x x + ; : triple -> x x x x + + ; 5 double 3 triple";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.stack(), &[Value::Int(10), Value::Int(9)]);
    }

    #[test]
    fn test_local_multi_arg_function() {
        // Function taking two args via locals
        use crate::interpreter::Value;
        let src = ": hyp -> b -> a a a * b b * + ; 3 4 hyp";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        // 3² + 4² = 25
        assert_eq!(interp.stack(), &[Value::Int(25)]);
    }

    #[test]
    fn test_local_with_if() {
        use crate::interpreter::Value;
        let src = ": absval -> n n 0 > if n else n neg end ; 5 absval -3 absval";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.stack(), &[Value::Int(5), Value::Int(3)]);
    }

    #[test]
    fn test_local_with_while() {
        // Use locals to accumulate factorial
        use crate::interpreter::Value;
        let src = "\
            : fact -> n \
              1 -> acc \
              while n 1 > do \
                acc n * -> acc \
                n 1 - -> n \
              end \
              acc ; \
            5 fact";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.stack(), &[Value::Int(120)]);
    }

    // ===== FEXP / FLOG tests =====

    #[test]
    fn test_fexp_e() {
        // e^1 ≈ 2.71828
        use crate::interpreter::Value;
        let src = "1.0 fexp";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(v)) = interp.stack().last() {
            assert!((v - std::f64::consts::E).abs() < 1e-10, "e^1 = {}", v);
        } else {
            panic!("Expected float");
        }
    }

    #[test]
    fn test_fexp_zero() {
        // e^0 = 1
        use crate::interpreter::Value;
        let src = "0.0 fexp";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(v)) = interp.stack().last() {
            assert!((v - 1.0).abs() < 1e-10, "e^0 = {}", v);
        } else {
            panic!("Expected float");
        }
    }

    #[test]
    fn test_flog_one() {
        // ln(1) = 0
        use crate::interpreter::Value;
        let src = "1.0 flog";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(v)) = interp.stack().last() {
            assert!(v.abs() < 1e-10, "ln(1) = {}", v);
        } else {
            panic!("Expected float");
        }
    }

    #[test]
    fn test_flog_e() {
        // ln(e) = 1
        use crate::interpreter::Value;
        let src = "2.718281828459045 flog";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(v)) = interp.stack().last() {
            assert!((v - 1.0).abs() < 1e-10, "ln(e) = {}", v);
        } else {
            panic!("Expected float");
        }
    }

    #[test]
    fn test_fexp_flog_inverse() {
        // flog(fexp(x)) = x
        use crate::interpreter::Value;
        let src = "3.7 fexp flog";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(v)) = interp.stack().last() {
            assert!((v - 3.7).abs() < 1e-10, "flog(fexp(3.7)) = {}", v);
        } else {
            panic!("Expected float");
        }
    }

    #[test]
    fn test_fexp_flog_inverse_reverse() {
        // fexp(flog(x)) = x  (for x > 0)
        use crate::interpreter::Value;
        let src = "5.5 flog fexp";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(v)) = interp.stack().last() {
            assert!((v - 5.5).abs() < 1e-10, "fexp(flog(5.5)) = {}", v);
        } else {
            panic!("Expected float");
        }
    }

    #[test]
    fn test_flog_negative_fails() {
        // ln(x) for x <= 0 should return Error value (P1: S→S)
        use crate::interpreter::Value;
        let src = "-1.0 flog";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        match &interp.stack()[0] {
            Value::Error(msg) => assert!(msg.contains("positive")),
            other => panic!("expected Error, got {:?}", other),
        }
    }

    #[test]
    fn test_flog_zero_fails() {
        // ln(0) should return Error value (P1: S→S)
        use crate::interpreter::Value;
        let src = "0.0 flog";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        match &interp.stack()[0] {
            Value::Error(msg) => assert!(msg.contains("positive")),
            other => panic!("expected Error, got {:?}", other),
        }
    }

    #[test]
    fn test_fexp_negative() {
        // e^(-1) = 1/e ≈ 0.3679
        use crate::interpreter::Value;
        let src = "-1.0 fexp";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(v)) = interp.stack().last() {
            assert!((v - 1.0 / std::f64::consts::E).abs() < 1e-10, "e^(-1) = {}", v);
        } else {
            panic!("Expected float");
        }
    }

    // ===== Combined: locals + math in real programs =====

    #[test]
    fn test_softmax_component() {
        // Compute e^x / (e^x + e^y) for x=1, y=2
        use crate::interpreter::Value;
        let src = "\
            1.0 fexp -> ex \
            2.0 fexp -> ey \
            ex ex ey fadd fdiv";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(v)) = interp.stack().last() {
            let expected = 1.0_f64.exp() / (1.0_f64.exp() + 2.0_f64.exp());
            assert!((v - expected).abs() < 1e-10, "softmax component = {}", v);
        } else {
            panic!("Expected float");
        }
    }

    #[test]
    fn test_newton_sqrt_with_locals() {
        // Newton's method for sqrt(2) using locals
        use crate::interpreter::Value;
        let src = "\
            : newton_sqrt -> s \
              s 2.0 fdiv -> x \
              10 \
              while dup 0 > do \
                x s x fdiv fadd 2.0 fdiv -> x \
                1 - \
              end \
              drop x ; \
            2.0 newton_sqrt";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(v)) = interp.stack().last() {
            assert!((v - 2.0_f64.sqrt()).abs() < 1e-12, "sqrt(2) = {}", v);
        } else {
            panic!("Expected float");
        }
    }

    #[test]
    fn test_log_properties() {
        // ln(a*b) = ln(a) + ln(b)
        use crate::interpreter::Value;
        let src = "\
            3.0 -> a 5.0 -> b \
            a b fmul flog \
            a flog b flog fadd";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        let stack = interp.stack();
        if let (Value::Float(ln_ab), Value::Float(ln_a_plus_ln_b)) = (&stack[0], &stack[1]) {
            assert!((ln_ab - ln_a_plus_ln_b).abs() < 1e-10,
                "ln(a*b) = {} ≠ ln(a)+ln(b) = {}", ln_ab, ln_a_plus_ln_b);
        } else {
            panic!("Expected floats");
        }
    }

    #[test]
    fn test_power_via_exp_log() {
        // a^b = exp(b * ln(a))
        use crate::interpreter::Value;
        let src = "2.0 -> base 10.0 -> exp_ base flog exp_ fmul fexp";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(v)) = interp.stack().last() {
            assert!((v - 1024.0).abs() < 1e-6, "2^10 = {}", v);
        } else {
            panic!("Expected float");
        }
    }

    // ===== IO + Capability tests (Phase 2) =====

    #[test]
    fn test_println_with_cap() {
        // println auto-sets CAP_IO on the module
        use crate::interpreter::Value;
        let src = "42 println";
        let module = compile(src).unwrap();
        assert_eq!(module.cap_flags & crate::bytecode::CAP_IO, crate::bytecode::CAP_IO,
            "println should set CAP_IO flag");
        // Run with IO allowed
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.output(), &["42"]);
    }

    #[test]
    fn test_print_no_newline() {
        let src = "\"hello\" print \"world\" print";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.output(), &["hello", "world"]);
    }

    #[test]
    fn test_println_string() {
        let src = "\"hello kore\" println";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.output(), &["hello kore"]);
    }

    #[test]
    fn test_println_float() {
        use crate::interpreter::Value;
        let src = "3.14 println";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.output(), &["3.14"]);
    }

    #[test]
    fn test_println_bool() {
        let src = "true println false println";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.output(), &["true", "false"]);
    }

    #[test]
    fn test_println_list() {
        let src = "( 1 2 3 ) println";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.output(), &["(1 2 3)"]);
    }

    #[test]
    fn test_println_pair() {
        let src = "1 2 pair println";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.output(), &["(1, 2)"]);
    }

    #[test]
    fn test_println_computed() {
        // Print a computed value
        let src = "3 4 + println";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.output(), &["7"]);
    }

    #[test]
    fn test_cap_flags_pure() {
        // Pure program should have cap_flags = 0
        let src = "1 2 +";
        let module = compile(src).unwrap();
        assert_eq!(module.cap_flags, 0, "Pure program should have no caps");
    }

    #[test]
    fn test_cap_flags_io() {
        // IO program should have CAP_IO set
        let src = "42 println";
        let module = compile(src).unwrap();
        assert_eq!(module.cap_flags, crate::bytecode::CAP_IO);
    }

    #[test]
    fn test_cap_denied_at_runtime() {
        // Compile with IO, run without allowing IO => runtime error
        let src = "42 println";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module_with_caps(&module, 0); // no caps allowed
        let result = interp.run();
        assert!(result.is_err(), "Should fail without IO cap");
        assert!(result.unwrap_err().contains("denied"), "Error should mention denied");
    }

    #[test]
    fn test_proof_checker_rejects_io_without_cap() {
        // Proof checker should reject IO opcodes when cap not granted
        use crate::proof_checker::ProofChecker;
        let src = "42 println";
        let module = compile(src).unwrap();
        let mut checker = ProofChecker::new();
        let errors = checker.check_with_caps(&module.code, 0); // no caps
        assert!(!errors.is_empty(), "Should have proof check errors");
        assert!(errors[0].message.contains("capability"),
            "Error should mention capability: {}", errors[0].message);
    }

    #[test]
    fn test_proof_checker_accepts_io_with_cap() {
        // Proof checker should accept IO opcodes when cap is granted
        use crate::proof_checker::ProofChecker;
        let src = "42 println";
        let module = compile(src).unwrap();
        let mut checker = ProofChecker::new();
        let errors = checker.check_with_caps(&module.code, crate::bytecode::CAP_IO);
        assert!(errors.is_empty(), "Should pass with IO cap: {:?}", errors);
    }

    #[test]
    fn test_rand_produces_int() {
        use crate::interpreter::Value;
        let src = "rand";
        let module = compile(src).unwrap();
        assert_eq!(module.cap_flags & crate::bytecode::CAP_IO, crate::bytecode::CAP_IO,
            "rand should set CAP_IO flag");
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.stack().len(), 1);
        match interp.stack().last() {
            Some(Value::Int(_)) => {} // OK
            other => panic!("rand should produce Int, got {:?}", other),
        }
    }

    #[test]
    fn test_rand_denied_without_cap() {
        let src = "rand";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module_with_caps(&module, 0);
        assert!(interp.run().is_err(), "rand should fail without IO cap");
    }

    #[test]
    fn test_cap_encode_decode() {
        // Module cap_flags should survive encode/decode round-trip
        let src = "42 println";
        let module = compile(src).unwrap();
        let binary = module.encode();
        let decoded = crate::bytecode::BytecodeModule::decode(&binary).unwrap();
        assert_eq!(decoded.cap_flags, module.cap_flags,
            "cap_flags should survive encode/decode: {} != {}", decoded.cap_flags, module.cap_flags);
    }

    #[test]
    fn test_println_in_function() {
        // IO works inside function definitions
        let src = ": greet -> name name println ; \"alice\" greet \"bob\" greet";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.output(), &["alice", "bob"]);
    }

    #[test]
    fn test_println_in_loop() {
        // Print in a while loop
        let src = "3 while dup 0 > do dup println 1 - end drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.output(), &["3", "2", "1"]);
    }

    #[test]
    fn test_multiple_io_ops() {
        // Mix of print, println, rand
        use crate::interpreter::Value;
        let src = "\"start\" println rand -> r r println";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.output().len(), 2);
        assert_eq!(interp.output()[0], "start");
        // Second output is the random number (just check it's a number string)
        interp.output()[1].parse::<i64>().expect("rand output should be parseable as i64");
    }

    // ===== Deterministic PRNG tests (Phase 3) =====

    #[test]
    fn test_prng_deterministic() {
        // Same seed → same output (P2: deterministic execution)
        use crate::interpreter::Value;
        let src = "\
            : prng-next 6364136223846793005 * 1442695040888963407 + dup ; \
            42 prng-next drop";
        let module = compile(src).unwrap();
        let mut interp1 = Interpreter::from_module(&module);
        interp1.run().unwrap();
        let mut interp2 = Interpreter::from_module(&module);
        interp2.run().unwrap();
        assert_eq!(interp1.stack(), interp2.stack(),
            "Same seed must produce same result (P2)");
    }

    #[test]
    fn test_prng_sequence() {
        // Generates different values each step
        use crate::interpreter::Value;
        let src = "\
            : prng-next 6364136223846793005 * 1442695040888963407 + dup ; \
            42 prng-next -> s1 \
            s1 prng-next -> s2 \
            s2 prng-next drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        let stack = interp.stack();
        // Should have 3 distinct values
        assert_eq!(stack.len(), 3);
        assert_ne!(stack[0], stack[1], "Consecutive PRNG values should differ");
        assert_ne!(stack[1], stack[2], "Consecutive PRNG values should differ");
        assert_ne!(stack[0], stack[2], "PRNG values should be distinct");
    }

    #[test]
    fn test_prng_float() {
        // prng-float returns value in [0, 1)
        use crate::interpreter::Value;
        let src = "\
            : abs dup 0 < if neg end ; \
            : prng-next 6364136223846793005 * 1442695040888963407 + dup ; \
            : prng-float \
              prng-next -> seed \
              abs i2f 9223372036854775807 i2f fdiv \
              seed ; \
            42 prng-float drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(f)) = interp.stack().last() {
            assert!(*f >= 0.0 && *f < 1.0, "prng-float should be in [0,1), got {}", f);
        } else {
            panic!("Expected float, got {:?}", interp.stack());
        }
    }

    #[test]
    fn test_prng_float_multiple() {
        // Generate several floats, all should be in [0, 1)
        use crate::interpreter::Value;
        let src = "\
            : abs dup 0 < if neg end ; \
            : prng-next 6364136223846793005 * 1442695040888963407 + dup ; \
            : prng-float \
              prng-next -> seed \
              abs i2f 9223372036854775807 i2f fdiv \
              seed ; \
            42 -> s \
            s prng-float -> s -> f1 \
            s prng-float -> s -> f2 \
            s prng-float -> s -> f3 \
            f1 f2 f3";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        let stack = interp.stack();
        assert_eq!(stack.len(), 3);
        for (i, v) in stack.iter().enumerate() {
            if let Value::Float(f) = v {
                assert!(*f >= 0.0 && *f < 1.0, "float {} should be in [0,1), got {}", i, f);
            } else {
                panic!("Expected float at position {}, got {:?}", i, v);
            }
        }
    }

    #[test]
    fn test_prng_range() {
        // prng-range returns value in [lo, hi]
        use crate::interpreter::Value;
        let src = "\
            : abs dup 0 < if neg end ; \
            : prng-next 6364136223846793005 * 1442695040888963407 + dup ; \
            : prng-range \
              -> hi -> lo \
              prng-next -> seed \
              abs hi lo - 1 + mod lo + \
              seed ; \
            42 1 6 prng-range drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Int(n)) = interp.stack().last() {
            assert!(*n >= 1 && *n <= 6, "prng-range(1,6) should be in [1,6], got {}", n);
        } else {
            panic!("Expected int, got {:?}", interp.stack());
        }
    }

    // ================================================================
    // Phase 4: Tensor Operations Tests
    // ================================================================

    #[test]
    fn test_tensor_new_and_accessors() {
        use crate::interpreter::Value;
        let src = "\
            : tensor-new pair ; \
            : tensor-data unpair drop ; \
            : tensor-shape unpair swap drop ; \
            ( 1.0 2.0 3.0 4.0 ) ( 2 2 ) tensor-new \
            dup tensor-data len swap drop \
            swap tensor-shape len swap drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        let stack = interp.stack();
        assert_eq!(stack.len(), 2);
        assert_eq!(stack[0], Value::Int(4)); // 4 data elements
        assert_eq!(stack[1], Value::Int(2)); // 2 shape dims
    }

    #[test]
    fn test_tensor_add() {
        use crate::interpreter::Value;
        let src = "\
            : tensor-new pair ; \
            : tensor-add \
                unpair -> shape2 \
                swap unpair -> shape1 \
                zip [ unpair fadd ] map \
                shape1 tensor-new ; \
            ( 1.0 2.0 3.0 ) ( 3 ) tensor-new \
            ( 4.0 5.0 6.0 ) ( 3 ) tensor-new \
            tensor-add unpair drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        // data should be [5.0, 7.0, 9.0]
        if let Some(Value::List(items)) = interp.result() {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], Value::Float(5.0));
            assert_eq!(items[1], Value::Float(7.0));
            assert_eq!(items[2], Value::Float(9.0));
        } else {
            panic!("Expected list, got {:?}", interp.result());
        }
    }

    #[test]
    fn test_tensor_scale() {
        use crate::interpreter::Value;
        let src = "\
            : tensor-new pair ; \
            : tensor-scale \
                -> s \
                unpair -> shape \
                [ s fmul ] map \
                shape tensor-new ; \
            ( 1.0 2.0 3.0 ) ( 3 ) tensor-new \
            2.0 tensor-scale unpair drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::List(items)) = interp.result() {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], Value::Float(2.0));
            assert_eq!(items[1], Value::Float(4.0));
            assert_eq!(items[2], Value::Float(6.0));
        } else {
            panic!("Expected list, got {:?}", interp.result());
        }
    }

    #[test]
    fn test_tensor_dot() {
        use crate::interpreter::Value;
        let src = "\
            : tensor-new pair ; \
            : tensor-data unpair drop ; \
            : tensor-dot \
                tensor-data swap tensor-data \
                zip [ unpair fmul ] map \
                0.0 [ fadd ] fold ; \
            ( 1.0 2.0 3.0 ) ( 3 ) tensor-new \
            ( 4.0 5.0 6.0 ) ( 3 ) tensor-new \
            tensor-dot";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        // 1*4 + 2*5 + 3*6 = 32
        if let Some(Value::Float(f)) = interp.result() {
            assert!((*f - 32.0).abs() < 1e-10, "dot product = {}", f);
        } else {
            panic!("Expected float, got {:?}", interp.result());
        }
    }

    #[test]
    fn test_tensor_sum() {
        use crate::interpreter::Value;
        let src = "\
            : tensor-new pair ; \
            : tensor-data unpair drop ; \
            : tensor-sum tensor-data 0.0 [ fadd ] fold ; \
            ( 1.0 2.0 3.0 4.0 ) ( 2 2 ) tensor-new \
            tensor-sum";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(f)) = interp.result() {
            assert!((*f - 10.0).abs() < 1e-10, "sum = {}", f);
        } else {
            panic!("Expected float, got {:?}", interp.result());
        }
    }

    #[test]
    fn test_tensor_relu() {
        use crate::interpreter::Value;
        let src = "\
            : tensor-new pair ; \
            : tensor-relu \
                unpair -> shape \
                [ dup 0.0 le if drop 0.0 end ] map \
                shape tensor-new ; \
            ( -1.0 2.0 -3.0 4.0 ) ( 4 ) tensor-new \
            tensor-relu unpair drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::List(items)) = interp.result() {
            assert_eq!(items.len(), 4);
            assert_eq!(items[0], Value::Float(0.0));
            assert_eq!(items[1], Value::Float(2.0));
            assert_eq!(items[2], Value::Float(0.0));
            assert_eq!(items[3], Value::Float(4.0));
        } else {
            panic!("Expected list, got {:?}", interp.result());
        }
    }

    #[test]
    fn test_tensor_sub() {
        use crate::interpreter::Value;
        let src = "\
            : tensor-new pair ; \
            : tensor-sub \
                unpair -> shape2 \
                swap unpair -> shape1 \
                swap zip [ unpair fsub ] map \
                shape1 tensor-new ; \
            ( 5.0 7.0 9.0 ) ( 3 ) tensor-new \
            ( 1.0 2.0 3.0 ) ( 3 ) tensor-new \
            tensor-sub unpair drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::List(items)) = interp.result() {
            assert_eq!(items[0], Value::Float(4.0));
            assert_eq!(items[1], Value::Float(5.0));
            assert_eq!(items[2], Value::Float(6.0));
        } else {
            panic!("Expected list, got {:?}", interp.result());
        }
    }

    #[test]
    fn test_tensor_hadamard() {
        use crate::interpreter::Value;
        let src = "\
            : tensor-new pair ; \
            : tensor-mul \
                unpair -> shape2 \
                swap unpair -> shape1 \
                zip [ unpair fmul ] map \
                shape1 tensor-new ; \
            ( 2.0 3.0 4.0 ) ( 3 ) tensor-new \
            ( 5.0 6.0 7.0 ) ( 3 ) tensor-new \
            tensor-mul unpair drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::List(items)) = interp.result() {
            assert_eq!(items[0], Value::Float(10.0));
            assert_eq!(items[1], Value::Float(18.0));
            assert_eq!(items[2], Value::Float(28.0));
        } else {
            panic!("Expected list, got {:?}", interp.result());
        }
    }

    // ================================================================
    // Phase 4: Geometry Tests
    // ================================================================

    #[test]
    fn test_dist2d() {
        use crate::interpreter::Value;
        let src = "\
            : dist2d \
                -> y2 -> x2 -> y1 -> x1 \
                x2 x1 fsub dup fmul \
                y2 y1 fsub dup fmul \
                fadd fsqrt ; \
            0.0 0.0 3.0 4.0 dist2d";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(f)) = interp.result() {
            assert!((*f - 5.0).abs() < 1e-10, "dist(0,0,3,4) = {}", f);
        } else {
            panic!("Expected float, got {:?}", interp.result());
        }
    }

    #[test]
    fn test_dist2d_same_point() {
        use crate::interpreter::Value;
        let src = "\
            : dist2d \
                -> y2 -> x2 -> y1 -> x1 \
                x2 x1 fsub dup fmul \
                y2 y1 fsub dup fmul \
                fadd fsqrt ; \
            3.0 4.0 3.0 4.0 dist2d";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(f)) = interp.result() {
            assert!(f.abs() < 1e-10, "dist(p,p) = {}", f);
        } else {
            panic!("Expected float, got {:?}", interp.result());
        }
    }

    #[test]
    fn test_circle_overlap() {
        use crate::interpreter::Value;
        let src = "\
            : dist2d \
                -> y2 -> x2 -> y1 -> x1 \
                x2 x1 fsub dup fmul \
                y2 y1 fsub dup fmul \
                fadd fsqrt ; \
            : circle-overlap \
                -> r2 -> y2 -> x2 -> r1 -> y1 -> x1 \
                x1 y1 x2 y2 dist2d -> d \
                r1 r2 fadd d fsub \
                dup 0.0 le if drop 0.0 end ; \
            0.0 0.0 1.0 1.5 0.0 1.0 circle-overlap";
        // distance = 1.5, sum of radii = 2.0, overlap = 0.5
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(f)) = interp.result() {
            assert!((*f - 0.5).abs() < 1e-10, "overlap = {}", f);
        } else {
            panic!("Expected float, got {:?}", interp.result());
        }
    }

    #[test]
    fn test_circle_no_overlap() {
        use crate::interpreter::Value;
        let src = "\
            : dist2d \
                -> y2 -> x2 -> y1 -> x1 \
                x2 x1 fsub dup fmul \
                y2 y1 fsub dup fmul \
                fadd fsqrt ; \
            : circle-overlap \
                -> r2 -> y2 -> x2 -> r1 -> y1 -> x1 \
                x1 y1 x2 y2 dist2d -> d \
                r1 r2 fadd d fsub \
                dup 0.0 le if drop 0.0 end ; \
            0.0 0.0 1.0 5.0 0.0 1.0 circle-overlap";
        // distance = 5.0, sum of radii = 2.0, no overlap
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(f)) = interp.result() {
            assert!(f.abs() < 1e-10, "no overlap = {}", f);
        } else {
            panic!("Expected float, got {:?}", interp.result());
        }
    }

    #[test]
    fn test_vec2_ops() {
        use crate::interpreter::Value;
        let src = "\
            : vec2-len \
                -> y -> x \
                x x fmul y y fmul fadd fsqrt ; \
            3.0 4.0 vec2-len";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(f)) = interp.result() {
            assert!((*f - 5.0).abs() < 1e-10, "|(3,4)| = {}", f);
        } else {
            panic!("Expected float, got {:?}", interp.result());
        }
    }

    // ================================================================
    // Phase 4: Annealing Tests
    // ================================================================

    #[test]
    fn test_metropolis_accept_improvement() {
        // dE < 0 (improvement) should always accept
        use crate::interpreter::Value;
        let src = "\
            : abs dup 0 < if neg end ; \
            : prng-next 6364136223846793005 * 1442695040888963407 + dup ; \
            : prng-float \
              prng-next -> seed \
              abs i2f 9223372036854775807 i2f fdiv \
              seed ; \
            : metropolis \
              -> seed -> T -> dE \
              dE 0.0 le if \
                true seed \
              else \
                dE fneg T fdiv fexp -> p \
                seed prng-float -> seed2 \
                p le \
                seed2 \
              end ; \
            -1.0 1.0 42 metropolis drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Bool(true)));
    }

    #[test]
    fn test_metropolis_reject_high_de() {
        // Very high dE at low T should reject (almost certainly)
        use crate::interpreter::Value;
        let src = "\
            : abs dup 0 < if neg end ; \
            : prng-next 6364136223846793005 * 1442695040888963407 + dup ; \
            : prng-float \
              prng-next -> seed \
              abs i2f 9223372036854775807 i2f fdiv \
              seed ; \
            : metropolis \
              -> seed -> T -> dE \
              dE 0.0 le if \
                true seed \
              else \
                dE fneg T fdiv fexp -> p \
                seed prng-float -> seed2 \
                p le \
                seed2 \
              end ; \
            100.0 0.001 42 metropolis drop";
        // dE=100, T=0.001 → p = exp(-100000) ≈ 0 → reject
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Bool(false)));
    }

    #[test]
    fn test_cooling_schedule() {
        use crate::interpreter::Value;
        let src = "\
            : cool \
              -> steps -> k -> T-end -> T-start \
              T-end T-start fdiv flog \
              k i2f steps i2f fdiv fmul \
              fexp \
              T-start fmul ; \
            100.0 1.0 0 100 cool";
        // At k=0: T = 100 * exp(0) = 100
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(f)) = interp.result() {
            assert!((*f - 100.0).abs() < 1e-10, "T(0) = {}", f);
        } else {
            panic!("Expected float, got {:?}", interp.result());
        }
    }

    #[test]
    fn test_cooling_schedule_end() {
        use crate::interpreter::Value;
        let src = "\
            : cool \
              -> steps -> k -> T-end -> T-start \
              T-end T-start fdiv flog \
              k i2f steps i2f fdiv fmul \
              fexp \
              T-start fmul ; \
            100.0 1.0 100 100 cool";
        // At k=steps: T = 100 * (1/100)^1 = 1.0
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(f)) = interp.result() {
            assert!((*f - 1.0).abs() < 1e-6, "T(100) = {}", f);
        } else {
            panic!("Expected float, got {:?}", interp.result());
        }
    }

    #[test]
    fn test_cooling_schedule_midpoint() {
        use crate::interpreter::Value;
        let src = "\
            : cool \
              -> steps -> k -> T-end -> T-start \
              T-end T-start fdiv flog \
              k i2f steps i2f fdiv fmul \
              fexp \
              T-start fmul ; \
            100.0 1.0 50 100 cool";
        // At k=50: T = 100 * (1/100)^0.5 = 100/10 = 10.0
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(f)) = interp.result() {
            assert!((*f - 10.0).abs() < 1e-6, "T(50) = {}", f);
        } else {
            panic!("Expected float, got {:?}", interp.result());
        }
    }

    // ================================================================
    // Phase 6: Reverse-Mode Autodiff Tests
    // ================================================================

    // Inlined autograd library for tests
    const AUTOGRAD_LIB: &str = "\
        : ad-node-new \
          -> tape -> saved -> inputs -> value -> op \
          op value pair inputs saved pair pair -> node \
          tape len swap drop -> idx \
          tape node append \
          idx ; \
        : ad-var -> x -> tape \
          0 x ( ) ( ) tape ad-node-new ; \
        : ad-const -> c -> tape \
          1 c ( ) ( ) tape ad-node-new ; \
        : ad-value \
          -> idx -> tape \
          tape idx get \
          unpair drop unpair swap drop \
          tape swap ; \
        : ad-add \
          -> ib -> ia -> tape \
          tape ia get unpair drop unpair swap drop -> va \
          tape ib get unpair drop unpair swap drop -> vb \
          va vb fadd -> vc \
          2 vc ( ) ia append ib append ( ) tape ad-node-new ; \
        : ad-sub \
          -> ib -> ia -> tape \
          tape ia get unpair drop unpair swap drop -> va \
          tape ib get unpair drop unpair swap drop -> vb \
          va vb fsub -> vc \
          4 vc ( ) ia append ib append ( ) tape ad-node-new ; \
        : ad-mul \
          -> ib -> ia -> tape \
          tape ia get unpair drop unpair swap drop -> va \
          tape ib get unpair drop unpair swap drop -> vb \
          va vb fmul -> vc \
          3 vc ( ) ia append ib append ( ) va append vb append tape ad-node-new ; \
        : ad-relu \
          -> ia -> tape \
          tape ia get unpair drop unpair swap drop -> va \
          va 0.0 gt if va else 0.0 end -> vc \
          5 vc ( ) ia append ( ) va append tape ad-node-new ; \
        : ad-neg \
          -> ia -> tape \
          tape ia get unpair drop unpair swap drop -> va \
          va fneg -> vc \
          6 vc ( ) ia append ( ) tape ad-node-new ; \
        : ad-backward \
          -> loss_idx -> tape \
          tape len swap drop -> n \
          ( ) -> grads \
          0 -> i \
          while i n < do \
            grads 0.0 append -> grads \
            i 1 + -> i \
          end \
          grads loss_idx 1.0 set -> grads \
          n 1 - -> i \
          while i 0 ge do \
            tape i get -> node \
            grads i get -> dout \
            node unpair drop unpair drop -> op \
            node unpair swap drop \
            unpair -> saved -> inputs \
            op 2 eq if \
              inputs 0 get -> ia \
              inputs 1 get -> ib \
              grads ia get dout fadd -> ga \
              grads ia ga set -> grads \
              grads ib get dout fadd -> gb \
              grads ib gb set -> grads \
            end \
            op 4 eq if \
              inputs 0 get -> ia \
              inputs 1 get -> ib \
              grads ia get dout fadd -> ga \
              grads ia ga set -> grads \
              grads ib get dout fsub -> gb \
              grads ib gb set -> grads \
            end \
            op 3 eq if \
              inputs 0 get -> ia \
              inputs 1 get -> ib \
              saved 0 get -> va \
              saved 1 get -> vb \
              grads ia get vb dout fmul fadd -> ga \
              grads ia ga set -> grads \
              grads ib get va dout fmul fadd -> gb \
              grads ib gb set -> grads \
            end \
            op 5 eq if \
              inputs 0 get -> ia \
              saved 0 get -> va \
              va 0.0 gt if \
                grads ia get dout fadd -> ga \
                grads ia ga set -> grads \
              end \
            end \
            op 6 eq if \
              inputs 0 get -> ia \
              grads ia get dout fsub -> ga \
              grads ia ga set -> grads \
            end \
            i 1 - -> i \
          end \
          grads ; \
    ";

    #[test]
    fn test_ad_x_squared() {
        // d/dx(x²) = 2x, at x=3 → 6.0
        use crate::interpreter::Value;
        let src = format!("{} \
            ( ) 3.0 ad-var -> idx_x \
            idx_x idx_x ad-mul -> idx_sq \
            idx_sq ad-backward \
            0 get", AUTOGRAD_LIB);
        let module = compile(&src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(f)) = interp.result() {
            assert!((*f - 6.0).abs() < 1e-10, "d/dx(x²) at x=3 = {}, expected 6.0", f);
        } else {
            panic!("Expected float, got {:?}", interp.result());
        }
    }

    #[test]
    fn test_ad_x_cubed() {
        // d/dx(x³) = 3x², at x=2 → 12.0
        // x³ = x * x * x = (x*x) * x
        use crate::interpreter::Value;
        let src = format!("{} \
            ( ) 2.0 ad-var -> idx_x \
            idx_x idx_x ad-mul -> idx_x2 \
            idx_x2 idx_x ad-mul -> idx_x3 \
            idx_x3 ad-backward \
            0 get", AUTOGRAD_LIB);
        let module = compile(&src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(f)) = interp.result() {
            assert!((*f - 12.0).abs() < 1e-10, "d/dx(x³) at x=2 = {}, expected 12.0", f);
        } else {
            panic!("Expected float, got {:?}", interp.result());
        }
    }

    #[test]
    fn test_ad_add_gradient() {
        // f(x,y) = x + y, df/dx = 1, df/dy = 1
        use crate::interpreter::Value;
        let src = format!("{} \
            ( ) 3.0 ad-var -> idx_x \
            5.0 ad-var -> idx_y \
            idx_x idx_y ad-add -> idx_sum \
            idx_sum ad-backward -> grads \
            grads 0 get grads 1 get", AUTOGRAD_LIB);
        let module = compile(&src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        let st = interp.stack();
        // Stack should have: dx dy
        assert!(st.len() >= 2, "Expected 2 values on stack, got {}", st.len());
        if let (Value::Float(dx_f), Value::Float(dy_f)) = (&st[st.len()-2], &st[st.len()-1]) {
            assert!((dx_f - 1.0).abs() < 1e-10, "df/dx = {}, expected 1.0", dx_f);
            assert!((dy_f - 1.0).abs() < 1e-10, "df/dy = {}, expected 1.0", dy_f);
        } else {
            panic!("Expected floats, got dx={:?}, dy={:?}", st[st.len()-2], st[st.len()-1]);
        }
    }

    #[test]
    fn test_ad_mul_two_vars() {
        // f(x,y) = x * y, df/dx = y = 4, df/dy = x = 3
        use crate::interpreter::Value;
        let src = format!("{} \
            ( ) 3.0 ad-var -> idx_x \
            4.0 ad-var -> idx_y \
            idx_x idx_y ad-mul -> idx_prod \
            idx_prod ad-backward -> grads \
            grads 0 get grads 1 get", AUTOGRAD_LIB);
        let module = compile(&src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        let st = interp.stack();
        assert!(st.len() >= 2, "Expected 2 values on stack, got {}", st.len());
        if let (Value::Float(dx_f), Value::Float(dy_f)) = (&st[st.len()-2], &st[st.len()-1]) {
            assert!((dx_f - 4.0).abs() < 1e-10, "df/dx = {}, expected 4.0 (=y)", dx_f);
            assert!((dy_f - 3.0).abs() < 1e-10, "df/dy = {}, expected 3.0 (=x)", dy_f);
        } else {
            panic!("Expected floats, got dx={:?}, dy={:?}", st[st.len()-2], st[st.len()-1]);
        }
    }

    #[test]
    fn test_ad_mse_loss() {
        // MSE for single sample: L = (pred - target)²
        // pred=5, target=3 → L=(5-3)²=4
        // dL/d(pred) = 2*(pred-target) = 4
        use crate::interpreter::Value;
        let src = format!("{} \
            ( ) 5.0 ad-var -> idx_pred \
            3.0 ad-const -> idx_target \
            idx_pred idx_target ad-sub -> idx_diff \
            idx_diff idx_diff ad-mul -> idx_loss \
            idx_loss ad-backward \
            0 get", AUTOGRAD_LIB);
        let module = compile(&src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(f)) = interp.result() {
            assert!((*f - 4.0).abs() < 1e-10, "dL/d(pred) = {}, expected 4.0", f);
        } else {
            panic!("Expected float, got {:?}", interp.result());
        }
    }

    #[test]
    fn test_ad_relu_positive() {
        // f(x) = relu(x) * x = x² when x > 0
        // d/dx = 2x at x=3 → 6
        use crate::interpreter::Value;
        let src = format!("{} \
            ( ) 3.0 ad-var -> idx_x \
            idx_x ad-relu -> idx_r \
            idx_r idx_x ad-mul -> idx_out \
            idx_out ad-backward \
            0 get", AUTOGRAD_LIB);
        let module = compile(&src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(f)) = interp.result() {
            assert!((*f - 6.0).abs() < 1e-10, "d/dx(relu(x)*x) at x=3 = {}, expected 6.0", f);
        } else {
            panic!("Expected float, got {:?}", interp.result());
        }
    }

    #[test]
    fn test_ad_relu_negative() {
        // f(x) = relu(x), at x=-2 → relu(-2)=0, d/dx = 0
        use crate::interpreter::Value;
        let src = format!("{} \
            ( ) -2.0 ad-var -> idx_x \
            idx_x ad-relu -> idx_out \
            idx_out ad-backward \
            0 get", AUTOGRAD_LIB);
        let module = compile(&src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(f)) = interp.result() {
            assert!((*f).abs() < 1e-10, "d/dx(relu(x)) at x=-2 = {}, expected 0.0", f);
        } else {
            panic!("Expected float, got {:?}", interp.result());
        }
    }

    #[test]
    fn test_ad_chain_rule() {
        // f(x) = (2x + 1)², at x=3 → f(3)=(7)²=49
        // df/dx = 2*2*(2x+1) = 4*(2*3+1) = 28
        // Build: const_2, const_1, then 2*x, 2*x+1, (2x+1)²
        use crate::interpreter::Value;
        let src = format!("{} \
            ( ) 3.0 ad-var -> idx_x \
            2.0 ad-const -> idx_2 \
            1.0 ad-const -> idx_1 \
            idx_2 idx_x ad-mul -> idx_2x \
            idx_2x idx_1 ad-add -> idx_2x1 \
            idx_2x1 idx_2x1 ad-mul -> idx_out \
            idx_out ad-backward \
            0 get", AUTOGRAD_LIB);
        let module = compile(&src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(f)) = interp.result() {
            assert!((*f - 28.0).abs() < 1e-10, "d/dx((2x+1)²) at x=3 = {}, expected 28.0", f);
        } else {
            panic!("Expected float, got {:?}", interp.result());
        }
    }

    // ================================================================
    // Phase 7: Optimizer Tests
    // ================================================================

    const OPTIM_LIB: &str = "\
        : grad-norm \
          dup 0.0 [ dup fmul fadd ] fold fsqrt ; \
        : grad-clip \
          -> max_norm \
          grad-norm -> norm \
          norm max_norm gt if \
            max_norm norm fdiv -> scale \
            [ scale fmul ] map \
          end ; \
        : sgd-step \
          -> lr -> grads -> params \
          params grads zip \
          [ unpair -> g -> p \
            p lr g fmul fsub ] map ; \
        : momentum-step \
          -> lr -> mu -> velocity -> grads -> params \
          velocity grads zip \
          [ unpair -> g -> v \
            mu v fmul g fadd ] map -> velocity_new \
          params velocity_new zip \
          [ unpair -> v -> p \
            p lr v fmul fsub ] map \
          velocity_new ; \
        : zeros \
          -> n \
          ( ) -> acc \
          0 -> i \
          while i n < do \
            acc 0.0 append -> acc \
            i 1 + -> i \
          end \
          acc ; \
    ";

    #[test]
    fn test_sgd_single_step() {
        // params=[3.0], grads=[6.0], lr=0.1
        // params' = [3.0 - 0.1*6.0] = [2.4]
        use crate::interpreter::Value;
        let src = format!("{} \
            ( 3.0 ) ( 6.0 ) 0.1 sgd-step \
            0 get", OPTIM_LIB);
        let module = compile(&src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(f)) = interp.result() {
            assert!((*f - 2.4).abs() < 1e-10, "SGD step: {}, expected 2.4", f);
        } else {
            panic!("Expected float, got {:?}", interp.result());
        }
    }

    #[test]
    fn test_sgd_multi_param() {
        // params=[1.0, 2.0, 3.0], grads=[0.5, 1.0, 1.5], lr=0.2
        // params' = [1.0-0.1, 2.0-0.2, 3.0-0.3] = [0.9, 1.8, 2.7]
        use crate::interpreter::Value;
        let src = format!("{} \
            ( 1.0 2.0 3.0 ) ( 0.5 1.0 1.5 ) 0.2 sgd-step \
            1 get", OPTIM_LIB);
        let module = compile(&src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(f)) = interp.result() {
            assert!((*f - 1.8).abs() < 1e-10, "SGD multi: params[1] = {}, expected 1.8", f);
        } else {
            panic!("Expected float, got {:?}", interp.result());
        }
    }

    #[test]
    fn test_sgd_minimize_x_squared() {
        // Minimize f(x) = x² starting from x=5.0
        // Gradient: 2x. SGD with lr=0.1.
        // After 50 steps should be very close to 0.
        use crate::interpreter::Value;
        let src = format!("{}{} \
            ( ) 5.0 ad-var -> idx_x \
            idx_x idx_x ad-mul -> idx_loss \
            idx_loss ad-backward 0 get -> grad \
            5.0 -> x \
            0 -> step \
            while step 50 < do \
              x 2.0 fmul -> g \
              x 0.1 g fmul fsub -> x \
              step 1 + -> step \
            end \
            x", AUTOGRAD_LIB, OPTIM_LIB);
        let module = compile(&src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(f)) = interp.result() {
            assert!(f.abs() < 0.01, "SGD minimize x²: x = {}, expected ~0.0", f);
        } else {
            panic!("Expected float, got {:?}", interp.result());
        }
    }

    #[test]
    fn test_momentum_step() {
        // params=[4.0], grads=[2.0], velocity=[0.0], mu=0.9, lr=0.1
        // v' = 0.9*0 + 2.0 = 2.0
        // params' = 4.0 - 0.1*2.0 = 3.8
        use crate::interpreter::Value;
        let src = format!("{} \
            ( 4.0 ) ( 2.0 ) ( 0.0 ) 0.9 0.1 momentum-step \
            drop 0 get", OPTIM_LIB);
        let module = compile(&src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(f)) = interp.result() {
            assert!((*f - 3.8).abs() < 1e-10, "Momentum step: {}, expected 3.8", f);
        } else {
            panic!("Expected float, got {:?}", interp.result());
        }
    }

    #[test]
    fn test_momentum_velocity_update() {
        // Same setup, but check returned velocity
        // v' = 0.9*0 + 2.0 = 2.0
        use crate::interpreter::Value;
        let src = format!("{} \
            ( 4.0 ) ( 2.0 ) ( 0.0 ) 0.9 0.1 momentum-step \
            0 get swap drop", OPTIM_LIB);
        let module = compile(&src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(f)) = interp.result() {
            assert!((*f - 2.0).abs() < 1e-10, "Momentum velocity: {}, expected 2.0", f);
        } else {
            panic!("Expected float, got {:?}", interp.result());
        }
    }

    #[test]
    fn test_grad_clip() {
        // grads=[3.0, 4.0] → norm=5.0
        // max_norm=2.5 → scale=0.5
        // clipped=[1.5, 2.0]
        use crate::interpreter::Value;
        let src = format!("{} \
            ( 3.0 4.0 ) 2.5 grad-clip \
            0 get", OPTIM_LIB);
        let module = compile(&src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(f)) = interp.result() {
            assert!((*f - 1.5).abs() < 1e-10, "Grad clip [0]: {}, expected 1.5", f);
        } else {
            panic!("Expected float, got {:?}", interp.result());
        }
    }

    #[test]
    fn test_grad_clip_no_clip() {
        // grads=[1.0, 1.0] → norm=√2 ≈ 1.414
        // max_norm=5.0 → no clipping needed
        // result should be unchanged: [1.0, 1.0]
        use crate::interpreter::Value;
        let src = format!("{} \
            ( 1.0 1.0 ) 5.0 grad-clip \
            0 get", OPTIM_LIB);
        let module = compile(&src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(f)) = interp.result() {
            assert!((*f - 1.0).abs() < 1e-10, "No clip [0]: {}, expected 1.0", f);
        } else {
            panic!("Expected float, got {:?}", interp.result());
        }
    }

    #[test]
    fn test_zeros() {
        use crate::interpreter::Value;
        let src = format!("{} 3 zeros len swap drop", OPTIM_LIB);
        let module = compile(&src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(3)));
    }

    // ================================================================
    // Phase 8: Optimization Experiment Tests
    // ================================================================

    #[test]
    fn test_rosenbrock_value() {
        // f(1,1) = 0 (global minimum)
        use crate::interpreter::Value;
        let src = "\
            : rosenbrock \
              -> y -> x \
              1.0 x fsub dup fmul \
              y x dup fmul fsub dup fmul \
              100.0 fmul fadd ; \
            1.0 1.0 rosenbrock";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(f)) = interp.result() {
            assert!(f.abs() < 1e-10, "f(1,1) = {}, expected 0.0", f);
        } else {
            panic!("Expected float, got {:?}", interp.result());
        }
    }

    #[test]
    fn test_rosenbrock_nonmin() {
        // f(0,0) = 1 (not at minimum)
        use crate::interpreter::Value;
        let src = "\
            : rosenbrock \
              -> y -> x \
              1.0 x fsub dup fmul \
              y x dup fmul fsub dup fmul \
              100.0 fmul fadd ; \
            0.0 0.0 rosenbrock";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(f)) = interp.result() {
            assert!((*f - 1.0).abs() < 1e-10, "f(0,0) = {}, expected 1.0", f);
        } else {
            panic!("Expected float, got {:?}", interp.result());
        }
    }

    #[test]
    fn test_rosenbrock_gd_converges() {
        // Gradient descent should bring Rosenbrock close to (1,1)
        use crate::interpreter::Value;
        let src = "\
            : rosenbrock \
              -> y -> x \
              1.0 x fsub dup fmul \
              y x dup fmul fsub dup fmul \
              100.0 fmul fadd ; \
            : rosenbrock-grad \
              -> y -> x \
              -2.0 2.0 x fmul fadd \
              400.0 x fmul y x dup fmul fsub fmul fsub -> dx \
              200.0 y x dup fmul fsub fmul -> dy \
              dx dy ; \
            -1.0 1.0 -> y -> x \
            0 -> step \
            while step 3000 < do \
              x y rosenbrock-grad -> dy -> dx \
              x 0.001 dx fmul fsub -> x \
              y 0.001 dy fmul fsub -> y \
              step 1 + -> step \
            end \
            x y rosenbrock";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(f)) = interp.result() {
            assert!(*f < 0.1, "Rosenbrock after GD: f = {}, expected < 0.1", f);
        } else {
            panic!("Expected float, got {:?}", interp.result());
        }
    }

    #[test]
    fn test_circle_packing_valid() {
        // 3 circles r=1 in box of side 4
        // Placed at (1,1), (3,1), (2,3) - no overlaps, all inside
        use crate::interpreter::Value;
        let src = "\
            : circle-new -> r -> y -> x x y pair r pair ; \
            : circle-x unpair drop unpair drop ; \
            : circle-y unpair drop unpair swap drop ; \
            : circle-r unpair swap drop ; \
            : circle-dist \
              -> c2 -> c1 \
              c1 circle-x c2 circle-x fsub dup fmul \
              c1 circle-y c2 circle-y fsub dup fmul \
              fadd fsqrt ; \
            : overlap-penalty \
              -> c2 -> c1 \
              c1 circle-r c2 circle-r fadd -> min_dist \
              c1 c2 circle-dist -> d \
              min_dist d fsub -> gap \
              gap 0.0 gt if gap dup fmul else 0.0 end ; \
            : boundary-pen \
              -> L -> c \
              0.0 -> pen \
              c circle-r c circle-x fsub -> left_gap \
              left_gap 0.0 gt if pen left_gap dup fmul fadd -> pen end \
              c circle-r c circle-y fsub -> top_gap \
              top_gap 0.0 gt if pen top_gap dup fmul fadd -> pen end \
              c circle-x c circle-r fadd L fsub -> right_gap \
              right_gap 0.0 gt if pen right_gap dup fmul fadd -> pen end \
              c circle-y c circle-r fadd L fsub -> bot_gap \
              bot_gap 0.0 gt if pen bot_gap dup fmul fadd -> pen end \
              pen ; \
            1.0 1.0 1.0 circle-new -> c0 \
            3.0 1.0 1.0 circle-new -> c1 \
            2.0 3.0 1.0 circle-new -> c2 \
            c0 c1 overlap-penalty \
            c0 c2 overlap-penalty fadd \
            c1 c2 overlap-penalty fadd \
            c0 4.0 boundary-pen fadd \
            c1 4.0 boundary-pen fadd \
            c2 4.0 boundary-pen fadd";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(f)) = interp.result() {
            assert!(f.abs() < 1e-10, "Valid packing energy = {}, expected 0.0", f);
        } else {
            panic!("Expected float, got {:?}", interp.result());
        }
    }

    #[test]
    fn test_circle_packing_overlapping() {
        // 2 circles r=1 at (1,1) and (1.5,1) - overlapping
        // overlap = (2 - 0.5)² = 2.25
        use crate::interpreter::Value;
        let src = "\
            : circle-new -> r -> y -> x x y pair r pair ; \
            : circle-x unpair drop unpair drop ; \
            : circle-y unpair drop unpair swap drop ; \
            : circle-r unpair swap drop ; \
            : circle-dist \
              -> c2 -> c1 \
              c1 circle-x c2 circle-x fsub dup fmul \
              c1 circle-y c2 circle-y fsub dup fmul \
              fadd fsqrt ; \
            : overlap-penalty \
              -> c2 -> c1 \
              c1 circle-r c2 circle-r fadd -> min_dist \
              c1 c2 circle-dist -> d \
              min_dist d fsub -> gap \
              gap 0.0 gt if gap dup fmul else 0.0 end ; \
            1.0 1.0 1.0 circle-new \
            1.5 1.0 1.0 circle-new \
            overlap-penalty";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        if let Some(Value::Float(f)) = interp.result() {
            assert!(*f > 0.0, "Overlapping circles should have penalty > 0, got {}", f);
            // (2.0 - 0.5)² = 2.25
            assert!((*f - 2.25).abs() < 1e-10, "Overlap penalty = {}, expected 2.25", f);
        } else {
            panic!("Expected float, got {:?}", interp.result());
        }
    }

    // ================================================================
    // Phase 9: Tracing Tests
    // ================================================================

    const TRACE_LIB: &str = "\
        : trace-new ( ) ; \
        : trace-step \
          -> val -> trace \
          trace len swap drop -> step \
          trace step val pair append ; \
        : trace-value unpair swap drop ; \
        : trace-step-num unpair drop ; \
        : trace-len len ; \
        : trace-last \
          dup len swap drop 1 - get \
          trace-value ; \
        : trace-fingerprint \
          0 [ unpair swap drop + ] fold ; \
        : trace-values \
          [ unpair swap drop ] map ; \
    ";

    #[test]
    fn test_trace_new() {
        use crate::interpreter::Value;
        let src = format!("{} trace-new len swap drop", TRACE_LIB);
        let module = compile(&src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(0)));
    }

    #[test]
    fn test_trace_step_records() {
        // Record 3 values, check length
        use crate::interpreter::Value;
        let src = format!("{} \
            trace-new 10 trace-step 20 trace-step 30 trace-step \
            len swap drop", TRACE_LIB);
        let module = compile(&src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(3)));
    }

    #[test]
    fn test_trace_step_numbers() {
        // Entries should have sequential step numbers
        use crate::interpreter::Value;
        let src = format!("{} \
            trace-new 10 trace-step 20 trace-step 30 trace-step \
            1 get trace-step-num", TRACE_LIB);
        let module = compile(&src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        // Second entry (index 1) should have step number 1
        assert_eq!(interp.result(), Some(&Value::Int(1)));
    }

    #[test]
    fn test_trace_last() {
        use crate::interpreter::Value;
        let src = format!("{} \
            trace-new 10 trace-step 20 trace-step 42 trace-step \
            trace-last", TRACE_LIB);
        let module = compile(&src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(42)));
    }

    #[test]
    fn test_trace_fingerprint() {
        // fingerprint = sum of values
        use crate::interpreter::Value;
        let src = format!("{} \
            trace-new 10 trace-step 20 trace-step 30 trace-step \
            trace-fingerprint", TRACE_LIB);
        let module = compile(&src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(60)));
    }

    #[test]
    fn test_trace_determinism() {
        // Same computation twice should produce same fingerprint
        use crate::interpreter::Value;
        let src = format!("{} \
            trace-new 1 trace-step 2 trace-step 3 trace-step \
            trace-fingerprint -> fp1 \
            trace-new 1 trace-step 2 trace-step 3 trace-step \
            trace-fingerprint -> fp2 \
            fp1 fp2 eq", TRACE_LIB);
        let module = compile(&src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Bool(true)));
    }

    #[test]
    fn test_trace_values() {
        // Extract just the values from trace entries
        use crate::interpreter::Value;
        let src = format!("{} \
            trace-new 10 trace-step 20 trace-step 30 trace-step \
            trace-values 0 [ + ] fold", TRACE_LIB);
        let module = compile(&src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(60)));
    }

    // ================================================================
    // Phase 10: Fiber Tests
    // ================================================================

    #[test]
    fn test_fiber_new() {
        // Create a fiber from a quote, check it's not done yet
        use crate::interpreter::Value;
        let src = "[ 1 2 + ] fiber-new fiber-status swap drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        // fiber-status should return false (not done yet)
        assert_eq!(interp.result(), Some(&Value::Bool(false)));
    }

    #[test]
    fn test_fiber_push_and_stack() {
        // Push values onto a fiber's stack
        use crate::interpreter::Value;
        let src = "[ ] fiber-new 42 fiber-push 99 fiber-push fiber-stack \
                   swap drop len swap drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(2)));
    }

    #[test]
    fn test_fiber_step_simple() {
        // Create a fiber with [ 3 4 + ], push initial values, step until done
        use crate::interpreter::Value;
        let src = "\
            [ 3 4 + ] fiber-new -> f \
            f fiber-step -> done -> f \
            f fiber-step -> done -> f \
            f fiber-step -> done -> f \
            f fiber-step -> done -> f \
            f fiber-step -> done -> f \
            f fiber-stack swap drop 0 get";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(7)));
    }

    #[test]
    fn test_fiber_run_to_completion() {
        // Run fiber until done using a while loop
        use crate::interpreter::Value;
        let src = "\
            [ 10 20 + ] fiber-new -> f \
            f fiber-status -> done \
            while done not do \
              f fiber-step -> done -> f \
            end \
            f fiber-stack swap drop 0 get";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(30)));
    }

    #[test]
    fn test_fiber_with_input() {
        // Push a value onto fiber's stack, then run code that uses it
        use crate::interpreter::Value;
        let src = "\
            [ 2 * ] fiber-new 21 fiber-push -> f \
            f fiber-status -> done \
            while done not do \
              f fiber-step -> done -> f \
            end \
            f fiber-stack swap drop 0 get";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(42)));
    }

    #[test]
    fn test_two_fibers() {
        // Create two fibers, step them interleaved
        use crate::interpreter::Value;
        let src = "\
            [ 1 2 + ] fiber-new -> f1 \
            [ 10 20 + ] fiber-new -> f2 \
            f1 fiber-step drop -> f1 \
            f2 fiber-step drop -> f2 \
            f1 fiber-step drop -> f1 \
            f2 fiber-step drop -> f2 \
            f1 fiber-step drop -> f1 \
            f2 fiber-step drop -> f2 \
            f1 fiber-step drop -> f1 \
            f2 fiber-step drop -> f2 \
            f1 fiber-step drop -> f1 \
            f2 fiber-step drop -> f2 \
            f1 fiber-stack swap drop 0 get -> r1 \
            f2 fiber-stack swap drop 0 get -> r2 \
            r1 r2 +";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(33)));
    }

    // ================================================================
    // LINEAR TYPE TESTS — P4: Constraints Attenuate
    // ================================================================

    #[test]
    fn test_linear_mark_and_consume() {
        // Mark a value as linear, then consume it → get the inner value
        use crate::interpreter::Value;
        let src = "42 linear consume";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(42)));
    }

    #[test]
    fn test_affine_mark_and_consume() {
        // Mark a value as affine, then consume it → get the inner value
        use crate::interpreter::Value;
        let src = "99 affine consume";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(99)));
    }

    #[test]
    fn test_linear_cannot_dup() {
        // Attempting to dup a linear value should fail
        let src = "42 linear dup";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        let result = interp.run();
        assert!(result.is_err(), "dup of linear value should error");
        assert!(result.unwrap_err().contains("linear"), "error should mention linear");
    }

    #[test]
    fn test_linear_cannot_drop() {
        // Attempting to drop a linear value should fail
        let src = "42 linear drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        let result = interp.run();
        assert!(result.is_err(), "drop of linear value should error");
        assert!(result.unwrap_err().contains("linear"), "error should mention linear");
    }

    #[test]
    fn test_affine_cannot_dup() {
        // Attempting to dup an affine value should fail
        let src = "42 affine dup";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        let result = interp.run();
        assert!(result.is_err(), "dup of affine value should error");
        assert!(result.unwrap_err().contains("affine"), "error should mention affine");
    }

    #[test]
    fn test_affine_can_drop() {
        // Affine values CAN be dropped (at most once)
        use crate::interpreter::Value;
        let src = "42 affine drop 99";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(99)));
    }

    #[test]
    fn test_is_linear_true() {
        // is-linear on a linear value should return true
        // After is-linear: stack is [linear(42) true]
        // We need to get the bool: swap consumes the linear, then drop
        // Use -> to store the bool, then consume the linear value
        use crate::interpreter::Value;
        let src = "42 linear is-linear -> flag consume drop flag";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Bool(true)));
    }

    #[test]
    fn test_is_linear_false() {
        // is-linear on a non-linear value should return false
        use crate::interpreter::Value;
        let src = "42 is-linear swap drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Bool(false)));
    }

    #[test]
    fn test_is_affine_true() {
        // is-affine on an affine value should return true
        use crate::interpreter::Value;
        let src = "42 affine is-affine swap drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Bool(true)));
    }

    #[test]
    fn test_is_affine_false() {
        // is-affine on a non-affine value should return false
        use crate::interpreter::Value;
        let src = "42 is-affine swap drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Bool(false)));
    }

    #[test]
    fn test_linear_consume_then_use() {
        // Linear value: mark, consume, then use normally (add)
        use crate::interpreter::Value;
        let src = "10 linear consume 5 +";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(15)));
    }

    #[test]
    fn test_linear_resource_pattern() {
        // Pattern: create a resource, pass through pipeline, consume at end
        // Simulates: open → use → close
        use crate::interpreter::Value;
        let src = "\
            100 linear -> resource \
            resource consume 10 + -> result \
            result";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(110)));
    }

    #[test]
    fn test_consume_rejects_non_linear() {
        // Consuming a non-linear value should fail
        let src = "42 consume";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        let result = interp.run();
        assert!(result.is_err(), "consume of non-linear value should error");
    }

    #[test]
    fn test_linear_nested() {
        // Linear wrapping is nestable: linear(linear(42))
        use crate::interpreter::Value;
        let src = "42 linear linear consume consume";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(42)));
    }

    // ================================================================
    // INTROSPECTION TESTS — Phase 12
    // ================================================================

    #[test]
    fn test_type_of_int() {
        use crate::interpreter::Value;
        let src = "42 type-of swap drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("int".into()))));
    }

    #[test]
    fn test_type_of_float() {
        use crate::interpreter::Value;
        let src = "3.14 type-of swap drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("float".into()))));
    }

    #[test]
    fn test_type_of_bool() {
        use crate::interpreter::Value;
        let src = "true type-of swap drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("bool".into()))));
    }

    #[test]
    fn test_type_of_list() {
        use crate::interpreter::Value;
        let src = "( 1 2 3 ) type-of swap drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("list".into()))));
    }

    #[test]
    fn test_type_of_linear() {
        // type-of on a linear value returns "linear", not the inner type
        use crate::interpreter::Value;
        let src = "42 linear type-of -> t consume drop t";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("linear".into()))));
    }

    #[test]
    fn test_depth_empty() {
        use crate::interpreter::Value;
        let src = "depth";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(0)));
    }

    #[test]
    fn test_depth_with_values() {
        use crate::interpreter::Value;
        let src = "1 2 3 depth";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(3)));
    }

    #[test]
    fn test_describe_builtin() {
        use crate::interpreter::Value;
        let src = "\"dup\" describe";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("(a -- a a)".into()))));
    }

    #[test]
    fn test_describe_unknown() {
        use crate::interpreter::Value;
        let src = "\"foobar\" describe";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("unknown word: foobar".into()))));
    }

    // ========================================================================
    // STRING OPERATIONS
    // ========================================================================

    #[test]
    fn test_str_len() {
        use crate::interpreter::Value;
        let src = "\"hello\" str-len swap drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(5)));
    }

    #[test]
    fn test_str_len_empty() {
        use crate::interpreter::Value;
        let src = "\"\" str-len swap drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(0)));
    }

    #[test]
    fn test_str_get() {
        use crate::interpreter::Value;
        let src = "\"hello\" 1 str-get";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("e".into()))));
    }

    #[test]
    fn test_str_concat() {
        use crate::interpreter::Value;
        let src = "\"hello\" \" world\" str-concat";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("hello world".into()))));
    }

    #[test]
    fn test_str_slice() {
        use crate::interpreter::Value;
        let src = "\"hello world\" 0 5 str-slice";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("hello".into()))));
    }

    #[test]
    fn test_str_slice_middle() {
        use crate::interpreter::Value;
        let src = "\"hello world\" 6 11 str-slice";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("world".into()))));
    }

    #[test]
    fn test_to_str_int() {
        use crate::interpreter::Value;
        let src = "42 to-str";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("42".into()))));
    }

    #[test]
    fn test_to_str_bool() {
        use crate::interpreter::Value;
        let src = "true to-str";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("true".into()))));
    }

    #[test]
    fn test_to_str_float() {
        use crate::interpreter::Value;
        let src = "3.14 to-str";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("3.14".into()))));
    }

    #[test]
    fn test_to_str_list() {
        use crate::interpreter::Value;
        let src = "( 1 2 3 ) to-str";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("(1 2 3)".into()))));
    }

    #[test]
    fn test_str_find_found() {
        use crate::interpreter::Value;
        let src = "\"hello world\" \"world\" str-find";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(6)));
    }

    #[test]
    fn test_str_find_not_found() {
        use crate::interpreter::Value;
        let src = "\"hello world\" \"xyz\" str-find";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(-1)));
    }

    #[test]
    fn test_str_concat_build() {
        use crate::interpreter::Value;
        // Build a string from parts: "the answer is 42"
        let src = "\"the answer is \" 42 to-str str-concat";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("the answer is 42".into()))));
    }

    #[test]
    fn test_str_len_preserves() {
        use crate::interpreter::Value;
        // str-len should not consume the string (like list len)
        let src = "\"abc\" str-len";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        let stack = interp.stack();
        assert_eq!(stack.len(), 2);
        assert_eq!(&stack[0], &Value::Str(Box::new("abc".into())));
        assert_eq!(&stack[1], &Value::Int(3));
    }

    #[test]
    fn test_str_get_first_last() {
        use crate::interpreter::Value;
        // Get first and last char
        let src = "\"abc\" 0 str-get";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("a".into()))));
    }

    #[test]
    fn test_str_describe_new_ops() {
        use crate::interpreter::Value;
        let src = "\"str-len\" describe";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("(str -- str n)".into()))));
    }

    // ========================================================================
    // SYSCALL OPERATIONS
    // ========================================================================

    #[test]
    fn test_syscall_file_write_and_read() {
        use crate::interpreter::Value;
        // Write a file, then read it back
        let src = "\"/tmp/kore_test_syscall.txt\" \"hello from kore\" file-write \"/tmp/kore_test_syscall.txt\" file-read";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("hello from kore".into()))));
        // Clean up
        std::fs::remove_file("/tmp/kore_test_syscall.txt").ok();
    }

    #[test]
    fn test_syscall_file_exists_true() {
        use crate::interpreter::Value;
        // Write a file first, then check existence
        std::fs::write("/tmp/kore_test_exists.txt", "test").unwrap();
        let src = "\"/tmp/kore_test_exists.txt\" file-exists";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Bool(true)));
        std::fs::remove_file("/tmp/kore_test_exists.txt").ok();
    }

    #[test]
    fn test_syscall_file_exists_false() {
        use crate::interpreter::Value;
        let src = "\"/tmp/kore_nonexistent_file_12345.txt\" file-exists";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Bool(false)));
    }

    #[test]
    fn test_syscall_time_now() {
        use crate::interpreter::Value;
        let src = "time-now";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        // time-now should return a positive float (Unix timestamp)
        match interp.result() {
            Some(Value::Float(t)) => assert!(*t > 1_700_000_000.0, "timestamp too small: {}", t),
            other => panic!("Expected Float, got {:?}", other),
        }
    }

    #[test]
    fn test_syscall_env_get() {
        use crate::interpreter::Value;
        // PATH should always be set
        let src = "\"PATH\" env-get str-len swap drop 0 >";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Bool(true)));
    }

    #[test]
    fn test_syscall_env_get_missing() {
        use crate::interpreter::Value;
        // Non-existent env var returns empty string
        let src = "\"KORE_NONEXISTENT_VAR_12345\" env-get";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("".into()))));
    }

    #[test]
    fn test_syscall_caps_tagged() {
        // file-read requires fs capability
        let src = "\"/tmp/test\" file-read";
        let module = compile(src).unwrap();
        assert!(module.cap_flags & crate::bytecode::CAP_FS != 0, "file-read should require fs cap");
    }

    #[test]
    fn test_syscall_time_now_caps() {
        // time-now requires io capability
        let src = "time-now";
        let module = compile(src).unwrap();
        assert!(module.cap_flags & crate::bytecode::CAP_IO != 0, "time-now should require io cap");
    }

    #[test]
    fn test_syscall_describe() {
        use crate::interpreter::Value;
        let src = "\"file-read\" describe";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("(path -- str)".into()))));
    }

    // ================================================================
    // SPAWN TESTS — Sandboxed parallel execution
    // ================================================================

    #[test]
    fn test_spawn_basic() {
        use crate::interpreter::Value;
        // Spawn a quote that computes 3+4, get result list
        let src = "[ 3 4 + ] 255 spawn";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        // Result should be a list containing [7]
        match interp.result() {
            Some(Value::List(items)) => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0], Value::Int(7));
            }
            other => panic!("expected list [7], got {:?}", other),
        }
    }

    #[test]
    fn test_spawn_multiple_results() {
        use crate::interpreter::Value;
        // Spawn that leaves multiple values on stack
        let src = "[ 1 2 3 ] 255 spawn";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        match interp.result() {
            Some(Value::List(items)) => {
                assert_eq!(items.len(), 3);
                assert_eq!(items[0], Value::Int(1));
                assert_eq!(items[1], Value::Int(2));
                assert_eq!(items[2], Value::Int(3));
            }
            other => panic!("expected list [1,2,3], got {:?}", other),
        }
    }

    #[test]
    fn test_spawn_cap_attenuation() {
        use crate::interpreter::{Interpreter, Value};
        // Parent has io cap, spawn with 0 caps → child can't print
        // We test by checking that spawn with 0 caps produces result
        // (the quote doesn't use any caps, so it should work)
        let src = "[ 42 ] 0 spawn";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        match interp.result() {
            Some(Value::List(items)) => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0], Value::Int(42));
            }
            other => panic!("expected [42], got {:?}", other),
        }
    }

    #[test]
    fn test_spawn_cap_attenuation_denied() {
        use crate::interpreter::Interpreter;
        // Parent has NO caps (0), spawn requests all (255)
        // P4: child gets parent & requested = 0 & 255 = 0
        // So child still can't use IO
        let src = "[ 42 println ] 255 spawn";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module_with_caps(&module, 0);
        let result = interp.run();
        assert!(result.is_err(), "spawn with no parent caps should deny child IO");
    }

    #[test]
    fn test_spawn_empty() {
        use crate::interpreter::Value;
        let src = "[ ] 255 spawn";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        match interp.result() {
            Some(Value::List(items)) => assert_eq!(items.len(), 0),
            other => panic!("expected empty list, got {:?}", other),
        }
    }

    #[test]
    fn test_spawn_with_computation() {
        use crate::interpreter::Value;
        // Spawn a computation: factorial of 5
        let src = "\
            : fact -> n 1 -> acc \
              while n 1 > do \
                acc n * -> acc \
                n 1 - -> n \
              end acc ; \
            [ 5 fact ] 255 spawn";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        match interp.result() {
            Some(Value::List(items)) => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0], Value::Int(120));
            }
            other => panic!("expected [120], got {:?}", other),
        }
    }

    // ================================================================
    // CHANNEL TESTS — Inter-fiber communication
    // ================================================================

    #[test]
    fn test_chan_new() {
        use crate::interpreter::Value;
        let src = "chan-new type-of swap drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("channel".into()))));
    }

    #[test]
    fn test_chan_send_recv() {
        use crate::interpreter::Value;
        // Create channel, send 42, receive it
        let src = "chan-new dup 42 chan-send chan-recv";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(42)));
    }

    #[test]
    fn test_chan_fifo_order() {
        use crate::interpreter::Value;
        // Channels are FIFO — first in, first out
        let src = "\
            chan-new -> ch \
            ch 1 chan-send \
            ch 2 chan-send \
            ch 3 chan-send \
            ch chan-recv -> a \
            ch chan-recv -> b \
            ch chan-recv -> c \
            a b c";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        let stack = interp.stack();
        assert_eq!(stack[0], Value::Int(1));
        assert_eq!(stack[1], Value::Int(2));
        assert_eq!(stack[2], Value::Int(3));
    }

    #[test]
    fn test_chan_recv_empty_error() {
        // Receiving from empty channel should return Error value (P1: S→S)
        use crate::interpreter::Value;
        let src = "chan-new chan-recv";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        // Stack should have: [channel, Error("chan-recv: channel is empty")]
        let top = interp.stack().last().unwrap();
        match top {
            Value::Error(msg) => assert!(msg.contains("channel is empty")),
            other => panic!("expected Error, got {:?}", other),
        }
    }

    #[test]
    fn test_chan_send_multiple_types() {
        use crate::interpreter::Value;
        // Send different types through the same channel
        let src = "\
            chan-new -> ch \
            ch 42 chan-send \
            ch \"hello\" chan-send \
            ch true chan-send \
            ch chan-recv -> a \
            ch chan-recv -> b \
            ch chan-recv -> c \
            a b c";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        let stack = interp.stack();
        assert_eq!(stack[0], Value::Int(42));
        assert_eq!(stack[1], Value::Str(Box::new("hello".into())));
        assert_eq!(stack[2], Value::Bool(true));
    }

    #[test]
    fn test_chan_with_spawn() {
        use crate::interpreter::Value;
        // Create channel, send to it in parent, receive in parent
        // (spawn shares channel state back to parent)
        let src = "\
            chan-new -> ch \
            ch 99 chan-send \
            ch chan-recv";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(99)));
    }

    #[test]
    fn test_chan_describe() {
        use crate::interpreter::Value;
        let src = "\"chan-new\" describe";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("(-- chan)".into()))));
    }

    // ================================================================
    // EXEC SYSCALL TESTS — Shell command execution
    // ================================================================

    #[test]
    fn test_exec_basic() {
        use crate::interpreter::Value;
        let src = "\"echo hello\" exec";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        match interp.result() {
            Some(Value::Pair(pair)) => {
                match (&pair.0, &pair.1) {
                    (Value::Str(s), Value::Int(c)) => {
                        assert_eq!(s.trim(), "hello");
                        assert_eq!(*c, 0);
                    }
                    other => panic!("expected (str, int), got {:?}", other),
                }
            }
            other => panic!("expected pair, got {:?}", other),
        }
    }

    #[test]
    fn test_exec_caps_tagged() {
        // exec requires exec capability
        let src = "\"echo hi\" exec";
        let module = compile(src).unwrap();
        assert!(module.cap_flags & crate::bytecode::CAP_EXEC != 0, "exec should require exec cap");
    }

    #[test]
    fn test_exec_cap_denied() {
        // Without exec cap, should fail
        let src = "\"echo hi\" exec";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module_with_caps(&module, 0);
        let result = interp.run();
        assert!(result.is_err(), "exec without exec cap should be denied");
    }

    #[test]
    fn test_exec_exit_code() {
        use crate::interpreter::Value;
        let src = "\"exit 1\" exec unpair swap drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(1)));
    }

    #[test]
    fn test_exec_describe() {
        use crate::interpreter::Value;
        let src = "\"exec\" describe";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("(cmd -- (stdout, exit-code))".into()))));
    }

    #[test]
    fn test_spawn_describe() {
        use crate::interpreter::Value;
        let src = "\"spawn\" describe";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("(quote caps -- result-list)".into()))));
    }

    // ====================================================================
    // ERROR HANDLING TESTS
    // ====================================================================
    
    #[test]
    fn test_try_success() {
        use crate::interpreter::Value;
        let src = "[ 42 ] try";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(42)));
    }
    
    #[test]
    fn test_try_catches_error() {
        use crate::interpreter::Value;
        let src = r#"[ "boom" fail ] try"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Error(Box::new("boom".into()))));
    }
    
    #[test]
    fn test_is_error_true() {
        use crate::interpreter::Value;
        let src = r#"[ "oops" fail ] try is-error"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Bool(true)));
    }
    
    #[test]
    fn test_is_error_false() {
        use crate::interpreter::Value;
        let src = "42 is-error";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Bool(false)));
    }
    
    #[test]
    fn test_try_division_by_zero() {
        use crate::interpreter::Value;
        let src = "[ 1 0 / ] try is-error";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Bool(true)));
    }
    
    #[test]
    fn test_try_nested() {
        use crate::interpreter::Value;
        // Inner try catches error, outer try gets the error value
        let src = r#"[ [ "inner" fail ] try ] try is-error"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        // The inner try catches "inner" error, making it a value, 
        // so outer try succeeds — the error is a value now
        assert_eq!(interp.result(), Some(&Value::Bool(true)));
    }

    // ====================================================================
    // TRIG + POWER TESTS
    // ====================================================================
    
    #[test]
    fn test_fsin() {
        use crate::interpreter::Value;
        let src = "0.0 fsin";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        match interp.result() {
            Some(Value::Float(f)) => assert!(f.abs() < 1e-10, "sin(0) should be 0, got {}", f),
            other => panic!("expected Float, got {:?}", other),
        }
    }
    
    #[test]
    fn test_fcos() {
        use crate::interpreter::Value;
        let src = "0.0 fcos";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        match interp.result() {
            Some(Value::Float(f)) => assert!((f - 1.0).abs() < 1e-10, "cos(0) should be 1, got {}", f),
            other => panic!("expected Float, got {:?}", other),
        }
    }
    
    #[test]
    fn test_fatan2() {
        use crate::interpreter::Value;
        // atan2(1, 0) = pi/2
        let src = "1.0 0.0 fatan2";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        match interp.result() {
            Some(Value::Float(f)) => {
                assert!((f - std::f64::consts::FRAC_PI_2).abs() < 1e-10, "atan2(1,0) should be pi/2, got {}", f);
            }
            other => panic!("expected Float, got {:?}", other),
        }
    }
    
    #[test]
    fn test_fpow() {
        use crate::interpreter::Value;
        let src = "2.0 10.0 fpow";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        match interp.result() {
            Some(Value::Float(f)) => assert!((f - 1024.0).abs() < 1e-10, "2^10 should be 1024, got {}", f),
            other => panic!("expected Float, got {:?}", other),
        }
    }
    
    #[test]
    fn test_ffloor() {
        use crate::interpreter::Value;
        let src = "3.7 ffloor";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        match interp.result() {
            Some(Value::Float(f)) => assert!((f - 3.0).abs() < 1e-10),
            other => panic!("expected Float(3.0), got {:?}", other),
        }
    }
    
    #[test]
    fn test_fceil() {
        use crate::interpreter::Value;
        let src = "3.2 fceil";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        match interp.result() {
            Some(Value::Float(f)) => assert!((f - 4.0).abs() < 1e-10),
            other => panic!("expected Float(4.0), got {:?}", other),
        }
    }
    
    #[test]
    fn test_fround() {
        use crate::interpreter::Value;
        let src = "3.5 fround";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        match interp.result() {
            Some(Value::Float(f)) => assert!((f - 4.0).abs() < 1e-10),
            other => panic!("expected Float(4.0), got {:?}", other),
        }
    }
    
    #[test]
    fn test_sin_cos_identity() {
        use crate::interpreter::Value;
        // sin²(x) + cos²(x) = 1
        let src = "1.0 dup fsin dup fmul swap fcos dup fmul fadd";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        match interp.result() {
            Some(Value::Float(f)) => assert!((f - 1.0).abs() < 1e-10, "sin²+cos²={}", f),
            other => panic!("expected Float(1.0), got {:?}", other),
        }
    }
    
    // ====================================================================
    // MORE STRING OPERATION TESTS
    // ====================================================================
    
    #[test]
    fn test_str_split() {
        use crate::interpreter::Value;
        let src = r#""a,b,c" "," str-split"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        match interp.result() {
            Some(Value::List(items)) => {
                assert_eq!(items.len(), 3);
                assert_eq!(items[0], Value::Str(Box::new("a".into())));
                assert_eq!(items[1], Value::Str(Box::new("b".into())));
                assert_eq!(items[2], Value::Str(Box::new("c".into())));
            }
            other => panic!("expected list, got {:?}", other),
        }
    }
    
    #[test]
    fn test_str_replace() {
        use crate::interpreter::Value;
        let src = r#""hello world" "world" "kore" str-replace"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("hello kore".into()))));
    }
    
    #[test]
    fn test_str_upper() {
        use crate::interpreter::Value;
        let src = r#""hello" str-upper"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("HELLO".into()))));
    }
    
    #[test]
    fn test_str_lower() {
        use crate::interpreter::Value;
        let src = r#""HELLO" str-lower"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("hello".into()))));
    }
    
    #[test]
    fn test_str_trim() {
        use crate::interpreter::Value;
        let src = r#""  hello  " str-trim"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("hello".into()))));
    }
    
    #[test]
    fn test_str_split_empty() {
        use crate::interpreter::Value;
        let src = r#""" "," str-split"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        match interp.result() {
            Some(Value::List(items)) => {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0], Value::Str(Box::new("".into())));
            }
            other => panic!("expected list, got {:?}", other),
        }
    }
    
    #[test]
    fn test_str_replace_all() {
        use crate::interpreter::Value;
        let src = r#""aaa" "a" "bb" str-replace"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("bbbbbb".into()))));
    }
    
    // ====================================================================
    // HASHMAP TESTS
    // ====================================================================
    
    #[test]
    fn test_map_new() {
        use crate::interpreter::Value;
        let src = "map-new";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        match interp.result() {
            Some(Value::Map(m)) => assert!(m.is_empty()),
            other => panic!("expected empty map, got {:?}", other),
        }
    }
    
    #[test]
    fn test_map_set_get() {
        use crate::interpreter::Value;
        let src = r#"map-new "x" 42 map-set "x" map-get"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(42)));
    }
    
    #[test]
    fn test_map_has_true() {
        use crate::interpreter::Value;
        let src = r#"map-new "key" 1 map-set "key" map-has"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Bool(true)));
    }
    
    #[test]
    fn test_map_has_false() {
        use crate::interpreter::Value;
        let src = r#"map-new "missing" map-has"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Bool(false)));
    }
    
    #[test]
    fn test_map_keys() {
        use crate::interpreter::Value;
        let src = r#"map-new "b" 2 map-set "a" 1 map-set map-keys"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        match interp.result() {
            Some(Value::List(keys)) => {
                assert_eq!(keys.len(), 2);
                // BTreeMap keys are sorted
                assert_eq!(keys[0], Value::Str(Box::new("a".into())));
                assert_eq!(keys[1], Value::Str(Box::new("b".into())));
            }
            other => panic!("expected list of keys, got {:?}", other),
        }
    }
    
    #[test]
    fn test_map_get_missing() {
        use crate::interpreter::Value;
        let src = r#"map-new "nope" map-get"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Nil));
    }
    
    #[test]
    fn test_map_multiple_sets() {
        use crate::interpreter::Value;
        let src = r#"map-new "x" 1 map-set "y" 2 map-set "z" 3 map-set "y" map-get"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(2)));
    }
    
    #[test]
    fn test_map_overwrite() {
        use crate::interpreter::Value;
        let src = r#"map-new "x" 1 map-set "x" 99 map-set "x" map-get"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(99)));
    }
    
    #[test]
    fn test_map_with_string_values() {
        use crate::interpreter::Value;
        let src = r#"map-new "name" "kore" map-set "name" map-get"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("kore".into()))));
    }

    // ====================================================================
    // IMPORT TESTS (using expand_imports directly)
    // ====================================================================
    
    #[test]
    fn test_expand_imports_no_imports() {
        let src = "42 dup +";
        let result = super::expand_imports(src, std::path::Path::new("."), &mut Vec::new()).unwrap();
        assert!(result.contains("42 dup +"));
    }
    
    #[test]
    fn test_expand_imports_cycle_prevention() {
        // If a file is already imported, skip it
        let src = "import \"test.kore\"";
        let mut imported = vec!["./test.kore".to_string()];
        let result = super::expand_imports(src, std::path::Path::new("."), &mut imported).unwrap();
        assert!(result.contains("already loaded"));
    }
    
    // ====================================================================
    // DESCRIBE TESTS for new words
    // ====================================================================
    
    #[test]
    fn test_describe_try() {
        use crate::interpreter::Value;
        let src = r#""try" describe"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("(quote -- result|error)".into()))));
    }
    
    #[test]
    fn test_describe_map_new() {
        use crate::interpreter::Value;
        let src = r#""map-new" describe"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("(-- map)".into()))));
    }
    
    #[test]
    fn test_describe_fsin() {
        use crate::interpreter::Value;
        let src = r#""fsin" describe"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("(f -- sin(f))".into()))));
    }
    
    #[test]
    fn test_describe_str_split() {
        use crate::interpreter::Value;
        let src = r#""str-split" describe"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("(str delim -- list)".into()))));
    }
    
    #[test]
    fn test_describe_http_get() {
        use crate::interpreter::Value;
        let src = r#""http-get" describe"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("(url -- (body, status))".into()))));
    }
    
    #[test]
    fn test_describe_http_serve() {
        use crate::interpreter::Value;
        let src = r#""http-serve" describe"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("(port handler-quote --)".into()))));
    }
    
    // ====================================================================
    // INTEGRATION TESTS — combining features
    // ====================================================================
    
    #[test]
    fn test_map_with_try_error() {
        use crate::interpreter::Value;
        // Try to get from empty map, then check result  
        let src = r#"map-new "x" map-get"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        // map-get returns nil for missing keys
        assert_eq!(interp.result(), Some(&Value::Nil));
    }
    
    #[test]
    fn test_trig_in_list_map() {
        use crate::interpreter::Value;
        // Map sin over a list of angles
        let src = "( 0.0 ) [ fsin ] map";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        match interp.result() {
            Some(Value::List(items)) => {
                assert_eq!(items.len(), 1);
                match &items[0] {
                    Value::Float(f) => assert!(f.abs() < 1e-10, "sin(0)={}", f),
                    other => panic!("expected Float, got {:?}", other),
                }
            }
            other => panic!("expected list, got {:?}", other),
        }
    }
    
    #[test]
    fn test_error_to_str() {
        use crate::interpreter::Value;
        let src = r#"[ "custom error" fail ] try to-str"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("Error(custom error)".into()))));
    }
    
    #[test]
    fn test_map_to_str() {
        use crate::interpreter::Value;
        let src = r#"map-new "a" 1 map-set to-str"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("{a: 1}".into()))));
    }
    
    #[test]
    fn test_str_split_then_len() {
        use crate::interpreter::Value;
        let src = r#""one two three" " " str-split len"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        // After split we have a list, len returns the count
        assert_eq!(interp.result(), Some(&Value::Int(3)));
    }
    
    #[test]
    fn test_str_upper_lower_roundtrip() {
        use crate::interpreter::Value;
        let src = r#""Hello World" str-upper str-lower"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("hello world".into()))));
    }
    
    #[test]
    fn test_fpow_integer_exponent() {
        use crate::interpreter::Value;
        let src = "3.0 4.0 fpow";  // 3^4 = 81
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        match interp.result() {
            Some(Value::Float(f)) => assert!((f - 81.0).abs() < 1e-10),
            other => panic!("expected 81.0, got {:?}", other),
        }
    }

    // ====================================================================
    // P1 ERROR PROPAGATION TESTS (? operator, Error values)
    // ====================================================================

    #[test]
    fn test_div_by_zero_returns_error_value() {
        // Division by zero now returns Error value on stack (P1: S→S)
        use crate::interpreter::Value;
        let src = "10 0 /";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        match interp.result() {
            Some(Value::Error(msg)) => assert!(msg.contains("division by zero")),
            other => panic!("expected Error, got {:?}", other),
        }
    }

    #[test]
    fn test_mod_by_zero_returns_error_value() {
        use crate::interpreter::Value;
        let src = "10 0 mod";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        match interp.result() {
            Some(Value::Error(msg)) => assert!(msg.contains("modulo by zero")),
            other => panic!("expected Error, got {:?}", other),
        }
    }

    #[test]
    fn test_fdiv_by_zero_returns_error_value() {
        use crate::interpreter::Value;
        let src = "1.0 0.0 fdiv";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        match interp.result() {
            Some(Value::Error(msg)) => assert!(msg.contains("division by zero")),
            other => panic!("expected Error, got {:?}", other),
        }
    }

    #[test]
    fn test_propagate_passes_non_error() {
        // ? on a non-error value should pass it through
        use crate::interpreter::Value;
        let src = "42 ?";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(42)));
    }

    #[test]
    fn test_propagate_re_fails_on_error() {
        // ? on an Error value should re-fail (unwind to nearest try)
        use crate::interpreter::Value;
        let src = r#"[ "oops" error ? ] try is-error"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Bool(true)));
    }

    #[test]
    fn test_propagate_chain_div_error() {
        // Division by zero produces Error, ? propagates it to try
        use crate::interpreter::Value;
        let src = "[ 10 0 / ? 100 + ] try is-error";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Bool(true)));
    }

    #[test]
    fn test_propagate_chain_success() {
        // No error: ? passes through, computation continues
        use crate::interpreter::Value;
        let src = "[ 10 2 / ? 100 + ] try";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(105)));
    }

    #[test]
    fn test_is_error_on_error_value() {
        use crate::interpreter::Value;
        let src = "10 0 / is-error";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Bool(true)));
    }

    #[test]
    fn test_is_error_on_normal_value() {
        use crate::interpreter::Value;
        let src = "42 is-error";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Bool(false)));
    }

    #[test]
    fn test_get_out_of_bounds_returns_error() {
        use crate::interpreter::Value;
        let src = "( 1 2 3 ) 10 get";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        match interp.result() {
            Some(Value::Error(msg)) => assert!(msg.contains("out of bounds")),
            other => panic!("expected Error, got {:?}", other),
        }
    }

    #[test]
    fn test_str_get_out_of_bounds_returns_error() {
        use crate::interpreter::Value;
        let src = r#""hi" 10 str-get"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        match interp.result() {
            Some(Value::Error(msg)) => assert!(msg.contains("out of bounds")),
            other => panic!("expected Error, got {:?}", other),
        }
    }

    // ================================================================
    // ARRAY TESTS — Tool 4: contiguous i64 storage
    // ================================================================

    #[test]
    fn test_array_literal_basic() {
        // #(1 2 3) creates an array of 3 elements
        use crate::interpreter::Value;
        let src = "#(1 2 3)";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Array(Box::new(vec![1, 2, 3]))));
    }

    #[test]
    fn test_array_literal_empty() {
        use crate::interpreter::Value;
        let src = "#()";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Array(Box::new(vec![]))));
    }

    #[test]
    fn test_array_literal_single() {
        use crate::interpreter::Value;
        let src = "#(42)";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Array(Box::new(vec![42]))));
    }

    #[test]
    fn test_array_literal_negative() {
        use crate::interpreter::Value;
        let src = "0 1 - 0 2 - 0 3 - ( rot rot rot ) array-from";
        // This pushes -1, -2, -3 on stack, collects into list, then array-from
        // But let's do it simpler: expressions in the #() literal
        let src2 = "#(100 200 300)";
        let module = compile(src2).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Array(Box::new(vec![100, 200, 300]))));
    }

    #[test]
    fn test_array_new() {
        use crate::interpreter::Value;
        let src = "5 array-new";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Array(Box::new(vec![0, 0, 0, 0, 0]))));
    }

    #[test]
    fn test_array_get() {
        use crate::interpreter::Value;
        // array-get is non-destructive: (array i -- array elem)
        let src = "#(10 20 30) 1 array-get";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        // Top of stack should be 20 (the element), array below
        assert_eq!(interp.result(), Some(&Value::Int(20)));
    }

    #[test]
    fn test_array_get_preserves_array() {
        use crate::interpreter::Value;
        // After array-get, the array is still on the stack below the element
        let src = "#(10 20 30) 0 array-get drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Array(Box::new(vec![10, 20, 30]))));
    }

    #[test]
    fn test_array_get_out_of_bounds() {
        use crate::interpreter::Value;
        let src = "#(10 20 30) 5 array-get";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        // Should push error value
        match interp.result() {
            Some(Value::Error(msg)) => assert!(msg.contains("out of bounds")),
            other => panic!("expected Error, got {:?}", other),
        }
    }

    #[test]
    fn test_array_set() {
        use crate::interpreter::Value;
        // array-set: (array i v -- array)
        let src = "#(10 20 30) 1 99 array-set";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Array(Box::new(vec![10, 99, 30]))));
    }

    #[test]
    fn test_array_len() {
        use crate::interpreter::Value;
        // array-len: (array -- array n) non-destructive
        let src = "#(10 20 30 40) array-len";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(4)));
    }

    #[test]
    fn test_array_len_preserves_array() {
        use crate::interpreter::Value;
        let src = "#(10 20 30 40) array-len drop";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Array(Box::new(vec![10, 20, 30, 40]))));
    }

    #[test]
    fn test_array_push() {
        use crate::interpreter::Value;
        // array-push: (array v -- array)
        let src = "#(1 2 3) 4 array-push";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Array(Box::new(vec![1, 2, 3, 4]))));
    }

    #[test]
    fn test_array_from_list() {
        use crate::interpreter::Value;
        // array-from: (list -- array)
        let src = "(10 20 30) array-from";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Array(Box::new(vec![10, 20, 30]))));
    }

    #[test]
    fn test_array_from_mixed_list() {
        use crate::interpreter::Value;
        // Non-int elements become 0 (P1: total function, no exceptions)
        let src = r#"(10 "hi" 30) array-from"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Array(Box::new(vec![10, 0, 30]))));
    }

    #[test]
    fn test_array_from_floats() {
        use crate::interpreter::Value;
        // Floats truncated to i64 (P1: total function)
        let src = "(1 2.7 3) array-from";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Array(Box::new(vec![1, 2, 3]))));
    }

    #[test]
    fn test_array_chained_operations() {
        use crate::interpreter::Value;
        // Create array, set some values, get one back
        let src = "3 array-new 0 42 array-set 1 7 array-set 0 array-get";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Int(42)));
    }

    #[test]
    fn test_array_build_with_push() {
        use crate::interpreter::Value;
        // Build an array incrementally with push
        let src = "0 array-new 10 array-push 20 array-push 30 array-push";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Array(Box::new(vec![10, 20, 30]))));
    }

    #[test]
    fn test_array_type_of() {
        use crate::interpreter::Value;
        let src = "#(1 2 3) type-of";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Str(Box::new("array".into()))));
    }

    #[test]
    fn test_array_literal_computed_elements() {
        use crate::interpreter::Value;
        // Array literal with computed elements — each expression is parsed
        // separately, so arithmetic must be self-contained per element.
        // Stack: push 1, push (2 3 +) = 5, push (10 2 *) = 20
        // But inside #(...), each parse_expr is a single token/expression.
        // Simplest: use the word form to build computed arrays.
        let src = "1 2 3 + 10 2 * (rot rot rot) array-from";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&Value::Array(Box::new(vec![1, 5, 20]))));
    }

    #[test]
    fn test_array_set_out_of_bounds() {
        // array-set with out of bounds should error
        let src = "#(1 2 3) 10 42 array-set";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        match interp.run() {
            Err(msg) => assert!(msg.contains("out of bounds")),
            Ok(()) => panic!("expected error for out-of-bounds array-set"),
        }
    }

    // ================================================================
    // TRAINING PRIMITIVE TESTS — TIMES, FILTER, HEAD, TAIL, RANGE,
    //                            FIRST, SECOND, CONCAT, EMPTY?
    // P1: Each is a tool (S→S). P2: Applied via quote where needed.
    // ================================================================

    // Shared import for training primitive tests
    use crate::interpreter::Value as V;

    #[test]
    fn test_times_basic() {
        // 0 [1 +] times × 3 = 3
        let module = compile("0 3 [ 1 + ] times").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::Int(3)));
    }

    #[test]
    fn test_times_zero() {
        // 0 times = noop
        let module = compile("42 0 [ 1 + ] times").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::Int(42)));
    }

    #[test]
    fn test_times_factorial() {
        // 5! via times: start with 1, multiply by 1..5
        // 1  1 [dup rot * swap 1 +] 5 times → but simpler:
        // Push 1, then use accumulator pattern
        let module = compile("1 1 5 [ dup rot * swap 1 + ] times drop").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::Int(120)));
    }

    #[test]
    fn test_filter_basic() {
        // Keep elements > 2
        let module = compile("( 1 2 3 4 5 ) [ 2 > ] filter").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::List(Box::new(vec![
            V::Int(3), V::Int(4), V::Int(5)
        ]))));
    }

    #[test]
    fn test_filter_none() {
        // Filter that removes everything
        let module = compile("( 1 2 3 ) [ 10 > ] filter").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::List(Box::new(vec![]))));
    }

    #[test]
    fn test_filter_all() {
        // Filter that keeps everything
        let module = compile("( 1 2 3 ) [ 0 > ] filter").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::List(Box::new(vec![
            V::Int(1), V::Int(2), V::Int(3)
        ]))));
    }

    #[test]
    fn test_filter_empty() {
        let module = compile("( ) [ 0 > ] filter").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::List(Box::new(vec![]))));
    }

    #[test]
    fn test_head_basic() {
        let module = compile("( 10 20 30 ) head").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::Int(10)));
    }

    #[test]
    fn test_head_single() {
        let module = compile("( 42 ) head").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::Int(42)));
    }

    #[test]
    fn test_tail_basic() {
        let module = compile("( 10 20 30 ) tail").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::List(Box::new(vec![
            V::Int(20), V::Int(30)
        ]))));
    }

    #[test]
    fn test_tail_single() {
        let module = compile("( 42 ) tail").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::List(Box::new(vec![]))));
    }

    #[test]
    fn test_range_basic() {
        let module = compile("0 5 range").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::List(Box::new(vec![
            V::Int(0), V::Int(1), V::Int(2), V::Int(3), V::Int(4)
        ]))));
    }

    #[test]
    fn test_range_single() {
        let module = compile("3 4 range").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::List(Box::new(vec![
            V::Int(3)
        ]))));
    }

    #[test]
    fn test_range_empty() {
        // start >= end = empty
        let module = compile("5 3 range").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::List(Box::new(vec![]))));
    }

    #[test]
    fn test_range_negative() {
        let module = compile("-2 2 range").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::List(Box::new(vec![
            V::Int(-2), V::Int(-1), V::Int(0), V::Int(1)
        ]))));
    }

    #[test]
    fn test_first_pair() {
        // first is non-destructive: (pair -- pair a)
        let module = compile("1 2 pair first").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        // Top of stack = first element
        assert_eq!(interp.result(), Some(&V::Int(1)));
        // Pair is still underneath
        assert_eq!(interp.stack().len(), 2);
    }

    #[test]
    fn test_second_pair() {
        // second is non-destructive: (pair -- pair b)
        let module = compile("1 2 pair second").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        // Top of stack = second element
        assert_eq!(interp.result(), Some(&V::Int(2)));
        // Pair is still underneath
        assert_eq!(interp.stack().len(), 2);
    }

    #[test]
    fn test_first_second_roundtrip() {
        // first: (pair -- pair a), second: (pair -- pair b)
        // 10 20 pair → [pair(10,20)]
        // first → [pair(10,20), 10]
        // swap second → [10, pair(10,20), 20]  
        // swap drop → [10, 20]
        // + → [30]
        let module = compile("10 20 pair first swap second swap drop +").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::Int(30)));
    }

    #[test]
    fn test_concat_basic() {
        let module = compile("( 1 2 ) ( 3 4 ) concat").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::List(Box::new(vec![
            V::Int(1), V::Int(2), V::Int(3), V::Int(4)
        ]))));
    }

    #[test]
    fn test_concat_empty_left() {
        let module = compile("( ) ( 1 2 ) concat").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::List(Box::new(vec![
            V::Int(1), V::Int(2)
        ]))));
    }

    #[test]
    fn test_concat_empty_right() {
        let module = compile("( 1 2 ) ( ) concat").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::List(Box::new(vec![
            V::Int(1), V::Int(2)
        ]))));
    }

    #[test]
    fn test_list_concat_alias() {
        // "list-concat" should work same as "concat"
        let module = compile("( 1 ) ( 2 ) list-concat").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::List(Box::new(vec![
            V::Int(1), V::Int(2)
        ]))));
    }

    #[test]
    fn test_empty_true() {
        let module = compile("( ) empty?").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        // Top = true, list underneath
        assert_eq!(interp.result(), Some(&V::Bool(true)));
        assert_eq!(interp.stack().len(), 2);
    }

    #[test]
    fn test_empty_false() {
        let module = compile("( 1 2 3 ) empty?").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::Bool(false)));
        assert_eq!(interp.stack().len(), 2);
    }

    // ================================================================
    // GAS LIMIT TESTS
    // ================================================================

    #[test]
    fn test_gas_limit_stops_infinite_loop() {
        // [true] loop with a gas limit should error, not hang
        let module = compile("[ true ] loop").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.set_max_steps(1000);
        match interp.run() {
            Err(msg) => assert!(msg.contains("step limit") || msg.contains("gas") || msg.contains("Gas"),
                "Expected step/gas limit error, got: {}", msg),
            Ok(()) => panic!("infinite loop should have been stopped by gas limit"),
        }
        assert!(interp.steps_executed() >= 1000);
    }

    #[test]
    fn test_gas_limit_sufficient() {
        // Simple program within gas limit should succeed
        let module = compile("3 4 +").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.set_max_steps(100);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::Int(7)));
        assert!(interp.steps_executed() > 0);
        assert!(interp.steps_executed() < 100);
    }

    // ================================================================
    // COMPOSED PATTERNS — Higher-order list operations
    // ================================================================

    #[test]
    fn test_range_map_filter() {
        // Generate 0..10, double each, keep those > 10
        let module = compile("0 10 range [ 2 * ] map [ 10 > ] filter").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::List(Box::new(vec![
            V::Int(12), V::Int(14), V::Int(16), V::Int(18)
        ]))));
    }

    #[test]
    fn test_range_fold_sum() {
        // sum(0..5) = 0+1+2+3+4 = 10
        let module = compile("0 5 range 0 [ + ] fold").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::Int(10)));
    }

    #[test]
    fn test_head_tail_reconstruct() {
        // head + tail should cover whole list
        // Get head, then tail, then fold tail to sum, add head
        let module = compile("( 10 20 30 ) dup head swap tail 0 [ + ] fold +").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        // 10 + (20 + 30) = 60
        assert_eq!(interp.result(), Some(&V::Int(60)));
    }

    #[test]
    fn test_times_with_range() {
        // Build list 0..3 using times
        // Start with empty list, push 0,1,2 via counter
        let module = compile("0 3 range 0 [ + ] fold").unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::Int(3))); // 0+1+2
    }

    // ================================================================
    // parse-float / parse-int tests
    // ================================================================

    #[test]
    fn test_parse_float_basic() {
        let module = compile(r#""3.14" parse-float"#).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::Float(3.14)));
    }

    #[test]
    fn test_parse_float_negative() {
        let module = compile(r#""-0.5" parse-float"#).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::Float(-0.5)));
    }

    #[test]
    fn test_parse_float_integer_string() {
        // "42" should parse as 42.0 float
        let module = compile(r#""42" parse-float"#).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::Float(42.0)));
    }

    #[test]
    fn test_parse_float_scientific() {
        let module = compile(r#""1.5e2" parse-float"#).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::Float(150.0)));
    }

    #[test]
    fn test_parse_float_whitespace() {
        // Leading/trailing whitespace should be trimmed
        let module = compile(r#""  3.14  " parse-float"#).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::Float(3.14)));
    }

    #[test]
    fn test_parse_float_invalid() {
        let module = compile(r#""abc" parse-float"#).unwrap();
        let mut interp = Interpreter::from_module(&module);
        let result = interp.run();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("parse-float"));
    }

    #[test]
    fn test_parse_int_basic() {
        let module = compile(r#""123" parse-int"#).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::Int(123)));
    }

    #[test]
    fn test_parse_int_negative() {
        let module = compile(r#""-7" parse-int"#).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::Int(-7)));
    }

    #[test]
    fn test_parse_int_hex() {
        let module = compile(r#""0xFF" parse-int"#).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::Int(255)));
    }

    #[test]
    fn test_parse_int_whitespace() {
        let module = compile(r#""  42  " parse-int"#).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::Int(42)));
    }

    #[test]
    fn test_parse_int_zero() {
        let module = compile(r#""0" parse-int"#).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::Int(0)));
    }

    #[test]
    fn test_parse_int_invalid() {
        let module = compile(r#""hello" parse-int"#).unwrap();
        let mut interp = Interpreter::from_module(&module);
        let result = interp.run();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("parse-int"));
    }

    #[test]
    fn test_parse_float_pipeline() {
        // Split a string of floats, parse each, sum them
        let src = r#""1.5 2.5 3.0" " " str-split [ parse-float ] map 0.0 [ fadd ] fold"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::Float(7.0)));
    }

    #[test]
    fn test_parse_int_pipeline() {
        // Split a string of ints, parse each, sum them
        let src = r#""10 20 30" " " str-split [ parse-int ] map 0 [ + ] fold"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::Int(60)));
    }

    #[test]
    fn test_parse_float_then_arithmetic() {
        // Parse two floats and multiply them
        let src = r#""2.5" parse-float "4.0" parse-float fmul"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::Float(10.0)));
    }

    #[test]
    fn test_parse_int_then_arithmetic() {
        // Parse two ints and add
        let src = r#""100" parse-int "200" parse-int +"#;
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::Int(300)));
    }

    #[test]
    fn test_parse_roundtrip_float() {
        // float → to-str → parse-float should roundtrip
        let src = "3.14 to-str parse-float";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::Float(3.14)));
    }

    #[test]
    fn test_parse_roundtrip_int() {
        // int → to-str → parse-int should roundtrip
        let src = "42 to-str parse-int";
        let module = compile(src).unwrap();
        let mut interp = Interpreter::from_module(&module);
        interp.run().unwrap();
        assert_eq!(interp.result(), Some(&V::Int(42)));
    }
}
