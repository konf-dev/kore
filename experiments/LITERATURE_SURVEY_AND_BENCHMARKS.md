# Kore Benchmark Literature Survey

> **Goal**: Identify areas where Kore's design should outperform existing approaches, supported by literature analysis.

---

## Executive Summary

Kore is a stack-based concatenative language designed for **machine-authored, verifiable code**. Based on literature review, we identify **5 key benchmark areas** where Kore's properties should provide measurable advantages:

| Area | Kore Advantage | Baseline Comparison | Measurement |
|------|----------------|---------------------|-------------|
| 1. LLM Code Generation Accuracy | Minimal syntax, compositional semantics | Python, JavaScript | Pass@k, syntax error rate |
| 2. Agent Code Safety | Capability lattice, static effects | Sandboxed Python, WASM | Escape attempts, resource violations |
| 3. Verification Speed | Effect signatures, bounded resources | SMT on imperative code | Verification time, completeness |
| 4. Program Synthesis | Search space structure | Neural program synthesis | Sample efficiency, correctness |
| 5. Compositional Reasoning | Monoid homomorphism property | Standard AST-based | Refactoring correctness |

---

## 1. LLM Code Generation

### Literature Foundation

#### Key Papers

1. **HumanEval** (Chen et al., 2021) - "Evaluating Large Language Models Trained on Code"
   - Benchmark: 164 Python programming problems with unit tests
   - Finding: Codex solves 28.8% at pass@1, 70.2% at pass@100
   - **Gap identified**: "difficulty with docstrings describing long chains of operations"
   - **Kore relevance**: Stack-based left-to-right evaluation = one chain of operations

2. **MBPP** (Austin et al., 2021) - "Program Synthesis with Large Language Models"
   - 974 entry-level programming tasks
   - Finding: Performance scales log-linearly with model size
   - Finding: "Unable to predict output of program given specific input"
   - **Kore relevance**: Deterministic stack semantics = predictable outputs

3. **StarCoder** (Li et al., 2023) - 15.5B parameters, 80+ languages
   - 40% pass@1 on HumanEval with prompting
   - **Gap**: Training on many languages = syntax confusion
   - **Kore relevance**: Single, minimal syntax

4. **Magicoder** (Wei et al., 2023) - "OSS-Instruct" for diverse code
   - 66.5% pass@1 on HumanEval+ (beats ChatGPT)
   - Shows synthetic instruction data helps
   - **Kore opportunity**: Generate Kore training data from specifications

5. **OpenCodeInterpreter** (Zheng et al., 2024)
   - 83.2% on HumanEval with execution feedback
   - Key insight: **iterative refinement** with execution improves accuracy
   - **Kore opportunity**: Faster execution + better error messages

### Why Kore Should Be Better

| Problem with Existing Languages | Kore Solution |
|--------------------------------|---------------|
| **Syntax variations**: Python has many ways to express same thing | One way: push values, call tools |
| **Hidden state**: Global variables, closures, object state | All state on explicit stack |
| **Evaluation order**: Nested calls evaluate inside-out | Linear left-to-right evaluation |
| **Error propagation**: Exceptions can jump anywhere | Explicit `Error` type on stack |
| **Effect opacity**: No way to know function side effects | Static effect signatures |

### Proposed Benchmark: KoreEval

```
Benchmark Structure:
├── Level 1: Stack Manipulation (50 tasks)
│   ├── Basic: dup, drop, swap, over, rot
│   ├── Arithmetic: add, sub, mul, div, mod
│   └── Comparison: eq, lt, gt, and, or
├── Level 2: Control Flow (50 tasks)
│   ├── Conditionals: if, when, unless
│   ├── Loops: times, loop, each
│   └── Error handling: try, fail, unwrap
├── Level 3: Data Structures (50 tasks)
│   ├── Lists: list-get, list-set, map, filter, fold
│   ├── Maps: map-get, map-set, map-keys
│   └── Text: str-concat, str-split, str-find
├── Level 4: Higher-Order (50 tasks)
│   ├── Quotations: [code] def, call, apply
│   ├── Combinators: dip, keep, bi, tri
│   └── Recursion: self-referential definitions
└── Level 5: Algorithms (50 tasks)
    ├── Sorting: bubble, merge, quick
    ├── Search: binary, graph traversal
    └── Math: factorial, fibonacci, gcd
```

### Metrics to Compare

| Metric | Definition | Expected Kore Advantage |
|--------|------------|------------------------|
| **Syntax Error Rate** | % of generations that don't parse | Lower (simpler syntax) |
| **Pass@1** | % correct on first try | Higher (less ambiguity) |
| **Pass@k** | % correct in k tries | Higher (structured search) |
| **Token Efficiency** | Correct solutions / tokens generated | Higher (less boilerplate) |
| **Semantic Consistency** | Same input → same output | Perfect (deterministic) |

---

## 2. Agent Code Safety

### Literature Foundation

#### Key Papers

1. **"The Rise of LLM-Based Agents"** (Xi et al., 2023)
   - Comprehensive survey: 86 pages on agent architectures
   - Key components: Brain (LLM), Perception, Action
   - **Gap identified**: No formal safety model for action execution
   - **Kore relevance**: Capability-based action restrictions

2. **"If LLM Is the Wizard, Then Code Is the Wand"** (Yang et al., 2024)
   - Survey on code for LLM agents
   - Finding: Code enables structured execution, but also vulnerabilities
   - **Gap**: "No standard for safe code execution"
   - **Kore relevance**: Built-in capability lattice

3. **"Language Agent Tree Search" (LATS)** (Zhou et al., 2023)
   - 92.7% pass@1 on HumanEval with GPT-4 + MCTS
   - Uses environment feedback for planning
   - **Gap**: Environment = arbitrary Python execution
   - **Kore relevance**: Bounded, safe execution environment

4. **WebAssembly/WASI** (Industry Standard)
   - Capability-based security for web runtimes
   - Sandboxed, deterministic execution
   - **Comparison**: Similar philosophy, but WASM is low-level
   - **Kore advantage**: High-level + capabilities

### Capability System Comparison

| Property | Python | JavaScript | WASM | Kore |
|----------|--------|------------|------|------|
| File system access | Unrestricted | Browser limited | WASI capabilities | Lattice-based |
| Network access | Unrestricted | Same-origin policy | WASI capabilities | Explicit `net:*` |
| CPU limits | None | Event loop timeout | Fuel-based | Resource monoid |
| Memory limits | System OOM | Heap limits | Linear memory | Tracked allocation |
| Syscall filtering | seccomp (complex) | N/A | WASI | Built into language |
| Attenuation | No | No | No | Yes (lattice ≤) |
| Effect analysis | No | No | No | Static analysis |

### Proposed Benchmark: SafeAgent

```
Attack Categories:
├── 1. Capability Escalation (20 tests)
│   ├── Request fs after granted net only
│   ├── Access parent directory from /tmp grant
│   ├── Spawn child with more capabilities
│   └── Forge capability tokens
├── 2. Resource Exhaustion (20 tests)
│   ├── Infinite loop
│   ├── Stack overflow
│   ├── Memory bomb (exponential allocation)
│   └── Fork bomb (spawn infinite children)
├── 3. Information Leakage (20 tests)
│   ├── Timing side channels
│   ├── Error message information leak
│   ├── Trace fingerprint manipulation
│   └── Covert channels through effects
├── 4. Isolation Bypass (20 tests)
│   ├── Shared state between contexts
│   ├── Handle forging
│   ├── Type confusion attacks
│   └── Serialization vulnerabilities
└── 5. Semantic Violations (20 tests)
    ├── Violate linear type (use twice)
    ├── Escape affine constraint
    ├── Trace falsification
    └── Effect mismatch
```

### Metrics

| Metric | Definition | Goal |
|--------|------------|------|
| **Escape Rate** | % of attacks that succeed | 0% |
| **Detection Rate** | % of attacks detected vs silently blocked | 100% detected |
| **False Positive Rate** | Legitimate code blocked | <1% |
| **Overhead** | Slowdown from safety checks | <10% |
| **Auditability** | Can reproduce and verify execution | 100% |

---

## 3. Verification Speed

### Literature Foundation

#### Key Papers

1. **Z3/SMT Solvers** (de Moura & Bjørner)
   - State-of-the-art for software verification
   - Challenge: Complex control flow → exponential paths
   - **Kore advantage**: Linear control flow, explicit effects

2. **Dafny/F\*** (Microsoft Research)
   - Verified programming languages
   - Require programmer annotations
   - **Kore advantage**: Effect inference is automatic

3. **Liquid Haskell** (Vazou et al.)
   - Refinement types for Haskell
   - SMT-based verification
   - **Limitation**: Complex type inference
   - **Kore advantage**: Runtime-checked, simpler inference

### Why Verification is Easier in Kore

| Property | Impact on Verification |
|----------|----------------------|
| **No hidden state** | No need to track global variables |
| **Linear control flow** | Fewer execution paths to consider |
| **Effect signatures** | Immediate IO classification |
| **Bounded resources** | Automatic termination guarantee |
| **Deterministic semantics** | Single execution path per input |
| **Stack effect algebra** | Compositional stack depth analysis |

### Proposed Benchmark: VerifyBench

```
Verification Tasks:
├── 1. Stack Safety (50 tasks)
│   ├── Prove: program doesn't underflow stack
│   ├── Prove: program produces exactly N outputs
│   └── Prove: stack depth bounded by K
├── 2. Termination (50 tasks)
│   ├── Prove: loop terminates
│   ├── Prove: recursion terminates
│   └── Prove: resource exhaustion bound
├── 3. Effect Safety (50 tasks)
│   ├── Prove: program is pure
│   ├── Prove: program only uses fs, not net
│   └── Prove: spawned children respect capability constraints
├── 4. Functional Correctness (50 tasks)
│   ├── Prove: sorting algorithm is correct
│   ├── Prove: arithmetic identity holds
│   └── Prove: search returns correct element
└── 5. Information Flow (50 tasks)
    ├── Prove: no data leak from high to low
    ├── Prove: capability doesn't leak
    └── Prove: handle isn't forged
```

### Comparison Setup

| Language | Verification Approach | Expected Time |
|----------|----------------------|---------------|
| Python | Type inference (Pyright) + SMT | Baseline |
| Rust | Borrow checker (compile time) | Fast |
| Dafny | Pre/post-conditions → SMT | Varies |
| **Kore** | Effect algebra + SMT | Faster |

---

## 4. Program Synthesis

### Literature Foundation

#### Key Papers

1. **AlphaTensor** (Fawzi et al., 2022) - DeepMind
   - Discovers novel matrix multiplication algorithms
   - RL agent plays "tensor zeroing" game
   - Found algorithms beating 50-year-old Strassen improvements
   - **Key insight**: "Search space is > atoms in universe"
   - **Kore relevance**: Kore programs = similar tensor structure

2. **Neural Program Synthesis** (various)
   - Sequence-to-sequence models for code
   - Challenge: Large action space, sparse rewards
   - **Kore advantage**: Smaller vocabulary, structured composition

3. **Inductive Logic Programming** (classic)
   - Learn programs from examples
   - **Limitation**: Requires careful feature engineering
   - **Kore advantage**: Stack effect signatures = natural features

### Why Synthesis is Easier in Kore

| Property | Synthesis Advantage |
|----------|-------------------|
| **Fixed vocabulary** | ~80 primitives vs infinite identifiers |
| **No variables** | No need to invent/track names |
| **Compositional** | A + B always works if types match |
| **Effect signatures** | Prune impossible combinations |
| **Verifiable** | Check correctness immediately |

### AlphaTensor-Style Experiment for Kore

**Problem**: Synthesize Kore programs that implement arithmetic algorithms

```
Target Algorithms:
├── Karatsuba multiplication (fewer multiplies)
├── Strassen matrix multiply (Kore tensor ops)
├── Fast exponentiation (square-and-multiply)
├── GCD (Euclidean algorithm)
└── Sorting networks (minimal comparisons)

Approach:
1. Define "program tensor" representing partial Kore program
2. Each action = append one token (value or tool)
3. Reward = correctness × efficiency (fewer operations)
4. Train RL agent to maximize reward
5. Verify discovered programs with Z3
```

### Proposed Benchmark: SynthBench

```
Synthesis Difficulty Levels:
├── Level 1: Identity (10 tasks)
│   ├── Implement: n → n (identity)
│   ├── Implement: a b → b a (swap)
│   └── Implement: n → n n (dup)
├── Level 2: Arithmetic (20 tasks)
│   ├── Implement: a b → a+b
│   ├── Implement: n → n²
│   └── Implement: n → n! (factorial)
├── Level 3: Conditionals (20 tasks)
│   ├── Implement: n → abs(n)
│   ├── Implement: a b → max(a,b)
│   └── Implement: n → fibonacci(n)
├── Level 4: Lists (30 tasks)
│   ├── Implement: list → reversed
│   ├── Implement: list → sorted
│   └── Implement: list → sum
└── Level 5: Algorithms (20 tasks)
    ├── Implement: a b → gcd(a,b)
    ├── Implement: matrix matrix → product
    └── Implement: graph start end → path
```

### Metrics

| Metric | Definition |
|--------|------------|
| **Sample Efficiency** | Correct programs / training samples |
| **Token Efficiency** | Program length / optimal length |
| **Novelty** | Discovered programs ≠ training examples |
| **Generalization** | Works on held-out test inputs |

---

## 5. Compositional Reasoning

### Literature Foundation

#### Concatenative Language Theory

1. **"Mathematical Foundations of Joy"** (von Thun)
   - Formal semantics of concatenative languages
   - Proof: Concatenation = function composition
   - Stack functions form a monoid
   - **Key theorem**: $\llbracket A \cdot B \rrbracket = \llbracket B \rrbracket \circ \llbracket A \rrbracket$

2. **"Why Concatenative Programming Matters"** (Purdy, 2012)
   - Argument for concatenative paradigm
   - Benefits: Factoring, algebraic manipulation, simple semantics

3. **Linear Logic and Permutation Stacks** (Baker, 1993)
   - Linear logic for stack machines
   - Proves: Concatenative languages avoid garbage
   - **Kore relevance**: Affine/linear types for handles

### Algebraic Properties for Optimization

| Identity | Equation | Application |
|----------|----------|-------------|
| Swap involution | `swap swap = id` | Dead code elimination |
| Not involution | `not not = id` | Boolean simplification |
| Neg involution | `neg neg = id` | Arithmetic simplification |
| Rot period 3 | `rot rot rot = id` | Stack cycle detection |
| Dup-drop | `dup drop = id` | Redundancy elimination |
| Push-drop | `X drop = id` | Dead value elimination |

### Proposed Benchmark: EquivBench

```
Equivalence Classes:
├── 1. Algebraic Identities (30 pairs)
│   ├── `swap swap` ≡ ``
│   ├── `dup drop` ≡ ``
│   └── `rot rot rot` ≡ ``
├── 2. Semantic Equivalence (30 pairs)
│   ├── `2 mul` ≡ `dup add`
│   ├── `0 add` ≡ ``
│   └── `1 mul` ≡ ``
├── 3. Refactoring Equivalence (20 pairs)
│   ├── Inline definition ≡ original
│   ├── Factor common prefix ≡ original
│   └── Reorder independent operations ≡ original
├── 4. Effect Equivalence (10 pairs)
│   ├── Pure programs with same stack effect
│   └── IO programs with same trace
└── 5. Non-Equivalence (10 pairs)
    ├── Stack effect differs
    ├── Different final values
    └── Different side effects
```

---

## Experimental Design

### Phase 1: Baseline Establishment (Week 1-2)

1. **Implement KoreEval** benchmark suite
2. **Test LLMs** on Kore generation:
   - GPT-4, Claude, Qwen-Coder
   - Zero-shot, few-shot, fine-tuned
3. **Compare** to same models on Python equivalent tasks

### Phase 2: Safety Experiments (Week 3-4)

1. **Implement SafeAgent** attack suite
2. **Test Kore sandbox** against all attacks
3. **Compare** to:
   - RestrictedPython
   - Node.js vm2
   - WASM + WASI

### Phase 3: Verification Experiments (Week 5-6)

1. **Implement VerifyBench** verification tasks
2. **Measure** verification time:
   - Kore effect checker + Z3
   - Python + Pyright + SMT
   - Rust compile time
3. **Compare** completeness and speed

### Phase 4: Synthesis Experiments (Week 7-8)

1. **Implement SynthBench** synthesis tasks
2. **Train** synthesis agents:
   - RL (REINFORCE, PPO)
   - Enumeration search
   - LLM-guided
3. **Compare** sample efficiency vs program synthesis baselines

### Phase 5: Analysis & Paper (Week 9-10)

1. **Aggregate** results
2. **Statistical** significance tests
3. **Write** paper targeting:
   - PLDI (programming languages)
   - NeurIPS/ICML (ML + synthesis)
   - AAAI (AI agents)

---

## Expected Results

### Hypothesis 1: LLM Generation Accuracy
> Kore programs generated by LLMs will have **higher pass rates** and **lower syntax error rates** than equivalent Python programs.

**Rationale**: 
- Fewer tokens to generate
- No variable naming required
- Compositional semantics = correct by construction

### Hypothesis 2: Agent Safety
> Kore's capability system will **block 100%** of agent escape attempts that succeed against Python sandboxes.

**Rationale**:
- Capabilities are mathematically enforced (lattice)
- Resources are tracked (monoid)
- No hidden state to exploit

### Hypothesis 3: Verification Speed
> Kore programs will verify **5-10x faster** than equivalent Python programs.

**Rationale**:
- Effect signatures enable early pruning
- Linear control flow = fewer paths
- Bounded resources = automatic termination

### Hypothesis 4: Synthesis Efficiency
> RL agents will synthesize correct Kore programs with **10x fewer samples** than equivalent Python synthesis.

**Rationale**:
- Smaller action space (~80 vs infinite)
- Effect signatures prune invalid combinations
- Immediate verification provides dense reward

---

## Key References

### Code Generation & Benchmarks
1. Chen et al. (2021). Evaluating Large Language Models Trained on Code. arXiv:2107.03374
2. Austin et al. (2021). Program Synthesis with Large Language Models. arXiv:2108.07732
3. Li et al. (2023). StarCoder: May the Source Be with You! arXiv:2305.06161
4. Jimenez et al. (2023). SWE-bench: Real-World GitHub Issues. arXiv:2310.06770

### LLM Agents
5. Xi et al. (2023). The Rise and Potential of Large Language Model Based Agents. arXiv:2309.07864
6. Yang et al. (2024). If LLM Is the Wizard, Then Code Is the Wand. arXiv:2401.00812
7. Zhou et al. (2023). Language Agent Tree Search. arXiv:2310.04406

### Program Synthesis
8. Fawzi et al. (2022). Discovering Novel Algorithms with AlphaTensor. Nature.
9. Wei et al. (2023). Magicoder: OSS-Instruct. arXiv:2312.02120
10. Zheng et al. (2024). OpenCodeInterpreter. arXiv:2402.14658

### Concatenative Languages
11. von Thun. Mathematical Foundations of Joy. La Trobe University.
12. Purdy (2012). Why Concatenative Programming Matters.
13. Baker (1993). Linear Logic and Permutation Stacks.

### Safety & Verification
14. de Moura & Bjørner. Z3: An Efficient SMT Solver.
15. WebAssembly/WASI Specification.

---

## Resource Allocation

| Experiment | GPU | CPU | Time |
|------------|-----|-----|------|
| LLM benchmarking | 3090 Ti | Heavy | 2 weeks |
| Safety testing | None | Light | 1 week |
| Verification | None | Heavy (SMT) | 1 week |
| RL synthesis | 3090 Ti | Light | 2 weeks |
| Analysis | None | Light | 1 week |

**Total GPU hours**: ~200 (3090 Ti)
**Total timeline**: 8-10 weeks

---

## Next Steps

1. [ ] Implement KoreEval benchmark (Level 1-3 first)
2. [ ] Create Kore↔Python equivalent task pairs
3. [ ] Set up LLM evaluation pipeline
4. [ ] Implement SafeAgent attack suite
5. [ ] Design verification harness
6. [ ] Train initial synthesis agent

---

*Document created: 2026-02-03*
*Authors: Kore Team*
