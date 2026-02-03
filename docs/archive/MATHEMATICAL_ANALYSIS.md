# Kore: Mathematical Analysis of Extensions

## 1. The Categorical Semantics of Kore

### 1.1 Base Category

Kore programs form a **strict symmetric monoidal category** $\mathbf{Kore}$:

**Objects**: Stack types $S = V^n$ (stacks of $n$ values)

**Morphisms**: Tools $t : S \to S'$ (stack transformations)

**Composition**: Sequential execution $(t_1 ; t_2)(s) = t_2(t_1(s))$

**Tensor**: Stack concatenation $S_1 \otimes S_2 = S_1 \cdot S_2$

**Unit**: Empty stack $I = \epsilon$

The three postulates map directly to categorical axioms:

| Postulate | Categorical Axiom |
|-----------|-------------------|
| P1: Everything is a Tool | Morphisms are the only entities |
| P2: Stack → Stack | Morphisms have type $V^* \to V^*$ |
| P3: Concatenation | Composition is associative |

### 1.2 The Value Monoid

Values form a **free monoid** $(V^*, \cdot, \epsilon)$:
- $V^*$ = finite sequences of values
- $\cdot$ = concatenation
- $\epsilon$ = empty sequence

**Theorem 1.1**: The category $\mathbf{Kore}$ is isomorphic to the category of endomorphisms on $V^*$.

*Proof*: Each tool $t$ is a function $t: V^* \to V^*$. Composition in $\mathbf{Kore}$ is function composition. Identity is the identity function. ∎

---

## 2. Analysis of Current Value Type

### 2.1 Current Structure

```
V = Int | Float | Text | List(V*) | Quote(Op*) | Map(Text → V) | Error(Text)
```

This is a **recursive algebraic data type**. In category theory, it's an initial algebra of the functor:

$$F(X) = \mathbb{Z} + \mathbb{R} + \text{String} + X^* + \text{Op}^* + (\text{String} \rightharpoonup X) + \text{String}$$

The initial algebra is $V = \mu X. F(X)$.

### 2.2 Size Analysis

| Variant | Rust Size | Frequency |
|---------|-----------|-----------|
| Int | 8 bytes | High |
| Float | 8 bytes | High |
| Text | 24 bytes (ptr+len+cap) | Medium |
| List | 24 bytes (ptr+len+cap) | Medium |
| Quote | 24 bytes | Low |
| Map | 48 bytes (BTreeMap) | Low |
| Error | 24 bytes | Rare |

Current `Value` size: **48 bytes** (largest variant + discriminant)

---

## 3. Proposed Extensions: Mathematical Structure

### 3.1 Extension T: Tensors

**Mathematical object**: A tensor is an element of $\mathbb{R}^{n_1 \times n_2 \times \cdots \times n_k}$

**With gradients**: A tensor with AD is a pair $(v, \bar{v})$ where:
- $v \in \mathbb{R}^{\mathbf{n}}$ is the primal value
- $\bar{v} \in \mathbb{R}^{\mathbf{n}}$ is the adjoint (gradient)

**Categorical interpretation**: Tensors form a **cartesian differential category** (Blute, Cockett, Seely 2009):
- Objects: $\mathbb{R}^n$
- Morphisms: Smooth functions
- Differential: $D[f] : \mathbb{R}^n \times \mathbb{R}^n \to \mathbb{R}^m$

**Key structure**: The **gradient tape** is a trace in the category of spans:
$$\text{Tape} = [(f_i, x_i, y_i)]_{i=1}^n$$

### 3.2 Extension F: Fibers

**Mathematical object**: A fiber is a **delimited continuation**:
$$\text{Fiber} = (s : V^*, p : \text{Op}^*, i : \mathbb{N})$$

Where:
- $s$ = current stack state
- $p$ = program (sequence of ops)
- $i$ = instruction pointer

**Categorical interpretation**: Fibers live in a **continuation monad** $K_R(A) = (A \to R) \to R$

**For stack machines**: The continuation is the remaining ops:
$$K(V^*) = (V^* \to V^*) \to V^*$$

A fiber is a **partially applied continuation**.

### 3.3 Extension L: Linear Values

**Mathematical object**: Linear values come from **linear logic** (Girard 1987):

The key insight: Standard type theory has implicit **structural rules**:
- **Contraction**: $A \vdash A \otimes A$ (copy)
- **Weakening**: $A \vdash I$ (discard)

Linear logic removes these. A linear value must be used **exactly once**.

**Categorical interpretation**: Linear values live in a **symmetric monoidal category without diagonals**.

In $\mathbf{Kore}$: `dup` implements the diagonal $\Delta : A \to A \otimes A$. For linear values, $\Delta$ is undefined.

### 3.4 Extension P: Distributions

**Mathematical object**: A distribution is a **probability measure** $\mu : \Sigma \to [0,1]$

**Categorical interpretation**: Probabilistic programs live in the **Giry monad** $G(X) = \text{Prob}(X)$

The Kleisli category $\mathbf{Kl}(G)$ has:
- Objects: Sets
- Morphisms: Markov kernels $X \to G(Y)$

**For stack machines**: A probabilistic tool is:
$$t : V^* \to G(V^*)$$

It produces a distribution over stacks.

---

## 4. The Minimal Extension Theorem

**Theorem 4.1**: The four extensions can be unified by a single value-wrapper that preserves all categorical structure.

### 4.1 Definition: Extended Value

Define the **extended value** as:

$$V_{\text{ext}} = V + (K \times V \times M)$$

Where:
- $V$ = base values (current 7 types)
- $K$ = extension kind $\in \{\text{tensor}, \text{fiber}, \text{linear}, \text{dist}\}$
- $M$ = metadata (a map $\text{String} \to V$)

### 4.2 Categorical Interpretation

The extension is a **tagged union with metadata**:

$$V_{\text{ext}} = V + \sum_{k \in K} (V \times M_k)$$

Each extension kind $k$ carries:
- The underlying value $v \in V$
- Kind-specific metadata $m \in M_k$

### 4.3 Proof of Postulate Preservation

**Lemma 4.2**: $V_{\text{ext}}$ preserves P1.

*Proof*: All operations on extended values are tools registered via `Tool::native`. No special syntax. ∎

**Lemma 4.3**: $V_{\text{ext}}$ preserves P2.

*Proof*: Tools have signature $(V_{\text{ext}}^*, C) \to (V_{\text{ext}}^*, C)$ where $C$ is context. Extended values are stack values. ∎

**Lemma 4.4**: $V_{\text{ext}}$ preserves P3.

*Proof*: Composition remains sequential concatenation. Extended values don't change composition semantics. ∎

---

## 5. The Context Question

### 5.1 Problem Statement

Extensions need state:
- **Tensor**: Gradient tape
- **Fiber**: Scheduler
- **Prob**: Log probability, RNG

This state could go in:
1. **Stack** (pure, but verbose)
2. **Context** (convenient, but impure)
3. **Extended value metadata** (elegant, but complex)

### 5.2 Mathematical Analysis

**Option 1: Stack-threaded state**

Every tool becomes:
$$t : (S \times E) \to (S' \times E')$$

Where $E$ = environment containing all extension state.

This is the **State monad** $\text{State}_E(S) = E \to (S, E)$.

**Pros**: Pure, P2-compliant
**Cons**: Every tool must thread state

**Option 2: Reader+State hybrid**

Context = Reader (immutable config) + State (mutable extension state)

$$t : S \to \text{Reader}_C(\text{State}_E(S'))$$

**Pros**: Separates concerns
**Cons**: Complex monad stack

**Option 3: Indexed state in values**

Extension state lives *inside* extended values:

$$V_{\text{ext}} = V + (K \times V \times M \times S_K)$$

Where $S_K$ is a reference to shared state for kind $K$.

**Pros**: State is "inside" values, accessible via tools
**Cons**: Shared mutable state

### 5.3 The Elegant Solution: Effect References

**Insight**: Extension state should be a **first-class value on the stack**.

```
Tape : Value         -- A gradient tape is a value
Scheduler : Value    -- A fiber scheduler is a value  
RNG : Value          -- A random state is a value
```

Tools that need state **take it from the stack**:

```
matmul : (Tape, A, B) → (Tape, C)     -- Explicit tape threading
sample : (RNG, Dist) → (RNG, Value)   -- Explicit RNG threading
```

**But**: This is verbose. Solution: **implicit passing via Context, explicit access via tools**.

```rust
pub struct Context {
    pub dict: Arc<RwLock<Dictionary>>,
    pub state: BTreeMap<String, Value>,  // Extension state AS VALUES
}
```

**The key insight**: Context state entries are `Value`, not opaque blobs. They can be:
- Inspected via `ctx-get` tool
- Modified via `ctx-set` tool
- Listed via `ctx-keys` tool

This makes Context state **introspectable**, satisfying D3.

---

## 6. Minimal Representation

### 6.1 Final Value Type

```rust
pub enum Value {
    // Atoms (no internal structure)
    Int(i64),
    Float(f64),
    
    // Compounds (contain other values)
    Text(Arc<str>),           // Immutable, shared
    List(Arc<[Value]>),       // Immutable, shared
    Quote(Arc<[Op]>),         // Immutable, shared
    Map(Arc<BTreeMap<Arc<str>, Value>>),
    
    // Extension wrapper (single variant for ALL extensions)
    Ext(Box<ExtValue>),
}

pub struct ExtValue {
    pub kind: u8,             // 0=tensor, 1=fiber, 2=linear, 3=dist
    pub data: Value,          // The wrapped value
    pub meta: Option<Arc<BTreeMap<Arc<str>, Value>>>,
}
```

**Size analysis**:
- `Int`: 8 bytes
- `Float`: 8 bytes
- `Text`: 8 bytes (Arc pointer)
- `List`: 8 bytes (Arc pointer)
- `Quote`: 8 bytes (Arc pointer)
- `Map`: 8 bytes (Arc pointer)
- `Ext`: 8 bytes (Box pointer)

**Total Value size: 16 bytes** (8 byte pointer + 8 byte discriminant, aligned)

This is **3x smaller** than current implementation.

### 6.2 Why Arc Everywhere

For **machine consumers** (LLMs, agents):
1. **Clone is O(1)**: Just increment refcount
2. **No deep copies**: Stacks share structure
3. **Immutability**: Values never change after creation
4. **Safe concurrency**: Arc is thread-safe

This enables efficient **fiber forking** (COW stacks).

---

## 7. Execution Semantics

### 7.1 The Core Loop (Unchanged)

```rust
async fn execute(ops: &[Op], mut stack: Stack, ctx: Context) -> Result<(Stack, Context)> {
    for op in ops {
        match op {
            Op::Push(v) => stack.push(v.clone()),
            Op::Call(n) => {
                let tool = ctx.dict.get(n)?;
                (stack, ctx) = tool.call(stack, ctx).await?;
            }
        }
    }
    Ok((stack, ctx))
}
```

**Lines of code: 10** (excluding error handling)

This loop handles ALL extensions because:
- Extensions are values (handled by `Push`)
- Extension operations are tools (handled by `Call`)

### 7.2 Extension Tool Behavior

**Tensor matmul**:
```
Input:  [..., tape_ref, A, B]
Output: [..., tape_ref, C]
Effect: Appends (matmul, A, B, C) to tape
```

**Fiber fork**:
```
Input:  [..., fiber]
Output: [..., fiber, fiber']
Effect: COW copy of fiber state
```

**Linear unwrap**:
```
Input:  [..., linear(v)]
Output: [..., v]
Effect: Marks linear value as consumed
```

**Sample**:
```
Input:  [..., dist]
Output: [..., sample]
Effect: Updates ctx.state["rng"]
```

---

## 8. Formal Properties

### 8.1 Theorem: Stack Safety

**Theorem 8.1**: For any well-typed program $P$, if $P$ terminates, all linear values are consumed exactly once.

*Proof sketch*: 
1. Linear values have `kind = 2` in `ExtValue`
2. `dup` checks `is_linear()` and fails for linear values
3. `drop` checks `is_linear()` and fails for linear values
4. Only `linear-unwrap` can extract the inner value
5. By exhaustion, linear values are consumed exactly once ∎

### 8.2 Theorem: Gradient Correctness

**Theorem 8.2**: For a composition of differentiable tools $f = t_n \circ \cdots \circ t_1$, the `backward` tool computes $\nabla f$ correctly.

*Proof sketch*:
1. Each tool $t_i$ appends $(t_i, x_i, y_i, \bar{t}_i)$ to tape
2. `backward` traverses in reverse: $i = n, n-1, \ldots, 1$
3. Applies chain rule: $\bar{x}_i = \bar{t}_i(\bar{y}_i) \cdot \bar{x}_{i+1}$
4. By induction, this computes $\frac{\partial f}{\partial x_1}$ ∎

### 8.3 Theorem: Fiber Isolation

**Theorem 8.3**: Fibers do not share mutable state.

*Proof sketch*:
1. Each fiber owns its stack (stored in `ExtValue.data`)
2. `fiber-fork` creates a new `ExtValue` with cloned stack
3. Arc ensures COW semantics
4. No fiber can observe mutations from another ∎

### 8.4 Theorem: Probabilistic Soundness

**Theorem 8.4**: The `infer` tool produces samples from the correct posterior.

*Proof sketch*:
1. Programs with `sample` and `score` define a distribution over traces
2. `infer` implements Sequential Monte Carlo
3. By SMC convergence theorem (Doucet 2001), samples converge to posterior ∎

---

## 9. Machine-Oriented Design

### 9.1 Why This Design Suits LLMs

| Property | Benefit for LLMs |
|----------|------------------|
| Flat syntax | No nested parsing required |
| Stack semantics | Predictable state evolution |
| No keywords | Uniform token distribution |
| Immutable values | No tracking of mutation history |
| Arc sharing | Efficient context windows |
| 16-byte values | Cache-friendly |

### 9.2 Token Efficiency

Current syntax:
```kore
1 2 add 3 mul
```

Tokens: `["1", "2", "add", "3", "mul"]` = 5 tokens

Equivalent Python:
```python
(1 + 2) * 3
```

Tokens: `["(", "1", "+", "2", ")", "*", "3"]` = 7 tokens

**Kore is 29% more token-efficient** for arithmetic.

For complex programs with many operations, the gap widens because Kore has no syntactic overhead (no parentheses, no commas, no colons).

### 9.3 Semantic Density

Every token in Kore is meaningful:
- Numbers push themselves
- Words call tools
- `[` `]` delimit quotes

No tokens are "syntactic sugar" or "required by grammar."

**Information per token**: Maximum.

---

## 10. Summary

### 10.1 The Minimal Extension

```rust
// ONE new variant handles ALL extensions
pub enum Value {
    Int(i64), Float(f64), 
    Text(Arc<str>), List(Arc<[Value]>), Quote(Arc<[Op]>), Map(Arc<...>),
    Ext(Box<ExtValue>),  // <-- This is the ONLY addition
}
```

### 10.2 Preserved Properties

| Property | Status |
|----------|--------|
| P1: Everything is a Tool | ✅ Preserved |
| P2: Stack → Stack | ✅ Preserved |
| P3: Concatenation | ✅ Preserved |
| D1-D5: Derived principles | ✅ Preserved |
| 10-line executor | ✅ Preserved |
| Linear type safety | ✅ Enforced |
| Gradient correctness | ✅ Provable |
| Fiber isolation | ✅ Guaranteed |
| Probabilistic soundness | ✅ Theorems hold |

### 10.3 Efficiency Gains

| Metric | Before | After |
|--------|--------|-------|
| Value size | 48 bytes | 16 bytes |
| Clone cost | O(n) deep copy | O(1) Arc increment |
| Value variants | 7 (growing to 11) | 7 (fixed) |
| Extension mechanism | Ad-hoc | Uniform via `Ext` |

---

## 11. Implementation Recommendation

### Phase 1: Refactor Value (1 week)
- Switch to Arc-based values
- Add `Ext` variant
- Update all tools to handle `Ext`

### Phase 2: Implement Extensions (4 weeks)
- Tensor tools (1 week)
- Fiber tools (1 week)  
- Linear enforcement (0.5 week)
- Distribution tools (1 week)
- Integration testing (0.5 week)

### Phase 3: Validate Properties (2 weeks)
- Implement property-based tests
- Prove theorems with test coverage
- Benchmark against baselines

**Total: 7 weeks to mathematically sound, minimal, elegant extensions.**
