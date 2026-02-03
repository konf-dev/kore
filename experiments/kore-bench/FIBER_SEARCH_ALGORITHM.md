# Fiber-Guided Program Search: A Kore-Native Learning Algorithm

## Executive Summary

This document proposes **Fiber-Guided Program Search (FGPS)**, a novel learning algorithm designed specifically to exploit Kore's unique features:

1. **Fibers** — Pausable, forkable computation states
2. **Checkpointing** — Persistent state snapshots  
3. **Traces** — Deterministic execution history
4. **Effect Analysis** — Static verification before execution

The key insight: In Kore, **computation state is a first-class value**. This enables search algorithms impossible in conventional languages.

---

## 1. Why Conventional RL Fails on Kore

### The Cold Start Problem

Standard RL (PPO, REINFORCE) assumes:
- Random exploration occasionally finds reward
- Small perturbations lead to gradual improvement

For Kore program synthesis:
- Random tokens → 99.9% syntax errors
- Partial correct programs → 0 reward (all-or-nothing)
- No gradient signal until complete success

### What Kore Offers Instead

| Feature | Conventional Language | Kore |
|---------|----------------------|------|
| Execution state | Opaque (registers, heap) | **First-class value (stack)** |
| Pause execution | Impossible | `fiber-yield` |
| Fork execution | Impossible | `fiber-fork` |
| Inspect state | Debugger only | `fiber-stack` |
| Checkpoint | Manual serialization | `checkpoint` / `restore` |
| Deterministic replay | Not guaranteed | **Trace monoid guarantees it** |

---

## 2. Fiber-Guided Program Search (FGPS)

### 2.1 Core Idea: Beam Search with Fiber Checkpoints

Instead of generating complete programs and hoping they work, we:

1. **Generate token-by-token** with an LLM
2. **Execute incrementally** in a fiber
3. **Fork promising states** to explore alternatives
4. **Checkpoint successful prefixes** for reuse
5. **Prune failing branches** early (stack underflow, type errors)

```
┌─────────────────────────────────────────────────────────────┐
│  Token 1     Token 2     Token 3     Token 4     Token 5    │
│    ▼           ▼           ▼           ▼           ▼        │
│   [3]   →   [3 4]   →  [3 4 add] → [7 2]    → [7 2 mul]    │
│    │           │           │           │           │        │
│  fiber₀     fiber₁      fiber₂     fiber₃      fiber₄      │
│                │                       │                    │
│              fork                    fork                   │
│                ↓                       ↓                    │
│            fiber₁'                  fiber₃'                 │
│           [3 4 mul]               [7 2 add]                 │
│               ↓                       ↓                     │
│              [12]                    [9]                    │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 Algorithm: FGPS

```python
def fiber_guided_search(llm, goal_stack, max_steps, beam_width):
    """
    Search for a Kore program that produces goal_stack.
    
    Key innovation: We execute incrementally and checkpoint
    promising partial programs.
    """
    
    # Initialize with empty program in fresh fiber
    initial_fiber = kore.fiber_new("[]")  # Empty quote
    beams = [(initial_fiber, "", 0.0)]    # (fiber, program, log_prob)
    
    checkpoints = {}  # stack_state → (program, fiber)
    
    for step in range(max_steps):
        candidates = []
        
        for fiber, program, log_prob in beams:
            # Get current stack state (Kore-native introspection!)
            stack = kore.fiber_stack(fiber)
            
            # Check if we've reached the goal
            if stack_matches(stack, goal_stack):
                return program  # Success!
            
            # LLM proposes next tokens given current stack state
            next_tokens = llm.propose(
                program=program,
                current_stack=stack,  # Key: LLM sees actual state
                goal_stack=goal_stack,
                top_k=beam_width
            )
            
            for token, token_prob in next_tokens:
                # Fork fiber (copy-on-write, O(1) space!)
                fiber_copy = kore.fiber_fork(fiber)
                
                try:
                    # Execute single token in the forked fiber
                    new_fiber = kore.fiber_resume(fiber_copy, token)
                    new_stack = kore.fiber_stack(new_fiber)
                    
                    # Static effect check before continuing
                    if kore.effect_valid(new_stack, remaining_program_hint):
                        candidates.append((
                            new_fiber,
                            program + " " + token,
                            log_prob + math.log(token_prob)
                        ))
                        
                        # Checkpoint novel stack states
                        stack_key = hash_stack(new_stack)
                        if stack_key not in checkpoints:
                            checkpoints[stack_key] = (program + " " + token, new_fiber)
                
                except KoreError as e:
                    # Early pruning: syntax error, stack underflow, type error
                    # This is free information Python doesn't give us!
                    pass
        
        # Beam selection: keep top-k by probability + stack progress
        beams = select_top_k(candidates, beam_width, goal_stack)
        
        if not beams:
            # All branches failed - try restoring from checkpoints
            beams = restore_from_checkpoints(checkpoints, goal_stack)
    
    return None  # No solution found
```

### 2.3 Why This Works Better Than Pure LLM Generation

| Approach | Failure Mode | FGPS Solution |
|----------|--------------|---------------|
| **Generate-then-execute** | Complete program, then discover syntax error in token 3 | Catch error at token 3, prune |
| **No state visibility** | LLM can't see what's on the stack | `fiber-stack` shows exact state |
| **Single path** | One wrong token ruins everything | `fiber-fork` explores alternatives |
| **Restart from scratch** | Failed program = wasted compute | Checkpoint successful prefixes |
| **No partial credit** | 95% correct = 0 reward | Track stack distance to goal |

---

## 3. Key Technical Innovations

### 3.1 Stack-Aware LLM Prompting

Unlike conventional code generation, we can show the LLM the **actual execution state**:

```
Current program: 3 4 add
Current stack: [7]
Goal stack: [14]

What token should come next?
Options:
1. "2" → stack becomes [7, 2]
2. "mul" → stack becomes [14] ← MATCHES GOAL
3. "dup" → stack becomes [7, 7]
```

The LLM learns to predict tokens **conditioned on stack state**, not just program text.

### 3.2 Incremental Effect Verification

Before executing, we can check if a token **will work**:

```kore
; Current stack: [a b]
; Proposed token: "add"
; Effect of add: (a b -- c)

effect-check: consumes 2, produces 1
stack-depth: 2
Result: ✓ Valid (2 ≥ 2)

; Proposed token: "rot"
; Effect of rot: (a b c -- b c a)

effect-check: consumes 3, produces 3
stack-depth: 2
Result: ✗ Invalid (2 < 3) — PRUNE IMMEDIATELY
```

This is **static analysis** — no execution needed to know `rot` will fail.

### 3.3 Checkpoint-Based Exploration

When we find a program that achieves a useful intermediate state, we save it:

```python
# Discovered: "3 4 add" produces [7]
checkpoints["[7]"] = ("3 4 add", fiber_at_7)

# Later, searching for [14]:
# We can start from [7] instead of []!
restored_fiber = checkpoints["[7]"]
# Now just need "2 mul" instead of "3 4 add 2 mul"
```

### 3.4 Trace-Based Verification

Kore traces are **deterministic**. Given a program and initial stack, we get the exact same trace every time:

```kore
"3 4 add 2 mul" trace-run
; → Trace: [PUSH 3, PUSH 4, CALL add, PUSH 2, CALL mul]
; → Final stack: [14]
; → Reproducible: YES
```

This enables:
- **Verification**: Run program N times, same result every time
- **Debugging**: Trace shows exactly what happened
- **Data augmentation**: Same program = same label (no random variation)

---

## 4. Training the LLM for FGPS

### 4.1 Data Format

Unlike standard code generation (problem → program), we train on:

```
{
    "stack_before": [3, 4],
    "stack_after": [7],
    "next_token": "add",
    "alternatives": ["mul", "sub", "swap"],
    "alternative_stacks": [[12], [-1], [4, 3]]
}
```

The model learns: **given stack state, what token produces desired state?**

### 4.2 Training Objective

$$\mathcal{L} = -\log P(t_{next} | \text{program}_{1:i}, \text{stack}_i, \text{stack}_{goal})$$

This is different from standard next-token prediction:
- Input includes **actual stack state** (not just program text)
- Output is conditioned on **goal state**

### 4.3 Curriculum Learning

1. **Phase 1: Single operations** (1 token programs)
   - `3` → produces `[3]`
   - `add` on `[2, 3]` → produces `[5]`

2. **Phase 2: Two-token compositions**
   - `3 dup` → `[3, 3]`
   - `add 2` on `[1, 2]` → `[3, 2]`

3. **Phase 3: Full programs**
   - Multi-step computation with conditionals, loops

### 4.4 Reinforcement Learning with Partial Credit

FGPS naturally provides **partial credit**:

$$r(s_i) = -\text{distance}(\text{stack}_i, \text{stack}_{goal})$$

Instead of 0/1 reward for complete programs, we reward:
- Each step that moves stack closer to goal
- Checkpoint discoveries (new reachable states)
- Shorter programs that achieve same result

---

## 5. Implementation Plan

### Phase 1: Core Infrastructure (Week 1)

```rust
// Add to kore-agent/src/tools/search.rs

pub fn fiber_search_tool() -> Tool {
    Tool::native("fiber-search", 
        "(goal:List program:Quote max-steps:Int beam:Int -- result:Quote)",
        |stack, ctx| {
            // Implementation of FGPS
        }
    )
}
```

### Phase 2: Training Data Generation (Week 2)

Generate training pairs with stack states:

```python
def generate_stack_transition_data():
    for program in corpus:
        fiber = kore.fiber_new(f"[{program}]")
        stack_history = []
        
        for token in program.split():
            stack_before = kore.fiber_stack(fiber)
            fiber = kore.fiber_resume(fiber, token)
            stack_after = kore.fiber_stack(fiber)
            
            yield {
                "stack_before": stack_before,
                "token": token,
                "stack_after": stack_after,
                "program_so_far": " ".join(tokens_so_far)
            }
```

### Phase 3: Model Training (Week 3)

Train small model (1B parameters) on stack-conditioned next-token prediction:

```python
class StackConditionedLLM(nn.Module):
    def forward(self, program_tokens, stack_state, goal_state):
        # Embed program
        prog_embed = self.program_encoder(program_tokens)
        
        # Embed stack (as sequence of type+value pairs)
        stack_embed = self.stack_encoder(stack_state)
        goal_embed = self.stack_encoder(goal_state)
        
        # Cross-attention between program and stack
        fused = self.cross_attention(prog_embed, stack_embed, goal_embed)
        
        # Predict next token
        return self.token_head(fused)
```

### Phase 4: FGPS Integration (Week 4)

Combine trained model with beam search:

```python
def fgps_solve(problem: str, goal: List[Value]) -> Optional[str]:
    llm = StackConditionedLLM.load("kore-fgps-1b")
    
    return fiber_guided_search(
        llm=llm,
        goal_stack=goal,
        max_steps=100,
        beam_width=16
    )
```

---

## 6. Expected Results

### 6.1 Comparison to Baselines

| Method | Pass@1 (expected) | Compute per Problem |
|--------|-------------------|---------------------|
| Direct generation (no search) | 15-25% | 1 forward pass |
| Generate-and-test (k=100) | 35-45% | 100 forward passes |
| FGPS (beam=16, steps=50) | **55-70%** | ~50 forward passes + search |

### 6.2 Key Advantages

1. **Early failure detection**: ~60% of branches pruned by effect analysis
2. **Checkpoint reuse**: ~30% of solutions found by extending checkpoints
3. **Stack visibility**: LLM accuracy +15% when shown stack state
4. **Partial credit**: Training converges 3x faster than 0/1 reward

### 6.3 Limitations

1. **Overhead**: Fiber fork/resume adds ~1ms per step
2. **Memory**: Beam of 16 fibers × 50 steps = ~800 fiber states
3. **LLM latency**: Still dominated by model inference time

---

## 7. Novel Research Contributions

### 7.1 Computation-as-Search-State

The core insight: **the executing program is a searchable object**.

In conventional languages:
- Execution is opaque
- State is in registers, heap, call stack
- No way to "fork" a running computation

In Kore:
- Execution state = stack + program counter
- State is a first-class value (`Fiber`)
- Fork is O(1) with copy-on-write

This enables **execution-guided search** — a fundamentally new approach.

### 7.2 Effect-Guided Pruning

Static effect analysis enables **type-directed search**:

```
Current: stack has 2 values
Goal: stack needs 3 values
Effect of candidate token: consumes 2, produces 1

Net change: 2 → 1 (decreases stack)
Goal requires: increase by 1

Conclusion: This token moves AWAY from goal → prune
```

This is impossible in dynamically-typed languages where you can't know what an operation will do to the stack without running it.

### 7.3 Deterministic Replay for Verification

All discovered programs can be verified by replay:

```
Program: "3 4 add 2 mul"
Verification:
  - Run 1: [14] ✓
  - Run 2: [14] ✓
  - Run 3: [14] ✓
  
Conclusion: Program is correct (deterministic guarantee)
```

Compare to Python where `random.random()` or `datetime.now()` make replay non-deterministic.

---

## 8. Implementation: Kore Primitives Needed

### 8.1 Already Implemented

| Primitive | Status | Location |
|-----------|--------|----------|
| `fiber-new` | ✅ | Value::Fiber |
| `fiber-resume` | ✅ | kore-agent |
| `fiber-yield` | ✅ | kore-agent |
| `checkpoint` | ✅ | kore-agent/tools/checkpoint.rs |
| `restore` | ✅ | kore-agent/tools/checkpoint.rs |
| `effect-infer` | ⚠️ Partial | effect.rs |

### 8.2 Needs Implementation

| Primitive | Description | Priority |
|-----------|-------------|----------|
| `fiber-fork` | COW duplicate of fiber | **HIGH** |
| `fiber-stack` | Get stack as list | **HIGH** |
| `effect-check` | Check if token valid given stack | **MEDIUM** |
| `stack-distance` | Distance metric between stacks | **MEDIUM** |
| `batch-resume` | Resume multiple fibers in parallel | **LOW** (optimization) |

### 8.3 Required Implementation (fiber-fork)

```rust
// In kore/src/value.rs

impl Fiber {
    pub fn fork(&self) -> Fiber {
        Fiber {
            stack: self.stack.clone(),  // COW in Rust: Arc<[Value]>
            ip: self.ip,
            program: self.program.clone(),
            status: FiberStatus::Paused,
            result: None,
        }
    }
}

// In kore-agent/src/tools/fiber.rs

pub fn fiber_fork_tool() -> Tool {
    Tool::native("fiber-fork", "(fiber -- fiber fiber)", |mut stack, ctx| {
        Box::pin(async move {
            let fiber = stack.pop()?.as_fiber()?;
            let fiber_clone = fiber.fork();
            stack.push(Value::Fiber(fiber))?;
            stack.push(Value::Fiber(fiber_clone))?;
            Ok((stack, ctx))
        })
    })
}
```

---

## 9. Conclusion

**Fiber-Guided Program Search (FGPS)** is not just "beam search for code generation." It's a fundamentally new approach enabled by Kore's unique properties:

1. **Computation is a value** → Can fork, inspect, checkpoint
2. **Effects are static** → Can prune impossible branches
3. **Traces are deterministic** → Can verify any solution

This is the learning algorithm **optimized for Kore** — it couldn't exist for Python, JavaScript, or any conventional language.

### Next Steps

1. Implement `fiber-fork` and `fiber-stack` primitives
2. Generate training data with stack states
3. Train stack-conditioned LLM
4. Evaluate FGPS on KoreEval benchmark
5. Compare to Python baseline on equivalent tasks

---

## Appendix: Pseudocode in Kore

The search algorithm itself can be written in Kore:

```kore
; fiber-guided-search: (goal beam-width -- program)
[
  ; Initialize beams with empty program
  [ [] fiber-new "" 0.0 ] list-wrap
  "beams" def
  
  ; Main search loop
  100 [
    beams [
      ; For each beam: (fiber, program, log_prob)
      unpack3
      
      ; Get current stack
      fiber-stack "stack" def
      
      ; Check goal
      stack goal stack-eq [
        ; Found solution!
        program "FOUND" return
      ] when
      
      ; Get LLM proposals
      program stack goal llm-propose
      
      ; Fork and execute each proposal
      [
        ; (token, prob) on stack
        swap "token" def "prob" def
        
        fiber fiber-fork
        token fiber-resume
        
        ; Check validity
        fiber-stack effect-valid [
          ; Add to candidates
          fiber program " " token str-concat prob candidates cons
        ] when
      ] each
      
    ] map flatten
    
    ; Select top-k
    beam-width beam-select "beams" def
    
  ] times
  
  ; No solution found
  null
] "fiber-search" def
```

This is **Kore searching for Kore** — the language is expressive enough to implement its own search algorithm.
