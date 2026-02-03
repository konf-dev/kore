# Kore RL Experiment Designs

> **Date**: 2026-02-03
> **Status**: Research Design

## Overview

Two training paradigms:
1. **Supervised Curriculum** - Progressive tasks with verification rewards
2. **Unsupervised Self-Play** - Agent explores, discovers, develops capabilities

Kore's algebraic structure enables training approaches impossible with conventional languages.

---

## Part I: Why Kore Enables Novel RL Architectures

### 1.1 The Verification Advantage

In conventional language RL (e.g., training on Python), we face:
- **Execution risk**: Running arbitrary code is dangerous
- **Reward sparsity**: "Does it work?" is binary
- **No intermediate signal**: Can't verify partial progress

Kore provides:
```
┌─────────────────────────────────────────────────────────────┐
│  Program p                                                  │
│      │                                                      │
│      ▼                                                      │
│  ┌─────────────┐     ┌──────────────┐     ┌──────────────┐ │
│  │ effect-infer│ ──▶ │ Static Props │ ──▶ │ Safe to Run? │ │
│  └─────────────┘     │ • consumes   │     │ • pure?      │ │
│                      │ • produces   │     │ • bounded?   │ │
│                      │ • io-effects │     │ • linear-ok? │ │
│                      └──────────────┘     └──────────────┘ │
│                             │                    │          │
│                             ▼                    ▼          │
│                    Intermediate Reward    Execute if Safe   │
└─────────────────────────────────────────────────────────────┘
```

**Key Insight**: We can compute dense rewards *before* execution.

### 1.2 The Effect Algebra as Reward Shaping

The effect composition rule:
```
compose((a,b), (c,d)) = 
  if b ≥ c then (a, b - c + d)
  else (a + c - b, d)
```

This defines a **monoid** on effects with identity (0,0).

**Theorem**: For any target effect ε_target, we can define a potential-based reward:
```
Φ(ε) = -||ε - ε_target||₂
R(s, a, s') = γΦ(s') - Φ(s)
```

This is **automatically shaped** - no reward hacking possible because effect composition is algebraic.

### 1.3 The Capability Lattice as Curriculum

Capabilities form a bounded lattice:
```
        ┌───────┐
        │  ALL  │  (⊤ = full capabilities)
        └───┬───┘
       ╱    │    ╲
   ┌──┴─┐ ┌─┴──┐ ┌┴───┐
   │ fs │ │net │ │exec│
   └──┬─┘ └─┬──┘ └┬───┘
       ╲    │    ╱
        ┌───┴───┐
        │ NONE  │  (⊥ = pure computation)
        └───────┘
```

**Curriculum via Lattice Descent**:
- Start at ⊤ (all capabilities)
- Progressively attenuate: cap-attenuate moves down lattice
- Agent learns to accomplish goals with fewer capabilities
- Final goal: solve problems at ⊥ (pure computation only)

---

## Part II: Experiment 1 - Supervised Curriculum (Baseline)

### 2.1 Setup

```python
# Curriculum phases with increasing difficulty
PHASES = {
    1: {"tools": ["add", "sub", "mul", "dup", "swap", "drop"],
        "tasks": "arithmetic expressions",
        "reward": "correctness"},
    
    2: {"tools": Phase1 + ["if", "times", "def"],
        "tasks": "simple functions (factorial, square)",
        "reward": "correctness + effect_match"},
    
    3: {"tools": Phase2 + ["list-*", "map", "filter", "fold"],
        "tasks": "list algorithms (sum, reverse, sort)",
        "reward": "correctness + effect_match + efficiency"},
    
    4: {"tools": Phase3 + ["words", "meta", "effect-infer"],
        "tasks": "introspective tasks",
        "reward": "correctness + self_verification"},
    
    5: {"tools": Phase4 + ["fs-*", "http-*", "cap-*"],
        "tasks": "capability-aware I/O",
        "reward": "correctness + minimal_capabilities"},
}
```

### 2.2 Reward Function

```python
def compute_reward(program: str, task: Task, phase: int) -> float:
    # Static analysis (no execution needed)
    try:
        analysis = effect_infer(program)
    except ParseError:
        return -1.0  # Syntax error
    
    effect = analysis["effect"]
    io_effects = analysis["io"]
    is_pure = analysis["pure"]
    
    # Effect matching reward (always computable)
    effect_reward = effect_similarity(effect, task.expected_effect)
    
    # Capability minimality (for phase 5+)
    cap_reward = 1.0 - len(io_effects) / len(ALL_EFFECTS)
    
    # Execution reward (only if safe)
    if analysis["safe"]:
        result = execute(program, task.inputs)
        correct = result == task.expected_output
        exec_reward = 1.0 if correct else 0.0
    else:
        exec_reward = 0.0
    
    # Phase-weighted combination
    weights = PHASE_WEIGHTS[phase]
    return (weights.effect * effect_reward +
            weights.cap * cap_reward +
            weights.exec * exec_reward)
```

### 2.3 Training Algorithm: GRPO with Effect Shaping

```python
def grpo_step(model, task_batch, phase):
    """Group Relative Policy Optimization with effect shaping."""
    
    programs = []
    for task in task_batch:
        # Generate K candidates per task
        candidates = model.generate(task.prompt, n=K)
        programs.append(candidates)
    
    # Compute rewards (parallelizable - static analysis is cheap)
    rewards = [[compute_reward(p, t, phase) for p in ps] 
               for ps, t in zip(programs, task_batch)]
    
    # GRPO: relative advantage within group
    for task_idx, (ps, rs) in enumerate(zip(programs, rewards)):
        baseline = np.mean(rs)
        advantages = [(r - baseline) / (np.std(rs) + 1e-8) for r in rs]
        
        for p, adv in zip(ps, advantages):
            if adv > 0:
                loss = -log_prob(p) * adv
                loss.backward()
    
    optimizer.step()
```

---

## Part III: Experiment 2 - Unsupervised Effect Exploration

### 3.1 Core Idea: Intrinsic Motivation via Effect Novelty

The agent explores the space of *program effects* rather than programs themselves.

```
Effect Space E = ℕ × ℕ × P(IO)
            = (consumes, produces, io_effects)

Coverage C ⊆ E = set of effects seen so far

Novelty(p) = min_{e ∈ C} ||effect(p) - e||
```

**Why this works**: 
- Syntactically different programs can have same effect
- Agent is rewarded for *behavioral* novelty, not syntactic
- Effect space is much smaller than program space
- Encourages discovering new *capabilities*, not just new syntax

### 3.2 The Effect Novelty Reward

```python
class EffectNoveltyReward:
    def __init__(self, embedding_dim=64):
        self.effect_memory = []  # List of seen effects
        self.knn = None          # For fast novelty lookup
        
    def embed_effect(self, effect: Dict) -> np.ndarray:
        """Embed effect into vector space."""
        # Continuous embedding of discrete effect
        cons = effect["consumes"]
        prod = effect["produces"]
        io = effect["io"]
        
        # One-hot for IO effects
        io_vec = np.zeros(len(ALL_IO_EFFECTS))
        for e in io:
            io_vec[IO_EFFECT_IDX[e]] = 1.0
        
        # Stack effect as 2D point
        stack_vec = np.array([cons, prod]) / 10.0
        
        return np.concatenate([stack_vec, io_vec])
    
    def novelty(self, program: str) -> float:
        """Compute novelty of program's effect."""
        try:
            analysis = effect_infer(program)
        except:
            return 0.0  # Invalid programs get no novelty
        
        effect_vec = self.embed_effect(analysis)
        
        if len(self.effect_memory) < 10:
            novelty = 1.0  # Everything is novel initially
        else:
            # K-nearest neighbor distance
            distances = [np.linalg.norm(effect_vec - e) 
                        for e in self.effect_memory]
            novelty = np.mean(sorted(distances)[:5])
        
        # Add to memory
        self.effect_memory.append(effect_vec)
        
        return novelty
```

### 3.3 Self-Talk via Effect Composition

Agent plays two roles in alternation:

```
┌─────────────────────────────────────────────────────────┐
│                    SELF-PLAY LOOP                       │
│                                                         │
│  ┌─────────────┐         ┌─────────────┐               │
│  │  PROPOSER   │         │  COMPOSER   │               │
│  │  "Create p  │         │  "Given p₁  │               │
│  │   with new  │ ──p₁──▶ │   extend to │               │
│  │   effect"   │         │   reach ε"  │               │
│  └─────────────┘         └──────┬──────┘               │
│         ▲                       │                       │
│         │                      p₂                       │
│         │                       │                       │
│         │                       ▼                       │
│         │              ┌─────────────┐                 │
│         │              │  VERIFIER   │                 │
│         │              │  Check:     │                 │
│         └──feedback────│  ε(p₁;p₂)=ε │                 │
│                        └─────────────┘                 │
└─────────────────────────────────────────────────────────┘
```

```python
def self_play_episode(model):
    """One episode of self-play exploration."""
    
    # Phase 1: Proposer generates a program
    p1 = model.generate(
        prompt="Generate a Kore program with interesting effect:",
        temperature=1.0  # High diversity
    )
    
    try:
        e1 = effect_infer(p1)
    except:
        return {"reward": -0.5, "reason": "invalid_p1"}
    
    # Phase 2: Sample target effect (compositional goal)
    target_effect = sample_target_effect(e1)
    
    # Phase 3: Composer extends to reach target
    p2 = model.generate(
        prompt=f"Given program:\n{p1}\nWith effect {e1}\n"
               f"Write continuation to achieve total effect {target_effect}:",
        temperature=0.7
    )
    
    # Phase 4: Verify composition
    composed = f"{p1} {p2}"
    try:
        e_total = effect_infer(composed)
    except:
        return {"reward": -0.3, "reason": "invalid_composition"}
    
    # Reward based on effect matching
    effect_match = effect_similarity(e_total, target_effect)
    novelty = effect_novelty_reward.novelty(composed)
    
    return {
        "reward": 0.5 * effect_match + 0.5 * novelty,
        "p1": p1,
        "p2": p2,
        "effects": (e1, e_total, target_effect)
    }
```

### 3.4 Mathematical Justification

**Claim**: Effect-based exploration is provably more efficient than syntax-based.

**Proof sketch**:
- Let P = space of programs, E = space of effects
- Effect map ε: P → E is many-to-one (ε factors through algebraic simplification)
- |E| << |P| (effect space is finite-dimensional, program space is countably infinite)
- Random exploration in P hits each effect with probability ~1/|P|
- Effect-targeted exploration hits each effect with probability ~1/|E|
- Efficiency ratio: |P|/|E| → ∞

Therefore, effect-novelty reward provides exponentially better coverage of the *capability space*.

---

## Part IV: Experiment 3 - Capability Descent Game

### 4.1 Core Idea: Learn to Do More with Less

```
Start: Agent has all capabilities (⊤)
Goal:  Accomplish task with minimal capabilities

Reward = correctness × (1 + capability_budget_remaining)
```

This is like a **resource management game** where capabilities are the resource.

### 4.2 Formal Setup

```python
@dataclass
class CapabilityBudget:
    fs: int = 10      # File system ops allowed
    net: int = 5      # Network ops allowed
    spawn: int = 2    # Spawn ops allowed
    exec: int = 0     # No shell exec (safety)
    
    def can_afford(self, io_effects: List[str]) -> bool:
        costs = Counter(io_effects)
        return (costs.get("fs", 0) <= self.fs and
                costs.get("net", 0) <= self.net and
                costs.get("spawn", 0) <= self.spawn and
                costs.get("exec", 0) <= self.exec)
    
    def remaining_fraction(self) -> float:
        total = self.fs + self.net + self.spawn + self.exec
        max_total = 10 + 5 + 2 + 0  # Initial budget
        return total / max_total if max_total > 0 else 1.0
```

### 4.3 Progressive Attenuation

```python
def capability_descent_episode(model, task, initial_budget):
    """Train agent to solve task with minimal capabilities."""
    
    budget = initial_budget.copy()
    solutions = []
    
    # Binary search for minimal budget
    while True:
        # Generate solution under current budget
        prompt = f"""
Task: {task.description}
Expected effect: {task.expected_effect}
Capability budget: {budget}

Write a Kore program that solves this task within the budget.
Remember: Each fs-read costs 1 fs budget, each http-get costs 1 net budget.
"""
        
        program = model.generate(prompt)
        analysis = effect_infer(program)
        
        if not budget.can_afford(analysis["io"]):
            # Over budget - this solution doesn't count
            reward = -0.5
            break
        
        if execute_and_verify(program, task):
            solutions.append((program, budget.copy()))
            # Try with less budget
            budget = attenuate_budget(budget)
        else:
            # Failed - can't go lower
            break
    
    if solutions:
        best_program, best_budget = min(solutions, 
                                        key=lambda x: -x[1].remaining_fraction())
        reward = 1.0 + best_budget.remaining_fraction()
    else:
        reward = 0.0
    
    return reward, solutions
```

### 4.4 The Capability Lattice Curriculum

```
Training Schedule:
─────────────────────────────────────────────────────────
Epoch 1-10:    Full capabilities, learn to solve tasks
Epoch 11-20:   Remove exec, learn pure alternatives  
Epoch 21-30:   Remove spawn, learn sequential solutions
Epoch 31-40:   Remove net, learn offline solutions
Epoch 41-50:   Remove fs, learn pure computation
─────────────────────────────────────────────────────────
                         │
                         ▼
Final agent can solve tasks with minimal IO
```

---

## Part V: Experiment 4 - Algebraic Equivalence Discovery

### 5.1 Core Idea: Learn Kore's Algebraic Structure

The agent discovers equivalences like:
```
swap swap ≡ ε
dup drop ≡ ε  
rot rot rot ≡ ε
over nip ≡ dup
```

**Without being told these identities.**

### 5.2 Self-Supervised Equivalence Learning

```python
def equivalence_discovery_episode(model):
    """Agent tries to discover algebraic equivalences."""
    
    # Generate a random program
    p1 = model.generate(
        prompt="Generate a short Kore program (3-10 operations):",
        max_tokens=50
    )
    
    # Ask model to simplify
    p2 = model.generate(
        prompt=f"Simplify this Kore program to equivalent shorter form:\n{p1}",
        max_tokens=50
    )
    
    # Verify equivalence by testing on random stacks
    equivalent = verify_equivalence(p1, p2, n_tests=100)
    
    if equivalent:
        len_ratio = len(p1) / max(len(p2), 1)
        if len_ratio > 1.5:
            # Discovered a non-trivial simplification!
            reward = len_ratio
            log_discovery(p1, p2, "simplification")
        else:
            reward = 0.1  # Trivial equivalence
    else:
        reward = -0.5  # Wrong simplification
    
    return reward

def verify_equivalence(p1: str, p2: str, n_tests: int = 100) -> bool:
    """Test equivalence on random stacks."""
    for _ in range(n_tests):
        stack = generate_random_stack(depth=random.randint(1, 5))
        try:
            r1 = execute(p1, initial_stack=stack)
            r2 = execute(p2, initial_stack=stack)
            if r1 != r2:
                return False
        except:
            return False
    return True
```

### 5.3 Equivalence Memory and Generalization

```python
class EquivalenceMemory:
    """Store discovered equivalences and use for training."""
    
    def __init__(self):
        self.equivalences = []  # (p1, p2) pairs
        self.patterns = {}      # Generalized patterns
        
    def add(self, p1: str, p2: str):
        self.equivalences.append((p1, p2))
        
        # Try to generalize
        pattern = self.extract_pattern(p1, p2)
        if pattern:
            self.patterns[pattern] = (p1, p2)
    
    def extract_pattern(self, p1, p2):
        """Try to find a general pattern."""
        # e.g., "X X ≡ ε" from "swap swap ≡ ε" and "not not ≡ ε"
        tokens1 = tokenize(p1)
        
        # Check for involution pattern: X X = ε
        if len(tokens1) == 2 and tokens1[0] == tokens1[1] and p2.strip() == "":
            return ("involution", tokens1[0])
        
        # Check for annihilation: dup drop = ε
        if tokens1 == ["dup", "drop"] and p2.strip() == "":
            return ("annihilation", "dup", "drop")
        
        return None
    
    def generate_training_data(self) -> List[Tuple[str, str]]:
        """Use discovered equivalences as training data."""
        data = []
        for p1, p2 in self.equivalences:
            # Bidirectional
            data.append((f"Simplify: {p1}", p2))
            data.append((f"Expand: {p2}", p1))
        return data
```

---

## Part VI: Experiment 5 - Linear Resource Game

### 6.1 Core Idea: Teach Resource Management via Linear Types

Linear values must be used exactly once. We create puzzles:

```kore
; Given 3 linear resources
"resource-a" linear-new
"resource-b" linear-new  
"resource-c" linear-new

; Goal: Use each exactly once to produce result
; (If you drop one, error. If you dup one, error.)
```

### 6.2 Linear Puzzle Generator

```python
def generate_linear_puzzle(n_resources: int, n_operations: int) -> Task:
    """Generate a puzzle requiring exact resource consumption."""
    
    # Create N linear resources with values
    setup = []
    for i in range(n_resources):
        value = random.randint(1, 10)
        setup.append(f"{value} linear-new")
    
    # Define target: must consume all resources in some computation
    # e.g., sum all values
    target_sum = sum(random.randint(1, 10) for _ in range(n_resources))
    
    prompt = f"""
You have {n_resources} linear resources on the stack.
Each MUST be unwrapped exactly once (no dup, no drop).
Compute their sum.

Initial: {' '.join(setup)}
Expected: single integer (the sum)

Hint: linear-unwrap consumes the linear wrapper and returns the value.
"""
    
    return Task(
        prompt=prompt,
        setup=setup,
        verify=lambda result: result == target_sum,
        constraints=["no_dup_linear", "no_drop_linear", "all_consumed"]
    )

def verify_linear_constraints(program: str, n_resources: int) -> bool:
    """Verify linear resources are handled correctly."""
    
    # Parse and simulate
    ops = parse(program)
    linear_count = n_resources
    
    for op in ops:
        if op == "linear-unwrap":
            linear_count -= 1
        elif op == "dup" and linear_on_top():
            return False  # Can't dup linear
        elif op == "drop" and linear_on_top():
            return False  # Can't drop linear
    
    # All must be consumed
    return linear_count == 0
```

### 6.3 Curriculum: Increasing Resource Complexity

```
Level 1: 2 linear resources, simple sum
Level 2: 3 linear resources, operations between them
Level 3: Mixed linear and unrestricted values
Level 4: Linear values inside lists (list-take required)
Level 5: Linear capability tokens (use exactly once)
```

---

## Part VII: Unified Unsupervised Training Loop

### 7.1 Multi-Task Exploration

```python
class UnsupervisedKoreTrainer:
    """Unified trainer for unsupervised Kore exploration."""
    
    def __init__(self, model):
        self.model = model
        
        # Intrinsic motivation modules
        self.effect_novelty = EffectNoveltyReward()
        self.equivalence_memory = EquivalenceMemory()
        self.capability_tracker = CapabilityTracker()
        
        # Task samplers
        self.task_types = [
            ("effect_exploration", 0.3),
            ("self_play_composition", 0.2),
            ("equivalence_discovery", 0.2),
            ("linear_puzzles", 0.15),
            ("capability_descent", 0.15),
        ]
    
    def sample_task(self) -> str:
        types, probs = zip(*self.task_types)
        return np.random.choice(types, p=probs)
    
    def train_step(self):
        task_type = self.sample_task()
        
        if task_type == "effect_exploration":
            reward, info = self.effect_exploration_episode()
        elif task_type == "self_play_composition":
            reward, info = self.self_play_episode()
        elif task_type == "equivalence_discovery":
            reward, info = self.equivalence_episode()
        elif task_type == "linear_puzzles":
            reward, info = self.linear_puzzle_episode()
        elif task_type == "capability_descent":
            reward, info = self.capability_episode()
        
        # Update model with reward
        self.update_policy(info["programs"], reward)
        
        # Log discoveries
        if reward > 1.0:
            self.log_discovery(task_type, info)
        
        return reward, info
    
    def effect_exploration_episode(self):
        """Generate program, reward effect novelty."""
        program = self.model.generate(
            "Generate an interesting Kore program:",
            temperature=1.2
        )
        
        try:
            analysis = effect_infer(program)
            novelty = self.effect_novelty.novelty(program)
            
            # Bonus for discovering new IO combinations
            io_novelty = self.capability_tracker.io_novelty(analysis["io"])
            
            reward = novelty + 0.5 * io_novelty
            
            return reward, {"programs": [program], "effect": analysis}
        except:
            return -0.5, {"programs": [program], "error": "parse_failed"}
    
    def self_play_episode(self):
        """Proposer-Composer-Verifier loop."""
        # ... (as defined earlier)
        pass
    
    def equivalence_episode(self):
        """Discover algebraic identities."""
        # ... (as defined earlier)
        pass
```

### 7.2 The Exploration-Exploitation Balance

```python
def compute_exploration_bonus(program: str, memory: ExplorationMemory) -> float:
    """
    Compute exploration bonus using Kore's unique properties.
    
    Unlike RND (Random Network Distillation), we use semantic features:
    - Effect signature
    - Trace fingerprint
    - Capability usage pattern
    """
    
    # 1. Effect-based novelty
    effect = effect_infer(program)
    effect_vec = embed_effect(effect)
    effect_bonus = memory.effect_novelty(effect_vec)
    
    # 2. Trace-based novelty (behavioral)
    trace = execute_with_trace(program)
    fingerprint = trace_fingerprint(trace)
    trace_bonus = memory.trace_novelty(fingerprint)
    
    # 3. Capability pattern novelty
    cap_pattern = frozenset(effect["io"])
    cap_bonus = memory.capability_novelty(cap_pattern)
    
    # Weighted combination
    return 0.4 * effect_bonus + 0.4 * trace_bonus + 0.2 * cap_bonus
```

---

## Part VIII: Mathematical Analysis

### 8.1 Convergence Properties

**Theorem 1 (Effect Space Coverage)**:
Under effect-novelty reward, the agent's coverage of effect space E grows as:
```
|C_t| ≥ |E| × (1 - e^{-λt})
```
where λ depends on model entropy and effect space structure.

**Proof sketch**: Each episode has probability ≥ p of discovering new effect (where p depends on novelty threshold). By coupon collector bounds, we cover E in O(|E| log |E|) episodes.

**Theorem 2 (Algebraic Discovery)**:
If the agent explores uniformly over short programs (length ≤ k), it discovers all algebraic identities of the form `X ≡ Y` where |X|, |Y| ≤ k with probability → 1 as training time → ∞.

**Proof sketch**: Finite program space at bounded length. Equivalence verification is exact. Novelty reward ensures exploration of all pairs.

### 8.2 Why This Doesn't Work for Python

```
Python:                          Kore:
─────────────────────────────────────────────────────────────
Effect unknown until runtime  │  Effect computed statically
Equivalence undecidable       │  Equivalence testable
Halting problem               │  Bounded by stack/caps
Side effects anywhere         │  Effects declared in types
─────────────────────────────────────────────────────────────
```

Kore's algebraic structure makes these experiments *mathematically tractable*.

### 8.3 Gradient Flow Analysis

For effect-based rewards, the gradient signal is:
```
∇_θ J = E_p~π_θ [ ∇_θ log π_θ(p) × R_effect(p) ]
```

where R_effect has the decomposition:
```
R_effect(p) = R_novelty(effect(p)) + R_correctness(p) + R_capability(p)
```

Each component provides independent gradient signal:
- R_novelty: Encourages diverse effects
- R_correctness: Encourages valid programs
- R_capability: Encourages minimal resource use

**Key insight**: These rewards are *non-interfering* because they operate on orthogonal aspects of the program.

---

## Part IX: Implementation Plan

### 9.1 Phase 1: Infrastructure (Week 1)

```
□ Effect-based reward computation
□ Trace fingerprint extraction
□ Equivalence verification harness
□ Linear type constraint checker
□ Capability budget tracker
```

### 9.2 Phase 2: Supervised Baseline (Week 2)

```
□ Curriculum task generators
□ GRPO training loop
□ Evaluation benchmarks
□ Baseline metrics
```

### 9.3 Phase 3: Unsupervised Experiments (Week 3-4)

```
□ Effect novelty exploration
□ Self-play composition
□ Equivalence discovery
□ Linear resource games
□ Capability descent
```

### 9.4 Phase 4: Analysis (Week 5)

```
□ Compare supervised vs unsupervised
□ Measure capability generalization
□ Analyze discovered equivalences
□ Ablation studies
```

---

## Part X: Expected Outcomes

### 10.1 Supervised Training
- Agent learns to solve curriculum tasks
- Effect signatures match expectations
- Generalization to unseen tasks within distribution

### 10.2 Unsupervised Training
- Agent discovers Kore's algebraic identities
- Coverage of effect space
- Emergent capability minimization
- Novel program patterns not in training data

### 10.3 Comparison Hypotheses

| Metric | Supervised | Unsupervised | Combined |
|--------|------------|--------------|----------|
| Task accuracy | High | Medium | High |
| Effect diversity | Low | High | Medium |
| Algebraic knowledge | None | High | High |
| Capability efficiency | Medium | High | High |
| Generalization | Medium | High | High |

**Prediction**: Combined training (supervised + unsupervised exploration) will outperform either alone.

---

## Appendix: Key Equations

### A.1 Effect Composition
```
ε(A ; B) = compose(ε(A), ε(B))

compose((a,b), (c,d)) = 
  (a + max(0, c-b), d + max(0, b-c))
```

### A.2 Novelty Reward
```
R_novelty(p) = min_{e ∈ Memory} ||embed(effect(p)) - e||_2
```

### A.3 Capability Cost
```
R_cap(p) = 1 - |io_effects(p)| / |ALL_IO_EFFECTS|
```

### A.4 Linear Constraint Violation
```
L_linear(p) = Σ_{v ∈ linear_values} |uses(v) - 1|
```

### A.5 Combined Objective
```
J(θ) = E_p~π_θ [ R_task(p) + α R_novelty(p) + β R_cap(p) - γ L_linear(p) ]
```

---

*Document version: 1.0 | Author: Kore Training Team*
