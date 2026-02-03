# Kore RL: Interaction Model & Hard Problem Domains

## Part I: LLM ↔ Runtime Interaction Design

### Current Model (Batch)
```
LLM → Program → Kore Runtime → Result → LLM
         │                        │
         └────── one shot ────────┘
```

### Enhanced Model (Interactive/Streaming)

The LLM can interact with Kore at multiple levels:

```
┌─────────────────────────────────────────────────────────────────┐
│                    INTERACTION LAYERS                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Layer 3: Container (Full Environment)                         │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ • File system (read Rust source, docs, examples)        │   │
│  │ • Network (fetch documentation when uncertain)          │   │
│  │ • Process management (spawn workers)                    │   │
│  │ • Persistent memory across episodes                     │   │
│  └─────────────────────────────────────────────────────────┘   │
│                           │                                     │
│  Layer 2: Kore Session (REPL-like)                             │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ • Define words that persist                             │   │
│  │ • Build up libraries over time                          │   │
│  │ • Introspect with `words`, `meta`                       │   │
│  │ • State accumulates between commands                    │   │
│  └─────────────────────────────────────────────────────────┘   │
│                           │                                     │
│  Layer 1: Execution Trace (Step-by-Step)                       │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ • See each operation as it executes                     │   │
│  │ • Observe stack state after each step                   │   │
│  │ • Learn stack manipulation visually                     │   │
│  │ • Intervene/correct during execution (future)           │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Layer 1: Trace-Augmented Learning

The trace IS the chain-of-thought for stack programs:

```python
# Example: LLM sees execution trace
Program: 5 3 swap dup mul add

Trace:
  Step 0: []           → push 5    → [5]
  Step 1: [5]          → push 3    → [5, 3]
  Step 2: [5, 3]       → swap      → [3, 5]
  Step 3: [3, 5]       → dup       → [3, 5, 5]
  Step 4: [3, 5, 5]    → mul       → [3, 25]
  Step 5: [3, 25]      → add       → [28]

Result: 28
```

**Why this helps:**
- LLM learns stack manipulation by observation
- Can predict next stack state (auxiliary task)
- Trace = natural language explanation of computation
- Errors show exactly where program went wrong

### Layer 2: Persistent Session

```python
# Episode 1: Define helper
LLM: [ dup mul ] "square" def
Kore: OK, defined 'square'

# Episode 2: Use helper
LLM: 5 square
Kore: [25]

# Episode 3: Build on it
LLM: [ square square ] "fourth" def
Kore: OK, defined 'fourth'

# Episode 4: Introspect
LLM: words
Kore: ["square", "fourth", ...]
```

**Why this helps:**
- Agent builds up library over training
- Learns abstraction and reuse
- Can "teach itself" by defining helpers
- Definitions persist = long-term memory

### Layer 3: Full Container Access

```python
# Read Kore source to understand primitives
LLM: fs-read "/app/src/core/stack.rs" 
Kore: "pub fn dup(stack: &mut Stack) -> Result<()> { ... }"

# Fetch documentation when uncertain
LLM: http-get "https://docs.kore.dev/tensor-matmul"
Kore: { "signature": "(A B rows cols -- C)", "examples": [...] }

# Save learned patterns
LLM: { pattern: "swap dup mul add", effect: "(2 -- 1)" } 
     "patterns/quadratic.json" fs-write
```

**Why this helps:**
- Agent can read its own implementation
- Self-documenting: reads source to understand behavior
- Internet access when genuinely uncertain
- Persistent artifacts across training runs

---

## Part II: Hard Problems Where Kore's Structure Helps

### Selection Criteria
1. **Mathematically tractable** - we can prove things
2. **Sparse reward normally** - effect system adds density
3. **Verifiable** - we know when we've solved it
4. **Real-world relevance** - not just toy problems

### Problem 1: Symbolic Regression (Finding Formulas)

**The problem**: Given $(x_i, y_i)$ pairs, find a program $p$ such that $p(x_i) \approx y_i$.

**Why Kore helps**:
```
Traditional approach:
  - Generate expression trees randomly
  - Only signal: final MSE
  - No guidance during construction

Kore approach:
  - Programs have effects: (1 -- 1) for x→y functions
  - Effect mismatch = early termination (wrong shape)
  - Trace shows intermediate computations
  - Can verify algebraic equivalence of solutions
```

**Mathematical formulation**:

$$\mathcal{L} = \underbrace{\frac{1}{N}\sum_i (p(x_i) - y_i)^2}_{\text{MSE}} + \lambda_1 \underbrace{d(\text{effect}(p), (1,1))}_{\text{effect penalty}} + \lambda_2 \underbrace{|p|}_{\text{complexity}}$$

**Concrete task**: Rediscover physics formulas
```
Input: [(1, 1), (2, 4), (3, 9), (4, 16), ...]
Target: Find program equivalent to `dup mul` (x²)

Input: [(1, 1), (2, 8), (3, 27), ...]  
Target: Find program for x³

Input: [(m, v, E) where E = 0.5*m*v²]
Target: Rediscover kinetic energy formula
```

**Effect advantage**:
- Wrong effect → immediate negative reward
- Don't waste compute on (2 -- 3) programs for (1 -- 1) task
- 100x search space reduction

---

### Problem 2: Chemical Reaction Network Synthesis

**The problem**: Design a sequence of reactions to transform substrate A into product B.

**Why Kore helps**: 
- Linear types model conservation of mass
- Each molecule = linear resource (consumed exactly once)
- Reactions = programs that transform resources
- Effect = stoichiometry

**Mapping**:
```kore
; Molecules as linear values
"H2" linear-new    ; Hydrogen (must use exactly once)
"O2" linear-new    ; Oxygen

; Reaction: 2H2 + O2 → 2H2O
: combustion ( H2 H2 O2 -- H2O H2O )
  ; Consumes 2 H2 and 1 O2, produces 2 H2O
  linear-unwrap linear-unwrap linear-unwrap  ; consume inputs
  drop drop drop                              ; use the values
  "H2O" linear-new "H2O" linear-new          ; produce outputs
;
```

**Conservation law enforcement**:
```
Effect of valid reaction: (n inputs -- m outputs)
where: Σ(input masses) = Σ(output masses)

Linear types guarantee: 
  - No duplication of molecules (conservation of mass)
  - No dropping of molecules (mass doesn't disappear)
  - All reactants consumed (complete reaction)
```

**Concrete task**: Pathway synthesis
```
Given:
  - Starting materials: [glucose, O2, ATP, enzymes...]
  - Target product: ethanol
  - Allowed reactions: glycolysis, fermentation, etc.

Find: Sequence of reactions (Kore program) that:
  1. Consumes starting materials
  2. Produces target + byproducts
  3. Respects stoichiometry (linear types)
  4. Minimizes steps/energy
```

**Mathematical formulation**:

State space: $S = \mathbb{N}^{|\text{molecules}|}$ (count of each molecule)

Reactions: $R: S \to S$ with conservation: $\sum_i m_i \cdot s_i = \sum_i m_i \cdot R(s)_i$

Kore program $p$ valid iff: $\text{linear-ok}(p) = \text{true}$

Reward:
$$R(p) = \mathbf{1}[\text{produces-target}(p)] \cdot \frac{1}{|p|} \cdot \mathbf{1}[\text{linear-ok}(p)]$$

---

### Problem 3: Quantum Circuit Synthesis

**The problem**: Given a target unitary $U$, find a circuit (sequence of gates) that implements $U$.

**Why Kore helps**:
- Quantum gates are stack operations on qubit states
- Circuit = program
- Effect tracks qubit count (no qubit creation/destruction in unitary circuits)
- Can verify properties without simulation

**Mapping**:
```kore
; Qubit state as value
[1 0] tensor-from-list    ; |0⟩ state

; Gates as operations
: H ( qubit -- qubit' )   ; Hadamard
  ; 1/√2 [[1,1],[1,-1]] matrix multiply
  [[0.707 0.707] [0.707 -0.707]] tensor-from-list
  swap 2 1 tensor-matmul
;

: CNOT ( control target -- control' target' )
  ; Controlled-NOT gate
  ; ... implementation using tensor ops
;

; Circuit = composition
: bell-state ( -- q1 q2 )
  |0⟩ |0⟩        ; Two qubits
  swap H swap    ; Hadamard on first
  CNOT           ; Entangle
;
```

**Effect constraint for unitarity**:
```
Valid quantum circuit: effect = (n -- n)  [same number of qubits]
Invalid: (2 -- 3)  [creates qubit - not unitary]
Invalid: (3 -- 2)  [destroys qubit - not unitary]
```

**Concrete task**: Gate synthesis
```
Target: Implement Toffoli gate using {H, T, CNOT}
Constraint: Minimize gate count

Effect requirement: (3 -- 3)  [3-qubit gate]

Verification: For all input states |ψ⟩:
  circuit(|ψ⟩) = Toffoli(|ψ⟩)
```

**Advantage**: Effect mismatch detected statically
- Wrong qubit count → immediate rejection
- Don't waste compute simulating invalid circuits

---

### Problem 4: Automated Theorem Proving (Curry-Howard)

**The problem**: Given a proposition, find a proof (= program of the right type).

**Why Kore helps**:
- Propositions = effects (types)
- Proofs = programs with those effects
- Effect inference = type checking
- Proof search = program synthesis with effect constraint

**The Curry-Howard correspondence in Kore**:
```
Proposition          | Kore Type/Effect
---------------------|------------------
A → B                | (A -- B)
A ∧ B                | (-- A B)
A ∨ B                | (-- Either A B)
∀x. P(x)            | polymorphic effect
∃x. P(x)            | existential + witness
```

**Concrete task**: Prove simple theorems
```kore
; Prove: (A → B) → (B → C) → (A → C)
; This is function composition!

; Given: f : (A -- B), g : (B -- C)
; Prove: (A -- C)

: compose ( f g -- fg )
  ; fg(a) = g(f(a))
  swap    ; g f
  [ swap call swap call ] ; Apply f then g
;

; Effect of compose: ((A--B) (B--C) -- (A--C)) ✓
```

**Mathematical formulation**:

Proof search as optimization:
$$\min_{p} |p| \quad \text{s.t.} \quad \text{effect}(p) = \tau$$

Where $\tau$ is the target type (proposition to prove).

Kore's effect inference: $\text{effect}: \text{Program} \to \text{Type}$

Search is tractable because:
1. Effect computed in O(n) for program of length n
2. Wrong effects pruned immediately
3. Effect composition is algebraic (predictable)

---

### Problem 5: Compiler Optimization (Peephole)

**The problem**: Find equivalent but faster code.

**Why Kore helps**:
- Algebraic identities = rewrite rules
- Effect preserved under valid rewrites
- Equivalence testable (not undecidable like general case)
- Agent discovers optimizations by exploration

**Setup**:
```kore
; Input: unoptimized program
swap swap drop dup mul swap rot rot rot

; Agent discovers:
; - swap swap = identity (involution)
; - rot rot rot = identity (order 3)
; - Optimized: drop dup mul

; Verification: test equivalence on random stacks
```

**Concrete task**: Learn Kore's algebraic identities
```
Through exploration, agent should discover:
1. swap swap ≡ ε
2. not not ≡ ε  
3. neg neg ≡ ε
4. dup drop ≡ ε
5. rot rot rot ≡ ε
6. over nip ≡ dup
7. swap nip ≡ drop
...
```

**Reward**:
$$R = \begin{cases} 
\frac{|p_1|}{|p_2|} & \text{if } p_1 \sim p_2 \text{ and } |p_2| < |p_1| \\
0 & \text{otherwise}
\end{cases}$$

---

### Problem 6: Game Playing with Stack Semantics

**The problem**: Learn to play games where state fits stack model.

**Why Kore helps**:
- Game state = stack
- Moves = stack operations
- Trace = game history
- Effect = board evaluation delta

**Example: Countdown Numbers Game**
```
Given: numbers [100, 25, 8, 3, 7, 2]
Target: 952

Find: arithmetic expression using each number at most once

Kore formulation:
  - Numbers as linear resources
  - Operations: add, sub, mul, div
  - Must consume inputs to produce result
  - Linear types prevent reusing numbers
```

```kore
; Solution: 100 × 8 + 25 × 7 - 3 - 2 - 100 + 8...
; Actually: (100 + 3) × (8 + 2) - 25 - 7 - ...

; Each number is linear
100 linear-new
25 linear-new  
8 linear-new
3 linear-new
7 linear-new
2 linear-new

; Must unwrap and consume each exactly once
; Agent learns to compose arithmetic to hit target
```

**Effect advantage**:
- Track "material count" via effect
- Invalid: using number twice (linear violation)
- Invalid: not using a number (linear violation)
- Immediate feedback on resource usage

---

## Part III: The Most Promising Direction

### Recommendation: Symbolic Regression + Verification

**Why this is the best first target:**

1. **Mathematically clean**
   - Clear objective: minimize prediction error
   - Effect constraint: (n -- 1) for n inputs
   - Verifiable: test on held-out data

2. **Dense reward via effects**
   - Wrong effect → reject immediately
   - Right effect → evaluate on data
   - Intermediate: effect distance as shaping

3. **Real scientific value**
   - Rediscover physics formulas from data
   - Aid scientific discovery
   - Interpretable results (unlike neural nets)

4. **Benchmarkable**
   - Existing benchmarks: SRBench, Feynman equations
   - Can compare to GP, neural symbolic regression
   - Clear metrics: accuracy, complexity, generalization

5. **Builds naturally on Kore's strengths**
   - Effect system guides search
   - Algebraic simplification for Occam's razor
   - Trace for interpretability

### Concrete Experiment: Feynman Equations

```python
FEYNMAN_TASKS = [
    # Mechanics
    {"name": "kinetic_energy", "formula": "0.5 * m * v^2", 
     "inputs": ["m", "v"], "effect": "(2 -- 1)"},
    
    {"name": "gravitational_force", "formula": "G * m1 * m2 / r^2",
     "inputs": ["G", "m1", "m2", "r"], "effect": "(4 -- 1)"},
    
    # Electromagnetism  
    {"name": "coulomb", "formula": "k * q1 * q2 / r^2",
     "inputs": ["k", "q1", "q2", "r"], "effect": "(4 -- 1)"},
    
    # Quantum
    {"name": "de_broglie", "formula": "h / (m * v)",
     "inputs": ["h", "m", "v"], "effect": "(3 -- 1)"},
    
    # Thermodynamics
    {"name": "ideal_gas", "formula": "n * R * T / V",
     "inputs": ["n", "R", "T", "V"], "effect": "(4 -- 1)"},
]

def evaluate_symbolic_regression(program, task):
    """Evaluate discovered formula."""
    
    # 1. Effect check (fast, no execution)
    analysis = effect_infer(program)
    if analysis["effect"] != task["effect"]:
        return {"reward": -0.5, "reason": "wrong_effect"}
    
    # 2. Numerical accuracy (execute on test data)
    errors = []
    for inputs, expected in task["test_data"]:
        result = execute(program, inputs)
        errors.append(abs(result - expected))
    
    mse = np.mean(np.square(errors))
    
    # 3. Complexity penalty (Occam's razor)
    complexity = len(program.split())
    
    # 4. Simplification bonus
    simplified = algebraic_simplify(program)
    if len(simplified) < len(program):
        simplification_bonus = 0.1
    else:
        simplification_bonus = 0.0
    
    reward = np.exp(-mse) - 0.01 * complexity + simplification_bonus
    
    return {
        "reward": reward,
        "mse": mse,
        "complexity": complexity,
        "program": program,
        "simplified": simplified,
    }
```

### What Success Looks Like

```
Training Progress:
─────────────────────────────────────────────────────────────
Epoch 1-10:   Agent learns stack operations, basic arithmetic
Epoch 11-20:  Discovers dup mul = x², starts finding patterns
Epoch 21-30:  Learns division, handles multi-input formulas
Epoch 31-40:  Rediscovers: KE, Coulomb, ideal gas law
Epoch 41-50:  Generalizes to unseen equations, simplifies
─────────────────────────────────────────────────────────────

Example Discovery:
  Input data: [(m,v,E)] where E = 0.5*m*v²
  
  Agent generates: "dup mul swap 2 div mul"
  Trace:
    [m, v] → dup → [m, v, v]
    → mul → [m, v²] 
    → swap → [v², m]
    → 2 → [v², m, 2]
    → div → [v², m/2]  ← Wrong!
    
  Agent refines: "swap dup mul mul 2 div"
  Trace:
    [m, v] → swap → [v, m]
    → dup → [v, m, m]  ← Wrong path
    
  Agent finally: "over mul mul 2 div"
  Trace:
    [m, v] → over → [m, v, m]
    → mul → [m, v*m]
    → mul → [m*v*m]  ← Wrong!
  
  Correct: "dup mul mul 2 div"
  [m, v] → dup → [m, v, v]
        → mul → [m, v²]
        → mul → [m*v²]
        → 2 div → [0.5*m*v²] ✓
```

---

## Part IV: Implementation Roadmap

### Week 1: Interactive Runtime
- [ ] Streaming trace output during execution
- [ ] REPL mode with persistent definitions
- [ ] Read-only access to Rust source in container

### Week 2: Symbolic Regression Setup
- [ ] Feynman equation benchmark
- [ ] Effect-guided reward function
- [ ] Algebraic simplifier integration

### Week 3: Training Loop
- [ ] GRPO with effect shaping
- [ ] Curriculum: arithmetic → algebra → physics
- [ ] Trace-conditioned generation

### Week 4: Evaluation
- [ ] Compare vs genetic programming baseline
- [ ] Measure effect of each component
- [ ] Publish results

---

## Conclusion

**The key insight**: Kore's algebraic structure turns sparse-reward problems into dense-reward problems by providing:

1. **Effect distance** as continuous signal
2. **Static analysis** before execution
3. **Testable equivalence** for verification
4. **Linear types** for resource tracking
5. **Trace** as interpretable reasoning

This enables RL approaches that simply don't work for conventional languages.

**Recommended first experiment**: Symbolic regression on Feynman equations
- Clean mathematical setup
- Real scientific value  
- Exploits Kore's advantages
- Benchmarkable against existing methods
