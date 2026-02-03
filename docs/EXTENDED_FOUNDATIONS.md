# Extended Foundations: Differentiable, Probabilistic, and Quantum Kore

A rigorous mathematical analysis of extending Kore while preserving its algebraic properties.

---

## Table of Contents

1. [Review: The Kore Axioms](#1-review-the-kore-axioms)
2. [Theorem: Differentiability Preserves the Axioms](#2-theorem-differentiability-preserves-the-axioms)
3. [Extension: Probabilistic Kore](#3-extension-probabilistic-kore)
4. [Extension: Quantum Kore](#4-extension-quantum-kore)
5. [Extension: Linear Kore](#5-extension-linear-kore)
6. [Unified Framework: Effectful Kore](#6-unified-framework-effectful-kore)
7. [Implementation Implications](#7-implementation-implications)
8. [Conclusion](#8-conclusion)

---

## 1. Review: The Kore Axioms

### 1.1 The Three Postulates

**Postulate 1 (Everything is a Tool):**
$$
\forall f \in \text{Kore}: f \text{ is a Tool}
$$

**Postulate 2 (Tools Transform Stacks):**
$$
\text{Tool} : \mathcal{S} \to \mathcal{S}
$$
where $\mathcal{S} = \mathcal{V}^*$ is the set of stacks (finite sequences of values).

**Postulate 3 (Composition is Concatenation):**
$$
\llbracket P_1 \cdot P_2 \rrbracket = \llbracket P_2 \rrbracket \circ \llbracket P_1 \rrbracket
$$

### 1.2 The Semantic Homomorphism

The denotation function is a monoid homomorphism:
$$
\llbracket \cdot \rrbracket : (\mathcal{T}^*, \cdot, \epsilon) \to (\mathcal{S} \rightharpoonup \mathcal{S}, \circ, \text{id})
$$

**Properties:**
- $\llbracket \epsilon \rrbracket = \text{id}$
- $\llbracket P_1 \cdot P_2 \rrbracket = \llbracket P_2 \rrbracket \circ \llbracket P_1 \rrbracket$

### 1.3 The Statelessness Property

A tool $f$ is **stateless** iff:
$$
\forall s \in \mathcal{S}: f(s) \text{ depends only on } s
$$

Equivalently: $f$ is a pure function with no side effects.

---

## 2. Theorem: Differentiability Preserves the Axioms

### 2.1 Extended Value Domain

**Definition 2.1 (Tensor Values):**

Let $\mathbb{T}$ be the set of tensors:
$$
\mathbb{T} = \bigcup_{n \geq 0} \mathbb{R}^{d_1 \times d_2 \times \cdots \times d_n}
$$

Each tensor $t \in \mathbb{T}$ may carry gradient metadata:
$$
t = (D, G, \phi)
$$
where:
- $D \in \mathbb{R}^{d_1 \times \cdots \times d_n}$ is the data
- $G \in \mathbb{R}^{d_1 \times \cdots \times d_n} \cup \{\bot\}$ is the gradient (or undefined)
- $\phi : \mathbb{R}^{d_1 \times \cdots \times d_n} \to \mathbb{R}^{*}$ is the gradient function (a pure function)

**Definition 2.2 (Extended Value Set):**
$$
\mathcal{V}' = \mathcal{V} \cup \mathbb{T}
$$

The extended stack domain:
$$
\mathcal{S}' = (\mathcal{V}')^*
$$

### 2.2 Differentiable Tools

**Definition 2.3 (Differentiable Tool):**

A differentiable tool $f_\partial$ is a tool operating on $\mathcal{S}'$ such that:

1. **Forward pass:** $f_\partial(s)$ computes output values
2. **Gradient propagation:** Output tensors carry $\phi$ functions derived from input tensors

**Example (tensor-mul):**

$$
\text{tensor-mul}: (s \cdot t_1 \cdot t_2) \mapsto (s \cdot t_3)
$$

where:
- $D_3 = D_1 \odot D_2$ (elementwise multiply)
- $\phi_3(\bar{g}) = (\bar{g} \odot D_2, \bar{g} \odot D_1)$ (product rule)

### 2.3 Main Theorem

**Theorem 2.1 (Differentiability Preserves Postulates):**

If every differentiable tool $f_\partial : \mathcal{S}' \to \mathcal{S}'$ is:
1. A pure function (output depends only on input stack)
2. Compositionally closed (composition of diff tools is a diff tool)

Then the extended system $(\mathcal{T}'^*, \cdot, \epsilon)$ with denotation $\llbracket \cdot \rrbracket' : \mathcal{T}'^* \to (\mathcal{S}' \rightharpoonup \mathcal{S}')$ satisfies all three postulates.

**Proof:**

**(P1) Everything is a Tool:**

Each differentiable primitive (tensor-add, tensor-mul, backward, etc.) is defined as a tool with signature $\mathcal{S}' \to \mathcal{S}'$. By construction, all operations are tools. ∎

**(P2) Tools Transform Stacks:**

We must show: $f_\partial : \mathcal{S}' \to \mathcal{S}'$ is well-defined.

Let $s = (v_1, v_2, \ldots, v_n) \in \mathcal{S}'$.

Case 1: $v_i \in \mathcal{V}$ (non-tensor). Existing tools handle this.

Case 2: $v_i = (D_i, G_i, \phi_i) \in \mathbb{T}$ (tensor).

The tool $f_\partial$:
- Reads $D_i$ (data) - a pure read
- Computes output data $D_{out} = h(D_1, D_2, \ldots)$ for some pure $h$
- Constructs $\phi_{out}$ from $\phi_1, \phi_2, \ldots$ - pure function composition

No mutation occurs. The output is a new stack with new values. ∎

**(P3) Composition is Concatenation:**

Let $P_1, P_2$ be programs over $\mathcal{T}'^*$.

We must show: $\llbracket P_1 \cdot P_2 \rrbracket' = \llbracket P_2 \rrbracket' \circ \llbracket P_1 \rrbracket'$

Since each $f_\partial$ is pure:
- $\llbracket P_1 \rrbracket'(s)$ produces stack $s_1$
- $\llbracket P_2 \rrbracket'(s_1)$ produces stack $s_2$
- The composition $\llbracket P_2 \rrbracket' \circ \llbracket P_1 \rrbracket'$ equals sequential execution

The gradient functions $\phi$ compose via the chain rule:
$$
\phi_{f \circ g} = \phi_g \circ \phi_f
$$

This is pure function composition, preserving the homomorphism property. ∎

**Corollary 2.2 (Backward is a Tool):**

The `backward` operation:
$$
\text{backward}: (s \cdot t_{loss}) \mapsto (s \cdot m_{grads})
$$

where $m_{grads}$ is a map from tensor names to gradient tensors, is a pure tool because:
1. It reads only the loss tensor $t_{loss}$ and its $\phi$ chain
2. It computes gradients via pure function application
3. It returns a new map value (no mutation)

∎

### 2.4 The Gradient Tape is Not State

**Lemma 2.3 (Tape Immutability):**

The "gradient tape" (Wengert list) is an **append-only structure** embedded in tensor values, not external mutable state.

*Proof:*

Define the tape as:
$$
\tau = [(op_1, \text{inputs}_1, \text{output}_1), \ldots, (op_n, \text{inputs}_n, \text{output}_n)]
$$

Each tensor $t_i$ references its position in $\tau$.

During forward pass:
- New operations append to $\tau$
- Existing entries are never modified
- Each tensor's $\phi$ points to a fixed tape entry

This is equivalent to a functional data structure with structural sharing.

During backward pass:
- $\tau$ is traversed in reverse
- Gradients are computed and returned as new values
- $\tau$ is not modified

Therefore, the tape is isomorphic to an immutable linked list, not mutable state. ∎

---

## 3. Extension: Probabilistic Kore

### 3.1 The Probability Monad

**Definition 3.1 (Distribution Values):**

Let $\mathcal{D}(A)$ denote the set of probability distributions over $A$:
$$
\mathcal{D}(A) = \{ p : A \to [0,1] \mid \sum_{a \in A} p(a) = 1 \}
$$

For continuous domains:
$$
\mathcal{D}(A) = \{ p : A \to \mathbb{R}_{\geq 0} \mid \int_A p(a) \, da = 1 \}
$$

**Definition 3.2 (Probabilistic Stack):**

A probabilistic stack is a distribution over stacks:
$$
\mathcal{S}_\mathcal{D} = \mathcal{D}(\mathcal{S})
$$

Alternatively, a stack of distributions:
$$
\mathcal{S}^{\mathcal{D}} = (\mathcal{D}(\mathcal{V}))^*
$$

### 3.2 Probabilistic Tools

**Definition 3.3 (Stochastic Tool):**

A stochastic tool is a Kleisli arrow:
$$
f : \mathcal{S} \to \mathcal{D}(\mathcal{S})
$$

**Theorem 3.1 (Kleisli Composition Preserves Postulates):**

Stochastic tools compose via Kleisli composition:
$$
(g \circ_K f)(s) = \int_{s' \in \mathcal{S}} f(s)(s') \cdot g(s') \, ds'
$$

This forms a monoid:
$$
(\mathcal{S} \to \mathcal{D}(\mathcal{S}), \circ_K, \eta)
$$

where $\eta(s) = \delta_s$ (Dirac delta / point mass).

*Proof:*

Identity: $(\eta \circ_K f)(s) = \int \delta_{s'}(s') \cdot f(s') = f(s)$ ✓

Associativity: By Fubini's theorem and monad laws for $\mathcal{D}$. ✓

∎

### 3.3 Probabilistic Primitives

| Tool | Signature | Semantics |
|------|-----------|-----------|
| `sample` | $(d : \text{Dist} \to v : \mathcal{V})$ | Draw from distribution |
| `normal` | $(\mu, \sigma \to d : \text{Dist})$ | Gaussian distribution |
| `bernoulli` | $(p \to d : \text{Dist})$ | Bernoulli distribution |
| `observe` | $(d, v \to)$ | Condition on observation |
| `infer` | $(q : \text{Quote} \to d : \text{Dist})$ | Posterior inference |

**Key Insight:** `sample` is the only non-deterministic primitive. All others are pure.

### 3.4 Preserving Determinism with Explicit Randomness

**Definition 3.4 (Pseudo-Probabilistic Tool):**

Instead of true randomness, use explicit random seeds:
$$
\text{sample}: (s \cdot d \cdot \text{seed}) \mapsto (s \cdot v \cdot \text{seed}')
$$

where $v = F^{-1}_d(\text{hash}(\text{seed}))$ using inverse CDF.

This is **deterministic**: same seed → same sample. Statelessness preserved.

**Theorem 3.2 (Seeded Sampling Preserves Postulates):**

If randomness is threaded explicitly through the stack, probabilistic tools satisfy all three postulates.

*Proof:* Direct. The tool is a pure function of its stack inputs. ∎

---

## 4. Extension: Quantum Kore

### 4.1 Quantum State Vectors

**Definition 4.1 (Qubit):**

A qubit is a unit vector in $\mathbb{C}^2$:
$$
|\psi\rangle = \alpha|0\rangle + \beta|1\rangle, \quad |\alpha|^2 + |\beta|^2 = 1
$$

**Definition 4.2 (Quantum Register):**

An $n$-qubit register is a unit vector in $\mathbb{C}^{2^n}$:
$$
|\psi\rangle \in \mathbb{C}^{2^n}, \quad \langle\psi|\psi\rangle = 1
$$

### 4.2 Quantum Values

**Definition 4.3 (Quantum Value Type):**
$$
\mathcal{Q}ubit = \{ |\psi\rangle \in \mathbb{C}^{2^n} : \||\psi\rangle\| = 1, n \in \mathbb{N} \}
$$

Extended value set:
$$
\mathcal{V}'' = \mathcal{V}' \cup \mathcal{Q}ubit
$$

### 4.3 Unitary Tools (Quantum Gates)

**Definition 4.4 (Unitary Tool):**

A quantum tool $U$ acts on qubits via unitary matrices:
$$
U : |\psi\rangle \mapsto U|\psi\rangle
$$

where $U^\dagger U = I$.

| Tool | Matrix | Effect |
|------|--------|--------|
| `H` (Hadamard) | $\frac{1}{\sqrt{2}}\begin{pmatrix}1 & 1 \\ 1 & -1\end{pmatrix}$ | Superposition |
| `X` (NOT) | $\begin{pmatrix}0 & 1 \\ 1 & 0\end{pmatrix}$ | Bit flip |
| `CNOT` | $\begin{pmatrix}1&0&0&0\\0&1&0&0\\0&0&0&1\\0&0&1&0\end{pmatrix}$ | Controlled NOT |
| `Rz(θ)` | $\begin{pmatrix}e^{-i\theta/2} & 0 \\ 0 & e^{i\theta/2}\end{pmatrix}$ | Z rotation |

**Theorem 4.1 (Unitary Tools Preserve Postulates):**

Unitary tools satisfy all three postulates.

*Proof:*

**(P1):** Each gate is defined as a tool. ✓

**(P2):** $U : \mathcal{S}'' \to \mathcal{S}''$. The tool pops qubits, applies $U$, pushes result. ✓

**(P3):** Unitary composition: $(U_2 U_1)|\psi\rangle = U_2(U_1|\psi\rangle)$. This matches stack semantics. ✓

∎

### 4.4 Measurement (The Subtlety)

**Definition 4.5 (Measurement Tool):**
$$
\text{measure} : |\psi\rangle \mapsto (|i\rangle, p_i)
$$

with probability $p_i = |\langle i|\psi\rangle|^2$.

**Problem:** Measurement is **probabilistic** and **destroys superposition**.

**Solution 1: Deferred Measurement**

Don't collapse until explicitly requested. The qubit value carries the superposition.

**Solution 2: Seeded Measurement**

Like probabilistic Kore:
$$
\text{measure}: (s \cdot |\psi\rangle \cdot \text{seed}) \mapsto (s \cdot |i\rangle \cdot \text{seed}')
$$

Deterministic given the seed.

### 4.5 The No-Cloning Constraint

**Theorem 4.2 (No-Cloning):**

There exists no unitary $U$ such that:
$$
U(|\psi\rangle \otimes |0\rangle) = |\psi\rangle \otimes |\psi\rangle \quad \forall |\psi\rangle
$$

**Implication for Kore:**

The `dup` tool **cannot apply to qubits**!

**Definition 4.6 (Linear Qubit Type):**

Qubits are **linear resources**:
- Each qubit can be used exactly once
- `dup` on qubits is a type error
- `drop` on qubits is disallowed (or implicitly measures)

This connects to **Linear Kore** (Section 5).

---

## 5. Extension: Linear Kore

### 5.1 Linear Types

**Definition 5.1 (Linear Value):**

A value $v$ is **linear** if it must be used exactly once.

**Definition 5.2 (Linear Stack Discipline):**

For linear values:
- `dup` is forbidden
- `drop` is forbidden (or has semantic effect)
- Values must be consumed by exactly one operation

### 5.2 The Linear-Non-Linear Adjunction

**Definition 5.3 (Value Kinds):**
$$
\mathcal{V} = \mathcal{V}_{lin} \uplus \mathcal{V}_{unr}
$$

where:
- $\mathcal{V}_{lin}$ = linear values (qubits, file handles, unique references)
- $\mathcal{V}_{unr}$ = unrestricted values (ints, strings, can be copied)

**Theorem 5.1 (Linear Kore is a Linear/Non-Linear Model):**

Kore with linear values forms a model of Intuitionistic Linear Logic.

| Linear Logic | Kore |
|--------------|------|
| $A \otimes B$ | Two values on stack |
| $A \multimap B$ | Quote consuming $A$, producing $B$ |
| $!A$ | Unrestricted value (can dup) |
| $A \& B$ | Quote returning either $A$ or $B$ |

### 5.3 Linear Tools

| Tool | Signature | Constraint |
|------|-----------|------------|
| `dup` | $(a \to a \; a)$ | Only if $a \in \mathcal{V}_{unr}$ |
| `drop` | $(a \to)$ | Only if $a \in \mathcal{V}_{unr}$ |
| `swap` | $(a \; b \to b \; a)$ | Always allowed (preserves linearity) |
| `consume` | $(a_{lin} \to)$ | Explicitly discards linear value |
| `copy` | $(a_{unr} \to a \; a)$ | Explicit duplication |

**Theorem 5.2 (Linear Tools Preserve Postulates):**

Linear Kore satisfies all three postulates, with the constraint that:
- Postulate 2 becomes: $\text{Tool} : \mathcal{S}_{well-typed} \to \mathcal{S}_{well-typed}$
- Well-typed stacks respect linear resource constraints

*Proof:* The linear type system is a refinement, not a violation, of the stack semantics. ∎

---

## 6. Unified Framework: Effectful Kore

### 6.1 The Key Insight

All extensions follow a pattern:

| Extension | Effect | Mathematical Structure |
|-----------|--------|------------------------|
| Differentiable | Gradient tracking | Dual numbers / Wengert lists |
| Probabilistic | Randomness | Probability monad $\mathcal{D}$ |
| Quantum | Superposition | Hilbert space $\mathbb{C}^n$ |
| Linear | Resource tracking | Linear logic |

Each is an **effect** that can be modeled as a **monad** or **graded monad**.

### 6.2 Graded Monads for Effects

**Definition 6.1 (Graded Monad):**

A graded monad over a monoid $(M, \cdot, 1)$ consists of:
- For each $m \in M$: a functor $T_m$
- Unit: $\eta : A \to T_1(A)$
- Multiplication: $\mu : T_m(T_n(A)) \to T_{m \cdot n}(A)$

**Definition 6.2 (Kore Effect Monoid):**
$$
M = \{ \text{pure}, \text{diff}, \text{prob}, \text{quantum}, \text{linear} \}
$$

with composition rules:
- $\text{pure} \cdot e = e$
- $\text{diff} \cdot \text{diff} = \text{diff}$
- $\text{prob} \cdot \text{prob} = \text{prob}$
- etc.

### 6.3 Effectful Tool Type

**Definition 6.3 (Graded Tool):**

A tool with effect $m$ has type:
$$
f : \mathcal{S} \to T_m(\mathcal{S})
$$

**Example:**
- Pure tool: $f : \mathcal{S} \to T_{\text{pure}}(\mathcal{S}) = \mathcal{S}$
- Probabilistic tool: $f : \mathcal{S} \to T_{\text{prob}}(\mathcal{S}) = \mathcal{D}(\mathcal{S})$
- Differentiable tool: $f : \mathcal{S} \to T_{\text{diff}}(\mathcal{S}) = \mathcal{S} \times \text{Tape}$

### 6.4 Effect Handlers as Tools

**Key Principle:** Effects are introduced and eliminated by tools.

| Effect | Introduction | Elimination |
|--------|--------------|-------------|
| Diff | `diff-through` | `backward` |
| Prob | `sample` | `infer`, `expectation` |
| Quantum | `qubit` | `measure` |
| Linear | `acquire` | `release` |

**Theorem 6.1 (Effect Tools Preserve Postulates):**

If effect introduction/elimination are tools, and effect composition follows monad laws, then effectful Kore preserves all three postulates.

*Proof:*

The effectful denotation:
$$
\llbracket \cdot \rrbracket_m : \mathcal{T}^* \to (\mathcal{S} \to T_m(\mathcal{S}))
$$

is a homomorphism under Kleisli composition:
$$
\llbracket P_1 \cdot P_2 \rrbracket_m = \llbracket P_2 \rrbracket_m \circ_K \llbracket P_1 \rrbracket_m
$$

Kleisli composition is associative with identity (monad laws). ∎

---

## 7. Implementation Implications

### 7.1 Value Type Extension

```rust
pub enum Value {
    // Current types
    Null, Bool(bool), Int(i64), Float(f64),
    Text(String), List(Vec<Value>), Map(IndexMap<String, Value>),
    Quote(Vec<Op>), Handle(Handle), Error(Box<ErrorValue>),
    
    // Differentiable
    Tensor(TensorValue),
    
    // Probabilistic
    Distribution(DistributionValue),
    
    // Quantum
    Qubit(QubitValue),
    
    // Linear (wrapper that tracks usage)
    Linear(Box<LinearValue>),
}
```

### 7.2 Effect Context

```rust
pub struct EffectContext {
    /// Gradient tape (for differentiable mode)
    tape: Option<GradientTape>,
    
    /// Random state (for probabilistic mode)
    rng_seed: Option<u64>,
    
    /// Quantum state (for quantum mode)
    quantum_register: Option<QuantumState>,
    
    /// Linear type checker
    linear_tracker: Option<LinearTracker>,
}
```

### 7.3 Tool Registration with Effects

```rust
// Pure tool
Tool::native("add", "(a b -- c)", |s, ctx| { ... })

// Differentiable tool
Tool::differentiable("tensor-mul", "(t t -- t)", |s, ctx| {
    // Forward pass
    let result = ...;
    // Record on tape if present
    if let Some(tape) = ctx.effect.tape.as_mut() {
        tape.record(Op::Mul, inputs, &result);
    }
    Ok(result)
})

// Probabilistic tool
Tool::probabilistic("sample", "(dist -- value)", |s, ctx| {
    let seed = ctx.effect.rng_seed.expect("no random seed");
    let value = dist.sample_deterministic(seed);
    Ok(value)
})
```

### 7.4 Composability Matrix

Which effects compose cleanly?

|  | Pure | Diff | Prob | Quantum | Linear |
|--|------|------|------|---------|--------|
| **Pure** | ✓ | ✓ | ✓ | ✓ | ✓ |
| **Diff** | ✓ | ✓ | ⚠️ | ✗ | ✓ |
| **Prob** | ✓ | ⚠️ | ✓ | ⚠️ | ✓ |
| **Quantum** | ✓ | ✗ | ⚠️ | ✓ | ✓ |
| **Linear** | ✓ | ✓ | ✓ | ✓ | ✓ |

Legend:
- ✓ = composes cleanly
- ⚠️ = composes with care (e.g., diff through stochastic requires reparameterization)
- ✗ = does not compose (quantum gradients require different machinery)

---

## 8. Conclusion

### 8.1 Summary of Results

| Extension | Preserves Postulates? | Mathematical Basis | Implementation Complexity |
|-----------|----------------------|--------------------|-----------------------------|
| **Differentiable** | ✅ Yes | Dual numbers, Wengert lists | Medium (~1500 LOC) |
| **Probabilistic** | ✅ Yes | Probability monad, Kleisli | Medium (~1000 LOC) |
| **Quantum** | ✅ Yes | Hilbert spaces, unitaries | High (~2500 LOC) |
| **Linear** | ✅ Yes | Linear logic, affine types | Medium (~1000 LOC) |

### 8.2 Recommendations

**Phase 1: Differentiable Kore**
- Add `Tensor` value type
- Implement forward-mode autodiff (dual numbers) first
- Add reverse-mode (tape) for efficiency
- Introduce `diff-through` as explicit effect boundary

**Phase 2: Probabilistic Kore**
- Add `Distribution` value type
- Use seeded randomness for determinism
- Implement `sample`, `observe`, `infer`
- Connect to probabilistic programming inference (HMC, VI)

**Phase 3: Linear Kore**
- Add `Linear` wrapper type
- Implement linear type checking in tools
- Use for: file handles, network connections, unique references
- Enables safe resource management without GC

**Phase 4: Quantum Kore** (Research)
- Add `Qubit` value type with no-cloning enforcement
- Implement unitary gates
- Integrate with quantum simulators (or real hardware)
- Combine linear types to enforce no-cloning

### 8.3 The Unified Vision

Kore becomes a **universal effectful stack machine**:

$$
\text{Kore}_{\text{unified}} : \mathcal{S} \to T_m(\mathcal{S})
$$

where $m$ is a composition of effects chosen by the programmer.

Each effect is:
1. Introduced by a tool
2. Composed via Kleisli/monad laws
3. Eliminated by a tool

This preserves Kore's essential character:
- **Everything is a Tool** (including effect handlers)
- **Tools Transform Stacks** (in the effectful category)
- **Composition is Concatenation** (via Kleisli composition)

The postulates are not violated—they are **lifted** to a richer mathematical structure.

---

## References

1. Bošnjak, M., Rocktäschel, T., Naradowsky, J., & Riedel, S. (2017). Programming with a Differentiable Forth Interpreter. *ICML 2017*.

2. Girard, J.-Y. (1987). Linear Logic. *Theoretical Computer Science*.

3. Abramsky, S., & Coecke, B. (2004). A Categorical Semantics of Quantum Protocols. *LICS 2004*.

4. Moggi, E. (1991). Notions of Computation and Monads. *Information and Computation*.

5. Katsumata, S. (2014). Parametric Effect Monads and Semantics of Effect Systems. *POPL 2014*.

6. Staton, S. (2017). Commutative Semantics for Probabilistic Programming. *ESOP 2017*.

7. Paykin, J., Rand, R., & Zdancewic, S. (2017). QWIRE: A Core Language for Quantum Circuits. *POPL 2017*.
