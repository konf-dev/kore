# Kore Language Guide
## Complete Reference for Writing Kore Programs

> **Version**: 0.18+ (training primitives, gas limit, serve mode, non-destructive pair access)
> **Last verified**: 527 tests passing
> **Target**: LLMs and developers writing Kore programs

---

## 1. What is Kore?

Kore is a **stack-based, proof-checked, compiled language** built on four postulates.
Programs are sequences of **tools** that transform a **stack**. Every value is a tool.
Code compiles to bytecode and runs on CPU (interpreter or Cranelift JIT), GPU (SPIR-V), or WASM.

### The Four Postulates

| # | Name | Meaning |
|---|------|---------|
| P1 | Everything is a Tool | Every value is a function `Stack → Stack` |
| P2 | Apply is Fundamental | The only primitive operation is applying a tool to a stack |
| P3 | Composition = Concatenation | Writing tools next to each other composes them |
| P4 | Constraints Attenuate | Types form a lattice; constraints can only narrow, never widen |

### Execution Model

```
Source (.kore) → Parser → Bytecode → Proof Checker → Execution
                                         ↓
                                   Type-checks at compile time
                                   Rejects ill-typed programs
```

---

## 2. Syntax Reference

### 2.1 Comments

```kore
-- This is a line comment (Haskell-style)
\ This is also a line comment (Forth-style)
```

### 2.2 Literals

| Syntax | Type | Example | Stack Effect |
|--------|------|---------|-------------|
| `42` | Int | `42` | `( -- 42)` |
| `-7` | Int | `-7` | `( -- -7)` |
| `3.14` | Float | `3.14` | `( -- 3.14)` |
| `-0.5` | Float | `-0.5` | `( -- -0.5)` |
| `"hello"` | Str | `"hello"` | `( -- "hello")` |
| `true` | Bool | `true` | `( -- true)` |
| `false` | Bool | `false` | `( -- false)` |
| `nil` | Unit | `nil` | `( -- nil)` |

**Integer encoding**: Integers from -128 to 127 use 2 bytes (i8). Larger integers use 9 bytes (i64).

**String escapes**: `\n` `\t` `\r` `\\` `\"`

### 2.3 Stack Operations

```
Stack grows → [bottom ... top]
Operations consume from top, push results on top.
```

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `drop` | `(a --)` | Remove top |
| `dup` | `(a -- a a)` | Duplicate top |
| `swap` | `(a b -- b a)` | Swap top two |
| `rot` | `(a b c -- b c a)` | Rotate third to top |
| `over` | `(a b -- a b a)` | Copy second to top |

**Example**:
```kore
1 2 3 rot   -- stack: [2, 3, 1]
drop         -- stack: [2, 3]
swap         -- stack: [3, 2]
over         -- stack: [3, 2, 3]
```

### 2.4 Arithmetic

**Integer/auto-promoting** (if both are Int → Int result; if either is Float → Float result):

| Word | Alt | Stack Effect | Description |
|------|-----|-------------|-------------|
| `+` | `add` | `(a b -- a+b)` | Addition |
| `-` | `sub` | `(a b -- a-b)` | Subtraction |
| `*` | `mul` | `(a b -- a*b)` | Multiplication |
| `/` | `div` | `(a b -- a/b)` | Division (integer for ints) |
| `%` | `mod` | `(a b -- a%b)` | Modulo (integers only) |
| `neg` | | `(a -- -a)` | Negate |

**Wrapping arithmetic**: When both operands are `Int`, `+`, `-`, and `*` use
wrapping (modular) i64 arithmetic. Overflow silently wraps around $\bmod 2^{64}$.
This is intentional — it enables LCG-based PRNGs and hash functions in pure Kore.
When either operand is `Float`, the operation promotes to f64 as usual.

**Strict float** (both operands must be Float, result is always Float):

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `fadd` | `(f f -- f)` | Float add |
| `fsub` | `(f f -- f)` | Float subtract |
| `fmul` | `(f f -- f)` | Float multiply |
| `fdiv` | `(f f -- f)` | Float divide |
| `fneg` | `(f -- f)` | Float negate |
| `fsqrt` | `(f -- f)` | Square root |
| `fabs` | `(f -- f)` | Absolute value |
| `fexp` | `(f -- f)` | $e^x$ |
| `flog` | `(f -- f)` | $\ln(x)$ |
| `i2f` | `(i -- f)` | Int → Float |
| `f2i` | `(f -- i)` | Float → Int (truncate) |

### 2.5 Comparison

| Word | Alt | Stack Effect | Description |
|------|-----|-------------|-------------|
| `<` | `lt` | `(a b -- bool)` | Less than |
| `>` | `gt` | `(a b -- bool)` | Greater than |
| `<=` | `le` | `(a b -- bool)` | Less or equal |
| `>=` | `ge` | `(a b -- bool)` | Greater or equal |
| `=` | `eq` | `(a b -- bool)` | Equal (any types) |
| `!=` | `ne` | `(a b -- bool)` | Not equal |

**Note**: Comparison operators work on both Int and Float. Int/Float comparisons auto-promote.

### 2.6 Logic

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `and` | `(bool bool -- bool)` | Logical AND |
| `or` | `(bool bool -- bool)` | Logical OR |
| `not` | `(bool -- bool)` | Logical NOT |
| `xor` | `(bool bool -- bool)` | Logical XOR |

### 2.7 Data Structures

#### Pairs (Product Types)

```kore
1 2 pair       -- stack: [(1, 2)]
unpair         -- stack: [1, 2]
```

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `pair` | `(a b -- (a,b))` | Create pair |
| `unpair` | `((a,b) -- a b)` | Destructure pair (consumes) |
| `first` | `((a,b) -- (a,b) a)` | Get first element (non-destructive) |
| `second` | `((a,b) -- (a,b) b)` | Get second element (non-destructive) |

**Non-destructive access**: `first` and `second` keep the pair on the stack, unlike `unpair`.
```kore
10 20 pair first            -- stack: [(10,20), 10]
10 20 pair second           -- stack: [(10,20), 20]
10 20 pair first swap second swap drop +  -- 30
```

#### Sum Types

```kore
42 left        -- stack: [Left(42)]
"hi" right     -- stack: [Right("hi")]
```

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `left` | `(a -- Left(a))` | Left injection |
| `right` | `(a -- Right(a))` | Right injection |
| `case` | `(Sum -- ...)` | Branch on sum type |

#### Lists

```kore
( 1 2 3 )      -- stack: [[1, 2, 3]]   (list literal)
( )            -- stack: [[]]           (empty list)
1 2 3 collect 3 -- stack: [[1, 2, 3]]  (collect from stack)
```

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `( ... )` | `( -- list)` | List literal |
| `collect N` | `(v₁ v₂ ... vₙ -- list)` | Collect N values into list |
| `unlist` | `(list -- v₁ v₂ ... vₙ)` | Spread list onto stack |
| `len` | `(list -- list n)` | Length (keeps list!) |
| `get` | `(list i -- elem)` | Get element at index |
| `set` | `(list i v -- list')` | Set element at index |
| `head` | `(list -- elem)` | First element (error on empty) |
| `tail` | `(list -- list')` | All but first (error on empty) |
| `empty?` | `(list -- list bool)` | Is list empty? (non-destructive) |
| `concat` | `(list list -- list')` | Concatenate two lists |
| `list-concat` | `(list list -- list')` | Alias for `concat` |
| `range` | `(start end -- list)` | Generate `[start..end)` |

**IMPORTANT**: `len` and `empty?` do NOT consume the list. They peek and push their result.

```kore
( 10 20 30 ) head           -- 10
( 10 20 30 ) tail           -- [20, 30]
( ) empty?                  -- stack: [[], true]
( 1 2 ) ( 3 4 ) concat      -- [1, 2, 3, 4]
0 5 range                   -- [0, 1, 2, 3, 4]
-2 2 range                  -- [-2, -1, 0, 1]
```

#### Higher-Order List Operations

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `map` | `(list quote -- list')` | Apply quote to each element |
| `filter` | `(list quote -- list')` | Keep elements where quote returns true |
| `fold` | `(list init quote -- result)` | Reduce list with accumulator |
| `zip` | `(list list -- list-of-pairs)` | Zip two lists into pairs |
| `append` | `(list v -- list')` | Append value to end of list |
| `reverse` | `(list -- list')` | Reverse a list |
| `times` | `(n quote -- ...)` | Execute quote N times |

**map**: The quote receives one element on the stack, must leave one result.
```kore
( 1 2 3 ) [ 2 * ] map   -- result: [2, 4, 6]
```

**fold**: The quote receives `(accumulator element)`, must leave one result.
```kore
( 1 2 3 ) 0 [ + ] fold   -- result: 6
```

**filter**: The quote receives one element, must leave a Bool.
```kore
( 1 2 3 4 5 ) [ 2 > ] filter   -- result: [3, 4, 5]
( 1 2 3 ) [ 10 > ] filter       -- result: [] (none match)
```

**zip**: Pure data operation, no quote needed.
```kore
( 1 2 3 ) ( 4 5 6 ) zip  -- result: [(1,4), (2,5), (3,6)]
```

**times**: Execute a quote N times. Useful for counted iteration.
```kore
0 3 [ 1 + ] times         -- result: 3 (0 + 1 + 1 + 1)
42 0 [ 1 + ] times         -- result: 42 (zero iterations = noop)
```

**Composition**: These tools compose naturally (P3).
```kore
0 10 range [ 2 * ] map [ 10 > ] filter   -- [12, 14, 16, 18]
0 5 range 0 [ + ] fold                   -- 10 (sum of 0..5)
```

### 2.8 Quotes (Deferred Computation)

A **quote** `[ ... ]` packages code as a value. It is not executed until `apply` is called.
This is the foundation of P1: code IS a value.

```kore
[ 2 * ]        -- push a quote that doubles
5 swap apply   -- apply it to 5 → 10
```

Quotes can be nested:
```kore
[ [ 1 + ] apply ]   -- a quote that applies a quote
```

Quotes are used with `map`, `fold`, `if`, `while`, `cond`, and `loop`.

### 2.9 Control Flow

Kore has two control flow styles:

| Style | Words | Philosophy |
|-------|-------|------------|
| **Keyword** | `if`/`else`/`end`, `while`/`do`/`end` | Compiled to jump instructions, fast |
| **Quote-tool** | `cond`, `loop` | P1-pure — control flow is just tools applied to quotes |

#### if-else-end

```kore
condition if
  then-body
else
  else-body
end
```

Or without else:
```kore
condition if
  then-body
end
```

**Stack effect**: Pops one Bool. Executes the matching branch.

```kore
5 3 > if
  "bigger"
else
  "smaller"
end
-- stack: ["bigger"]
```

#### while-do-end

```kore
while condition do
  body
end
```

**Stack effect**: The condition is evaluated each iteration, must push a Bool.
The loop runs while the condition is true.

```kore
-- Countdown: 5 4 3 2 1
5
while dup 0 > do
  dup     -- use current value
  1 -     -- decrement
end
drop      -- remove the 0
```

#### cond (quote-style conditional)

```kore
bool [then-body] [else-body] cond
```

**Stack effect**: `(bool quote quote -- ...)` — pops a Bool and two quotes, executes the matching branch.
This is the P1-pure version of `if`: control flow is just a tool applied to data.

```kore
true  [42] [99] cond   -- stack: [42]
false [42] [99] cond   -- stack: [99]

-- Equivalent to if-else-end:
5 3 > ["bigger"] ["smaller"] cond
```

#### loop (quote-style loop)

```kore
[body] loop
```

**Stack effect**: `(quote -- ...)` — pops a quote and executes it repeatedly.
After each iteration the quote must leave a Bool on top: `true` to continue, `false` to stop.
The Bool is consumed each iteration.

```kore
-- Countdown: 5 4 3 2 1 0
5 [dup 1 - dup 0 >] loop
-- stack: [5, 4, 3, 2, 1, 0]
```

**Why both styles?** The keyword `if`/`while` compiles to jump instructions (fast, optimizable).
The quote-tool `cond`/`loop` treats control flow as first-class data (P1-pure, composable, homoiconic).
Use whichever fits your program.

### 2.10 Function Definitions

```kore
: function-name body ;
```

Functions are defined with `: name ... ;` (Forth-style). They can call each other and recurse.

```kore
: square dup * ;
: cube dup dup * * ;

5 square    -- 25
3 cube      -- 27
```

**Function names** can contain letters, digits, `_`, `-`, and `?`.

```kore
: is-positive? 0 > ;
: my-helper_2 2 + ;
```

**Calling order**: Functions can be defined in any order. Forward references work.

```kore
: main 5 helper ;
: helper 2 * ;
main   -- result: 10
```

### 2.11 Local Variables

Local variables bind stack values to names using `-> name` syntax.

```kore
-> x       -- pops top of stack, binds to x
x          -- pushes value of x
```

**Multiple locals**:
```kore
: distance  -- (x1 y1 x2 y2 -- d)
  -> y2 -> x2 -> y1 -> x1
  x2 x1 fsub dup fmul
  y2 y1 fsub dup fmul
  fadd fsqrt
;
```

**Rules**:
- `-> name` pops from stack and stores in a named slot
- Referencing `name` pushes its value (can be referenced multiple times)
- Locals are scoped to the current function
- Up to 256 locals per function (slots 0-255)
- Local names follow the same rules as function names

**Why locals matter**: Without locals, complex computations with 4+ values require
deep stack manipulation (`rot rot swap over rot ...`). Locals make code readable.

### 2.12 IO Operations

IO operations require the **`io` capability**. Programs using them are tagged `[caps: io]`
at compile time and must be run with `--allow io`.

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `print` | `(a --)` | Print top value (no newline) |
| `println` | `(a --)` | Print top value + newline |
| `rand` | `( -- int)` | Non-deterministic random i64 |

`print` and `println` display any value type in human-readable form:
```kore
42 println              -- prints: 42
"hello" println         -- prints: hello
3.14 println            -- prints: 3.14
true println            -- prints: true
( 1 2 3 ) println       -- prints: (1 2 3)
1 2 pair println        -- prints: (1, 2)
```

`rand` uses time-based entropy and is **non-deterministic** (violates P2 purity).
For deterministic randomness, use the PRNG stdlib instead (see §3.2).

### 2.13 Capability System

Kore uses a **capability system** to control effects (P4: constraints attenuate).
Capabilities are auto-detected at compile time and enforced at runtime.

**Available capabilities**:

| Cap | Flag | Words |
|-----|------|-------|
| `io` | 0x01 | `print`, `println`, `rand`, `time-now`, `env-get` |
| `fs` | 0x02 | `file-read`, `file-write`, `file-exists` |
| `net` | 0x04 | (future: network) |
| `exec` | 0x08 | `exec` |

**Compile-time**: Programs are classified automatically:
```bash
$ kore compile pure.kore         # → "compiled 42 bytes [pure]"
$ kore compile hello.kore        # → "compiled 58 bytes [caps: io]"
```

**Runtime**: Capabilities must be explicitly granted:
```bash
$ kore run hello.korec                  # → Error: Capability denied: io
$ kore run hello.korec --allow io       # → hello world
$ kore run hello.korec --allow all      # → grants all capabilities
```

**Double gating**: The proof checker validates capability usage at compile time,
AND the runtime re-checks at execution time. A `.korec` file stores its required
capabilities in byte 6 of the binary header.

**Pure programs** (no capabilities) are unrestricted — they can run anywhere,
on any backend (CPU, GPU, WASM) with no permission flags needed.

---

## 3. Standard Library (stdlib/prelude.kore)

The prelude defines derived tools using only the primitives above. Zero new opcodes.

### Pair Projections
```kore
: fst unpair drop ;          -- (pair -- first)
: snd unpair swap drop ;     -- (pair -- second)
```

### Numeric Utilities
```kore
: abs dup 0 < if neg end ;              -- (int -- |int|)
: fabs dup 0.0 le if fneg end ;         -- (float -- |float|)
: max over over < if swap end drop ;    -- (a b -- max)
: min over over > if swap end drop ;    -- (a b -- min)
: fmax over over le if swap end drop ;  -- (f f -- max)
: fmin over over ge if swap end drop ;  -- (f f -- min)
: relu dup 0.0 le if drop 0.0 end ;    -- (float -- max(0,f))
```

### List Reductions
```kore
: sum 0 [ + ] fold ;                    -- (list<int> -- int)
: product 1 [ * ] fold ;               -- (list<int> -- int)
: fsum 0.0 [ fadd ] fold ;             -- (list<float> -- float)
```

### Vector Operations
```kore
: dot zip [ unpair * ] map 0 [ + ] fold ;        -- (vec vec -- scalar) [int]
: fdot zip [ unpair fmul ] map 0.0 [ fadd ] fold ; -- (vec vec -- scalar) [float]
: vadd zip [ unpair + ] map ;           -- (vec vec -- vec) [int]
: fvadd zip [ unpair fadd ] map ;       -- (vec vec -- vec) [float]
: vmul zip [ unpair * ] map ;           -- (vec vec -- vec) [int]
```

### Dual Numbers (Forward-Mode Autodiff)
```kore
: dual pair ;                -- (val der -- dual)
: undual unpair ;            -- (dual -- val der)
: dconst 0.0 dual ;         -- (x -- (x,0))
: dvar 1.0 dual ;           -- (x -- (x,1))
: dadd unpair rot unpair rot fadd rot rot swap fadd swap pair ;
: dmul ... ;                 -- product rule: (a*b, a*db + da*b)
: dneg unpair fneg swap fneg swap pair ;
: dsub dneg dadd ;
```

### 3.1b Higher-Order Compositions (stdlib/prelude.kore)

These functions compose training primitives. Zero new opcodes — pure P3.

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `any` | `(list pred -- bool)` | True if any element satisfies pred |
| `all` | `(list pred -- bool)` | True if all elements satisfy pred |
| `count` | `(list pred -- n)` | Count elements satisfying pred |
| `take` | `(list n -- list')` | First n elements |
| `drop-n` | `(list n -- list')` | Drop first n elements |
| `flatten` | `(list<list> -- list)` | Flatten nested lists |
| `enumerate` | `(list -- list<pair>)` | Pair each element with index |

```kore
( 1 2 3 ) [ 2 > ] any              -- true
( 2 4 6 ) [ 2 mod 0 = ] all        -- true
( 1 2 3 4 5 ) [ 3 > ] count        -- 2
( 10 20 30 40 50 ) 3 take           -- [10, 20, 30]
( 10 20 30 40 50 ) 2 drop-n         -- [30, 40, 50]
( ( 1 2 ) ( 3 4 ) ) flatten         -- [1, 2, 3, 4]
( 10 20 30 ) enumerate              -- [(0,10), (1,20), (2,30)]
```

**Implementation note**: `len` and `empty?` are non-destructive (peek stack).
The accumulator for `flatten` must be `( )` (empty list), NOT `nil` — `nil` is `Value::Nil`, not `Value::List([])`.

A pure Kore pseudo-random number generator. **No IO capability required.**
Uses LCG (Knuth MMIX: $a = 6364136223846793005$, $c = 1442695040888963407$, $m = 2^{64}$).

```kore
-- Integer absolute value
: abs dup 0 < if neg end ;

-- Advance PRNG state: (seed -- new_seed new_seed)
: prng-next
  6364136223846793005 * 1442695040888963407 +
  dup
;

-- Random float in [0, 1): (seed -- float new_seed)
: prng-float
  prng-next -> seed
  abs i2f 9223372036854775807 i2f fdiv
  seed
;

-- Random integer in [lo, hi]: (seed lo hi -- value new_seed)
: prng-range
  -> hi -> lo
  prng-next -> seed
  abs hi lo - 1 + mod lo +
  seed
;
```

**Usage pattern** — thread the seed through calls:
```kore
42 -> s                     -- initial seed
s prng-float -> s -> f1     -- f1 ∈ [0, 1)
s prng-float -> s -> f2     -- f2 ∈ [0, 1)
s 1 6 prng-range -> s -> d  -- d ∈ [1, 6]
f1 f2 d                     -- three random values
```

The PRNG is deterministic: same seed → same sequence. Period is $2^{64}$.

### 3.3 Tensor Operations (stdlib/tensor.kore)

Tensors are `Pair(data: List<Float>, shape: List<Int>)` — pure P1 values.

```kore
-- Construction / Accessors
: tensor-new pair ;                -- (data shape -- tensor)
: tensor-data unpair drop ;        -- (tensor -- data)
: tensor-shape unpair swap drop ;  -- (tensor -- shape)

-- Element-wise arithmetic (shapes must match)
: tensor-add                       -- (t1 t2 -- t3)
  unpair -> shape2
  swap unpair -> shape1
  zip [ unpair fadd ] map
  shape1 tensor-new ;

: tensor-sub                       -- (t1 t2 -- t3)
  unpair -> shape2
  swap unpair -> shape1
  swap zip [ unpair fsub ] map
  shape1 tensor-new ;

: tensor-mul                       -- Hadamard: (t1 t2 -- t3)
  unpair -> shape2
  swap unpair -> shape1
  zip [ unpair fmul ] map
  shape1 tensor-new ;

-- Scalar operations
: tensor-scale                     -- (tensor scalar -- tensor')
  -> s unpair -> shape
  [ s fmul ] map shape tensor-new ;

-- Reductions
: tensor-sum tensor-data 0.0 [ fadd ] fold ;    -- (tensor -- scalar)
: tensor-dot                       -- (t1 t2 -- scalar)
  tensor-data swap tensor-data
  zip [ unpair fmul ] map 0.0 [ fadd ] fold ;

-- Activations
: tensor-relu                      -- (tensor -- tensor')
  unpair -> shape
  [ dup 0.0 le if drop 0.0 end ] map
  shape tensor-new ;
```

**Example**: Dot product of two vectors
```kore
( 1.0 2.0 3.0 ) ( 3 ) tensor-new
( 4.0 5.0 6.0 ) ( 3 ) tensor-new
tensor-dot
-- result: 32.0  (1·4 + 2·5 + 3·6)
```

### 3.4 Geometry (stdlib/geometry.kore)

2D geometry primitives for spatial problems.

```kore
-- Euclidean distance
: dist2d  -- (x1 y1 x2 y2 -- d)
  -> y2 -> x2 -> y1 -> x1
  x2 x1 fsub dup fmul
  y2 y1 fsub dup fmul
  fadd fsqrt ;

-- Circles: Pair(Pair(x, y), r)
: circle-new -> r pair r pair ;
: circle-x unpair drop unpair drop ;
: circle-y unpair drop unpair swap drop ;
: circle-r unpair swap drop ;

-- Circle overlap: max(0, r1+r2-dist)
: circle-overlap  -- (x1 y1 r1 x2 y2 r2 -- overlap)
  -> r2 -> y2 -> x2 -> r1 -> y1 -> x1
  x1 y1 x2 y2 dist2d -> d
  r1 r2 fadd d fsub
  dup 0.0 le if drop 0.0 end ;
```

### 3.5 Simulated Annealing (stdlib/anneal.kore)

Metropolis-Hastings acceptance + exponential cooling for non-convex optimization.

```kore
-- Metropolis acceptance: (dE T seed -- accept? seed')
: metropolis
  -> seed -> T -> dE
  dE 0.0 le if true seed
  else
    dE fneg T fdiv fexp -> p
    seed prng-float -> seed2
    p le seed2
  end ;

-- Exponential cooling: (T-start T-end k steps -- T)
: cool
  -> steps -> k -> T-end -> T-start
  T-end T-start fdiv flog
  k i2f steps i2f fdiv fmul
  fexp T-start fmul ;
```

### 3.6 Reverse-Mode Autodiff (stdlib/autograd.kore)

Tape-based reverse-mode automatic differentiation. Every differentiable operation 
records a node on a computation tape. Backward pass walks the tape in reverse, 
applying chain rules to accumulate gradients.

**Node representation**: `Pair(Pair(op:Int, value:Float), Pair(inputs:List<Int>, saved:List<Float>))`

Op codes: 0=var, 1=const, 2=add, 3=mul, 4=sub, 5=relu, 6=neg

```kore
-- Create variables on a fresh tape
( ) 3.0 ad-var -> idx_x       -- tape has 1 node, idx_x=0

-- Build computation: x²
idx_x idx_x ad-mul -> idx_sq  -- tape has 2 nodes, idx_sq=1

-- Backward pass: compute all gradients
idx_sq ad-backward -> grads   -- grads is a list of floats

-- Get gradient of x
grads 0 get                   -- 6.0 (d/dx(x²) = 2x = 6 at x=3)
```

**Supported operations**:

| Function   | Forward             | Backward (chain rule)                |
|------------|---------------------|--------------------------------------|
| `ad-var`   | x (leaf)            | accumulates gradient                 |
| `ad-const` | c (constant)        | gradient = 0                         |
| `ad-add`   | a + b               | da += dout, db += dout               |
| `ad-sub`   | a - b               | da += dout, db -= dout               |
| `ad-mul`   | a × b               | da += b·dout, db += a·dout           |
| `ad-relu`  | max(0, x)           | dx += (x>0 ? 1 : 0)·dout            |
| `ad-neg`   | -x                  | dx -= dout                           |

**Complex expressions via chain rule**:

```kore
-- f(x) = (2x + 1)², df/dx = 4(2x+1) = 28 at x=3
( ) 3.0 ad-var -> idx_x
2.0 ad-const -> idx_2
1.0 ad-const -> idx_1
idx_2 idx_x ad-mul -> idx_2x      -- 2x
idx_2x idx_1 ad-add -> idx_2x1    -- 2x + 1
idx_2x1 idx_2x1 ad-mul -> idx_sq  -- (2x+1)²
idx_sq ad-backward 0 get          -- 28.0

-- MSE loss: L = (pred - target)², dL/d(pred) = 2(pred-target)
( ) 5.0 ad-var -> idx_pred
3.0 ad-const -> idx_target
idx_pred idx_target ad-sub -> idx_diff
idx_diff idx_diff ad-mul -> idx_loss
idx_loss ad-backward 0 get        -- 4.0
```

---

### 3.7 Optimizers (stdlib/optim.kore)

First-order gradient descent algorithms. All operate on parameter and gradient
vectors (List<Float>).

```kore
-- SGD: θ' = θ - lr * ∇θ
: sgd-step  -- (params grads lr -- params')
  -> lr -> grads -> params
  params grads zip
  [ unpair -> g -> p  p lr g fmul fsub ] map ;

-- Momentum: v' = μv + g, θ' = θ - lr*v'
: momentum-step  -- (params grads velocity mu lr -- params' velocity')
  -> lr -> mu -> velocity -> grads -> params
  velocity grads zip
  [ unpair -> g -> v  mu v fmul g fadd ] map -> v_new
  params v_new zip
  [ unpair -> v -> p  p lr v fmul fsub ] map  v_new ;

-- Gradient clipping by L2 norm
: grad-clip  -- (grads max_norm -- grads')
  -> max_norm
  grad-norm -> norm
  norm max_norm gt if
    max_norm norm fdiv -> scale
    [ scale fmul ] map
  end ;
```

**Adam optimizer** also available with bias-corrected first/second moments.

**Training loop example** (minimize x² from x=5):

```kore
5.0 -> x
0 -> step
while step 50 < do
  x 2.0 fmul -> g    -- gradient of x² is 2x
  x 0.1 g fmul fsub -> x
  step 1 + -> step
end
x  -- ≈ 0.0 (converged)
```

### 3.8 Tracing (stdlib/trace.kore)

Record computation steps for debugging and reproducibility.

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `trace-new` | `(-- trace)` | Create empty trace (empty list) |
| `trace-step` | `(trace name value -- trace)` | Record a step with auto-incrementing step number |
| `trace-get` | `(trace i -- trace step)` | Get step at index i |
| `trace-last` | `(trace -- trace step)` | Get most recent step |
| `trace-fingerprint` | `(trace -- trace int)` | Hash of all values for determinism checks |
| `trace-values` | `(trace -- trace list)` | Extract just the values from all steps |
| `trace-replay` | `(trace quote -- results)` | Apply quote to each step's value |

```kore
trace-new
"x" 1.0 trace-step
"x" 2.0 trace-step
trace-fingerprint -> fp   -- hash for reproducibility
```

### 3.9 Fibers (stdlib/fiber.kore)

Cooperative multitasking via suspended computations. Each fiber has its own stack.

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `fiber-new` | `(quote -- fiber)` | Create fiber from quoted code |
| `fiber-step` | `(fiber -- fiber' bool)` | Execute one instruction, push done? |
| `fiber-push` | `(fiber value -- fiber')` | Push value onto fiber's stack |
| `fiber-stack` | `(fiber -- fiber list)` | Get fiber's stack as a list |
| `fiber-status` | `(fiber -- fiber bool)` | Check if fiber is done |
| `fiber-run` | `(fiber -- fiber)` | Run fiber to completion (stdlib) |
| `fiber-result` | `(fiber -- value)` | Get top of completed fiber's stack (stdlib) |
| `fiber-map` | `(list quote -- results)` | Run quote as fiber for each element (stdlib) |

```kore
-- Create and step a fiber
[ 3 4 + ] fiber-new -> f
f fiber-step drop -> f     -- step once
f fiber-step drop -> f     -- step again
f fiber-stack swap drop    -- get result stack

-- Interleave two computations
[ 1 2 + ] fiber-new -> f1
[ 10 20 + ] fiber-new -> f2
f1 fiber-step drop -> f1   -- step f1
f2 fiber-step drop -> f2   -- step f2
-- continue interleaving...
```

### 3.10 Linear Types (stdlib/linear.kore)

Linear types enforce resource discipline. P4: linearity is a constraint that attenuates.

**Linearity levels** (from most to least constrained):
- **Linear**: Must be used exactly once — no `dup`, no `drop`
- **Affine**: Must be used at most once — no `dup`, but `drop` OK
- **Normal**: Unrestricted (default)

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `linear` | `(a -- Linear(a))` | Mark value as linear |
| `affine` | `(a -- Affine(a))` | Mark value as affine |
| `consume` | `(Linear(a) -- a)` | Unwrap linearity tag |
| `is-linear` | `(a -- a bool)` | Check if value is linear-tagged |
| `is-affine` | `(a -- a bool)` | Check if value is affine-tagged |

```kore
-- Resource pattern: must consume before done
100 linear -> resource
resource consume 10 + -> result   -- OK: consumed then used
result   -- 110

-- Error: cannot duplicate
42 linear dup   -- ERROR: Cannot dup a linear value

-- Error: cannot discard
42 linear drop  -- ERROR: Cannot drop a linear value

-- Affine: can discard but not duplicate
42 affine drop  -- OK: at most once
42 affine dup   -- ERROR: Cannot dup an affine value
```

**stdlib helpers**:

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `with-linear` | `(a quote -- result)` | Mark as linear, apply quote |
| `linear-map` | `(Linear(a) quote -- Linear(b))` | Transform inner value, preserve linearity |
| `linear-bind` | `(Linear(a) (a--Linear(b)) -- Linear(b))` | Monadic bind |
| `linear-pair` | `(Linear(a) Linear(b) -- Linear((a,b)))` | Combine two linear values |

### 3.11 Introspection

Runtime type queries and self-description.

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `type-of` | `(a -- a str)` | Push type name without consuming value |
| `depth` | `(-- n)` | Push current stack depth |
| `describe` | `(str -- str)` | Look up built-in word's stack effect |

```kore
42 type-of      -- stack: [42, "int"]
3.14 type-of    -- stack: [..., 3.14, "float"]
true type-of    -- stack: [..., true, "bool"]
( 1 2 ) type-of -- stack: [..., (1 2), "list"]

1 2 3 depth     -- stack: [1, 2, 3, 3]

"dup" describe  -- "(a -- a a)"
"fold" describe -- "(list init quote -- result)"
```

### 3.12 Algebraic Optimizer

The optimizer runs between parsing and execution, applying provably-correct
algebraic identities to reduce instruction count.

**Identities applied**:

| Pattern | Rewrite | Justification |
|---------|---------|---------------|
| `swap swap` | ε | swap is involution |
| `neg neg` | ε | negation is involution |
| `fneg fneg` | ε | float negation is involution |
| `not not` | ε | boolean negation is involution |
| `bnot bnot` | ε | bitwise NOT is involution |
| `rot rot rot` | ε | rot has order 3 |
| `dup drop` | ε | retraction |
| `0 +` | ε | additive identity |
| `1 *` | ε | multiplicative identity |
| `3 4 +` | `7` | constant folding (integers) |
| `2.5 3.5 fadd` | `6.0` | constant folding (floats) |
| `0 [...] times` | ε | 0 iterations = noop |
| `( ) concat` | drop empty + concat | empty list concat identity |
| `concat ( )` | drop concat + empty | trailing empty concat identity |
| `range head` | drop | start of range = start value |
| `empty? drop` | ε | unused empty check |

The optimizer uses fixed-point iteration: it applies rules until no more changes occur.
Training identity rules eliminate common no-op patterns that RL agents tend to generate.

### 3.13 String Operations (stdlib/string.kore)

Built-in opcodes for string manipulation. P1: strings are tools.

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `str-len` | `(str -- str n)` | Length without consuming string |
| `str-get` | `(str i -- str)` | Get character at index (1-char string) |
| `str-concat` | `(str str -- str)` | Concatenate two strings |
| `str-slice` | `(str start end -- str)` | Substring [start..end) |
| `to-str` | `(a -- str)` | Convert any value to string |
| `str-find` | `(str pattern -- int)` | Find substring, -1 if not found |

```kore
-- Basic operations
"hello" str-len swap drop    -- 5
"hello" 1 str-get            -- "e"
"hello" " world" str-concat  -- "hello world"
"hello world" 0 5 str-slice  -- "hello"
42 to-str                    -- "42"
"hello world" "world" str-find  -- 6

-- Build strings from values
"the answer is " 42 to-str str-concat  -- "the answer is 42"
```

**stdlib helpers** (stdlib/string.kore):

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `str-empty?` | `(str -- str bool)` | Check if empty |
| `str-starts?` | `(str prefix -- bool)` | Check prefix match |
| `str-ends?` | `(str suffix -- bool)` | Check suffix match |
| `str-contains?` | `(str pattern -- bool)` | Check if contains pattern |
| `str-repeat` | `(str n -- str)` | Repeat string n times |
| `str-reverse` | `(str -- str)` | Reverse a string |
| `str-join` | `(list sep -- str)` | Join list elements with separator |
| `str-split-at` | `(str pos -- left right)` | Split at position |
| `str-chars` | `(str -- list)` | Convert to list of chars |

### 3.14 SYSCALL — Generalized Host Calls

SYSCALL provides extensible host-level operations, each gated by a specific capability.

| Word | Capability | Stack Effect | Description |
|------|-----------|-------------|-------------|
| `file-read` | `fs` | `(path -- str)` | Read file contents to string |
| `file-write` | `fs` | `(path content --)` | Write string to file |
| `file-exists` | `fs` | `(path -- bool)` | Check if file exists |
| `time-now` | `io` | `(-- float)` | Current Unix timestamp |
| `env-get` | `io` | `(name -- str)` | Read environment variable |
| `exec` | `exec` | `(cmd -- (stdout, exit-code))` | Run shell command |

```kore
-- File I/O (requires --allow fs)
"/tmp/data.txt" "hello world" file-write
"/tmp/data.txt" file-read          -- "hello world"
"/tmp/data.txt" file-exists        -- true

-- Time (requires --allow io)
time-now                           -- 1735000000.123456

-- Environment (requires --allow io)
"HOME" env-get                     -- "/home/user"
```

### 3.15 Spawn — Sandboxed Parallel Execution

`spawn` executes a quote in a **sandboxed child context** with capability attenuation (P4).
The child gets `child_caps = parent_caps & requested_caps` — it can never have more
capabilities than the parent.

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `spawn` | `(quote caps -- list)` | Execute quote in sandbox, return child's stack |

```kore
-- Basic spawn: run code in sandbox
[ 3 4 + ] 255 spawn              -- result: (7)  (list with one value)

-- Spawn with no capabilities (pure sandbox)
[ 42 dup + ] 0 spawn             -- result: (84)

-- Multiple results
[ 1 2 3 ] 255 spawn              -- result: (1 2 3)

-- P4 attenuation: parent has io, spawn with 0 → child can't print
[ "hello" println ] 0 spawn      -- ERROR: PRINT denied
```

**Capability attenuation** (P4):
```kore
-- Parent has caps 0xFF (all). Spawn requests 0x01 (io only).
-- Child gets: 0xFF & 0x01 = 0x01 (io only). No fs, no exec.
[ "hello" println ] 1 spawn      -- OK: io granted
[ "/tmp/x" file-read ] 1 spawn   -- ERROR: fs denied
```

**stdlib helpers** (stdlib/spawn.kore):

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `spawn-pure` | `(quote -- list)` | Spawn with zero capabilities |
| `spawn-io` | `(quote -- list)` | Spawn with IO capability only |
| `spawn-all` | `(quote -- list)` | Spawn with all capabilities |
| `spawn-result` | `(quote caps -- value)` | Spawn and get single top result |
| `exec-stdout` | `(cmd -- str)` | Run shell command, return stdout |
| `exec-ok?` | `(cmd -- bool)` | Run shell command, check success |

### 3.16 Channels — Inter-Fiber Communication

Channels provide buffered, FIFO communication between fibers, spawned contexts,
or any code that shares a channel reference. Channels are P1 values — they can
be stored, passed on the stack, put in lists, etc.

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `chan-new` | `(-- chan)` | Create a new buffered channel |
| `chan-send` | `(chan value --)` | Send value to channel (appends to buffer) |
| `chan-recv` | `(chan -- value)` | Receive value from channel (FIFO, errors if empty) |

```kore
-- Basic send/receive
chan-new -> ch
ch 42 chan-send
ch "hello" chan-send
ch chan-recv          -- 42 (FIFO: first in, first out)
ch chan-recv          -- "hello"

-- Channels are FIFO (first-in, first-out)
chan-new -> ch
ch 1 chan-send
ch 2 chan-send
ch 3 chan-send
ch chan-recv   -- 1
ch chan-recv   -- 2
ch chan-recv   -- 3

-- Receive from empty channel is an error
chan-new chan-recv     -- ERROR: channel is empty
```

**Channels with spawn**:
```kore
-- Channel shared between parent and child
chan-new -> ch
ch 99 chan-send       -- parent sends
ch chan-recv           -- parent receives: 99
```

### 3.17 Exec — Shell Command Execution

`exec` runs a shell command and returns a `(stdout, exit-code)` pair.
Requires the `exec` capability (`--allow exec`).

| Word | Capability | Stack Effect | Description |
|------|-----------|-------------|-------------|
| `exec` | `exec` | `(cmd -- (stdout, exit-code))` | Run shell command |

```kore
-- Run a command (requires --allow exec)
"echo hello" exec               -- ("hello\n", 0)

-- Get just stdout
"echo hello" exec unpair drop   -- "hello\n"

-- Check exit code
"exit 1" exec unpair swap drop  -- 1

-- Pipe commands
"ls /tmp | head -5" exec unpair drop  -- first 5 entries
```

### 3.18 Error Handling

Kore provides **Result-style error handling** inspired by Rust's `Result<T, E>`.
Operations that can fail return `Value::Error(msg)` on the stack instead of aborting —
this keeps all operations as S→S tools (P1 compliance).

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `error` | `(str -- error)` | Create an Error value (P1: S→S, no unwinding) |
| `?` | `(a -- a)` | If Error, propagate (re-fail); else pass through |
| `is-error` | `(a -- a bool)` | Check if top is an Error (non-destructive) |
| `try` | `(quote -- result\|error)` | Run quote; catch failures as `Error` values |
| `fail` | `(str -- ∅)` | Raise an error (unwinds to nearest `try`) |

**Key insight**: Partial operations (`/`, `mod`, `fdiv`, `flog`, `get`, `chan-recv`,
`str-get`, `str-slice`) now return `Error` values instead of aborting. Use `?` to
propagate errors upward (like Rust's `?` operator) or `is-error` to check and handle.

```kore
-- Division by zero returns Error value (not abort)
10 0 /                           -- Error("division by zero")
10 0 / is-error                  -- Error("division by zero") true

-- Use ? to propagate errors (Rust-style)
: safe-divide  / ? ;             -- propagates Error to caller's try
[ 10 0 safe-divide ] try is-error -- true

-- Create error values directly
"not found" error                -- Error("not found")
"not found" error ?              -- propagates to nearest try

-- Catch a division by zero
[ 10 0 / ? ] try is-error        -- true (Error propagated by ?)

-- Success passes through ?
[ 10 2 / ? 100 + ] try           -- 105 (no error, ? passes 5 through)

-- Catch an explicit fail
[ "oops" fail ] try to-str       -- "Error: oops"
```

**P4 compliance**: `try` attenuates the error path — it can only catch, never widen what errors can escape.

**stdlib/error.kore helpers**:
```kore
import "error.kore"
42 [ 1 + ] try-or         -- 43 (success: returns result)
10 0 / 99 unwrap-or       -- 99 (Error: use default)
10 2 / 99 unwrap-or       -- 5  (not Error: use value)
"oops" error [ "prefix: " swap str-concat ] map-error  -- Error("prefix: oops")
```

### 3.19 Trigonometry & Power (stdlib/math.kore)

Extended math operations for scientific/geometric computing. All operate on `Float`.

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `fsin` | `(f -- f)` | $\sin(x)$ |
| `fcos` | `(f -- f)` | $\cos(x)$ |
| `fatan2` | `(f f -- f)` | $\text{atan2}(y, x)$ |
| `fpow` | `(f f -- f)` | $x^y$ |
| `ffloor` | `(f -- f)` | $\lfloor x \rfloor$ |
| `fceil` | `(f -- f)` | $\lceil x \rceil$ |
| `fround` | `(f -- f)` | Round to nearest |

```kore
-- Pythagorean identity: sin²(x) + cos²(x) = 1
1.0 dup fsin dup fmul swap fcos dup fmul fadd  -- 1.0

-- Power: 2^10
2.0 10.0 fpow       -- 1024.0

-- atan2 for angle
1.0 1.0 fatan2       -- 0.7854... (π/4)

-- Floor/ceil/round
3.7 ffloor           -- 3.0
3.2 fceil            -- 4.0
3.5 fround           -- 4.0
```

**stdlib/math.kore** provides derived helpers:
```kore
import "math.kore"
pi                        -- 3.14159...
45.0 deg->rad fsin        -- sin(45°) ≈ 0.7071
```

### 3.20 Extended String Operations

Additional string manipulation built-ins beyond the base set in §2.

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `str-split` | `(str delim -- list)` | Split string by delimiter |
| `str-replace` | `(str old new -- str)` | Replace all occurrences |
| `str-upper` | `(str -- str)` | Convert to uppercase |
| `str-lower` | `(str -- str)` | Convert to lowercase |
| `str-trim` | `(str -- str)` | Trim leading/trailing whitespace |

```kore
-- Split
"a,b,c" "," str-split              -- ["a", "b", "c"]

-- Replace
"hello world" "world" "kore" str-replace  -- "hello kore"

-- Case conversion
"hello" str-upper                   -- "HELLO"
"HELLO" str-lower                   -- "hello"

-- Trim
"  hello  " str-trim                -- "hello"
```

**stdlib/string.kore** provides derived helpers:
```kore
import "string.kore"
"hello world" words              -- ["hello", "world"]
( "a" "b" "c" ) ", " str-join   -- "a, b, c"
```

### 3.21 HashMap (Associative Maps)

Hash maps (implemented as `BTreeMap` for deterministic ordering) provide key-value storage.
Keys are strings; values can be any Kore value.

| Word | Stack Effect | Description |
|------|-------------|-------------|
| `map-new` | `( -- map)` | Create empty map |
| `map-get` | `(map key -- map value)` | Get value (non-destructive, returns `nil` if missing) |
| `map-set` | `(map key value -- map)` | Set key-value pair |
| `map-keys` | `(map -- map list)` | Get sorted list of keys |
| `map-has` | `(map key -- map bool)` | Check if key exists |

**IMPORTANT**: `map-get`, `map-keys`, and `map-has` are **non-destructive** — they keep the map on the stack.

```kore
-- Create and populate a map
map-new "name" "Kore" map-set "version" 16 map-set

-- Query
dup "name" map-get               -- map "Kore"
swap "version" map-has           -- map true

-- Get all keys
dup map-keys                     -- map ["name", "version"]

-- Nested maps
map-new "x" 1 map-set
map-new "point" rot map-set      -- {"point": {"x": 1}}
```

**stdlib/map.kore** provides derived helpers:
```kore
import "map.kore"
map-new "a" 1 map-set dup map-count     -- map 1
map-new map-empty?                       -- map true
```

### 3.22 Module Import System

Kore supports importing definitions from other files using the `import` directive.

```kore
import "math.kore"       -- imports from same directory, falls back to stdlib/
import "mylib.kore"      -- imports from same directory
```

**Resolution order**:
1. Relative to the importing file's directory
2. Falls back to the `stdlib/` directory

**Features**:
- Cycle prevention: importing a file that's already been imported is a no-op
- Recursive: imported files can import other files
- Preprocessing: imports are expanded before compilation

```kore
-- file: mylib.kore
: double 2 * ;
: triple 3 * ;

-- file: main.kore
import "mylib.kore"
5 double     -- 10
3 triple     -- 9
```

### 3.23 REPL (Interactive Mode)

Start the Kore REPL (Read-Eval-Print Loop) with no file argument:

```bash
kore --allow io          # start REPL with IO capability
kore --allow all         # start REPL with all capabilities
```

**REPL features**:
- Each line is compiled → optimized → type-checked → executed
- Function definitions persist across lines
- Stack is displayed after each expression
- Special commands: `:help`, `:clear` (reset stack), `exit`/`quit`

```
kore> 2 3 +
[5]
kore> dup *
[25]
kore> : sq dup * ;
[25]
kore> drop 7 sq
[49]
kore> :clear
Stack cleared.
kore> exit
```

### 3.24 Serve Mode (RL Training Protocol)

Serve mode turns `korec` into a **persistent, line-oriented evaluation server** optimized for RL training loops. It reads programs from stdin, executes them in-process (no fork/exec overhead), and returns structured JSON on stdout.

```bash
korec serve
```

**Input format**: One program per line (raw source or JSON):
```
3 4 +
: sq dup * ; 7 sq
{"source": "[ true ] loop", "max_steps": 500}
```

**Output format**: One JSON object per line with rich feedback:
```json
{"ok":true, "stage":"run", "result":"Int(7)", "stack":["Int(7)"], "steps":3, "compile_ok":true, "typecheck_ok":true}
{"ok":false, "stage":"compile", "error":"Unknown word: foo", "steps":0, "compile_ok":false, "typecheck_ok":false}
{"ok":false, "stage":"typecheck", "error":"...", "type_errors":["..."], "steps":0, "compile_ok":true, "typecheck_ok":false}
{"ok":false, "stage":"run", "error":"step limit exceeded (500 steps)", "partial_stack":["Int(3)"], "steps":500, "compile_ok":true, "typecheck_ok":true}
```

**Key fields**:

| Field | Type | Description |
|-------|------|-------------|
| `ok` | bool | Whether execution succeeded |
| `stage` | string | Where it failed/succeeded: `"compile"`, `"typecheck"`, or `"run"` |
| `result` | string | Top-of-stack value on success |
| `stack` | array | Full stack contents on success |
| `partial_stack` | array | Stack snapshot on runtime error |
| `steps` | int | Bytecode instructions executed |
| `compile_ok` | bool | Whether compilation passed |
| `typecheck_ok` | bool | Whether proof checking passed |
| `type_errors` | array | Individual type errors (typecheck failures only) |
| `error` | string | Error message (failures only) |

**Gas limit**: Default 100,000 steps. Override with JSON input `{"source": "...", "max_steps": N}`. Prevents infinite loops from hanging the training process.

**Performance**: ~58,000 programs/sec (simple arithmetic), ~27,000 prog/sec (range+map+filter+fold pipelines).

### 3.25 HTTP & Extended SYSCALLs

New SYSCALL operations for network I/O, filesystem, and HTTP server capabilities.

#### HTTP Client (requires `--allow net`)

| Word | Capability | Stack Effect | Description |
|------|-----------|-------------|-------------|
| `http-get` | `net` | `(url -- (status body))` | HTTP GET request |
| `http-post` | `net` | `(url body -- (status body))` | HTTP POST request |

```kore
-- GET request
"https://httpbin.org/get" http-get
unpair swap drop                       -- response body

-- POST request
"https://httpbin.org/post" "{\"key\":\"val\"}" http-post
unpair drop                            -- status code
```

#### HTTP Server (requires `--allow net`)

| Word | Capability | Stack Effect | Description |
|------|-----------|-------------|-------------|
| `http-serve` | `net` | `(port handler -- ∅)` | Start HTTP server (blocks) |

The handler quote receives `(method path body)` on the stack and must leave `(status response-body)`.

```kore
-- Simple HTTP server on port 8080
: my-handler    -- (method path body -- status response-body)
  drop          -- ignore body
  swap drop     -- ignore method
  -- dispatch on path
  dup "/" = if
    drop 200 "Hello from Kore!"
  else dup "/health" = if
    drop 200 "OK"
  else
    drop 404 "Not Found"
  end end
;

8080 [ my-handler ] http-serve   -- blocks, serving requests
```

**stdlib/http.kore** provides response helpers:
```kore
import "http.kore"
"Hello" http-ok              -- 200 "Hello"
http-not-found               -- 404 "Not Found"
```

#### Extended Filesystem SYSCALLs (requires `--allow fs`)

| Word | Capability | Stack Effect | Description |
|------|-----------|-------------|-------------|
| `readline` | `io` | `(prompt -- str)` | Read line from stdin |
| `file-append` | `fs` | `(path content --)` | Append to file |
| `file-delete` | `fs` | `(path -- bool)` | Delete file, returns success |
| `file-list` | `fs` | `(path -- list)` | List directory entries |

```kore
-- Read user input
"Enter name: " readline        -- reads a line

-- Append to log
"log.txt" "entry\n" file-append

-- List directory
"." file-list                   -- ["file1.txt", "dir/", ...]

-- Delete a file
"temp.txt" file-delete          -- true/false
```

---

## 4. Complete Examples

### 4.1 Fibonacci (While Loop)

```kore
-- Compute F(n) using iterative while loop
-- Input: n on stack. Output: F(n)

: fib  -- (n -- F(n))
  -> n
  0 1    -- F(0) F(1)
  n
  while dup 0 > do
    1 -           -- decrement counter
    swap over +   -- [a b] → [b a+b]
    rot           -- bring counter back to top
  end
  drop            -- remove counter (0)
  drop            -- remove F(n+1), keep F(n)
;

20 fib   -- result: 6765
```

### 4.2 GCD (Euclidean Algorithm)

```kore
: gcd  -- (a b -- gcd)
  while dup 0 != do
    over over %     -- a b (a%b)
    rot drop        -- b (a%b)
    swap            -- (a%b) b → new a=b, new b=a%b... 
                    -- wait, need: swap over mod swap drop
  end
  drop
;

48 18 gcd   -- result: 6
```

### 4.3 Dot Product

```kore
( 1.0 2.0 3.0 ) ( 4.0 5.0 6.0 )
zip [ unpair fmul ] map
0.0 [ fadd ] fold
-- result: 32.0  (1*4 + 2*5 + 3*6)
```

### 4.4 2×2 Matrix Multiply

```kore
-- A = [[1,2],[3,4]], B = [[5,6],[7,8]]
-- C = A*B = [[19,22],[43,50]]

: dot2  -- (a0 a1 b0 b1 -- a0*b0+a1*b1)
  -> b1 -> b0 -> a1 -> a0
  a0 b0 fmul a1 b1 fmul fadd
;

1.0 2.0 5.0 7.0 dot2   -- C[0][0] = 19
1.0 2.0 6.0 8.0 dot2   -- C[0][1] = 22
3.0 4.0 5.0 7.0 dot2   -- C[1][0] = 43
3.0 4.0 6.0 8.0 dot2   -- C[1][1] = 50
collect 4               -- [19, 22, 43, 50]
```

### 4.5 Forward-Mode Autodiff

```kore
-- Compute d/dx(x² + 2x) at x=3
-- f(x) = x² + 2x
-- f'(x) = 2x + 2
-- At x=3: f(3)=15, f'(3)=8

3.0 dvar          -- (3.0, 1.0)  — x is the variable
dup dmul          -- (9.0, 6.0)  — x²
3.0 dvar          -- (3.0, 1.0)
2.0 dconst dmul   -- (6.0, 2.0)  — 2x
dadd              -- (15.0, 8.0) — x² + 2x
undual            -- 15.0 8.0
-- TOS=8.0 (derivative), below=15.0 (value)
```

### 4.6 Newton's Square Root (While Loop)

```kore
: isqrt  -- (n -- floor(√n))
  -> n
  n          -- initial guess = n
  while dup dup * n > do
    n over /    -- n/guess
    +           -- guess + n/guess
    2 /         -- (guess + n/guess) / 2
  end
;

144 isqrt   -- result: 12
```

---

## 5. Compilation and Execution

### 5.1 Compile a Program

```bash
cargo run -- compile program.kore -o program.korec
```

Output: Proof-checked bytecode file. Shows disassembly.
If proof checker finds errors, compilation is rejected.

### 5.2 Run a Program

```bash
cargo run -- run program.korec
```

Output: Final stack state.

### 5.3 GPU Execution (SPIR-V)

```bash
cargo run -- gpu-map program.kore
```

For programs using `map` — compiles the map body to SPIR-V compute shader.

### 5.4 JIT Compilation (Cranelift)

```bash
cargo run -- jit program.kore
```

Compiles to native code via Cranelift. ~21× faster than interpreter.

---

## 6. Type System

### 6.1 Type Lattice

```
         ⊤ (Top — any value)
        /|\
       / | \
    Int Float Bool Str Unit
       \ | /
        \|/
         ⊥ (Bottom — no value)
```

- `Int ≤ Float` (integers promote to floats)
- `⊥ ≤ T ≤ ⊤` for all types T
- `Top` accepts any value (P4 attenuation)

### 6.2 Compound Types

| Type | Syntax | Example |
|------|--------|---------|
| Pair | `(A × B)` | `(Int × Float)` |
| Sum | `(A + B)` | `(Int + Str)` |
| List | `[A]` | `[Int]`, `[Float]` |
| Quote | `[effect]` | `[( -- )]` |

### 6.3 Proof Checker Behavior

The proof checker simulates execution at the type level:
- Tracks the type of every stack position
- Verifies inputs match expected types
- Rejects programs with type mismatches or stack underflows
- For function calls (CALL), conservatively pushes `Top`
- For backward jumps (loops), does not re-check (P4: once valid, always valid)

---

## 7. Bytecode Reference

### 7.1 Opcode Table

| Hex | Name | Stack Effect | Operand |
|-----|------|-------------|---------|
| **Stack** | | | |
| 0x00 | NOP | `( -- )` | — |
| 0x01 | DROP | `(a -- )` | — |
| 0x02 | DUP | `(a -- a a)` | — |
| 0x03 | SWAP | `(a b -- b a)` | — |
| 0x04 | ROT | `(a b c -- b c a)` | — |
| 0x05 | OVER | `(a b -- a b a)` | — |
| **Data** | | | |
| 0x10 | PAIR | `(a b -- (a,b))` | — |
| 0x11 | UNPAIR | `((a,b) -- a b)` | — |
| 0x12 | LEFT | `(a -- Left(a))` | — |
| 0x13 | RIGHT | `(a -- Right(a))` | — |
| 0x14 | CASE | `(Sum -- ...)` | i16 i16 |
| **Control** | | | |
| 0x20 | QUOTE | `( -- quote)` | u16 (body length) |
| 0x21 | APPLY | `(quote -- ...)` | — |
| 0x22 | CALL | `( -- ...)` | u16 (symbol index) |
| 0x23 | RET | `( -- )` | — |
| **Literals** | | | |
| 0x30 | INT8 | `( -- int)` | i8 |
| 0x31 | INT16 | `( -- int)` | i16 |
| 0x32 | INT32 | `( -- int)` | i32 |
| 0x33 | INT64 | `( -- int)` | i64 |
| 0x34 | F32 | `( -- float)` | f32 |
| 0x35 | F64 | `( -- float)` | f64 |
| 0x36 | STR | `( -- str)` | u16 len + bytes |
| 0x37 | NIL | `( -- nil)` | — |
| 0x38 | TRUE | `( -- true)` | — |
| 0x39 | FALSE | `( -- false)` | — |
| **Arithmetic** | | | |
| 0x40 | ADD | `(a b -- a+b)` | — |
| 0x41 | SUB | `(a b -- a-b)` | — |
| 0x42 | MUL | `(a b -- a*b)` | — |
| 0x43 | DIV | `(a b -- a/b)` | — |
| 0x44 | MOD | `(a b -- a%b)` | — |
| 0x45 | NEG | `(a -- -a)` | — |
| 0x46 | FADD | `(f f -- f)` | — |
| 0x47 | FSUB | `(f f -- f)` | — |
| 0x48 | FMUL | `(f f -- f)` | — |
| 0x49 | FDIV | `(f f -- f)` | — |
| 0x4A | FNEG | `(f -- f)` | — |
| 0x4B | FSQRT | `(f -- f)` | — |
| 0x4C | FABS | `(f -- f)` | — |
| 0x4D | I2F | `(i -- f)` | — |
| 0x4E | F2I | `(f -- i)` | — |
| 0x4F | FEXP | `(f -- f)` | — |
| 0x56 | FLOG | `(f -- f)` | — |
| 0x57 | FSIN | `(f -- f)` | — |
| 0x58 | FCOS | `(f -- f)` | — |
| 0x59 | FATAN2 | `(f f -- f)` | — |
| 0x5A | FPOW | `(f f -- f)` | — |
| 0x5B | FFLOOR | `(f -- f)` | — |
| 0x5C | FCEIL | `(f -- f)` | — |
| 0x5D | FROUND | `(f -- f)` | — |
| **Comparison** | | | |
| 0x50 | EQ | `(a b -- bool)` | — |
| 0x51 | LT | `(a b -- bool)` | — |
| 0x52 | GT | `(a b -- bool)` | — |
| 0x53 | LE | `(a b -- bool)` | — |
| 0x54 | GE | `(a b -- bool)` | — |
| 0x55 | NE | `(a b -- bool)` | — |
| **Logic** | | | |
| 0x60 | AND | `(b b -- b)` | — |
| 0x61 | OR | `(b b -- b)` | — |
| 0x62 | NOT | `(b -- b)` | — |
| 0x63 | XOR | `(b b -- b)` | — |
| **Jumps** | | | |
| 0x70 | JMP | `( -- )` | i16 (relative) |
| 0x71 | JZ | `(bool -- )` | i16 (relative) |
| 0x72 | JNZ | `(bool -- )` | i16 (relative) |
| **Lists** | | | |
| 0x80 | LIST | `(v₁..vₙ -- list)` | u16 (count) |
| 0x81 | UNLIST | `(list -- v₁..vₙ)` | — |
| 0x82 | LEN | `(list -- list n)` | — |
| 0x83 | GET | `(list i -- elem)` | — |
| 0x84 | SET | `(list i v -- list')` | — |
| 0x85 | MAP | `(list q -- list')` | — |
| 0x86 | FOLD | `(list init q -- r)` | — |
| 0x87 | ZIP | `(list list -- list)` | — |
| 0x88 | APPEND | `(list v -- list')` | — |
| 0x89 | REVERSE | `(list -- list')` | — |
| **Fibers** | | | |
| 0xB0 | FIBER_NEW | `(quote -- fiber)` | — |
| 0xB1 | FIBER_STEP | `(fiber -- fiber' bool)` | — |
| 0xB2 | FIBER_PUSH | `(fiber v -- fiber')` | — |
| 0xB3 | FIBER_STACK | `(fiber -- fiber list)` | — |
| 0xB4 | FIBER_STATUS | `(fiber -- fiber bool)` | — |
| **Spawn/Channels** | | | |
| 0xB5 | SPAWN | `(quote caps -- list)` | — |
| 0xB6 | CHAN_NEW | `(-- chan)` | — |
| 0xB7 | CHAN_SEND | `(chan value --)` | — |
| 0xB8 | CHAN_RECV | `(chan -- value)` | — |
| **Linear Types** | | | |
| 0xC0 | LINEAR | `(a -- Linear(a))` | — |
| 0xC1 | AFFINE | `(a -- Affine(a))` | — |
| 0xC2 | CONSUME | `(Linear(a) -- a)` | — |
| 0xC3 | IS_LINEAR | `(a -- a bool)` | — |
| 0xC4 | IS_AFFINE | `(a -- a bool)` | — |
| **Introspection** | | | |
| 0xD0 | TYPE_OF | `(a -- a str)` | — |
| 0xD1 | DEPTH | `( -- n)` | — |
| 0xD2 | DESCRIBE | `(str -- str)` | — |
| **String** | | | |
| 0xE0 | STR_LEN | `(str -- str n)` | — |
| 0xE1 | STR_GET | `(str i -- str)` | — |
| 0xE2 | STR_CONCAT | `(str str -- str)` | — |
| 0xE3 | STR_SLICE | `(str start end -- str)` | — |
| 0xE4 | TO_STR | `(a -- str)` | — |
| 0xE5 | STR_FIND | `(str pattern -- int)` | — |
| 0xE6 | STR_SPLIT | `(str delim -- list)` | — |
| 0xE7 | STR_REPLACE | `(str old new -- str)` | — |
| 0xE8 | STR_UPPER | `(str -- str)` | — |
| 0xE9 | STR_LOWER | `(str -- str)` | — |
| 0xEA | STR_TRIM | `(str -- str)` | — |
| **Locals** | | | |
| 0xA8 | STORE | `(v -- )` | u8 (slot) |
| 0xA9 | LOAD | `( -- v)` | u8 (slot) |
| **HashMap** | | | |
| 0xAA | MAP_NEW | `( -- map)` | — |
| 0xAB | MAP_GET | `(map key -- map value)` | — |
| 0xAC | MAP_SET | `(map key value -- map)` | — |
| 0xAD | MAP_KEYS | `(map -- map list)` | — |
| 0xAE | MAP_HAS | `(map key -- map bool)` | — |
| **SYSCALL** | | | |
| 0xAF | SYSCALL | varies | u8 (call_id) |
| **IO** | | | |
| 0xA0 | PRINT | `(a --)` | — |
| 0xA1 | PRINTLN | `(a --)` | — |
| 0xA2 | RAND | `( -- int)` | — |
| **Error Handling** | | | |
| 0xA3 | TRY | `(quote -- result\|error)` | — |
| 0xA4 | FAIL | `(str -- ∅)` | — |
| 0xA5 | IS_ERROR | `(a -- a bool)` | — |
| 0xA6 | PROPAGATE | `(a -- a)` | re-fail if Error (Rust `?`) |
| 0xA7 | MAKE_ERROR | `(str -- error)` | create Error value (P1) |
| **Reflection** | | | |
| 0x90 | FETCH | `(i -- byte)` | — |
| 0x91 | SIZE | `( -- n)` | — |
| **Extension** | | | |
| 0xF0 | EXT | varies | — |
| 0xFF | HALT | `( -- )` | — |

### 7.2 Binary Format (.korec)

```
Offset  Size  Field
0x00    4     Magic: "KORE" (0x4B 0x4F 0x52 0x45)
0x04    1     Version major
0x05    1     Version minor
0x06    1     Cap flags (0x01=io, 0x02=fs, 0x04=net)
0x07    1     Reserved
0x08    4     Code section offset (usually 32)
0x0C    4     Code section length
0x10    4     Data section offset (0 = unused)
0x14    4     Data section length (0 = unused)
0x18    4     Symbol table offset
0x1C    4     Symbol table length
0x20    ...   Code section (bytecode)
...     ...   Symbol table section
```

---

## 8. Patterns and Idioms

### 8.1 Accumulator Pattern (Fold)

To reduce a list to a single value:
```kore
list initial-value [ combine-function ] fold
```

Examples:
```kore
( 1 2 3 4 5 ) 0 [ + ] fold          -- sum: 15
( 1 2 3 4 5 ) 1 [ * ] fold          -- product: 120
( 3 1 4 1 5 ) 0 [ max ] fold        -- max: 5
```

### 8.2 Transform Pattern (Map)

To transform each element:
```kore
list [ transform-function ] map
```

Examples:
```kore
( 1 2 3 ) [ 2 * ] map               -- [2, 4, 6]
( 1 4 9 ) [ i2f fsqrt ] map         -- [1.0, 2.0, 3.0]
( 1 2 3 ) [ dup * ] map             -- [1, 4, 9]
```

### 8.3 Filter Pattern (Map + Fold)

Kore has no built-in `filter`. Build it:
```kore
: filter  -- (list predicate-quote -- filtered-list)
  -- Use map to tag elements, then fold to collect
  -- For now, manual approach:
  swap ( ) rot rot   -- acc list predicate
  -- This is complex without locals. Better with -> syntax:
;
```

With locals:
```kore
: keep-positives  -- (list -- list')
  -> xs
  ( )
  xs [ dup 0 > if ( ) swap 1 collect else drop ( ) end ] map
  -- flatten... (needs concat, coming in future phases)
;
```

### 8.4 Loop Counter Pattern

```kore
-- Do something N times
N
while dup 0 > do
  1 -         -- decrement
  -- body here (counter is NOT on top during body)
  -- if you need counter: dup before the body
end
drop          -- remove the 0
```

### 8.5 Stack Manipulation for 2 Values

Common patterns:
```kore
-- Apply binary op keeping originals:
over over op    -- (a b -- a b result)

-- Apply binary op consuming:
op              -- (a b -- result)

-- Swap and apply:
swap op         -- (a b -- b-op-a)
```

### 8.6 Naming Values (Local Variables)

When you have 3+ values to juggle, use locals:
```kore
: quadratic  -- (a b c x -- result)
  -> x -> c -> b -> a
  a x dup * fmul          -- ax²
  b x fmul fadd           -- ax² + bx
  c fadd                  -- ax² + bx + c
;
```

---

## 9. Error Messages

### 9.1 Compile-Time (Proof Checker)

| Error | Cause | Fix |
|-------|-------|-----|
| `Stack underflow` | Consuming from empty stack | Push values before using them |
| `Expected Float, got Int` | Float op on Int | Use `i2f` to convert |
| `Expected Bool, got Int` | Logic op on Int | Use comparison to produce Bool |
| `UNPAIR expects Pair` | Unpair on non-pair | Ensure pair is on stack |
| `Unknown word: xxx` | Undefined function | Define with `: xxx ... ;` |
| `Unterminated 'if'` | Missing `end` | Add `end` keyword |

### 9.2 Runtime

| Error | Cause | Fix |
|-------|-------|-----|
| `stack underflow` | Pop from empty stack | Logic error in function |
| `division by zero` | Divide by 0 | Check divisor before `/` |
| `index out of bounds` | List index too large | Check with `len` first |
| `expected float, got int` | Float op got Int | Use `i2f` |

---

## 10. Design Principles for Tool Authors

### 10.1 One Tool, One Job (P1)

Each tool should do exactly one transformation. No side effects.

```kore
-- GOOD: one clear transformation
: double 2 * ;
: square dup * ;

-- BAD: does too many things
: process-and-print dup * 2 + print ;  -- mixing computation and IO
```

### 10.2 Composition Over Complexity (P3)

Build complex tools by composing simple ones:

```kore
: sum-of-squares     -- (list -- n)
  [ dup * ] map      -- square each
  0 [ + ] fold       -- sum them
;
```

### 10.3 Types Narrow, Never Widen (P4)

If a tool expects `Float`, it will never accept `String` at runtime.
The proof checker enforces this at compile time.

```kore
-- The proof checker ensures:
-- fsqrt only gets Float (or Int promoted to Float)
-- and only gets Bool
-- get only operates on Lists
```

---

## Appendix: Identifier Rules

Valid identifier characters: `a-z A-Z 0-9 _ - ?`
Must start with letter or `_`.

Valid: `my-func`, `is-positive?`, `dot_product`, `x2`
Invalid: `2fast` (starts with digit), `a+b` (contains operator)

## Appendix: Reserved Words

These cannot be used as function names:
```
true false nil
if else end while do
drop dup swap rot over
add sub mul div mod neg
fadd fsub fmul fdiv fneg fsqrt fabs fexp flog
fsin fcos fatan2 fpow ffloor fceil fround
i2f f2i
lt gt le ge eq ne
and or not xor
pair unpair left right case
apply ret return halt nop
collect unlist len get set
map fold zip append reverse
filter head tail empty? concat range list-concat times
first second
fiber-new fiber-step fiber-push fiber-stack fiber-status
linear affine consume is-linear is-affine
type-of depth describe
str-len str-get str-concat str-slice to-str str-find
str-split str-replace str-upper str-lower str-trim
try fail is-error
map-new map-get map-set map-keys map-has
file-read file-write file-exists time-now env-get exec
readline file-append file-delete file-list
http-get http-post http-serve
spawn chan-new chan-send chan-recv
import
```
