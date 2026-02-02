# The Kore Postulates

> "Perfection is achieved not when there is nothing more to add, but when there is nothing left to take away." — Antoine de Saint-Exupéry

These are the **inviolable axioms** of Kore. They are not guidelines or best practices. They are the foundational truths from which everything else derives. No feature, optimization, or convenience may violate them.

---

## The Three Postulates

### Postulate 1: Everything is a Tool

```
∀x ∈ Kore: x is a Tool
```

**Statement:** There is only one kind of thing in Kore: the Tool. There are no special constructs, no privileged operations, no magic. If something exists in the system, it is a tool.

**Implications:**

- Capability checking? It's a tool: `cap-check`
- Resource allocation? It's a tool: `res-alloc`
- Spawning agents? It's a tool: `spawn`
- Error handling? It's a tool: `try`
- Even introspection? Tools: `words`, `describe`, `meta`

**What this forbids:**

- ❌ Special syntax that isn't expressible as tools
- ❌ Built-in operations that tools cannot access
- ❌ Hidden mechanisms that "just happen"
- ❌ Privileged code paths for "system" features

**The test:** Can this feature be implemented as a tool that an agent could theoretically define? If no, it violates Postulate 1.

---

### Postulate 2: Tools Transform the Stack

```
Tool : Stack → Stack
```

**Statement:** A tool is a function that takes a stack and returns a stack. Nothing more. All communication between tools happens through the stack. There are no side channels, no hidden parameters, no ambient state.

**Implications:**

- Input comes from the stack
- Output goes to the stack
- There is no "return value" separate from the stack
- There are no "out parameters" or "references"

**What this forbids:**

- ❌ Global variables
- ❌ Implicit context (except capabilities/resources, which are explicit)
- ❌ Tools that communicate through anything other than the stack
- ❌ Hidden state that affects tool behavior

**The test:** Given the same stack, does a tool always produce the same result? (Modulo declared effects like I/O)

---

### Postulate 3: Composition is Concatenation

```
(P ; Q)(s) = Q(P(s))
```

**Statement:** To compose two programs, concatenate them. The output stack of the first becomes the input stack of the second. This is the *only* composition mechanism.

**Implications:**

- No function calls with named arguments
- No complex control flow primitives
- Conditionals and loops are tools that operate on quotes
- Program structure is flat (a list of operations)

**What this forbids:**

- ❌ Special syntax for "calling" vs "composing"
- ❌ Named parameters to tools
- ❌ Return statements or early exit (except via error)
- ❌ Nested scopes or lexical binding

**The test:** Can this program be written as a flat sequence of Push/Call/Quote/If? If no, it violates Postulate 3.

---

## Derived Principles

From the three postulates, we derive:

### D1: Configuration is Initial State

The only way to configure Kore is to set up the initial state:
- Which tools exist in the dictionary
- What capabilities the context has
- What resources are available
- What's in persistent storage

There are no "configuration flags" or "modes". Different behaviors come from different initial tool sets.

```
Configuration = (Dictionary, Capabilities, Resources, Storage)
```

### D2: Safety is Enforced by Tools

Capability checking, resource limits, and spawn safety are not "runtime magic". They are tools that other tools use.

```kore
# spawn is defined in terms of other tools
: spawn ( quote caps resources -- handle )
  # Check capabilities
  over cap-attenuate    # Verify caps ≤ current caps
  unwrap                # Fail if invalid
  
  # Check resources  
  over res-split        # Split resources from parent
  unwrap                # Fail if insufficient
  
  # Actually spawn (primitive that creates new context)
  spawn-raw
;
```

### D3: Introspection is Tools

Understanding the system is done through tools, not special debugging modes.

```kore
words        # List all tools
describe     # Get tool's effect signature
meta         # Get tool's metadata
calls        # What tools does this tool call?
caps-list    # What capabilities do we have?
res-status   # What resources do we have?
```

### D4: Extension is Definition

To extend Kore, define new tools. There is no plugin system, no module system, no extension API beyond the tool definition mechanism.

```kore
# This IS the extension mechanism:
: my-new-feature ( a b -- c )
  existing-tool-1
  existing-tool-2
  existing-tool-3
;
```

### D5: Programs are Values

A program (sequence of operations) can be pushed onto the stack as a Quote. Programs manipulate programs. This enables metaprogramming without special syntax.

```kore
[ 1 2 add ]      # Push a quote (program as value)
call             # Execute it
```

---

## The Genesis Configuration

When Kore starts, there is exactly one configuration point: **what tools exist**.

### Minimal Configuration (Bare Metal)

```rust
// Only the 4 primitive operations exist
// No builtins, no capabilities, no resources
Context::minimal()
```

### Standard Configuration (Default)

```rust
// Register standard library of ~140 tools
// Set capabilities from environment
// Set resources from environment
Context::standard()
```

### Trusted Configuration (Full Access)

```rust
// All tools, all capabilities, unlimited resources
Context::trusted()
```

### Custom Configuration (Your Choice)

```rust
// Pick exactly which tools exist
let mut ctx = Context::minimal();
register_math_tools(&mut ctx);      // add, sub, mul, div
register_stack_tools(&mut ctx);     // dup, drop, swap
register_capability_tools(&mut ctx); // cap-check, cap-attenuate
// Don't register file system tools → no file access possible
// Don't register network tools → no network access possible
```

**The elegant truth:** Security is not a feature. It's the absence of tools.

---

## Implementing the Fixes (As Tools)

Given the postulates, here's how the formal foundations become tools:

### Capability Tools

```kore
# Check if we have a capability
cap-has      ( cap:Text -- bool:Bool )

# List all capabilities
cap-list     ( -- caps:List )

# Check if caps1 ≤ caps2 (subsumption)
cap-leq      ( caps1:List caps2:List -- bool:Bool )

# Compute intersection
cap-meet     ( caps1:List caps2:List -- caps:List )

# Compute union (only if we have authority)
cap-join     ( caps1:List caps2:List -- caps:List )

# Attenuate: return reduced caps if valid, error if not
cap-attenuate ( requested:List -- attenuated:List )
```

### Resource Tools

```kore
# Check current resource status
res-status   ( -- mem:Int rom:Int compute:Int net:Int )

# Check if we have enough resources
res-check    ( mem:Int rom:Int compute:Int net:Int -- bool:Bool )

# Split resources: deduct from self, return amount for child
res-split    ( mem:Int rom:Int compute:Int net:Int -- )

# Consume resources (called by other tools)
res-consume  ( kind:Text amount:Int -- )
```

### Trace Tools

```kore
# Start recording a trace
trace-start  ( -- )

# Stop and return trace
trace-stop   ( -- trace:List )

# Get fingerprint of trace
trace-hash   ( trace:List -- hash:Text )

# Compare two traces
trace-eq     ( t1:List t2:List -- bool:Bool )
```

### Effect Tools

```kore
# Get effect of a tool
effect-of    ( name:Text -- consumed:Int produced:Int )

# Compose two effects
effect-compose ( c1:Int p1:Int c2:Int p2:Int -- c:Int p:Int )

# Check if quote has valid stack effect
effect-check ( quote:Quote -- bool:Bool )
```

### The spawn Tool (Composed from Above)

```kore
: spawn ( quote:Quote caps:List resources:Map -- handle:Handle )
  # 1. Validate capabilities (attenuate to subset of ours)
  rot                          # ( caps resources quote )
  rot                          # ( resources quote caps )
  cap-attenuate                # ( resources quote attenuated-caps )
  
  # 2. Validate and split resources
  rot                          # ( quote attenuated-caps resources )
  dup res-check                # ( quote caps resources bool )
  [ ] [ "insufficient resources" panic ] if
  res-split                    # ( quote caps ) -- resources deducted from parent
  
  # 3. Create child context with attenuated caps and split resources
  spawn-context                # ( quote context )
  
  # 4. Start execution
  spawn-execute                # ( handle )
;
```

**Note:** `spawn-context` and `spawn-execute` are the only "primitives" needed. Everything else is composition.

---

## What This Achieves

### Elegance

The entire system is described by:
1. Three postulates
2. Four operations (Push, Call, Quote, If)
3. Initial configuration (which tools exist)

Everything else—safety, security, introspection, metaprogramming—emerges from composition.

### Self-Consistency

The system that enforces rules follows the same rules. Capability checking is a tool. Resource tracking is a tool. There's no "privileged kernel" that escapes scrutiny.

### Configurability

Different configurations give different behaviors:
- Want a sandboxed agent? Don't include `exec` or `fs-write` tools.
- Want unlimited compute? Set resources to unlimited.
- Want formal verification? Include `effect-check` in the standard library and use it.

### Auditability

Because everything is a tool, everything is inspectable:
```kore
"spawn" describe    # See spawn's effect
"spawn" meta        # See spawn's metadata
"spawn" calls       # See what tools spawn uses
```

### Extensibility

To add new safety features, add new tools. No core changes needed:
```kore
# Future: add rate limiting
: rate-limit ( ops-per-second:Int -- ) ... ;

# Future: add quota tracking
: quota-track ( category:Text -- ) ... ;
```

---

## The Acid Test

For any proposed feature, ask:

1. **Can it be a tool?** If yes, make it a tool. If no, reject it.

2. **Does it transform stacks?** If it needs something other than Stack → Stack, reject it.

3. **Is it composable via concatenation?** If it needs special composition syntax, reject it.

4. **Can it be configured by initial state?** If it needs runtime configuration flags, reconsider.

5. **Is it inspectable?** If it hides behavior from introspection tools, reject it.

---

## Summary

| Postulate | Statement | Test |
|-----------|-----------|------|
| **P1** | Everything is a tool | Can it be `def`'d? |
| **P2** | Tools transform stacks | Stack in → Stack out? |
| **P3** | Composition is concatenation | Flat sequence of ops? |

| Derived | Statement |
|---------|-----------|
| **D1** | Configuration is initial state |
| **D2** | Safety is enforced by tools |
| **D3** | Introspection is tools |
| **D4** | Extension is definition |
| **D5** | Programs are values |

These postulates are not negotiable. They are the essence of Kore. Every feature, every optimization, every convenience must honor them—or be rejected.

> "In the beginner's mind there are many possibilities, but in the expert's mind there are few." — Shunryu Suzuki

Kore chooses few possibilities, deeply explored.
