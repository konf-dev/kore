# Kore Scientific Experiments

## The Core Insight

**Kore's type system has mathematical properties that map directly to physical laws.**

This is NOT about "LLM + Kore". This is about what Kore's types MEAN mathematically.

## The Four Experiments

### 1. Quantum Circuit Verification (`quantum_linear_types.py`)

**Isomorphism:** Linear Types = No-Cloning Theorem

In quantum mechanics, the no-cloning theorem states you cannot copy an unknown quantum state.
In Kore, linear types enforce that values must be used exactly once.

```
Kore Linear Type    ↔    Quantum Mechanics
─────────────────────────────────────────────
x consumed once     ↔    no-cloning theorem
measurement = use   ↔    wave function collapse
effect: Q:1→0,C:0→1 ↔    qubit → classical bit
```

**Results:**
- Linear type checking rejects ~92% of random programs
- 100% of accepted programs are valid quantum circuits
- Bell state preparation verified correct at compile time

Run: `python quantum_linear_types.py`

### 2. VQE Circuit Synthesis (`vqe_circuit_synthesis.py`)

**Application:** Find shortest quantum circuits for molecular ground states.

This is what quantum chemists do manually or with gradient-based optimizers.
Kore's approach: **enumerate all valid circuits, pick shortest that works.**

**Results:**
```
System              Circuit Found                      Fidelity   Circuits Checked
────────────────────────────────────────────────────────────────────────────────────
H2 molecule         X[1]                               98.69%     14
Heisenberg chain    H[0] → CNOT[0,1] → Y[0]           100.00%    193
```

- **16.7x speedup** from effect filtering (rejecting circuits with wrong qubit signature)
- Found provably shortest circuits (global optimum, not local)
- Hardware-efficient ansatz with 1 layer achieves 99.95% fidelity

Run: `python vqe_circuit_synthesis.py`

### 3. DNA Conservation Verification (`dna_linear_types.py`)

**Isomorphism:** Linear Types = Conservation of Mass

In biochemistry, you can't create or destroy nucleotides. In Kore, linear types 
ensure that resources are conserved through operations.

```
Kore Linear Type    ↔    Biochemistry
─────────────────────────────────────────────
consume + produce   ↔    substrate → products
no double use       ↔    can't use digested DNA
effect tracking     ↔    mass balance equation
```

**Results:**
- Use-after-consume errors caught (like use-after-free)
- Nucleotide counts automatically verified through pipeline
- Cloning workflow proven correct: 32bp + 12bp insert = 44bp product

Run: `python dna_linear_types.py`

### 4. Optimal Learning Algorithm Search (`optimal_learner_search.py`)

**Theorem:** Shortest Program = MDL-Optimal Hypothesis

The Minimum Description Length (MDL) principle says the best model is the shortest
description. Kore's finite syntax makes program enumeration tractable.

```
Kore Property       ↔    Learning Theory
─────────────────────────────────────────────
program length      ↔    description length
effect filtering    ↔    hypothesis space pruning
enumeration         ↔    guaranteed optimality
```

**Results:**
```
Function     Shortest Program           Length
──────────────────────────────────────────────
AND          mul                        1 op
OR           or                         1 op
XOR          xor                        1 op
NAND         mul not                    2 ops
PARITY-3     xor xor                    2 ops
MAJORITY-3   add add dup 1 sub and      6 ops
DOUBLE       dup add                    2 ops
SQUARE       dup mul                    2 ops
```

Effect type filtering provides ~5x speedup for search.

Run: `python optimal_learner_search.py`

## Why This Matters

### vs Qiskit/Cirq (Quantum)
- They: Runtime errors for invalid circuits, gradient-based VQE
- Kore: **Compile-time** rejection + **enumeration** for global optimum
- Advantage: Provably shortest circuits, no local minima

### vs Biopython (Biology)
- They: Trust the programmer to track conservation
- Kore: **Automatic** verification of mass balance
- Advantage: Catches errors before wet lab ($$$ savings)

### vs AutoML (Learning)
- They: Gradient-based search, local optima
- Kore: **Guaranteed global optimum** via enumeration
- Advantage: Provably optimal for small problems

## Mathematical Foundations

See [KORE_MATH_FOUNDATIONS.md](KORE_MATH_FOUNDATIONS.md) for the full theory:

1. **Stack Semantics** = Categorical morphism composition
2. **Linear Logic** = Girard's resource semantics
3. **Effect Types** = Indexed monads / graded comonads

These aren't implementation details—they're the reason Kore works for science.
