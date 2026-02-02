# Kore OS Status

> **Current Version**: v0.1.0 (stable-v0.1-agent-working branch)
> **Last Updated**: Session 2

## Summary

Kore is now a feature-complete OS for LLM agents with **100 primitives**.

## Primitives by Category

### Execution (5)
- `call` - Run a quote
- `try` - Run quote, capture errors
- `if` - Conditional execution (three-branch: cond, true, false)
- `loop` - Repeat until false
- `def` - Define new tool from quote

### Error Handling (2)
- `is-error` - Check if value is Error
- `unwrap` - Extract or stop if Error

### Stack Manipulation (5)
- `dup`, `drop`, `swap`, `over`, `rot`

### Arithmetic (6)
- `add`, `sub`, `mul`, `div`, `mod`, `neg`

### Comparison (6)
- `eq`, `neq`, `lt`, `gt`, `le`, `ge`

### Logic (3)
- `and`, `or`, `not`

### String (13)
- `str-len`, `str-get`, `str-slice`, `str-split`, `str-join`
- `str-concat`, `str-trim`, `str-find`, `str-starts`, `str-ends`
- `str-replace`, `char-code`, `code-char`

### List (10)
- `list-len`, `list-get`, `list-set`, `list-push`, `list-pop`
- `list-slice`, `list-concat`, `list-reverse`, `list-empty`, `collect`

### Map (7)
- `map-get`, `map-set`, `map-has`, `map-del`
- `map-keys`, `map-vals`, `map-empty`

### Type Checking (9)
- `type-of`, `is-null`, `is-bool`, `is-int`, `is-float`
- `is-text`, `is-list`, `is-map`, `is-quote`

### Conversion (5)
- `to-int`, `to-float`, `to-text`, `to-bool`, `to-list`

### Combinators (6)
- `map`, `filter`, `fold`, `each`, `times`, `while`

### OS: File System (7)
- `fs-read`, `fs-write`, `fs-append`, `fs-exists`, `fs-list`, `fs-rm`, `fs-mkdir`

### OS: Process (1)
- `exec` - Run shell command

### OS: I/O (3)
- `print`, `println`, `read-line`

### OS: Time (2)
- `now`, `sleep`

### OS: Misc (2)
- `uuid` - Generate UUID v4
- `random` - Random float 0.0-1.0

### OS: Module Loading (1)
- `load` - Execute a .kore file

### OS: Environment (2)
- `env-get`, `env-set`

### OS: HTTP (3)
- `http-get` - GET request
- `http-post` - POST with body and headers
- `http-request` - Generic HTTP method

### Data: JSON (2)
- `json-parse` - JSON text to value
- `json-encode` - Value to JSON text

## Self-Hosted Compiler

The Kore compiler is written in Kore itself:

- **lib/compiler/tokenizer.kore** - Source → Tokens
- **lib/compiler/parser.kore** - Tokens → AST  
- **lib/compiler/codegen.kore** - AST → Kore code

Usage:
```
"5 3 add" tokenize parse codegen
# => "5 3 add"
```

## REPL Mode

Run `kore` with no arguments for interactive mode:

```
$ kore
Kore OS v0.1.0
Type 'exit' to quit, 'help' for commands.

Loaded prelude.
> 5 3 add
8
> [1] > 
```

Features:
- Persistent stack across lines
- Stack depth shown in prompt
- Commands: help, clear, stack, .s, exit
- Auto-loads lib/prelude.kore

## Prelude (lib/prelude.kore)

Standard library loaded at boot:

**Stack**: `nip`, `tuck`, `2dup`, `2drop`
**List**: `list1`, `list2`, `first`, `last`, `rest`
**Arithmetic**: `inc`, `dec`, `square`, `abs`, `max`, `min`
**Higher-order**: `sum`, `product`
**Predicates**: `empty-str?`, `empty-list?`

## OS Libraries (lib/os/)

- **fs.kore** - File system utilities
- **shell.kore** - Shell command helpers
- **io.kore** - I/O utilities
- **time.kore** - Time utilities
- **module.kore** - Module loading helpers

## Directory Structure

```
kore/
├── src/
│   ├── lib.rs           # Library root
│   ├── value.rs         # 10 value types
│   ├── op.rs            # Operations
│   ├── stack.rs         # Stack implementation
│   ├── context.rs       # Execution context
│   ├── executor.rs      # Execute ops
│   ├── tool.rs          # Tool abstraction
│   ├── builtins.rs      # 100 primitives
│   ├── error.rs         # Error types
│   └── bin/kore.rs      # CLI binary
├── lib/
│   ├── prelude.kore     # Standard library
│   ├── compiler/        # Self-hosted compiler
│   └── os/              # OS utilities
├── tests/
│   └── integration.kore # Integration tests
├── docs/
│   ├── PHILOSOPHY.md    # 5 principles
│   ├── ARCHITECTURE.md  # System design
│   └── STATUS.md        # This file
└── examples/
    └── *.kore           # Example programs
```

## Philosophy (docs/PHILOSOPHY.md)

1. **Divide**: Break complex tools into simpler ones
2. **Single Purpose**: Each primitive does ONE thing
3. **Explicit**: No magic, clear inputs/outputs
4. **Reuse**: Compose, don't create new primitives
5. **Verify**: Test everything that can fail

## Next Steps

1. **Port concept** - Uniform I/O sources (stdin, HTTP, file-watch)
2. **HTTP server mode** - `kore --serve :8080`
3. **Agent integration** - Connect to LLM for autonomous operation
4. **Self-modification** - Agent can define new primitives
