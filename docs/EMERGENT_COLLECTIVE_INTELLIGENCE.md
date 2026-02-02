# Emergent Collective Intelligence in Kore

> **STATUS: DEEP DESIGN EXPLORATION**
> 
> Thinking through: What happens when N workflow graphs share RAM and ROM?

---

## The Core Question

```
Given:
  - N independent workflow graphs
  - Shared volatile memory (RAM)
  - Shared persistent memory (ROM)
  - Each graph can read/write both

What emerges?
What can we achieve?
How do we make it self-improving?
```

---

## Part 1: Biological Analogies

### Ant Colonies

```
Individual ant: Very simple
  - Follow pheromone trails
  - Deposit pheromones when finding food
  - Random walk when no trail

Shared environment: Pheromone trails (their "RAM/ROM")

Emergent behavior:
  - Optimal foraging paths
  - Dynamic reallocation when food source depletes
  - Collective problem solving
  - No central controller
```

Key insight: **Intelligence is in the environment, not the agent.**

### Neural Networks

```
Individual neuron: Very simple
  - Sum inputs
  - Apply activation
  - Send output

Shared environment: Weights (their "ROM")

Emergent behavior:
  - Pattern recognition
  - Generalization
  - Learning
```

Key insight: **Learning is weight adjustment based on feedback.**

### Immune System

```
Individual cell: Specialized
  - B cells: Remember pathogens
  - T cells: Kill infected cells
  - Macrophages: Clean up

Shared environment: Bloodstream (chemical signals)

Emergent behavior:
  - Recognize new threats
  - Remember old threats
  - Coordinate response
  - Self/non-self distinction
```

Key insight: **Memory enables rapid response to known patterns.**

---

## Part 2: What Would Kore Agents Share?

### Memory Types

```
┌─────────────────────────────────────────────────────────────────┐
│                         ROM (Persistent)                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  STRATEGIES: What approaches work for what tasks               │
│    "web-search" → {approach: "google-first", success: 0.92}    │
│    "booking"    → {approach: "api-direct", success: 0.78}      │
│                                                                 │
│  WORKFLOWS: The actual code/quotes that succeeded              │
│    "web-search-v3" → [google query parse-results rank]         │
│    "booking-v2"    → [check-price login fill confirm]          │
│                                                                 │
│  KNOWLEDGE: Facts learned about the world                      │
│    "api-rate-limits" → {"google": 100/min, "openai": 60/min}  │
│    "user-prefs"      → {"timezone": "PST", "format": "brief"} │
│                                                                 │
│  FAILURES: What NOT to do                                      │
│    "booking-pitfalls" → ["don't double-submit", "check TOS"]  │
│                                                                 │
│  METRICS: Historical performance data                          │
│    "task-history" → [{task, approach, duration, success}, ...] │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                         RAM (Session)                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ACTIVE TASKS: What's being worked on now                      │
│    "task-123" → {status: "in-progress", agent: "graph-2"}     │
│                                                                 │
│  LOCKS: Coordination between agents                            │
│    "booking-lock" → {holder: "graph-1", since: 1234567890}    │
│                                                                 │
│  SIGNALS: Messages between agents                              │
│    "graph-1:inbox" → [{from: "graph-2", msg: "found better"}] │
│                                                                 │
│  OBSERVATIONS: Real-time state of the world                    │
│    "current-price" → 299                                       │
│    "api-healthy" → true                                        │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### What Gets Shared vs Private?

```
SHARED (in ROM/RAM):
  ├── What works (strategies)
  ├── How to do it (workflows)
  ├── What we know (knowledge)
  ├── What to avoid (failures)
  └── Current state (observations)

PRIVATE (per agent):
  ├── Current stack
  ├── Current execution point
  ├── Local variables
  └── Agent identity
```

---

## Part 3: Agent Roles

Not all agents need to be identical. Specialization:

### Explorer Agents
```
Purpose: Try new approaches
Behavior:
  - High tolerance for failure
  - Random/creative variations
  - Write findings to ROM
  
Resources: High compute, low stakes tasks
```

### Exploiter Agents
```
Purpose: Use proven approaches
Behavior:
  - Read best strategies from ROM
  - Execute reliably
  - Handle real user tasks

Resources: Actual user-facing work
```

### Verifier Agents
```
Purpose: Validate explorer findings
Behavior:
  - Re-test approaches that explorers found
  - Confirm success rates
  - Update confidence in ROM

Resources: Duplicate effort, but increases reliability
```

### Meta Agents
```
Purpose: Optimize the system itself
Behavior:
  - Analyze ROM metrics
  - Adjust agent allocation (more explorers? more exploiters?)
  - Prune bad strategies
  - Compose new workflows from parts

Resources: System-level control
```

---

## Part 4: The Learning Loop

### Simple Version (Single Agent)

```
┌────────────────────────────────────────────────┐
│                                                │
│  1. OBSERVE: Read task requirements            │
│              Read ROM for relevant strategies  │
│                                                │
│  2. DECIDE: Select approach                    │
│             - Use best known (exploit)         │
│             - Try something new (explore)      │
│                                                │
│  3. ACT: Execute the approach                  │
│                                                │
│  4. MEASURE: Did it work? How long? Errors?    │
│                                                │
│  5. RECORD: Write results to ROM               │
│             Update strategy success rates      │
│                                                │
│  Loop back to 1                                │
│                                                │
└────────────────────────────────────────────────┘
```

### Multi-Agent Version

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                 │
│  EXPLORER AGENTS              EXPLOITER AGENTS                 │
│  ┌─────────────────┐          ┌─────────────────┐              │
│  │ Try new things  │          │ Do real tasks   │              │
│  │ Record findings │          │ Use best known  │              │
│  └────────┬────────┘          └────────┬────────┘              │
│           │                            │                        │
│           ▼                            ▼                        │
│  ┌─────────────────────────────────────────────────┐           │
│  │                    ROM                          │           │
│  │  Strategies, Workflows, Knowledge, Metrics      │           │
│  └─────────────────────────────────────────────────┘           │
│           │                            │                        │
│           ▼                            ▼                        │
│  ┌─────────────────┐          ┌─────────────────┐              │
│  │ VERIFIER AGENTS │          │  META AGENTS    │              │
│  │ Confirm findings│          │ Optimize system │              │
│  │ Update confidence│         │ Allocate agents │              │
│  └─────────────────┘          └─────────────────┘              │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## Part 5: Workflow Evolution

### Storing Workflows as Data

```
; A workflow is just a quote - it's data
"web-search-v1" [
  query google-search 
  results parse-html 
  links first fetch
] rom-set

; An agent can READ this workflow and EXECUTE it
"web-search-v1" rom-get call

; An agent can MODIFY this workflow
"web-search-v1" rom-get       ; get current
[cache-check] swap list-concat ; prepend cache check
"web-search-v2" swap rom-set   ; save as new version
```

### Mutation Operations

```
; Workflow mutations (genetic algorithm style)

; 1. Add a step
workflow [new-step] list-concat

; 2. Remove a step
workflow 2 list-del  ; remove step at index 2

; 3. Swap steps
workflow 1 3 list-swap

; 4. Combine two workflows
workflow-a workflow-b list-concat

; 5. Extract sub-workflow
workflow 2 5 list-slice  ; steps 2-5
```

### Selection Pressure

```
; Keep workflows that:
;   - Succeed more often
;   - Complete faster
;   - Use fewer resources

; ROM stores performance:
"workflows" rom-get
; → {
;     "v1": {code: [...], success: 0.6, avg_time: 10s},
;     "v2": {code: [...], success: 0.8, avg_time: 8s},
;     "v3": {code: [...], success: 0.75, avg_time: 5s},
;   }

; Meta-agent selects best:
; v2 has best success, v3 has best time
; Maybe combine them?
```

---

## Part 6: Coordination Primitives Needed

### What We Have

| Primitive | Purpose |
|-----------|---------|
| `mem-set/get` | Shared volatile state |
| `rom-set/get` | Shared persistent state |
| `spawn` | Start parallel agents |
| `join` | Wait for agent completion |

### What We Need

| Primitive | Purpose | Why |
|-----------|---------|-----|
| `atomic` | Read-modify-write safely | Prevent lost updates |
| `watch` | React to state changes | Event-driven coordination |
| `lock/unlock` | Exclusive access | Prevent conflicts |
| `metric` | Record structured perf data | Enable learning |
| `select-best` | Choose by criteria | Strategy selection |

### Atomic Operations

```
; Problem: Two agents updating same counter
Agent 1: mem-get "count" → 5
Agent 2: mem-get "count" → 5
Agent 1: 5 1 add "count" mem-set → 6
Agent 2: 5 1 add "count" mem-set → 6  ; WRONG! Should be 7

; Solution: atomic
"count" [1 add] atomic
; Read, apply function, write - all in one uninterruptible operation
```

### Watch (Reactive)

```
; Instead of polling:
[
  "price" mem-get
  old-price neq [handle-price-change] when
  100 sleep
] loop  ; wasteful

; Use watch:
"price" [handle-price-change] watch
; Callback runs whenever "price" changes
```

### Metrics

```
; Structured performance recording
{
  task: "booking"
  approach: "direct-api"
  start: 1234567890
  end: 1234567900
  success: true
  error: null
} metric-record

; Query metrics
{task: "booking"} metric-query
; → [{...}, {...}, ...]
```

---

## Part 7: Failure Modes & Safety

### Runaway Exploration

```
Problem: Explorers keep trying random things, never converge

Solution: Exploration budget
  - Each explorer gets N tries per hour
  - Must have > X% success to continue
  - Meta-agent adjusts allocation
```

### Strategy Rot

```
Problem: Old strategies in ROM no longer work (world changed)

Solution: Decay + Re-verification
  - Success rates decay over time
  - Verifiers periodically re-test old strategies
  - Prune strategies below threshold
```

### Coordination Deadlock

```
Problem: Agent A waits for B, B waits for A

Solution: Timeouts + Locks with TTL
  - All waits have timeout
  - Locks auto-expire
  - Detect cycles in wait graph
```

### Memory Exhaustion

```
Problem: ROM fills up with old data

Solution: Garbage collection
  - LRU eviction for strategies
  - Keep N best versions of each workflow
  - Archive old metrics, keep summaries
```

---

## Part 8: What Emerges?

If we build this correctly, we get:

### 1. Collective Memory
- Knowledge persists across agent restarts
- New agents immediately benefit from past learning
- No "cold start" problem

### 2. Continuous Improvement
- Strategies get better over time
- Bad approaches automatically pruned
- Good approaches automatically selected

### 3. Adaptive Behavior
- System responds to environment changes
- Re-learns when old strategies stop working
- Explores when stuck, exploits when confident

### 4. Emergent Optimization
- No explicit optimizer needed
- Selection pressure + mutation = evolution
- Optimal workflows discovered, not designed

### 5. Fault Tolerance
- Individual agent crashes don't lose knowledge
- New agents pick up where others left off
- System degrades gracefully

---

## Part 9: Implementation Phases

### Phase 1: Foundation (DONE)
- ✅ RAM (mem-*)
- ✅ ROM (rom-*)
- ✅ Capabilities
- ✅ Resources

### Phase 2: Concurrency
- 🔲 spawn/join/select
- 🔲 cancel
- 🔲 task handles

### Phase 3: Coordination
- 🔲 atomic (read-modify-write)
- 🔲 lock/unlock with TTL
- 🔲 watch (reactive)

### Phase 4: Learning Infrastructure
- 🔲 metric-record
- 🔲 metric-query
- 🔲 Strategy selection primitives

### Phase 5: Self-Improvement
- 🔲 Workflow mutation operations
- 🔲 Meta-agent framework
- 🔲 Exploration/exploitation balance

### Phase 6: Safety & Stability
- 🔲 Decay mechanisms
- 🔲 Garbage collection
- 🔲 Circuit breakers
- 🔲 Deadlock detection

---

## Part 10: Open Questions

### Q1: Exploration vs Exploitation Balance
```
How much should agents explore vs exploit?
  - Fixed ratio? (e.g., 20% explore, 80% exploit)
  - Adaptive? (explore more when stuck)
  - Per-task? (new task types need more exploration)
```

### Q2: Strategy Representation
```
How to represent strategies in ROM?
  - Just the quote/code?
  - Code + metadata (when created, success rate)?
  - Code + provenance (what it was derived from)?
  - Code + explanation (why it works)?
```

### Q3: Credit Assignment
```
When a workflow succeeds, which parts get credit?
  - Whole workflow?
  - Individual steps?
  - What if step A enables step B?
```

### Q4: Composability
```
Can we build new workflows from parts of successful ones?
  - How to identify reusable parts?
  - How to test compositions?
  - How to handle dependencies?
```

### Q5: Human-in-the-Loop
```
Where do humans fit?
  - Override strategy selection?
  - Inject new strategies?
  - Approve before deploy?
  - Review meta-agent decisions?
```

### Q6: Multi-Tenancy
```
If multiple users share the system:
  - Shared strategies across users?
  - Per-user strategies?
  - Privacy implications?
```

---

## Part 11: A Concrete Example

### Scenario: Travel Booking Agent

```
Initial State:
  ROM is empty
  3 explorer agents, 1 exploiter, 1 meta

User: "Book me the cheapest flight NYC→LA next Friday"
```

### Day 1: Exploration

```
Explorer 1 tries:
  [search-google "flights nyc la" parse-links first click]
  Result: FAIL - can't actually click

Explorer 2 tries:
  [search-kayak "NYC" "LA" date parse-prices sort first]
  Result: SUCCESS - found $299

Explorer 3 tries:
  [call-openai "find flights" parse-response]
  Result: FAIL - hallucinated prices

ROM after Day 1:
  strategies:
    "flight-search": {
      "kayak-direct": {success: 1.0, attempts: 1},
      "google-parse": {success: 0.0, attempts: 1},
      "llm-direct": {success: 0.0, attempts: 1}
    }
  workflows:
    "kayak-direct-v1": [search-kayak ... sort first]
```

### Day 2: Exploitation + More Exploration

```
Exploiter handles real user request:
  Reads ROM → selects "kayak-direct" (100% success)
  Executes workflow
  Result: SUCCESS - booked $279 flight

Explorer 1 tries variation:
  [search-kayak ... sort first | search-skyscanner ... sort first] race
  Result: SUCCESS - skyscanner was faster

ROM after Day 2:
  strategies:
    "flight-search": {
      "kayak-direct": {success: 1.0, attempts: 2},
      "race-kayak-skyscanner": {success: 1.0, attempts: 1},
      ...
    }
  workflows:
    "race-v1": [... race ...]
```

### Day 30: Mature System

```
ROM now contains:
  - 15 flight search strategies (3 with >90% success)
  - 8 hotel booking strategies
  - 12 car rental strategies
  - Cross-category combos ("flight+hotel bundle")
  
Meta-agent has learned:
  - Explore 10% of the time
  - Use "race" strategies for speed-critical tasks
  - Use "thorough" strategies for price-critical tasks
  - Kayak rate-limits, so rotate with skyscanner
```

---

## Part 12: The Vision

```
┌─────────────────────────────────────────────────────────────────┐
│                                                                 │
│  TODAY: We write agents by hand                                │
│         Agent fails → we debug → we fix → we redeploy          │
│         Knowledge in developer's head                          │
│                                                                 │
│  TOMORROW: Agents improve themselves                           │
│            Agent fails → system learns → system adapts         │
│            Knowledge in shared memory                          │
│                                                                 │
│  We don't program agents.                                      │
│  We create conditions for agents to learn.                     │
│                                                                 │
│  The "optimal workflow" isn't designed.                        │
│  It emerges from selection pressure on shared memory.          │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## Next Steps

1. **Think deeper** about each open question
2. **Prototype** the simplest version:
   - 2 agents sharing ROM
   - Simple strategy: try random, record what works
   - See what emerges
3. **Build primitives** in order:
   - spawn/join (parallel agents)
   - atomic (safe updates)
   - metric (structured recording)
4. **Experiment** with real tasks

---

*This document is for thinking. Not everything here will be built. But understanding the possibility space helps us build the right primitives.*
