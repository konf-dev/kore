# The Kore Philosophy

> **STATUS: ✅ ACTIVE** - These principles guide current development.

## The Five Principles

These principles guide every decision in Kore - from the smallest tool to the largest workflow. They are embedded in the genesis prompt and must be followed at every level.

---

## 1. Divide Tasks Into the Smallest Pieces Possible

> "Make each program do one thing well." — Unix Philosophy

### What This Means

- Break complex goals into atomic sub-goals
- Each tool should be indivisible
- If you can split it, split it
- Complex behavior emerges from simple compositions

### Examples

**Bad**: A tool that "fetches data, transforms it, and saves to database"

**Good**: 
```
http-get      # fetch data
json-parse    # parse response  
transform     # apply transformation
db-insert     # save to database
```

**Bad**: A workflow that "researches and implements"

**Good**:
```
research-workflow
  ├── search-papers
  ├── read-paper
  ├── extract-insights
  └── summarize-findings

implement-workflow  
  ├── design-api
  ├── write-code
  ├── write-tests
  └── validate
```

### Why This Matters

- **Testability**: Small pieces are easy to test
- **Reusability**: Atomic tools can be reused in many contexts
- **Debuggability**: Failures are localized
- **Composability**: More combinations possible

---

## 2. Each Piece Does One Thing Only

> "A class should have only one reason to change." — Single Responsibility Principle

### What This Means

- One purpose per tool
- One goal per workflow
- If a tool has two responsibilities, make two tools
- Clear, focused, predictable

### Examples

**Bad**: 
```
add-and-log   # adds numbers AND logs the result
```

**Good**:
```
add           # adds numbers
log           # logs a value
```

Usage: `5 3 add dup log`

**Bad**: A workflow that does research AND implementation

**Good**: 
- `research` workflow → returns findings
- `implement` workflow → takes spec, returns code
- Genesis composes them

### The Test

Ask: "Can I describe this in one simple sentence without 'and'?"

- ✅ "dup duplicates the top stack value"
- ✅ "spawn starts a new concurrent workflow"
- ❌ "parse-and-validate parses JSON and validates the schema"

---

## 3. Clear Input/Output Description and Defined Side Effects

> "Explicit is better than implicit." — Zen of Python

### What This Means

- Every tool has an Effect signature: `(inputs -- outputs)`
- Types are explicit and checked
- Side effects are declared capabilities
- No hidden behavior

### The Effect System

```
Effect Notation: (inputs -- outputs)

Examples:
  add       (a:Int b:Int -- sum:Int)
  concat    (a:Text b:Text -- result:Text)
  http-get  (url:Text -- response:Map)        [NetworkAccess]
  spawn     (quote:Quote -- handle:Handle)    [SpawnWorkflow]
```

### Side Effects as Capabilities

```rust
pub enum Capability {
    // Declared side effects
    NetworkAccess,    // Can make network calls
    FileSystem,       // Can access files
    ProcessExec,      // Can run processes
    SpawnWorkflow,    // Can create workflows
    RegisterTool,     // Can modify tool registry
}
```

### Why This Matters

- **Safety**: Type mismatches caught before execution
- **Documentation**: Effect IS the documentation
- **Composition**: Can verify compositions are valid
- **Security**: Capabilities can be restricted

---

## 4. See What Tools Are Available and Reuse Them

> "Good artists copy, great artists steal." — Picasso (paraphrased)

### What This Means

- Check existing tools before creating new ones
- Composition over creation
- The tool library is a shared resource
- Discover and leverage what exists

### Introspection Tools

```
list-tools    (-- names:List)           # List all available tools
tool-exists   (name:Text -- bool:Bool)  # Check if tool exists
tool-effect   (name:Text -- effect:Text) # Get tool's effect
tool-source   (name:Text -- source:Quote) # Get composed tool's body
search-tools  (pattern:Text -- names:List) # Search by name/effect
```

### Genesis Behavior

Before creating a new tool, Genesis should:

1. **Search** for existing tools that might work
2. **Compose** existing tools if possible
3. **Extend** existing tools if close match exists
4. **Create** new tool only if truly needed

### Example

**Goal**: Need to count words in a text

**Wrong approach**: Create `count-words` tool from scratch

**Right approach**:
```
# Check what exists
"split" tool-exists   # true!
"len" tool-exists     # true!

# Compose existing tools
" " split len         # Split by space, count elements
```

Only if no composition works should a new tool be created.

---

## 5. Minimal, Elegant, High Quality — Check and Verify at Each Iteration

> "Simplicity is the ultimate sophistication." — Leonardo da Vinci

### What This Means

- Less is more
- Verify before proceeding
- Quality over quantity
- Test at every step
- Elegant solutions over clever hacks

### The Verification Loop

```
For every action:
  1. Plan what to do
  2. Do the minimum viable version
  3. Test/verify it works
  4. Only then proceed to next action
```

### Minimal

- Use the fewest tools possible
- Prefer built-in tools over custom
- Avoid unnecessary abstraction
- Simple > clever

### Elegant

- Code should read naturally
- Stack flow should be clear
- Composition should be obvious
- Beauty in simplicity

### High Quality

- Test every tool
- Verify every workflow
- Handle errors explicitly
- No "it probably works"

### Check and Verify

```
# After every significant step:
checkpoint "before-risky-operation"

# Try the operation
risky-operation

# Verify success
verify-result
is-error
(
  # If failed, restore and try differently
  "before-risky-operation" restore
  alternative-approach
)
() if
```

---

## Applying the Philosophy

### In Tool Design

```rust
// WRONG: Too many responsibilities
Tool::native("process-data", "(data:Map -- result:Map)", ...)

// RIGHT: Single responsibilities
Tool::native("validate", "(data:Map -- valid:Bool)", ...)
Tool::native("transform", "(data:Map schema:Map -- result:Map)", ...)
Tool::native("enrich", "(data:Map source:Text -- enriched:Map)", ...)
```

### In Workflow Design

```
// WRONG: Monolithic workflow
big-workflow:
  - research everything
  - design everything  
  - implement everything
  - test everything

// RIGHT: Focused workflows
research-workflow: (topic -- findings)
design-workflow: (findings -- spec)
implement-workflow: (spec -- code)
test-workflow: (code -- report)

// Genesis composes them
```

### In Genesis Decisions

```json
{
  "reasoning": "The goal is to process user data. I checked existing tools: 'validate', 'transform', 'save' all exist. I can compose them rather than creating a new tool. This follows principle #4 (reuse) and #5 (minimal).",
  "action": "spawn",
  "params": {
    "workflow": "validate transform save",
    "input": "user_data"
  }
}
```

---

## Anti-Patterns

### ❌ The Kitchen Sink

Creating a tool that does everything:
```
super-tool: fetches, parses, transforms, validates, saves, logs, notifies
```

### ❌ The Hidden Side Effect

```
# Looks pure but actually logs to file
add: (a b -- sum)  # secretly writes to log file
```

### ❌ Not Invented Here

Creating custom tools when compositions exist:
```
# BAD: Creating custom average tool
average: (list -- avg)

# GOOD: Compose existing
dup sum swap len div
```

### ❌ Premature Creation

Creating tools before verifying need:
```
# Creates tool, then discovers it's not needed
"complex-analyzer" register
# ... later realizes simple composition would work
```

### ❌ The Big Bang

Implementing everything before testing anything:
```
# BAD: Build entire system, then test
implement-all-features
run-all-tests  # Everything fails

# GOOD: Incremental with verification
implement-feature-1
test-feature-1
implement-feature-2
test-feature-2
...
```

---

## The Philosophy in the Genesis Prompt

The genesis prompt explicitly includes these principles:

```markdown
## Your Philosophy (Apply to EVERY decision)

1. **DIVIDE** tasks into the smallest possible pieces
   - Can this goal be broken down further?
   - Is each sub-goal atomic?

2. **SINGLE PURPOSE** - each piece does exactly one thing
   - Can I describe this in one sentence without "and"?
   - Does this have multiple responsibilities?

3. **EXPLICIT** - clear inputs, outputs, and side effects
   - Is the effect signature complete?
   - Are all side effects declared?

4. **REUSE** - check existing tools before creating new ones
   - Does a tool for this already exist?
   - Can I compose existing tools?

5. **VERIFY** - test and validate at every step
   - Did the last action succeed?
   - Should I checkpoint before continuing?
```

---

## Summary

| Principle | Question to Ask | Action |
|-----------|-----------------|--------|
| Divide | Can this be smaller? | Split it |
| Single Purpose | Does it do one thing? | Focus it |
| Explicit | Are I/O/effects clear? | Document them |
| Reuse | Does this exist? | Search first |
| Verify | Did it work? | Test it |

These five principles, consistently applied, lead to:

- **Composable** systems
- **Testable** components  
- **Maintainable** code
- **Reliable** execution
- **Elegant** solutions
---

## Design Decisions: Why Kore Doesn't Have Streaming

### The Question

When processing large datasets (big files, paginated APIs, infinite streams), why doesn't Kore support streaming or chunked processing?

### The Formal Answer

Kore's formal model defines tool semantics as **complete stack transformations**:

```
⟦P⟧ : Stack → Stack
```

Every tool takes a complete stack state and produces a complete stack state. There is no concept of partial values, lazy evaluation, or suspended computation.

### Why Not Add Chunked Primitives?

We considered adding primitives like:
- `fs-lines`: Read file line by line with callback
- `str-chunks`: Process text in chunks with callback  
- `list-batch`: Process list in batches with callback

**These were rejected because they violate the philosophy:**

| Primitive | Does ONE thing? | Minimal? | Composable? |
|-----------|-----------------|----------|-------------|
| `fs-lines` | ❌ iterates AND calls | ❌ complex | ❌ callback pattern |
| `str-chunks` | ❌ chunks AND calls | ❌ complex | ❌ callback pattern |
| `list-batch` | ❌ batches AND calls | ❌ complex | ❌ callback pattern |

They bundle two responsibilities (iteration AND application) and use a callback pattern that doesn't compose with stack semantics.

### The Right Approaches

**1. Pre-process externally:**
```
# Use external tools for streaming
"cat large.csv | head -100 > sample.csv" exec
"sample.csv" fs-read
```

**2. Paginate at the source:**
```
# Use API pagination
"/api/items?page=1&size=100" http-get
```

**3. Reduce in Kore:**
```
# Process in memory after loading
data json-parse
{ "relevant" filter } call
```

**4. Accept the trade-off:**

Kore prioritizes **simplicity** and **formal correctness** over handling arbitrarily large data. For truly streaming workloads, use a streaming-native tool (Unix pipes, Kafka, Flink) and integrate via `exec`.

### The Composable Alternative

If you need chunked processing, build it from primitives:

```
# Manual pagination loop
{ page:Int -- items:List }
page 100 *               # offset
100                      # limit  
"/api?offset=" swap "+" swap "&limit=" swap "+" swap
http-get json-parse      # fetch page
;

# Compose: fetch page, process, next page
1 { 
  dup fetch-page         # fetch
  process-items          # process
  1 +                    # next page
  dup max-pages <        # continue?
} while
```

This keeps each tool doing ONE thing while achieving the same result.

---