# Self-Hosting Compiler: Implementation Roadmap

> **STATUS: ACTION PLAN**  
> Step-by-step guide to implementing Kore's self-hosting compiler.

## Overview

This document provides the exact implementation steps, file-by-file, for building a self-hosting Kore compiler.

---

## Phase 0: Foundation (Current State)

### What We Have

```
src/
├── builtins.rs    # 12 primitives: call, try, is-error, unwrap, dup, drop, swap, over, rot, if, loop
├── value.rs       # 10 types: Null, Bool, Int, Float, Text, List, Map, Quote, Handle, Error
├── stack.rs       # Stack operations
├── op.rs          # Op enum (Push, Call, Quote, Define)
├── executor.rs    # Execute ops
├── context.rs     # Context (dictionary, capabilities)
├── tool.rs        # Tool definition
└── lib.rs         # Re-exports
```

### What We Need

| Category | Count | Priority |
|----------|-------|----------|
| Arithmetic | 6 | P1 |
| Comparison | 6 | P1 |
| Logic | 3 | P1 |
| String | 13 | P1 |
| List | 9 | P1 |
| Map | 7 | P1 |
| Combinators | 6 | P2 |
| Type | 8 | P2 |
| Conversion | 6 | P2 |

**Total: 64 new primitives**

---

## Phase 1: Core Primitives

### Step 1.1: Arithmetic (6 tools)

**File:** `src/builtins.rs`

Add after the Control Flow section:

```rust
// === Arithmetic ===

// add: (a b -- a+b)
dict.register(Tool::native("add", "(a:Num b:Num -- c:Num)", |mut stack, ctx| {
    Box::pin(async move {
        let b = stack.pop()?;
        let a = stack.pop()?;
        let result = match (a, b) {
            (Value::Int(a), Value::Int(b)) => Value::Int(a + b),
            (Value::Float(a), Value::Float(b)) => Value::Float(a + b),
            (Value::Int(a), Value::Float(b)) => Value::Float(a as f64 + b),
            (Value::Float(a), Value::Int(b)) => Value::Float(a + b as f64),
            _ => return Err(Error::TypeError { expected: "Num".into(), got: "other".into() }),
        };
        stack.push(result)?;
        Ok((stack, ctx))
    })
}));

// sub: (a b -- a-b)
dict.register(Tool::native("sub", "(a:Num b:Num -- c:Num)", |mut stack, ctx| {
    Box::pin(async move {
        let b = stack.pop()?;
        let a = stack.pop()?;
        let result = match (a, b) {
            (Value::Int(a), Value::Int(b)) => Value::Int(a - b),
            (Value::Float(a), Value::Float(b)) => Value::Float(a - b),
            (Value::Int(a), Value::Float(b)) => Value::Float(a as f64 - b),
            (Value::Float(a), Value::Int(b)) => Value::Float(a - b as f64),
            _ => return Err(Error::TypeError { expected: "Num".into(), got: "other".into() }),
        };
        stack.push(result)?;
        Ok((stack, ctx))
    })
}));

// mul: (a b -- a*b)
// div: (a b -- a/b)
// mod: (a b -- a%b)
// neg: (a -- -a)
```

**Tests:** Add corresponding test functions.

### Step 1.2: Comparison (6 tools)

```rust
// === Comparison ===

// eq: (a b -- a==b)
dict.register(Tool::native("eq", "(a:Any b:Any -- c:Bool)", |mut stack, ctx| {
    Box::pin(async move {
        let b = stack.pop()?;
        let a = stack.pop()?;
        stack.push(Value::Bool(a == b))?;
        Ok((stack, ctx))
    })
}));

// neq: (a b -- a!=b)
// lt: (a b -- a<b)
// gt: (a b -- a>b)
// le: (a b -- a<=b)
// ge: (a b -- a>=b)
```

### Step 1.3: Logic (3 tools)

```rust
// === Logic ===

// and: (a b -- a&&b)
dict.register(Tool::native("and", "(a:Bool b:Bool -- c:Bool)", |mut stack, ctx| {
    Box::pin(async move {
        let b = stack.pop()?.as_bool()?;
        let a = stack.pop()?.as_bool()?;
        stack.push(Value::Bool(a && b))?;
        Ok((stack, ctx))
    })
}));

// or: (a b -- a||b)
// not: (a -- !a)
```

### Step 1.4: String Operations (13 tools)

```rust
// === String ===

// str-len: (s -- n)
dict.register(Tool::native("str-len", "(s:Text -- n:Int)", |mut stack, ctx| {
    Box::pin(async move {
        let s = stack.pop()?.into_text()?;
        stack.push(Value::Int(s.chars().count() as i64))?;
        Ok((stack, ctx))
    })
}));

// str-get: (s i -- c) - get character at index
dict.register(Tool::native("str-get", "(s:Text i:Int -- c:Text)", |mut stack, ctx| {
    Box::pin(async move {
        let i = stack.pop()?.as_int()? as usize;
        let s = stack.pop()?.into_text()?;
        let c = s.chars().nth(i)
            .ok_or_else(|| Error::IndexOutOfBounds { index: i as i64, len: s.chars().count() as i64 })?;
        stack.push(Value::Text(c.to_string()))?;
        Ok((stack, ctx))
    })
}));

// str-slice: (s start end -- sub)
// str-split: (s delim -- list)
// str-join: (list delim -- s)
// str-concat: (a b -- ab)
// str-trim: (s -- s)
// str-find: (s pattern -- i)  returns -1 if not found
// str-starts: (s prefix -- bool)
// str-ends: (s suffix -- bool)
// str-replace: (s old new -- s)
// char-code: (c -- n)
// code-char: (n -- c)
```

### Step 1.5: List Operations (9 tools)

```rust
// === List ===

// list-len: (l -- n)
dict.register(Tool::native("list-len", "(l:List -- n:Int)", |mut stack, ctx| {
    Box::pin(async move {
        let list = stack.pop()?.into_list()?;
        stack.push(Value::Int(list.len() as i64))?;
        Ok((stack, ctx))
    })
}));

// list-get: (l i -- v)
// list-set: (l i v -- l)
// list-push: (l v -- l)
// list-pop: (l -- l v)
// list-slice: (l start end -- l)
// list-concat: (a b -- c)
// list-reverse: (l -- l)
// list-empty: ( -- l)
```

### Step 1.6: Map Operations (7 tools)

```rust
// === Map ===

// map-get: (m k -- v)
dict.register(Tool::native("map-get", "(m:Map k:Text -- v:Any)", |mut stack, ctx| {
    Box::pin(async move {
        let k = stack.pop()?.into_text()?;
        let m = stack.pop()?.into_map()?;
        let v = m.get(&k).cloned().unwrap_or(Value::Null);
        stack.push(v)?;
        Ok((stack, ctx))
    })
}));

// map-set: (m k v -- m)
// map-has: (m k -- bool)
// map-keys: (m -- list)
// map-values: (m -- list)
// map-empty: ( -- m)
// map-remove: (m k -- m)
```

---

## Phase 2: Combinators

### Step 2.1: Iteration Combinators

```rust
// === Combinators ===

// each: (list quote -- ...) - execute quote for each element
dict.register(Tool::native("each", "(l:List q:Quote -- ...)", |mut stack, ctx| {
    Box::pin(async move {
        let quote = stack.pop()?.into_quote()?;
        let list = stack.pop()?.into_list()?;
        
        for item in list {
            stack.push(item)?;
            let (new_stack, new_ctx) = execute(&quote, stack, ctx.clone()).await?;
            stack = new_stack;
        }
        Ok((stack, ctx))
    })
}));

// map: (list quote -- list) - transform each element
dict.register(Tool::native("map", "(l:List q:Quote -- l:List)", |mut stack, ctx| {
    Box::pin(async move {
        let quote = stack.pop()?.into_quote()?;
        let list = stack.pop()?.into_list()?;
        
        let mut result = Vec::with_capacity(list.len());
        for item in list {
            stack.push(item)?;
            let (mut new_stack, _) = execute(&quote, stack, ctx.clone()).await?;
            result.push(new_stack.pop()?);
            stack = new_stack;
        }
        stack.push(Value::List(result))?;
        Ok((stack, ctx))
    })
}));

// filter: (list quote -- list)
// fold: (list init quote -- result)
// times: (n quote -- ...)
// while: (cond-quote body-quote -- ...)
```

---

## Phase 3: Tokenizer in Kore

### Step 3.1: Create Tokenizer File

**File:** `examples/compiler/tokenizer.kore`

```kore
# Kore Tokenizer
# Input: source string on stack
# Output: list of token maps on stack

# Token types:
# - INT: integer literal
# - FLOAT: float literal  
# - TEXT: string literal
# - WORD: bare word
# - LBRACKET: [
# - RBRACKET: ]
# - DEFINE: define keyword

# Helper: is-whitespace (c -- bool)
define is-whitespace [
  dup " " eq
  swap dup "\n" eq
  swap dup "\t" eq
  swap "\r" eq
  or or or
]

# Helper: is-digit (c -- bool)
define is-digit [
  dup char-code
  dup 48 ge  # >= '0'
  swap 57 le # <= '9'
  and
  swap drop
]

# Helper: skip-whitespace (source -- source)
define skip-whitespace [
  [
    dup str-len 0 gt
    [ dup 0 str-get is-whitespace ] [ false ] if
  ]
  [
    1 -1 str-slice  # remove first char
  ]
  while
]

# Helper: read-word (source -- source word)
define read-word [
  "" swap  # word source
  [
    dup str-len 0 gt
    [
      dup 0 str-get
      dup is-whitespace not
      swap "[" eq not and
      swap "]" eq not and
    ] [ false ] if
  ]
  [
    # source word
    dup 0 str-get  # source word char
    rot swap str-concat swap  # word+char source
    1 -1 str-slice  # word remaining
  ]
  while
  swap  # word source -> source word
]

# Main tokenizer
define tokenize [
  list-empty swap  # tokens source
  
  [
    dup str-len 0 gt
  ]
  [
    skip-whitespace
    
    dup str-len 0 gt
    [
      dup 0 str-get
      
      # Check for brackets
      dup "[" eq
      [
        drop
        1 -1 str-slice  # consume [
        swap
        map-empty "LBRACKET" "type" map-set
        swap list-push swap
      ]
      [
        dup "]" eq
        [
          drop
          1 -1 str-slice  # consume ]
          swap
          map-empty "RBRACKET" "type" map-set
          swap list-push swap
        ]
        [
          # Otherwise read word
          drop
          read-word  # source word
          
          # Check if it's a number
          # For now, all words are WORD type
          map-empty
          over "value" map-set
          "WORD" "type" map-set
          
          swap list-push swap
        ]
        if
      ]
      if
    ]
    [ ]
    if
  ]
  while
  
  drop  # remove empty source
]
```

### Step 3.2: Test Tokenizer

**File:** `examples/compiler/test_tokenizer.kore`

```kore
# Test the tokenizer

"5 3 add" tokenize

# Should produce:
# [ { type: "WORD", value: "5" }
#   { type: "WORD", value: "3" }  
#   { type: "WORD", value: "add" } ]

# Print result
dup to-text print
```

---

## Phase 4: Parser in Kore

### Step 4.1: Create Parser File

**File:** `examples/compiler/parser.kore`

```kore
# Kore Parser
# Input: list of tokens on stack
# Output: AST (list of op maps) on stack

# Op types:
# - push: { type: "push", value: <value> }
# - call: { type: "call", name: <string> }
# - quote: { type: "quote", body: <list of ops> }
# - define: { type: "define", name: <string>, body: <list of ops> }

# Helper: is-number (word -- bool)
define is-number [
  dup str-len 0 gt
  [
    # Check first char is digit or minus
    dup 0 str-get
    dup is-digit
    swap "-" eq or
  ]
  [ false ]
  if
  swap drop
]

# Main parser  
define parse [
  list-empty swap  # ops tokens
  
  [
    dup list-len 0 gt
  ]
  [
    0 list-get  # peek first token
    dup "type" map-get
    
    "WORD" eq
    [
      dup "value" map-get
      dup is-number
      [
        # It's a number - create push op
        parse-int
        map-empty
        swap "value" map-set
        "push" "type" map-set
      ]
      [
        # It's a word - create call op
        map-empty
        swap "name" map-set
        "call" "type" map-set
      ]
      if
      
      # Add to ops, consume token
      swap drop  # remove token map
      rot swap list-push swap
      1 -1 list-slice  # consume token from list
    ]
    [
      # Handle brackets, etc.
      drop drop
      1 -1 list-slice
    ]
    if
  ]
  while
  
  drop  # remove empty token list
]
```

---

## Phase 5: Code Generator

### Step 5.1: Identity Codegen (Kore to Kore)

**File:** `examples/compiler/codegen.kore`

```kore
# Kore Code Generator
# Input: AST (list of op maps) on stack
# Output: Kore source string on stack

define codegen [
  "" swap  # output ops
  
  [
    dup list-len 0 gt
  ]
  [
    0 list-get  # get first op
    dup "type" map-get
    
    "push" eq
    [
      "value" map-get to-text
      swap str-concat " " str-concat swap
    ]
    [
      dup "type" map-get "call" eq
      [
        "name" map-get
        swap str-concat " " str-concat swap
      ]
      [
        drop  # unknown op type
      ]
      if
    ]
    if
    
    1 -1 list-slice  # consume op from list
  ]
  while
  
  drop  # remove empty op list
  str-trim  # clean up whitespace
]
```

### Step 5.2: Full Compiler Pipeline

**File:** `examples/compiler/compile.kore`

```kore
# Kore Compiler
# Full pipeline: source -> tokens -> AST -> source

define compile [
  tokenize
  parse
  codegen
]

# Test
"5 3 add dup mul" compile

# Should output: "5 3 add dup mul"
print
```

---

## Phase 6: Python Transpiler

### Step 6.1: Python Code Generator

**File:** `examples/compiler/codegen_python.kore`

```kore
# Python Code Generator
# Input: AST on stack
# Output: Python source string on stack

# Runtime header
define python-header [
  "# Generated by Kore Compiler\n"
  "stack = []\n\n"
  "def add():\n"
  "    b = stack.pop()\n"
  "    a = stack.pop()\n"
  "    stack.append(a + b)\n\n"
  "def dup():\n"
  "    stack.append(stack[-1])\n\n"
  "def mul():\n"
  "    b = stack.pop()\n"
  "    a = stack.pop()\n"
  "    stack.append(a * b)\n\n"
  str-concat str-concat str-concat str-concat str-concat
]

define codegen-python [
  python-header swap  # header ops
  
  [
    dup list-len 0 gt
  ]
  [
    0 list-get
    dup "type" map-get
    
    "push" eq
    [
      "value" map-get to-text
      "stack.append(" swap str-concat ")\n" str-concat
      swap str-concat swap
    ]
    [
      dup "type" map-get "call" eq
      [
        "name" map-get
        swap str-concat "()\n" str-concat swap
      ]
      [ drop ]
      if
    ]
    if
    
    1 -1 list-slice
  ]
  while
  
  drop
  "print(stack[-1])\n" str-concat  # print result
]
```

---

## Phase 7: Validation

### Step 7.1: Self-Compilation Test

```bash
# Stage 0: Run compiler on test source
kore run examples/compiler/compile.kore < test.kore > output1.kore

# Stage 1: Run output on same source (if it's valid kore)
# This validates the compiler produces working code

# Stage 2: Compare
diff test.kore output1.kore
```

### Step 7.2: Python Transpiler Test

```bash
# Compile to Python
kore run examples/compiler/compile_to_python.kore < test.kore > test.py

# Run Python
python test.py

# Should produce same result as:
kore run test.kore
```

---

## Milestones

| Week | Milestone | Verification |
|------|-----------|--------------|
| 1 | All P1 primitives implemented | Unit tests pass |
| 2 | Tokenizer working | Can tokenize "5 3 add" |
| 3 | Parser working | Can parse to AST |
| 4 | Identity codegen | compile(source) ≈ source |
| 5-6 | Python transpiler | Generated Python runs correctly |
| 7-8 | Self-hosting test | Compiler compiles itself |

---

## File Checklist

### Phase 1: Primitives
- [ ] `src/builtins.rs` - Add 44 new primitives
- [ ] `tests/primitives.rs` - Unit tests for each

### Phase 2: Combinators
- [ ] `src/builtins.rs` - Add 6 combinators
- [ ] `tests/combinators.rs` - Unit tests

### Phase 3: Tokenizer
- [ ] `examples/compiler/tokenizer.kore`
- [ ] `examples/compiler/test_tokenizer.kore`

### Phase 4: Parser
- [ ] `examples/compiler/parser.kore`
- [ ] `examples/compiler/test_parser.kore`

### Phase 5: Codegen
- [ ] `examples/compiler/codegen.kore`
- [ ] `examples/compiler/compile.kore`

### Phase 6: Transpiler
- [ ] `examples/compiler/codegen_python.kore`
- [ ] `examples/compiler/compile_to_python.kore`

### Phase 7: Validation
- [ ] `examples/compiler/self_host_test.sh`
- [ ] `docs/SELF_HOSTING_COMPLETE.md`

---

## Definition of Done

The self-hosting compiler is **complete** when:

1. ✅ All 64 primitives implemented and tested
2. ✅ Tokenizer correctly tokenizes all Kore syntax
3. ✅ Parser correctly parses all Kore programs
4. ✅ Identity codegen produces equivalent source
5. ✅ Python transpiler produces working Python
6. ✅ Compiler can compile itself
7. ✅ Documentation updated

This proves Kore is a **complete, capable language** that can implement non-trivial programs - including its own implementation.
