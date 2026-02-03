# Kore: Formal Extensions and Experimental Validation

**A Literature Survey and Research Proposal**

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [Literature Survey](#2-literature-survey)
3. [Proposed Extensions](#3-proposed-extensions)
4. [Formal Properties](#4-formal-properties)
5. [Experimental Validation](#5-experimental-validation)
6. [Implementation Roadmap](#6-implementation-roadmap)
7. [References](#7-references)

---

## 1. Introduction

### 1.1 Problem Statement

Current runtime systems for intelligent agents exhibit a fundamental mismatch between their operational semantics and their optimization objectives:

1. **Gradient Discontinuity**: Control flow (if/while) creates non-differentiable boundaries
2. **Concurrency Opacity**: Thread state cannot be inspected, forked, or rewound
3. **Resource Unsoundness**: Memory/capability bounds are advisory, not enforced
4. **Probabilistic Afterthought**: Uncertainty is bolted on via external libraries

### 1.2 Thesis

A concatenative (stack-based) calculus with extended value domains can unify these concerns while maintaining:
- **Minimality**: Three postulates, four primitive operations
- **Introspectability**: All state is reified and inspectable
- **Compositionality**: Extensions compose without interference

### 1.3 Scope

This document surveys relevant literature, proposes formal extensions, states provable properties, and designs experiments. It does **not** claim novelty in individual components—the contribution is their unification under a minimal axiomatic system.

---

## 2. Literature Survey

### 2.1 Concatenative Languages

**Foundational Work:**

- **Forth** (Moore, 1970): Demonstrated that stack-based execution enables extreme simplicity. The entire language is defined by ~30 primitives. Kore inherits this minimalism.

- **Joy** (von Thun, 2001): Introduced quotations as first-class values, enabling higher-order programming without lambda calculus. Joy's insight: "Programs are lists of symbols; composition is concatenation."

- **Cat** (Diggins, 2007): Added static typing to concatenative languages via stack effect inference. Proved that type inference is decidable for a restricted class of stack programs.

- **Factor** (Pestov, 2010): Industrial-strength concatenative language with quotations, continuations, and effect systems. Demonstrates scalability of the paradigm.

**Key Insight**: Concatenative semantics naturally correspond to **monoidal categories** rather than cartesian closed categories (Melliès, 2009). This distinction is crucial for linear typing.

**Gap**: No concatenative language has unified differentiability, concurrency, linearity, and probability in a single coherent framework.

### 2.2 Linear Logic and Types

**Foundational Work:**

- **Linear Logic** (Girard, 1987): Introduced resource-sensitive reasoning. The key connectives:
  - $A \otimes B$ (tensor): Use both $A$ and $B$ exactly once
  - $A \multimap B$ (lollipop): Consume $A$ to produce $B$
  - $!A$ (of course): Allows duplication/discard

- **Linear Types** (Wadler, 1990): Embedded linear logic into type systems. Proved that linear types guarantee:
  - No dangling references
  - Deterministic deallocation
  - Safe concurrency (no data races)

- **Rust Ownership** (Matsakis & Klock, 2014): Industrial application of affine types (linear types with weakening). Demonstrates practicality at scale.

- **Session Types** (Honda, 1993): Linear types for communication protocols. Guarantees protocol adherence.

**Key Insight**: Stack machines naturally enforce linearity—values are consumed when popped. The challenge is making this explicit for specific resource types.

**Theorem (Wadler, 1990)**: In a linear type system, if a program type-checks, every linear resource is used exactly once.

### 2.3 Automatic Differentiation

**Foundational Work:**

- **Forward Mode AD** (Wengert, 1964): Compute derivatives alongside values using dual numbers: $(v, \dot{v})$ where $\dot{v} = \partial v / \partial x$.

- **Reverse Mode AD** (Speelpenning, 1980): Record operations to a tape, then traverse backward. Efficient for $f: \mathbb{R}^n \to \mathbb{R}$ (many inputs, scalar output).

- **Differentiable Programming** (LeCun, 2018): Treat entire programs as differentiable. Requires handling control flow.

- **JAX** (Bradbury et al., 2018): Composable transformations including `grad`, `jit`, `vmap`. Demonstrates that AD is a program transformation.

- **Differentiable Control Flow** (Chung et al., 2021): Soft conditionals and loops via temperature-scaled approximations.

**Key Insight**: AD is fundamentally about **tracing execution** and **reversing the trace**. A stack machine's execution trace is linear, making reverse-mode natural.

**Theorem (Griewank, 2008)**: Reverse-mode AD computes the gradient of $f: \mathbb{R}^n \to \mathbb{R}$ in $O(1)$ times the cost of computing $f$.

### 2.4 Delimited Continuations and Fibers

**Foundational Work:**

- **Continuations** (Reynolds, 1972): Reify "the rest of the computation" as a first-class value.

- **Delimited Continuations** (Felleisen, 1988): Bound the continuation to avoid capturing the entire program. Operators: `shift`/`reset`, `control`/`prompt`.

- **Effect Handlers** (Plotkin & Pretnar, 2009): Generalize delimited continuations to user-defined effects. Enable modular effect composition.

- **Fibers/Green Threads** (Li & Zdancewic, 2007): Lightweight concurrency via explicit yield points. $O(1)$ context switch vs $O(\mu s)$ for OS threads.

**Key Insight**: A stack machine's state is **exactly** its stack plus instruction pointer. Capturing this is trivial compared to lambda-calculus environments.

**Theorem (Biernacki et al., 2011)**: Delimited continuations with `shift₀`/`reset₀` are equivalent in expressiveness to effect handlers.

### 2.5 Probabilistic Programming

**Foundational Work:**

- **Church** (Goodman et al., 2008): Probabilistic programming via stochastic memoization in Scheme.

- **Trace-Based Inference** (Wingate et al., 2011): Execution traces as the target of inference. Metropolis-Hastings on program traces.

- **Pyro** (Bingham et al., 2019): Probabilistic programming with differentiable inference (variational methods).

- **Particles and Programs** (Wood et al., 2014): Sequential Monte Carlo for probabilistic programs.

**Key Insight**: A probabilistic program defines a distribution over execution traces. Inference = sampling/scoring traces.

**Theorem (Staton et al., 2016)**: Probabilistic programs with `sample` and `score` form a commutative monad, enabling compositional semantics.

### 2.6 Category-Theoretic Foundations

**Foundational Work:**

- **Cartesian Closed Categories** (Lambek & Scott, 1986): Semantics for lambda calculus. Every object has diagonal (copy) and terminal (delete) morphisms.

- **Symmetric Monoidal Categories** (Mac Lane, 1963): Weaker than CCC—no implicit copy/delete. Natural semantics for linear logic.

- **String Diagrams** (Joyal & Street, 1991): Graphical language for monoidal categories. Programs as wirings.

- **Traced Monoidal Categories** (Joyal et al., 1996): Add feedback/loops to monoidal categories.

**Key Insight**: Stack machines are naturally **symmetric monoidal**, not cartesian. This is why linear types fit naturally.

**Theorem (Hasegawa, 1997)**: Traced symmetric monoidal categories with a trace operator model iteration and recursion.

---

## 3. Proposed Extensions

Each extension adds a new value type and associated tools. All extensions preserve the three postulates.

### 3.1 Extension T: Tensors with Automatic Differentiation

**New Value Type:**
```
Tensor = {
    data: Vec<f64>,
    shape: Vec<usize>,
    requires_grad: bool,
    grad: Option<Box<Tensor>>,
    tape_ref: Option<TapeId>
}
```

**New Tools:**
```
tensor-new    ( shape data -- tensor )           # Create tensor
tensor-randn  ( shape -- tensor )                # Random normal init
tensor-shape  ( tensor -- shape )                # Get dimensions
requires-grad ( tensor -- tensor )               # Enable gradient tracking
matmul        ( A B -- C )                       # Matrix multiply
tensor-add    ( A B -- C )                       # Element-wise add
tensor-mul    ( A B -- C )                       # Hadamard product
relu          ( tensor -- tensor )               # ReLU activation
softmax       ( tensor -- tensor )               # Softmax (row-wise)
backward      ( tensor -- )                      # Compute gradients
grad          ( tensor -- tensor )               # Get gradient
zero-grad     ( tensor -- tensor )               # Reset gradients
```

**Semantic Extension:**

Operations on tensors with `requires_grad = true` append to a thread-local gradient tape:
```
Tape = [(op_id, inputs, output, adjoint_fn)]
```

The `backward` tool traverses the tape in reverse, applying adjoint functions.

**Postulate Compliance:**
- P1: All operations are tools in the dictionary
- P2: Tensors flow through the stack; tape is accessed via stack-pushed tensors
- P3: Composition remains concatenation

### 3.2 Extension F: Fibers (Reified Computations)

**New Value Type:**
```
Fiber = {
    stack: Stack,
    ip: usize,
    program: Vec<Op>,
    status: Running | Paused | Done | Failed,
    result: Option<Value>
}
```

**New Tools:**
```
fiber-new     ( quote -- fiber )                 # Create paused fiber
fiber-resume  ( value fiber -- value fiber )    # Step fiber, get result
fiber-yield   ( value -- value )                 # Pause current fiber
fiber-status  ( fiber -- status )                # Check state
fiber-fork    ( fiber -- fiber fiber )           # COW clone
fiber-stack   ( fiber -- list )                  # Inspect stack
fiber-join    ( fiber -- value )                 # Block until done
```

**Semantic Extension:**

The runtime maintains a fiber scheduler. `fiber-yield` is the only cooperative yield point. `fiber-fork` performs copy-on-write duplication of fiber state.

**Key Property**: Fibers are values. They can be:
- Stored in lists
- Passed to functions
- Duplicated (forked)
- Inspected

**Postulate Compliance:**
- P1: Fiber operations are dictionary entries
- P2: Fibers are stack values; resume consumes/produces values
- P3: No special syntax for concurrency

### 3.3 Extension L: Linear Values

**New Type Wrapper:**
```
Linear<T> = {
    value: T,
    consumed: bool
}
```

**New Tools:**
```
linear-wrap   ( value -- linear )                # Make value linear
linear-unwrap ( linear -- value )                # Consume, extract value
is-linear     ( value -- bool )                  # Check linearity
linear-move   ( linear -- linear )               # Transfer ownership
```

**Runtime Enforcement:**

When a `Linear<T>` is on the stack:
- `dup` → Runtime error "Cannot duplicate linear value"
- `drop` → Runtime error "Cannot discard linear value; use linear-unwrap"
- Scope exit without consumption → Runtime error

**Use Cases:**
```kore
# File handle (must be closed)
"file.txt" fs-open linear-wrap  # ( linear-handle )
# ... use handle ...
linear-unwrap fs-close          # Consume handle, close file

# Capability token (cannot be forged)
"network" cap-token linear-wrap
# ... only one holder can use network ...
```

**Postulate Compliance:**
- P1: Linearity is enforced by tools (`dup`, `drop` check types)
- P2: Linear values are stack values
- P3: No syntax change

### 3.4 Extension P: Probability Distributions

**New Value Type:**
```
Distribution = 
    | Categorical(weights: Vec<f64>)
    | Normal(mean: f64, std: f64)
    | Empirical(samples: Vec<Value>)
    | Delta(value: Value)
```

**New Tools:**
```
dist-normal    ( mean std -- dist )              # Normal distribution
dist-categorical ( weights -- dist )             # Categorical
dist-delta     ( value -- dist )                 # Point mass
sample         ( dist -- value )                 # Draw sample
score          ( value dist -- )                 # Add to log-probability
observe        ( value dist -- )                 # Condition on observation
infer          ( quote n -- samples )            # Run inference (n particles)
log-prob       ( -- float )                      # Current trace log-prob
```

**Semantic Extension:**

The context maintains:
```
trace_log_prob: f64          # Log probability of current trace
rng: StdRng                  # Seeded RNG for reproducibility
```

`sample` draws from distribution and records the choice. `score` adds to `trace_log_prob`. Inference algorithms use these traces.

**Postulate Compliance:**
- P1: Probabilistic operations are tools
- P2: Distributions are stack values; log-prob is accessed via tool
- P3: Inference (`infer`) takes a quote (program as value)

---

## 4. Formal Properties

### 4.1 Properties of Extension T (Tensors)

**Property T1 (Correctness of Gradients):**
For any differentiable composition of tensor tools $f = t_n \circ \cdots \circ t_1$:
$$\texttt{backward}(f(x)) \text{ computes } \nabla_x f(x)$$

*Proof sketch*: Each tool $t_i$ has a registered adjoint $\bar{t}_i$. The tape records $(t_i, x_i, y_i)$. Backward traversal applies chain rule: $\bar{x}_i = \bar{t}_i(\bar{y}_i)$.

**Property T2 (Complexity):**
For $f: \mathbb{R}^n \to \mathbb{R}$ computed by a tape of length $m$:
$$\text{Time}(\texttt{backward}) = O(m) = O(\text{Time}(f))$$

*Proof*: Each tape entry is visited once during backward pass.

**Property T3 (Differentiable Control Flow):**
If `soft-if` is used instead of `if`, gradients flow through branches:
```kore
x requires-grad
x 0.5 soft-lt              # ( x, sigmoid((0.5-x)/τ) )
[ relu ] [ sigmoid ] soft-if
backward                    # Gradients flow through both branches
```

*Mechanism*: `soft-if` records both branches with soft weights $w, 1-w$.

### 4.2 Properties of Extension F (Fibers)

**Property F1 (Isolation):**
For fibers $f_1, f_2$:
$$\text{stack}(f_1) \cap \text{stack}(f_2) = \emptyset$$

*Proof*: Each fiber owns its stack. No shared mutable state by construction.

**Property F2 (Determinism):**
For a fiber $f$ with initial stack $s$ and program $p$:
$$\texttt{fiber-resume}^n(f) \text{ is deterministic}$$

*Proof*: Fiber execution depends only on its owned state.

**Property F3 (Fork Correctness):**
$$\texttt{fiber-fork}(f) = (f', f'') \implies \text{exec}(f') = \text{exec}(f'')$$

*Proof*: Fork creates identical copies. Subsequent divergence comes only from different inputs.

**Property F4 (Space Efficiency):**
Copy-on-write forking:
$$\text{Space}(\texttt{fiber-fork}(f)) = O(1) \text{ initially}$$

Growing to $O(|f|)$ only as pages are modified.

### 4.3 Properties of Extension L (Linear)

**Property L1 (Use-Once):**
If $v$ is wrapped with `linear-wrap`, exactly one of the following occurs:
1. $v$ is consumed via `linear-unwrap`
2. Execution fails with "linear value not consumed"

*Proof*: Runtime tracks `consumed` flag. Stack operations check linearity.

**Property L2 (No Duplication):**
$$\texttt{dup}(\texttt{linear-wrap}(v)) \to \text{Error}$$

*Proof*: `dup` implementation checks `is-linear` and fails.

**Property L3 (Resource Safety):**
For any resource $r$ with type `Linear<Handle>`:
$$\text{(program terminates)} \implies \text{(}r\text{ was consumed)}$$

*Proof*: Linear values cannot be dropped; only `linear-unwrap` removes them.

### 4.4 Properties of Extension P (Probability)

**Property P1 (Trace Semantics):**
A probabilistic program $p$ defines a distribution over traces $\tau$:
$$P(\tau) = \prod_{i} P(\text{sample}_i) \cdot \exp(\sum_j \text{score}_j)$$

**Property P2 (Inference Correctness):**
For `infer` using Sequential Monte Carlo with $n$ particles:
$$\lim_{n \to \infty} \frac{1}{n}\sum_i f(\tau_i) = \mathbb{E}_{P(\tau)}[f(\tau)]$$

*Proof*: Standard SMC convergence (Doucet et al., 2001).

**Property P3 (Compositionality):**
Probabilistic programs compose:
$$P(p_1 ; p_2) = P(p_2 | p_1) \cdot P(p_1)$$

---

## 5. Experimental Validation

Each experiment tests a specific claim. No experiment is designed to "show off"—each validates a formal property.

### 5.1 Experiment: Gradient Correctness

**Hypothesis**: Extension T computes correct gradients for non-trivial compositions.

**Method**:
1. Implement $f(x) = \text{softmax}(\text{relu}(Wx + b))$ in Kore
2. Compute $\nabla_W f$ via `backward`
3. Compare to finite differences: $\frac{f(W + \epsilon e_i) - f(W - \epsilon e_i)}{2\epsilon}$

**Success Criterion**: $\|\nabla_{\text{Kore}} - \nabla_{\text{FD}}\| < 10^{-5}$ for $\epsilon = 10^{-7}$.

**What It Proves**: Property T1 (gradient correctness).

### 5.2 Experiment: MNIST Classification

**Hypothesis**: Extension T enables practical neural network training.

**Method**:
1. Implement 2-layer MLP: 784 → 128 → 10
2. Train on MNIST for 10 epochs
3. Compare accuracy and time to PyTorch baseline

**Metrics**:
- Test accuracy (target: >95%)
- Training time (target: <10x PyTorch)
- Memory usage

**What It Proves**: Extension T is practical, not just theoretically correct.

### 5.3 Experiment: Fiber Scalability

**Hypothesis**: Extension F enables massive concurrency beyond OS thread limits.

**Method**:
1. Create $n$ fibers, each running a "thought loop" (100 ops)
2. Measure: total memory, context-switch latency, completion time
3. Compare: $n \in \{1000, 10000, 100000, 1000000\}$

**Baseline Comparison**:
| System | Expected Max $n$ | Memory per unit |
|--------|-----------------|-----------------|
| Python threading | ~1,000 | ~8KB |
| Go goroutines | ~100,000 | ~2KB |
| Kore fibers | ~1,000,000 | ~200B |

**What It Proves**: Property F4 (space efficiency).

### 5.4 Experiment: MCTS via Fiber Fork

**Hypothesis**: `fiber-fork` enables Monte Carlo Tree Search without serialization.

**Method**:
1. Implement simple game (Tic-Tac-Toe) as Kore program
2. At each decision point, fork fiber and explore branch
3. Compare to traditional MCTS with explicit state serialization

**Metrics**:
- Nodes explored per second
- Memory overhead per node
- Code complexity (lines of code)

**What It Proves**: Fibers as reified state enable planning algorithms naturally.

### 5.5 Experiment: Linear Resource Safety

**Hypothesis**: Extension L prevents resource leaks at runtime.

**Method**:
1. Write programs with intentional resource bugs:
   - Open file, forget to close
   - Duplicate a linear capability
   - Drop a linear value
2. Verify all cases produce runtime errors
3. Write correct versions, verify they succeed

**Success Criterion**: 100% detection rate for violations.

**What It Proves**: Properties L1, L2, L3.

### 5.6 Experiment: File Handle Linearity

**Hypothesis**: Linear types enable zero-leak file I/O.

**Method**:
1. Open 10,000 files with linear handles
2. Process each file, close handle
3. Verify no file descriptor leaks (check `/proc/self/fd`)
4. Intentionally "lose" a handle, verify error

**What It Proves**: Practical utility of linear types for system resources.

### 5.7 Experiment: Bayesian Inference

**Hypothesis**: Extension P enables correct posterior inference.

**Method**:
1. Define prior: `0.5 dist-normal sample` (latent variable)
2. Define likelihood: `observed latent dist-normal observe`
3. Run `infer` with 1000 particles
4. Compare posterior mean to analytical solution

**Test Cases**:
- Conjugate Normal-Normal (analytical solution known)
- Beta-Binomial (analytical solution known)

**Success Criterion**: $|\mu_{\text{infer}} - \mu_{\text{true}}| < 0.1$

**What It Proves**: Property P2 (inference correctness).

### 5.8 Experiment: Differentiable Control Flow

**Hypothesis**: `soft-if` enables gradient flow through branches.

**Method**:
1. Define piecewise function: $f(x) = \begin{cases} x^2 & x < 0 \\ x^3 & x \geq 0 \end{cases}$
2. Implement with `soft-if` (temperature $\tau = 0.1$)
3. Compute gradient at $x = 0.1$
4. Compare to finite differences

**What It Proves**: Property T3 (differentiable control flow).

### 5.9 Experiment: Symbolic Regression

**Hypothesis**: Combining fibers (search) and tensors (optimization) solves hybrid problems.

**Method**:
1. Generate data from $y = 2.5x^2 - 1.3x + 0.7$
2. Search space: compositions of `add`, `mul`, `square`, `const`
3. Use fiber-fork for tree search
4. Use backward for constant optimization
5. Measure: equations tested, time to find solution

**Baseline**: Genetic programming (no gradient guidance)

**What It Proves**: The four pillars compose to solve problems neither can solve alone.

### 5.10 Experiment: Self-Modifying Code Safety

**Hypothesis**: Linear capabilities prevent unauthorized code modification.

**Method**:
1. Define `code-modify` tool requiring `Linear<ModifyToken>`
2. Give agent program without token
3. Verify all modification attempts fail
4. Give agent token, verify single modification succeeds

**What It Proves**: Linear types enforce authorization for sensitive operations.

---

## 6. Implementation Roadmap

### Phase 1: Tensor Extension (4 weeks)

| Week | Deliverable |
|------|-------------|
| 1 | `Tensor` value type, basic ops (add, mul, matmul) |
| 2 | Gradient tape, `backward`, `grad` |
| 3 | Softmax, cross-entropy, ReLU with adjoints |
| 4 | Experiments 5.1, 5.2, 5.8 |

**Lines of Code**: ~1,500
**New Tools**: 15
**Test Cases**: 50

### Phase 2: Fiber Extension (4 weeks)

| Week | Deliverable |
|------|-------------|
| 1 | `Fiber` value type, `fiber-new`, `fiber-resume` |
| 2 | `fiber-yield`, cooperative scheduler |
| 3 | `fiber-fork` with COW semantics |
| 4 | Experiments 5.3, 5.4 |

**Lines of Code**: ~1,200
**New Tools**: 8
**Test Cases**: 40

### Phase 3: Linear Extension (2 weeks)

| Week | Deliverable |
|------|-------------|
| 1 | `Linear<T>` wrapper, runtime checks |
| 2 | Integration with `dup`, `drop`; Experiments 5.5, 5.6 |

**Lines of Code**: ~500
**New Tools**: 5
**Test Cases**: 30

### Phase 4: Probability Extension (3 weeks)

| Week | Deliverable |
|------|-------------|
| 1 | `Distribution` type, `sample`, `score` |
| 2 | `infer` with SMC |
| 3 | Experiment 5.7, integration with tensors |

**Lines of Code**: ~1,000
**New Tools**: 8
**Test Cases**: 35

### Phase 5: Integration (3 weeks)

| Week | Deliverable |
|------|-------------|
| 1 | Cross-pillar experiments (5.9) |
| 2 | Performance optimization |
| 3 | Documentation, paper draft |

---

## 7. References

### Concatenative Languages

1. Moore, C. (1970). FORTH: A new way to program. *Astronomy & Astrophysics Supplement*.
2. von Thun, M. (2001). Joy: Forth's functional cousin. *Proc. EuroForth*.
3. Diggins, C. (2007). Cat: A functional stack-based language. *Unpublished*.
4. Pestov, S. (2010). Factor: A practical stack language. *ACM SIGPLAN Notices*.

### Linear Logic and Types

5. Girard, J.-Y. (1987). Linear logic. *Theoretical Computer Science*, 50(1), 1-102.
6. Wadler, P. (1990). Linear types can change the world! *Programming Concepts and Methods*.
7. Matsakis, N. D., & Klock, F. S. (2014). The Rust language. *ACM SIGAda Ada Letters*, 34(3), 103-104.
8. Honda, K. (1993). Types for dyadic interaction. *CONCUR*, 509-523.

### Automatic Differentiation

9. Wengert, R. E. (1964). A simple automatic derivative evaluation program. *CACM*, 7(8), 463-464.
10. Speelpenning, B. (1980). *Compiling fast partial derivatives of functions given by algorithms*. PhD thesis, UIUC.
11. Griewank, A., & Walther, A. (2008). *Evaluating Derivatives: Principles and Techniques of Algorithmic Differentiation*. SIAM.
12. Bradbury, J., et al. (2018). JAX: Composable transformations of Python+NumPy programs. *GitHub*.

### Continuations and Concurrency

13. Reynolds, J. C. (1972). Definitional interpreters for higher-order programming languages. *HOSC*, 11(4), 363-397.
14. Felleisen, M. (1988). The theory and practice of first-class prompts. *POPL*, 180-190.
15. Plotkin, G., & Pretnar, M. (2009). Handlers of algebraic effects. *ESOP*, 80-94.
16. Li, P., & Zdancewic, S. (2007). Combining events and threads for scalable network services. *PLDI*, 189-199.

### Probabilistic Programming

17. Goodman, N. D., et al. (2008). Church: A language for generative models. *UAI*, 220-229.
18. Wingate, D., Stuhlmüller, A., & Goodman, N. D. (2011). Lightweight implementations of probabilistic programming languages via transformational compilation. *AISTATS*, 770-778.
19. Bingham, E., et al. (2019). Pyro: Deep universal probabilistic programming. *JMLR*, 20(28), 1-6.
20. Staton, S., et al. (2016). Semantics for probabilistic programming. *LICS*, 60-69.

### Category Theory

21. Lambek, J., & Scott, P. J. (1986). *Introduction to Higher Order Categorical Logic*. Cambridge.
22. Mac Lane, S. (1963). Natural associativity and commutativity. *Rice University Studies*, 49(4), 28-46.
23. Joyal, A., & Street, R. (1991). The geometry of tensor calculus I. *Advances in Mathematics*, 88(1), 55-112.
24. Hasegawa, M. (1997). Recursion from cyclic sharing. *TLCA*, 196-213.
25. Melliès, P.-A. (2009). Categorical semantics of linear logic. *Panoramas et Synthèses*, 27, 1-196.

### Inference

26. Doucet, A., De Freitas, N., & Gordon, N. (2001). *Sequential Monte Carlo Methods in Practice*. Springer.
27. Wood, F., van de Meent, J. W., & Mansinghka, V. (2014). A new approach to probabilistic programming inference. *AISTATS*, 1024-1032.

---

## Appendix A: Postulate Compliance Verification

For each extension, we verify adherence to the three postulates:

| Extension | P1 (Tool) | P2 (Stack→Stack) | P3 (Concatenation) |
|-----------|-----------|------------------|-------------------|
| Tensor | `backward`, `grad` are tools | Tensors are values; tape accessed via tensor | No syntax change |
| Fiber | `spawn`, `resume` are tools | Fibers are values | `fiber-fork` returns values |
| Linear | Enforced by `dup`/`drop` | Linear values on stack | No syntax change |
| Prob | `sample`, `infer` are tools | Distributions are values | `infer` takes quote |

## Appendix B: Category-Theoretic Interpretation

Kore's operational semantics correspond to:

- **Base calculus**: Free symmetric monoidal category on value types
- **With linearity**: *Compact closed category* (every object has a dual)
- **With fibers**: *Traced monoidal category* (feedback loops)
- **With probability**: *Markov category* (copy and delete for non-linear types)

This provides:
1. Equational reasoning via string diagrams
2. Denotational semantics independent of implementation
3. Formal basis for optimizations (graph rewriting)

---

*Document Version: 0.1*
*Last Updated: 2026-02-03*
*Status: Draft Research Proposal*
