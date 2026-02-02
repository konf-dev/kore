# Kore Research Synthesis: Path to Self-Hosting

> **STATUS: RESEARCH DOCUMENT**  
> Synthesizing literature on concatenative languages, self-hosting compilers, and bootstrapping.

## Executive Summary

The goal: **Build a Kore compiler in Kore itself.**

This north star forces us to build generally useful text processing primitives that will benefit all Kore programs, while also serving as the ultimate validation that Kore is a complete, capable language.

---

## 1. Literature Survey

### 1.1 Concatenative Languages

**Key Sources:**
- Wikipedia: Concatenative Programming Languages
- concatenative.org wiki
- Joy language papers (Manfred von Thun)
- Factor language (Slava Pestov)

**Core Insight:**
> "The concatenation of two programs denotes the composition of the functions denoted by the two programs."

In concatenative languages:
- Programs are **compositions** of functions
- Functions take stacks as input, produce stacks as output
- Composition is indicated by **juxtaposition** (placing side by side)
- No variables needed for data flow - the stack is implicit

**Why This Matters for Kore:**
- Kore is already concatenative with `Op::Call("tool-name")`
- Quotes `[...]` are deferred programs (first-class functions)
- Combinators like `map`, `filter`, `fold` work on quotes
- This is the right foundation for self-hosting

### 1.2 The Joy Language

**Key Insight - Quotation and Combinators:**
```
Joy uses quotation [...] instead of lambda abstraction
Combinators manipulate quotations:
  i     - execute quotation
  map   - apply to each element
  fold  - reduce list
  ifte  - if-then-else
  linrec - linear recursion
  binrec - binary recursion
```

**Relevance to Kore:**
- Kore's `Quote(Vec<Op>)` is exactly Joy's quotation
- Kore's `call` is Joy's `i`
- We need combinators: `map`, `filter`, `fold`, `each`

### 1.3 Forth and Self-Hosting

**How Forth Self-Hosts:**
1. Define words (functions) that build on primitives
2. The compiler is just another set of words
3. To recompile: redefine `@` (fetch) and `!` (store) to target code space
4. Execute the compiler on itself

**Minimum Bootstrap for Forth:**
- Fetch byte from memory
- Store byte to memory  
- Execute word
- Everything else is defined in Forth itself

**Relevance to Kore:**
- Kore needs: read, write, execute, define
- Current builtins: `dup`, `drop`, `swap`, `over`, `rot`, `call`, `try`, `if`, `loop`
- Missing: string operations, parsing, file I/O

### 1.4 Bootstrapping Stages

**From Wikipedia - Traditional Stages:**

| Stage | Input | Output |
|-------|-------|--------|
| Stage 0 | Bootstrap compiler (different language) | Basic compiler |
| Stage 1 | Source code + Stage 0 | Compiler that may not self-host |
| Stage 2 | Source code + Stage 1 | Full compiler |
| Stage 3 | Source code + Stage 2 | Should match Stage 2 output |

**First Self-Hosting Compiler:** LISP (Hart & Levin, MIT, 1962)

**Adaptation for Kore:**

| Phase | Description | Language Used |
|-------|-------------|---------------|
| Phase 0 | Current Rust implementation | Rust |
| Phase 1 | Kore interpreter subset written in Kore | Kore (running on Rust) |
| Phase 2 | Kore-to-Python transpiler | Kore |
| Phase 3 | Kore-to-JavaScript transpiler | Kore |
| Phase 4 | Kore-to-Kore optimizer | Kore |

### 1.5 Factor's Evolution

**Key Lessons from Factor:**
- Started as JFactor (Java implementation, 2003)
- Evolved based on what programs needed
- Now: self-hosted, optimizing compiler
- Written in Factor + C++ for runtime

**Pattern:**
1. Build in existing language
2. Add features as needed
3. Eventually self-host
4. Keep minimal native runtime

---

## 2. Current Kore Inventory

### 2.1 Existing Value Types

```rust
enum Value {
    Null,              // Absence
    Bool(bool),        // Boolean
    Int(i64),          // Integer
    Float(f64),        // Float
    Text(String),      // String ← KEY FOR PARSING
    List(Vec<Value>),  // List ← KEY FOR AST
    Map(IndexMap),     // Map ← KEY FOR SYMBOL TABLES
    Quote(Vec<Op>),    // Deferred program
    Handle(Handle),    // Opaque resource
    Error(ErrorValue), // Captured error
}
```

**Types we have that support self-hosting:**
- ✅ `Text` - for source code and tokens
- ✅ `List` - for token streams and AST nodes
- ✅ `Map` - for symbol tables and environments
- ✅ `Quote` - for generated code

### 2.2 Existing Builtins

| Category | Tools | Status |
|----------|-------|--------|
| Execution | `call`, `try` | ✅ Complete |
| Error | `is-error`, `unwrap` | ✅ Complete |
| Stack | `dup`, `drop`, `swap`, `over`, `rot` | ✅ Complete |
| Control | `if`, `loop` | ✅ Complete |
| Arithmetic | - | ❌ Missing |
| Comparison | - | ❌ Missing |
| String | - | ❌ Missing |
| List | - | ❌ Missing |
| I/O | (in agent crate) | Partial |

### 2.3 Agent-Level Tools

From `crates/kore-agent`:
- File read/write
- Shell execution
- LLM calling

These are too high-level for a compiler - we need primitives.

---

## 3. Primitives Needed for Self-Hosting

### 3.1 String Primitives (Priority 1)

These are needed to build a tokenizer:

| Tool | Signature | Purpose |
|------|-----------|---------|
| `str-len` | `(s:Text -- n:Int)` | Get string length |
| `str-get` | `(s:Text i:Int -- c:Text)` | Get character at index |
| `str-slice` | `(s:Text start:Int end:Int -- sub:Text)` | Extract substring |
| `str-split` | `(s:Text delim:Text -- parts:List)` | Split string |
| `str-join` | `(parts:List delim:Text -- s:Text)` | Join strings |
| `str-concat` | `(a:Text b:Text -- c:Text)` | Concatenate strings |
| `str-trim` | `(s:Text -- s:Text)` | Remove whitespace |
| `str-find` | `(s:Text pattern:Text -- i:Int)` | Find substring (-1 if not found) |
| `str-starts` | `(s:Text prefix:Text -- b:Bool)` | Check prefix |
| `str-ends` | `(s:Text suffix:Text -- b:Bool)` | Check suffix |
| `str-replace` | `(s:Text old:Text new:Text -- s:Text)` | Replace first occurrence |
| `char-code` | `(c:Text -- n:Int)` | Character to code point |
| `code-char` | `(n:Int -- c:Text)` | Code point to character |

### 3.2 List Primitives (Priority 1)

These are needed for AST manipulation:

| Tool | Signature | Purpose |
|------|-----------|---------|
| `list-len` | `(l:List -- n:Int)` | Get list length |
| `list-get` | `(l:List i:Int -- v:Any)` | Get element at index |
| `list-set` | `(l:List i:Int v:Any -- l:List)` | Set element at index |
| `list-push` | `(l:List v:Any -- l:List)` | Append element |
| `list-pop` | `(l:List -- l:List v:Any)` | Remove last element |
| `list-slice` | `(l:List start:Int end:Int -- l:List)` | Extract sublist |
| `list-concat` | `(a:List b:List -- c:List)` | Concatenate lists |
| `list-reverse` | `(l:List -- l:List)` | Reverse list |
| `list-empty` | `( -- l:List)` | Create empty list |

### 3.3 Map Primitives (Priority 1)

These are needed for symbol tables:

| Tool | Signature | Purpose |
|------|-----------|---------|
| `map-get` | `(m:Map k:Text -- v:Any)` | Get value by key |
| `map-set` | `(m:Map k:Text v:Any -- m:Map)` | Set key-value |
| `map-has` | `(m:Map k:Text -- b:Bool)` | Check if key exists |
| `map-keys` | `(m:Map -- l:List)` | Get all keys |
| `map-values` | `(m:Map -- l:List)` | Get all values |
| `map-empty` | `( -- m:Map)` | Create empty map |
| `map-remove` | `(m:Map k:Text -- m:Map)` | Remove key |

### 3.4 Arithmetic Primitives (Priority 1)

| Tool | Signature | Purpose |
|------|-----------|---------|
| `add` | `(a:Num b:Num -- c:Num)` | Addition |
| `sub` | `(a:Num b:Num -- c:Num)` | Subtraction |
| `mul` | `(a:Num b:Num -- c:Num)` | Multiplication |
| `div` | `(a:Num b:Num -- c:Num)` | Division |
| `mod` | `(a:Int b:Int -- c:Int)` | Modulo |
| `neg` | `(a:Num -- b:Num)` | Negation |

### 3.5 Comparison Primitives (Priority 1)

| Tool | Signature | Purpose |
|------|-----------|---------|
| `eq` | `(a:Any b:Any -- c:Bool)` | Equality |
| `neq` | `(a:Any b:Any -- c:Bool)` | Inequality |
| `lt` | `(a:Num b:Num -- c:Bool)` | Less than |
| `gt` | `(a:Num b:Num -- c:Bool)` | Greater than |
| `le` | `(a:Num b:Num -- c:Bool)` | Less or equal |
| `ge` | `(a:Num b:Num -- c:Bool)` | Greater or equal |

### 3.6 Logic Primitives (Priority 1)

| Tool | Signature | Purpose |
|------|-----------|---------|
| `and` | `(a:Bool b:Bool -- c:Bool)` | Logical AND |
| `or` | `(a:Bool b:Bool -- c:Bool)` | Logical OR |
| `not` | `(a:Bool -- b:Bool)` | Logical NOT |

### 3.7 Combinators (Priority 2)

| Tool | Signature | Purpose |
|------|-----------|---------|
| `each` | `(l:List q:Quote -- ...)` | Execute quote for each element |
| `map` | `(l:List q:Quote -- l:List)` | Transform each element |
| `filter` | `(l:List q:Quote -- l:List)` | Keep elements where quote returns true |
| `fold` | `(l:List init:Any q:Quote -- result:Any)` | Reduce list to single value |
| `times` | `(n:Int q:Quote -- ...)` | Execute quote n times |
| `while` | `(cond:Quote body:Quote -- ...)` | Loop while condition true |

### 3.8 Type Inspection (Priority 2)

| Tool | Signature | Purpose |
|------|-----------|---------|
| `type-of` | `(v:Any -- t:Text)` | Get type name |
| `is-null` | `(v:Any -- b:Bool)` | Check if null |
| `is-bool` | `(v:Any -- b:Bool)` | Check if bool |
| `is-int` | `(v:Any -- b:Bool)` | Check if int |
| `is-float` | `(v:Any -- b:Bool)` | Check if float |
| `is-text` | `(v:Any -- b:Bool)` | Check if text |
| `is-list` | `(v:Any -- b:Bool)` | Check if list |
| `is-map` | `(v:Any -- b:Bool)` | Check if map |
| `is-quote` | `(v:Any -- b:Bool)` | Check if quote |

### 3.9 Conversion (Priority 2)

| Tool | Signature | Purpose |
|------|-----------|---------|
| `to-int` | `(v:Any -- n:Int)` | Convert to integer |
| `to-float` | `(v:Any -- n:Float)` | Convert to float |
| `to-text` | `(v:Any -- s:Text)` | Convert to string |
| `to-list` | `(v:Any -- l:List)` | Convert to list |
| `parse-int` | `(s:Text -- n:Int)` | Parse integer from string |
| `parse-float` | `(s:Text -- n:Float)` | Parse float from string |

---

## 4. Phased Implementation Plan

### Phase 1: Core Primitives (Week 1)
**Goal:** Add all Priority 1 primitives to `src/builtins.rs`

1. Arithmetic: `add`, `sub`, `mul`, `div`, `mod`, `neg`
2. Comparison: `eq`, `neq`, `lt`, `gt`, `le`, `ge`
3. Logic: `and`, `or`, `not`
4. String: `str-len`, `str-get`, `str-slice`, `str-split`, `str-concat`
5. List: `list-len`, `list-get`, `list-push`, `list-pop`, `list-empty`
6. Map: `map-get`, `map-set`, `map-has`, `map-empty`

**Verification:** Unit tests for each primitive

### Phase 2: Tokenizer in Kore (Week 2)
**Goal:** Write a tokenizer that works on Kore source

```kore
# Tokenize: source -> list of tokens
define tokenize [
  # Input: source string on stack
  # Output: list of tokens

  list-empty swap  # tokens source
  
  [ dup str-len 0 gt ]  # while source not empty
  [
    # Skip whitespace
    dup 0 str-get " " eq
    [ 1 str-slice-from ]
    [ ] if

    # Parse token based on first char
    # ...
  ] while
]
```

**Tokens needed:**
- `INT` - integer literal
- `FLOAT` - float literal
- `TEXT` - string literal
- `WORD` - bare word (tool call)
- `LBRACKET` / `RBRACKET` - quote delimiters
- `DEFINE` - definition keyword
- `COMMENT` - comment (discarded)

### Phase 3: Parser in Kore (Week 3)
**Goal:** Parse token stream into AST

```kore
# Parse: tokens -> AST (as nested lists/maps)
define parse [
  # Input: list of tokens
  # Output: list of Op values
  
  list-empty swap  # ops tokens
  
  [ dup list-len 0 gt ]
  [
    list-pop  # ops remaining token
    
    # Match token type
    dup "type" map-get
    
    "INT" eq [ "value" map-get push-literal ] if
    "WORD" eq [ "value" map-get push-call ] if
    "LBRACKET" eq [ parse-quote ] if
    # ...
  ] while
]
```

### Phase 4: Code Generator (Week 4)
**Goal:** Generate code from AST

For initial version, generate Kore text (identity compiler):
```kore
define codegen [
  # Input: AST (list of ops)
  # Output: Kore source text
  
  "" swap  # output ops
  
  [ dup list-len 0 gt ]
  [
    list-pop  # output remaining op
    
    dup "type" map-get
    
    "push" eq [ 
      "value" map-get to-text 
      swap str-concat " " str-concat swap 
    ] if
    
    "call" eq [
      "name" map-get
      swap str-concat " " str-concat swap
    ] if
    
    # ...
  ] while
  
  drop  # remove empty ops list
]
```

### Phase 5: Transpiler to Python (Week 5-6)
**Goal:** Extend codegen to emit Python

```kore
define codegen-python [
  # Input: AST
  # Output: Python source
  
  "stack = []\n" swap
  
  [ dup list-len 0 gt ]
  [
    list-pop
    
    dup "type" map-get
    
    "push" eq [
      "stack.append(" swap str-concat
      swap "value" map-get to-text str-concat
      ")\n" str-concat swap
    ] if
    
    "call" eq [
      # Emit Python function call
    ] if
  ] while
]
```

### Phase 6: Self-Hosting Test (Week 7-8)
**Goal:** Compiler compiles itself

1. Run Kore compiler on Kore compiler source
2. Get output (Kore text or Python)
3. Run output compiler on same source
4. Compare outputs (should match)

---

## 5. Architecture Principles

### 5.1 Following the Philosophy

Every primitive must:
1. **Divide** - Do exactly one operation
2. **Single Purpose** - No hidden behaviors
3. **Explicit** - Clear signature `(inputs -- outputs)`
4. **Reusable** - Useful in many contexts
5. **Verified** - Unit tested

### 5.2 Layered Design

```
Layer 5: Self-Hosting Compiler
    ↓ uses
Layer 4: Compiler Words (tokenize, parse, codegen)
    ↓ uses  
Layer 3: Combinators (map, filter, fold, each)
    ↓ uses
Layer 2: Collection Operations (str-*, list-*, map-*)
    ↓ uses
Layer 1: Primitives (add, eq, dup, call)
    ↓ uses
Layer 0: Rust Runtime (Stack, Value, Tool, Context)
```

### 5.3 No Magic

- Tokenizer is explicit character-by-character
- Parser is explicit token-by-token
- Codegen is explicit AST-to-text
- No regex (too complex), no magic parse combinators

---

## 6. Success Criteria

### Minimum Viable Self-Host
1. ✅ Tokenize Kore source into tokens
2. ✅ Parse tokens into AST
3. ✅ Generate Kore source from AST
4. ✅ Output matches input (modulo whitespace)

### Extended Goals
1. Generate Python from AST
2. Generate JavaScript from AST
3. Optimize AST (constant folding, dead code)
4. Compile to bytecode (not just source)

### Ultimate Validation
```bash
# Stage 1: Compile compiler with Rust runtime
kore compile compiler.kore > compiler1.py

# Stage 2: Compile compiler with Stage 1
python compiler1.py compiler.kore > compiler2.py

# Stage 3: Outputs should match
diff compiler1.py compiler2.py  # Should be empty
```

---

## 7. References

1. **Concatenative Languages**
   - concatenative.org wiki
   - Wikipedia: Concatenative programming language

2. **Joy Language**
   - "Joy: Forth's Functional Cousin" - Manfred von Thun
   - "Mathematical Foundations of Joy" - Manfred von Thun
   - hypercubed.github.io/joy

3. **Forth**
   - "Starting FORTH" - Leo Brodie
   - Wikipedia: Forth programming language

4. **Self-Hosting**
   - Wikipedia: Self-hosting (compilers)
   - Wikipedia: Bootstrapping (compilers)
   - "Reflections on Trusting Trust" - Ken Thompson

5. **Factor**
   - factorcode.org
   - "Factor: a dynamic stack-based programming language" - Pestov & Ehrenberg

---

## 8. Next Steps

1. **Immediate:** Implement Priority 1 primitives in `src/builtins.rs`
2. **Week 1:** Write tokenizer in Kore
3. **Week 2:** Write parser in Kore
4. **Week 3:** Write identity codegen
5. **Week 4:** Test self-hosting
6. **Week 5-6:** Python transpiler
7. **Week 7-8:** Validation and optimization
