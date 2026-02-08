# Kore: Master Reference Document

**Version:** 1.0 — June 2025  
**Authors:** Sourav Sharan + Copilot  
**Status:** Living document — update as capabilities evolve  
**Principle:** No hype. No false claims. Every claim is verifiable.

---

## Table of Contents

1. [What Kore Is](#1-what-kore-is)
2. [The Four Postulates](#2-the-four-postulates)
3. [VM Architecture & Inventory](#3-vm-architecture--inventory)
4. [Tool Composition: Saving & Reusing](#4-tool-composition-saving--reusing)
5. [The Konf Ecosystem — What Already Exists](#5-the-konf-ecosystem--what-already-exists)
6. [Integration Plan: Konf + Sutra as Kore Tools](#6-integration-plan-konf--sutra-as-kore-tools)
7. [Pure-Kore Training: Can We Skip PyTorch?](#7-pure-kore-training-can-we-skip-pytorch)
8. [KoreZero: The Agent Architecture](#8-korezero-the-agent-architecture)
9. [Hardware Reality](#9-hardware-reality)
10. [Honest Comparison with Existing Agents](#10-honest-comparison-with-existing-agents)
11. [Risk Analysis & Failure Modes](#11-risk-analysis--failure-modes)
12. [Implementation Roadmap](#12-implementation-roadmap)
13. [References](#13-references)

---

## 1. What Kore Is

Kore is a **stack-based, proof-checked, compiled programming language** designed as a universal substrate for AI agent tool-use.

### Measured Facts

| Metric | Value | How to verify |
|--------|-------|---------------|
| Lines of code | 23,549 | `find src stdlib -name '*.rs' -o -name '*.kore' \| xargs wc -l` |
| Tests passing | 527 | `cargo test 2>&1 \| grep 'test result'` |
| Opcodes | 2 (`Push`, `Call`) | `src/op.rs` |
| Native tools | 143+ | `cargo test -- --list 2>&1 \| grep test \| wc -l` cross-ref `src/` |
| Tensor tools (with autodiff) | 30+ | `src/ext/tensor.rs` |
| Data types | 10 base + 3 ext | `src/value.rs` — Int, Float, Bool, Text, List, Map, Quote, Error, Handle, Nil + Tensor, Fiber, Linear |
| Capability bits | 4 | `CAP_IO=0x01, CAP_FS=0x02, CAP_NET=0x04, CAP_EXEC=0x08` |
| Prelude definitions | ~40 composed words | `stdlib/prelude.kore` |

### Core Design Choices

1. **Two opcodes only**: `Push(Value)` and `Call(String)`. Everything else is a tool looked up by name at runtime. This is the simplest possible instruction set — every program is just "push data" and "call tool."

2. **Stack-based**: No variable names in the core language. Data flows through the stack. This makes composition trivial: `f g` means "run f, then run g, output of f feeds into g."

3. **Proof-checked**: The proof checker statically verifies stack effects (`(a b -- c)` signatures) and linear/affine type consumption *before* execution. Programs that would underflow the stack or leak linear resources are rejected at compile time.

4. **Capability-attenuated**: Every execution context carries a capability bitmask. Syscalls check capabilities before executing. A child process can never have more capabilities than its parent (monotone attenuation on a lattice).

---

## 2. The Four Postulates

These are not aspirational. They are implemented and enforced.

### P1: Everything is a Tool

**Statement:** Every computation is a function `Stack → Stack`.

**Implementation:** `src/tool.rs` — the `Tool` struct. Two variants:
- `ToolBody::Native(Arc<dyn Fn>)` — Rust closure
- `ToolBody::Ops(Vec<Op>)` — Composed from other tools

**Verification:** Try calling anything in Kore. Arithmetic (`add`), I/O (`print`), tensor ops (`tensor-matmul`), control flow (`if`), definition (`def`) — they're all entries in a `HashMap<String, Tool>`.

**Mathematical formulation:**  
Let $\mathcal{S}$ be the set of all stack states. Every tool $t$ is a partial function $t: \mathcal{S} \rightharpoonup \mathcal{S}$. Partiality arises from type errors or stack underflow.

### P2: One Operation — Apply

**Statement:** The only operation is applying a tool to the stack.

**Implementation:** `src/executor.rs` — the execute loop:
```
for op in program:
    match op:
        Push(v) → stack.push(v)
        Call(name) → tool = dict.get(name); tool.apply(stack)
```

`Push` is technically also "apply the push-tool," but it's optimized into a direct stack operation. There is no other dispatch mechanism.

**Mathematical formulation:**  
Program execution is function composition in the monoid $(\mathcal{S} \to \mathcal{S}, \circ, \text{id})$.

### P3: Composition is Concatenation

**Statement:** Composing two programs means concatenating their instruction sequences.

**Implementation:** If program A is `[Push(1), Call("add")]` and program B is `[Call("dup"), Call("mul")]`, then A∘B is `[Push(1), Call("add"), Call("dup"), Call("mul")]`. No wrappers, no closures, no indirection.

**Mathematical formulation:**  
Programs form a **free monoid** $([\text{Op}]^*, +\!\!+, [\ ])$ where $+\!\!+$ is list concatenation and $[\ ]$ is the empty program (identity element). This is the simplest possible composition rule.

**Why this matters:** Concatenation is $O(n+m)$ — no overhead. Composition doesn't create nested structures. A composed tool of 100 tools is still a flat list of ops. This means the agent can compose tools without hitting combinatorial complexity.

### P4: Constraints Attenuate

**Statement:** Capabilities can only decrease, never increase, when passing from caller to callee.

**Implementation:** `src/interpreter.rs` — the `spawn` syscall:
```rust
child_caps = parent_caps & requested_caps  // bitwise AND
```

The capability lattice is $(\{0..15\}, \text{AND}, \text{OR})$ where AND is meet (attenuate) and OR is join. Every spawn computes the meet, so the child's capability set is always $\leq$ the parent's.

**Mathematical formulation:**  
Let $(L, \sqcap, \sqsubseteq)$ be the capability lattice. For any spawn: $\text{caps}_\text{child} = \text{caps}_\text{parent} \sqcap \text{caps}_\text{requested} \sqsubseteq \text{caps}_\text{parent}$.

**Consequence:** A tool that starts with `CAP_IO | CAP_FS` (0x03) can spawn a child with `CAP_IO` (0x01) or `0x00`, but never with `CAP_NET` (0x04) which it doesn't have. This is enforced at the VM level, not by convention.

---

## 3. VM Architecture & Inventory

### Execution Model

```
Source (.kore) → Parser → Vec<Op> → Proof Checker → Executor
                                                        ↓
                                              Stack + Dictionary + Capabilities
```

- **Parser** (`src/parser.rs`): Text → `Vec<Op>`. Handles `: name ... ;` definitions, string/number/bool literals, and word lookup.
- **Proof Checker** (`src/proof_checker.rs`): Statically verifies stack effects. Rejects programs with stack underflow or linear type violations.
- **Executor** (`src/executor.rs`): Runs `Vec<Op>` against a `Stack` and `Dictionary`. The execute loop is ~50 lines.
- **Dictionary** (`src/dictionary.rs`): `HashMap<String, Tool>` wrapped in `Arc<RwLock<>>`. Shared across fibers.

### Complete Tool Categories

| Category | Count | Examples | Source |
|----------|-------|---------|--------|
| Arithmetic | 6 | `add`, `sub`, `mul`, `div`, `mod`, `neg` | `core/arithmetic.rs` |
| Comparison | 6 | `eq`, `neq`, `lt`, `gt`, `le`, `ge` | `core/comparison.rs` |
| Stack manipulation | 8 | `dup`, `drop`, `swap`, `over`, `rot`, `nip`, `tuck`, `depth` | `core/stack.rs` |
| Logic | 3 | `and`, `or`, `not` | `core/logic.rs` |
| Control flow | 5 | `if`, `when`, `unless`, `while`, `times` | `core/control.rs` |
| Type ops | 7 | `type`, `to-int`, `to-float`, `to-str`, `to-bool`, `is-nil`, `is-err` | `core/types.rs` |
| String ops | 13 | `str-len`, `str-get`, `str-concat`, `str-split`, `str-find`, etc. | `core/strings.rs` |
| List ops | 14 | `list`, `list-len`, `list-get`, `list-push`, `map`, `filter`, `fold`, `each`, etc. | `core/lists.rs` |
| Map ops | 7 | `map-new`, `map-get`, `map-set`, `map-del`, `map-has`, `map-keys`, `map-vals` | `core/maps.rs` |
| I/O | 4 | `print`, `println`, `read-line`, `debug` | `core/io.rs` |
| Definition | 3 | `def`, `def-verified`, `words` | `core/meta.rs` |
| Higher-order | 4 | `call`, `apply`, `compose`, `curry` | `core/higher.rs` |
| Tensor (autodiff) | 30+ | `tensor-matmul`, `tensor-softmax`, `tensor-relu`, `backward`, `grad-get`, `sgd-step`, etc. | `ext/tensor.rs` |
| Fiber | 9 | `fiber-new`, `fiber-step`, `fiber-run`, `fiber-inject`, etc. | `ext/fiber.rs` |
| Syscalls | 10+ | `file-read`, `file-write`, `http-get`, `http-post`, `exec`, `spawn`, `send`, `recv` | `interpreter.rs` |
| ROM storage | 2 | `rom-set`, `rom-get` | `ext/rom.rs` |
| Prelude (composed) | ~40 | `inc`, `dec`, `abs`, `max`, `min`, `sum`, `range`, `head`, `tail`, `zip`, etc. | `stdlib/prelude.kore` |

### Autodiff System

Kore has a **built-in reverse-mode automatic differentiation** engine:

```kore
# Forward pass
3 tensor-from-scalar requires-grad    # x = 3, track gradients
dup dup tensor-mul tensor-mul          # x³
1.0 tensor-from-scalar tensor-mse     # loss = MSE(x³, 1.0)
backward                              # compute ∂loss/∂x
"x" grad-get                          # retrieve gradient
```

The autodiff graph is built during forward execution and `backward` performs reverse-mode differentiation. This is the same algorithm used in PyTorch — the difference is Kore's implementation is ~2,000 lines of Rust in a single file vs PyTorch's ~500K lines of C++/CUDA.

**Operations with gradient support:** `tensor-add`, `tensor-sub`, `tensor-mul`, `tensor-matmul`, `tensor-matmul-t`, `tensor-softmax`, `tensor-sigmoid`, `tensor-relu`, `tensor-mse`, `tensor-exp`, `tensor-log`, `tensor-scale`, `tensor-neg`, `tensor-sum`, `tensor-mean`.

**SGD optimizer:** `sgd-step` implements $\theta \leftarrow \theta - \eta \nabla_\theta L$ directly.

---

## 4. Tool Composition: Saving & Reusing

### The Question

> "Will Kore be able to save and reuse tool compositions as it learns and inferences?"

### Current State: What Works Today

**Yes, at the source level:**

```kore
# Define a composed tool
: double  dup add ;
: quadruple  double double ;

# Use it
5 quadruple   # → 20
```

The `: name ... ;` syntax creates a `Tool::composed(name, body=Vec<Op>)` and registers it in the dictionary. Within a session (REPL or single script execution), all definitions persist and are reusable.

**Source file persistence:**

```kore
# Save to file: my-tools.kore
: double  dup add ;
: quadruple  double double ;
: power-of-two  1 swap times [double] call ;
```

Load with `--prelude`:
```bash
kore --prelude my-tools.kore program.kore
```

The prelude is parsed and executed first, populating the dictionary before the main program runs.

**ROM persistence:**

```kore
42 "my-constant" rom-set    # Persist a value to disk (JSON)
"my-constant" rom-get       # Retrieve it later
```

ROM uses file-system storage. Values are serialized as JSON.

### What's Missing: Binary Composition Cache

The key gap: **no binary serialization of compiled tool dictionaries.**

Today, reusing compositions requires re-parsing source text on every load. This works but has two costs:

1. **Parse overhead**: O(n) in source text length per load. For the current prelude (~40 definitions), this is <1ms. For hundreds of learned compositions, it could reach 10-50ms.

2. **No intermediate representation**: If the agent composes a tool during inference (e.g., discovers that `[dup mul dup mul]` is useful and names it `pow4`), there's no built-in way to persist that *compiled* form without converting back to source text.

### What's Already in Place for Binary Persistence

The architecture is **90% ready**:

| Component | Serde Status | Notes |
|-----------|-------------|-------|
| `Op::Push(Value)` | ✅ `#[derive(Serialize, Deserialize)]` | Core instruction |
| `Op::Call(String)` | ✅ `#[derive(Serialize, Deserialize)]` | Core instruction |
| `Value::Int/Float/Bool/Text/List/Map/Quote/Nil` | ✅ Serializable | All base types |
| `Value::Error(String)` | ✅ Serializable | Error messages |
| `Value::Handle(u64)` | ✅ Serializable | Opaque ID |
| `Value::Ext(Box<dyn Any>)` | ❌ Not serializable | Tensors, Fibers, Linear values |
| `Tool::Composed { body: Vec<Op> }` | ✅ Body is serializable | Can be saved |
| `Tool::Native { body: Arc<dyn Fn> }` | ❌ Closures not serializable | Cannot be saved (by design) |

### Implementation Plan for Full Persistence

**Effort: ~2-3 days of Rust work.**

1. **`dict-save` tool** `(path -- )`: Filter dictionary to composed tools only → serialize as `Vec<(String, Vec<Op>)>` → write as bincode/JSON to file.

2. **`dict-load` tool** `(path -- )`: Read file → deserialize → register each tool in dictionary.

3. **Binary prelude cache**: Hash source file → if cached binary exists and hash matches → load binary (fast). Otherwise recompile and cache.

4. **Agent learning loop**: After inference discovers a useful composition, the agent calls `def` to name it, then `dict-save` to persist it. Next session, `dict-load` restores all learned compositions.

### Postulate Compliance

- **P1** ✅ Saved compositions are still tools (`Stack → Stack`)
- **P2** ✅ Loading is still `Call("dict-load")`
- **P3** ✅ Compositions are still flat `Vec<Op>` — serialization doesn't change structure
- **P4** ✅ Capabilities are properties of the execution context, not the tool. A loaded tool inherits the loader's capabilities

---

## 5. The Konf Ecosystem — What Already Exists

Three sibling codebases sit alongside Kore in this workspace. They represent **years of engineering** that can be leveraged.

### 5.1 Konflux / Konf (Rust)

**What it is:** A decentralized, multi-tenant, peer-to-peer AI agent runtime.

**Architecture:**
```
┌─────────────────────────────────────────────────────────┐
│  CLI (clap)  •  HTTP API (axum)  •  P2P (iroh/QUIC)    │  Interfaces
├─────────────────────────────────────────────────────────┤
│  Router / Resolver: local → HTTP → P2P chain           │  Dispatch
├─────────────────────────────────────────────────────────┤
│  ToolRegistry: Arc<dyn ToolHandle>                      │  Storage
├─────────────────────────────────────────────────────────┤
│  ParallelExecutor: DAG-based workflow engine            │  Execution
├─────────────────────────────────────────────────────────┤
│  Security: Authorizer + Encryptor + RateLimiter         │  Security
├─────────────────────────────────────────────────────────┤
│  Crypto: Ed25519 (identity) + AES-256-GCM (state)      │  Identity
├─────────────────────────────────────────────────────────┤
│  Network: iroh P2P (QUIC), mDNS discovery, gossip      │  Transport
└─────────────────────────────────────────────────────────┘
```

**Philosophy:** "In Unix, everything is a file. In Konf, everything is a Callable."

**Core kernel (~200 LoC):** Route → Auth → Invoke. Everything else is userspace.

**Tool inventory (30+ stdlib tools):**

| Namespace | Tools |
|-----------|-------|
| `core` | `identity`, `delay`, `extract`, `set`, `concat`, `format`, `json-encode`, `json-decode`, `log`, `fail` |
| `ai` | `complete` (LLM), `embeddings`, `stream-complete` (SSE) |
| `http` | `get`, `post`, `request` |
| `research` | `arxiv-search`, `semantic-scholar-search`, `paper-details` |
| `text` | `extract-json`, `word-count`, `split`, `join`, `replace`, `template` |
| `state` | `save` (AES-256-GCM + SQLite), `load`, `delete`, `list-keys` |
| `meta` | `list-peers`, `select-peer`, `add-peer`, `remove-peer` |
| `workflow` | `execute-workflow` (DAG with fan-out/fan-in) |

**Workflow engine features:**
- Directed Acyclic Graph (DAG) execution with dependency resolution
- Parallel fan-out / fan-in with convergence policies (strict, lenient, quorum)
- Conditional edge evaluation
- Retry with exponential/linear/fixed backoff
- Per-step timeouts
- Full execution tracing

**Security:**
- Ed25519 keypairs for identity (nodes and tenants)
- AES-256-GCM for state encryption at rest
- Pluggable authorization (allow-all, deny-all, action-based)
- Rate limiting per peer
- Capability-gated operations

### 5.2 Konf-Stack (Rust)

**What it is:** A stack-based (Forth-like) runtime built **directly on top of Kore**.

```toml
# konf-stack/Cargo.toml
kore = { path = "../kore" }
```

This is the **bridge** between Kore's VM and Konf's tool ecosystem. It re-exports Kore's `Value`, `Stack`, `Tool`, `Context`, and `Dictionary` types, then wraps practical tools as Kore-native stack operations.

**Tool categories (17 namespaces, 100+ tools):**

| Category | Examples |
|----------|---------|
| stack_ops | `dup`, `drop`, `swap`, `over`, `rot`, `nip`, `tuck`, `depth`, `clear` |
| math | `add`, `sub`, `mul`, `div`, `mod`, `neg`, `abs`, `min`, `max`, `inc`, `dec`, `floor`, `ceil`, `round` |
| logic | `eq`, `neq`, `lt`, `gt`, `le`, `ge`, `and`, `or`, `not` |
| text | `concat`, `split`, `join`, `upper`, `lower`, `trim`, `len`, `contains`, `replace` |
| list | `append`, `map`, `filter`, `reduce`, `sort`, `reverse`, `flatten` |
| map | `get`, `set`, `keys`, `values`, `merge`, `contains` |
| flow | `if`, `when`, `unless`, `call`, `apply`, `times`, `each`, `while`, `catch`, `throw`, `cond` |
| meta | `type-of`, `define`, `defined?` |
| json | JSON encode/decode |
| encode | Base64, hex, URL encoding |
| hash | SHA-256, SHA-512, MD5 |
| time | Now, format, parse |
| random | Random int/float/choice |
| env | Environment variable access |
| io | File read/write (**capability-gated**) |
| http | HTTP client (**capability-gated**) |
| shell | Shell execution (**capability-gated**) |

### 5.3 Kore-Std (Rust)

**What it is:** Shared standard library providing foundational modules.

| Module | Purpose |
|--------|---------|
| `handle` | Resource lifecycle management (files, connections) |
| `capability` | Gate access to sensitive operations |
| `error` | Structured errors with context chaining |
| `async_tools` | Spawn, await, timeout, cancel |
| `serde_tools` | JSON/MessagePack serialization |
| `schema` | JSON Schema validation |

### 5.4 Sutra (Python)

**What it is:** A provider-agnostic, YAML-driven AI agent orchestration framework built on LangGraph.

**Architecture:** YAML spec → Parser → LangGraph `StateGraph` → Execution

**Core components:**

| Component | Purpose |
|-----------|---------|
| Declarative loader | YAML parsing with format auto-detection (Simple/Advanced) |
| Schema validation | Pydantic models for workflow specs |
| SimpleBuilder | Linear/branching workflow compilation |
| AdvancedBuilder | Parallel DAG with fan-out/fan-in, convergence, conditional routing |
| LLM adapters | Provider-agnostic (OpenAI reference impl, LiteLLM compatible) |
| Tool registry | Namespaced, wraps LangChain `BaseTool` |
| Template engine | Sandboxed Jinja2 with safe filters and AST-validated conditions |
| Smrti protocol | Memory protocol (facts, episodes, sessions) — interface only |
| Tracing | Langfuse integration with NullTracer fallback |

**Status:** Production-ready (alpha). 4,843 lines of tests. Critical state-propagation bug fixed Oct 2025.

**Dependencies:** Pydantic, Jinja2, LangChain, LangGraph, typing-extensions.

---

## 6. Integration Plan: Konf + Sutra as Kore Tools

### 6.1 What This Gives Kore

Without integration, Kore can:
- Manipulate data (stack, lists, maps, strings)
- Do math and tensor operations
- Define and compose tools
- Read/write files, make HTTP requests, execute processes

With integration, Kore gains:
- **LLM access** (any provider, streaming, structured output)
- **Academic search** (arXiv, Semantic Scholar)
- **Encrypted persistent state** (AES-256-GCM + SQLite)
- **Workflow orchestration** (parallel DAGs with retry)
- **P2P distributed execution** (iroh/QUIC mesh)
- **Cryptographic identity** (Ed25519 signatures)
- **YAML-driven multi-step agent workflows** (via Sutra)
- **Hashing** (SHA-256, SHA-512, MD5)
- **Encoding** (Base64, hex, URL)
- **Template rendering** (Jinja2-style)

### 6.2 Integration Architecture

Three tiers, ordered by implementation effort:

#### Tier 1: Direct Rust Bridge via konf-stack (effort: days)

Konf-stack **already depends on Kore** and uses its types. The bridge pattern exists:

```rust
Tool::native("ai/complete", "(prompt:Text -- result:Text)", |mut stack, ctx| {
    Box::pin(async move {
        let prompt = stack.pop_text()?;
        let result = konf_ai_complete(json!({"prompt": prompt})).await?;
        stack.push(Value::Text(result))?;
        Ok((stack, ctx))
    })
});
```

**Tools to port first (highest value):**
1. `ai/complete` — LLM completion
2. `state/save`, `state/load` — Encrypted persistent state
3. `research/arxiv-search` — Paper search
4. `hash/sha256` — Cryptographic hashing
5. `encode/base64` — Encoding

**Kore program using integrated tools:**
```kore
"What is the softmax function?" ai/complete println
```

#### Tier 2: HTTP Bridge (effort: hours)

Run Konf as a separate process with its HTTP API. Add a generic `konf-call` tool to Kore:

```kore
"complete" { "prompt" "explain softmax" } konf-call
# Calls POST http://localhost:PORT/tools/complete
```

**Pros:** No compilation dependency. Tools are hot-swappable.  
**Cons:** Latency (~1-5ms per call). Requires running two processes.

#### Tier 3: Sutra Bridge via Python subprocess (effort: days)

Sutra is Python. Kore's `exec` syscall can invoke Python:

```kore
"python3 -c 'from sutra import run; print(run(\"research-assistant\", {\"query\": \"MCTS\"}))'""" exec
```

Better: Write a thin Python CLI wrapper and call it:
```kore
"sutra-run research-assistant query=MCTS" exec
```

**Pros:** Access to entire LangChain/LangGraph ecosystem.  
**Cons:** Cold start (~2s for Python). Heavy dependency chain.

### 6.3 Capability Mapping

Konf's capability system uses hierarchical strings (`"gpu:nvidia:a100"`).  
Kore uses a 4-bit lattice (`CAP_IO=0x01, CAP_FS=0x02, CAP_NET=0x04, CAP_EXEC=0x08`).

**Mapping:**

| Konf Capability | Kore Capability | Notes |
|----------------|-----------------|-------|
| `"filesystem"` | `CAP_FS` (0x02) | File I/O tools |
| `"network"` | `CAP_NET` (0x04) | HTTP, P2P tools |
| `"shell"` | `CAP_EXEC` (0x08) | Process execution |
| `"gpu"`, `"gpu:nvidia"` | No direct mapping | Kore would need `CAP_GPU` or handle via `CAP_EXEC` |

**P4 compliance:** Konf tools accessed from Kore inherit Kore's capability context. A Kore program with `caps=0x04` (NET only) can call `ai/complete` (uses network) but not `state/save` (uses filesystem). The bridge enforces this by checking Kore's capability bitmask before dispatching to Konf.

### 6.4 Postulate Compliance Check

| Postulate | Status | Reasoning |
|-----------|--------|-----------|
| P1 (Everything is a Tool) | ✅ | Each Konf/Sutra capability is wrapped as a Kore `Tool` with signature `Stack → Stack` |
| P2 (One Operation: Apply) | ✅ | Integration uses `Call("ai/complete")` — same dispatch as any other tool |
| P3 (Composition is Concatenation) | ✅ | `"hello" ai/complete str-len` — Konf tools compose with Kore tools via concatenation |
| P4 (Constraints Attenuate) | ✅ | Bridge checks Kore's capability bitmask. Konf tools requiring `CAP_NET` fail if caller lacks it |

---

## 7. Pure-Kore Training: Can We Skip PyTorch?

### The Question

> "What are the pros and cons of training and doing everything in Kore itself, no PyTorch?"

### What Kore Has for Training

| Capability | Kore Tool | PyTorch Equivalent |
|------------|-----------|-------------------|
| Tensor creation | `tensor-zeros`, `tensor-ones`, `tensor-randn`, `tensor-from-list` | `torch.zeros`, `torch.ones`, `torch.randn` |
| Matrix multiply | `tensor-matmul`, `tensor-matmul-t` | `torch.matmul`, `@` |
| Element-wise ops | `tensor-add`, `tensor-sub`, `tensor-mul` | `+`, `-`, `*` |
| Activations | `tensor-relu`, `tensor-sigmoid`, `tensor-softmax` | `F.relu`, `F.sigmoid`, `F.softmax` |
| Loss | `tensor-mse` | `F.mse_loss` |
| Gradient tracking | `requires-grad` | `tensor.requires_grad_(True)` |
| Backpropagation | `backward` | `loss.backward()` |
| Gradient access | `grad-get` | `tensor.grad` |
| SGD step | `sgd-step` | `optimizer.step()` |

### What Kore is Missing for Training

| Missing | Why It Matters | Effort to Add |
|---------|----------------|---------------|
| **GPU execution** | Kore tensors are CPU-only (`Vec<f64>`). A 14B parameter model needs ~7 TFLOP/s. CPU gives ~0.1 TFLOP/s. GPU gives 35 TFLOP/s (3090 Ti). | Very high — requires CUDA/OpenCL backend |
| **Batched operations** | No batch dimension broadcasting. Must manually loop. | Medium — add broadcasting rules |
| **Conv2d / Attention layers** | No convolution or multi-head attention primitives. Must compose from matmul + reshape. | Medium — add as native tools |
| **Adam / AdamW optimizer** | Only `sgd-step` exists. Modern training uses Adam with momentum and weight decay. | Low — ~100 lines of Rust |
| **Mixed precision (fp16/bf16)** | Kore uses f64 only. Training at f64 is 4× slower and 4× more memory than fp16. | High — requires new data type |
| **Data loading** | No parallel data pipeline. Must load all data into memory. | Medium — add streaming iterator |
| **Checkpointing** | No model save/load. Training must complete in one run. | Low — serialize tensor dictionary |
| **Distributed training** | No multi-GPU or gradient synchronization. | Very high |
| **Learning rate schedulers** | No warmup, cosine decay, etc. | Low — implement as composed tools |
| **Dropout / BatchNorm** | No regularization layers. | Low — add as native tools |

### Quantitative Reality Check

**Can Kore train a neural network?**

Yes. A small one.

**Example: 2-layer MLP on MNIST (784→128→10)**

- Parameters: $784 \times 128 + 128 + 128 \times 10 + 10 = 101,770$
- Forward pass: 2 matmuls + 2 activations
- Per-sample FLOP: ~200K
- Per-epoch (60K samples): ~12 GFLOP
- CPU throughput (single-thread f64): ~2 GFLOP/s
- **Time per epoch: ~6 seconds** ← This is feasible

**Example: Transformer (14B parameters, GPT-scale)**

- Parameters: $14 \times 10^9$
- Per-token forward FLOP: $\approx 6 \times 14 \times 10^9 = 84$ GFLOP (Kaplan et al., 2020)
- Per-token backward FLOP: $\approx 2 \times \text{forward} = 168$ GFLOP
- Training on 1M tokens: $168 \times 10^9 \times 10^6 = 1.68 \times 10^{17}$ FLOP
- CPU throughput: ~2 GFLOP/s
- **Time: $\frac{1.68 \times 10^{17}}{2 \times 10^9} = 8.4 \times 10^7$ seconds ≈ 2.7 years** ← Not feasible
- GPU (3090 Ti fp16): ~35 TFLOP/s → $\frac{1.68 \times 10^{17}}{3.5 \times 10^{13}} \approx 4,800$ seconds ≈ 1.3 hours ← Feasible, but Kore can't use GPU

### The Honest Pros and Cons

#### Pros of Pure-Kore Training

1. **Postulate purity**: Everything is a tool. The training loop is a Kore program. Composition is concatenation. Capability attenuation applies. No escape hatches.

2. **Inspection and proof**: Every training step is a sequence of `Call` operations. The proof checker can verify stack effects. You can inspect intermediate values at any point by inserting `debug` between tools.

3. **Compositional training**: The model itself is a composed tool. If you define `: layer1 W1 tensor-matmul b1 tensor-add tensor-relu ;` then `layer1` is both a tool and a trainable module. No separate "model" abstraction needed.

4. **Safety**: Capability attenuation means training code can be sandboxed. A training loop with `caps=0` has no I/O, no network, no filesystem access — it can only compute.

5. **Self-improvement**: If the agent's tools are Kore programs, and the agent can compose new tools, then training the agent to compose tools means the agent learns to extend itself. The training loop and the inference loop use the same primitives.

6. **Simplicity**: No Python dependency. No pip install. No CUDA toolkit. No version conflicts. `cargo build` and go.

#### Cons of Pure-Kore Training

1. **Speed**: CPU f64 is 1,000-10,000× slower than GPU fp16 for matrix operations. Training anything beyond small MLPs is impractical without GPU support.

   > $\text{Speedup}_\text{GPU/CPU} = \frac{\text{35 TFLOP/s (3090 Ti, fp16)}}{\text{0.002 TFLOP/s (single-core f64)}} \approx 17,500\times$

2. **No ecosystem**: PyTorch has 10,000+ person-years of engineering. Hugging Face has 200K+ pretrained models. Kore has zero pretrained models and one optimizer.

3. **No pre-trained models**: You cannot load a Llama/GPT/DeepSeek checkpoint into Kore. There's no safetensors/GGUF loader. Building from scratch means training from random initialization, which requires orders of magnitude more compute.

4. **Missing primitives**: No attention layer, no layer normalization, no dropout, no embedding lookup, no positional encoding. Each must be built from matmul + element-wise ops.

5. **Memory management**: Kore's tensors are `Vec<f64>` on the heap with reference counting. No memory pooling, no in-place operations, no gradient accumulation buffer reuse. A 128M parameter model at f64 = 1 GB just for weights, plus activations and gradients ≈ 4-5 GB. This fills the 2070S completely.

6. **Debugging**: No TensorBoard, no Weights & Biases, no gradient histogram visualization. You'd need to build monitoring tools from scratch.

### Verdict: Hybrid Approach

**Pure Kore for small models (< 1M params):** ✅ Feasible and elegant.

A reward model or value network with < 1M parameters can be trained entirely in Kore. This preserves postulate purity and keeps the training loop inspectable.

**PyTorch for large models (> 10M params):** ✅ Pragmatic and necessary.

The 14B parameter model we're using for code generation requires GPU + fp16 + Adam + gradient checkpointing. Kore can't compete here.

**The hybrid:**

```
Kore VM (inference + MCTS + tool composition)
    ↑ produces training data
    ↓ loads model weights
PyTorch (SFT + GRPO training of large model)
```

Kore handles the parts it's good at (composition, safety, stack manipulation, MCTS search). PyTorch handles the part it's good at (GPU-accelerated gradient descent on large tensors).

**What Kore *can* train natively:**
- Small reward models for MCTS value estimation (< 1M params, MLP)
- Tabular Q-functions for tool selection
- Heuristic combination weights
- Feature extractors from stack states

**What needs PyTorch:**
- The 14B code generation model (SFT, GRPO)
- Transformer world models (if using UniZero-style MBRL)
- Any model requiring GPU acceleration

---

## 8. KoreZero: The Agent Architecture

### Design Philosophy

The goal is **not** to rebuild Claude/Devin/Cursor. Those systems use 100B+ parameter models with massive compute budgets. We cannot compete on parameter count.

The goal is to leverage Kore's unique properties:
- **Formal composition** (free monoid, provably correct tool chains)
- **Capability safety** (lattice attenuation, zero-trust by default)
- **Self-extension** (agent can `def` new tools, compose existing ones)
- **Verifiable reasoning** (proof checker validates stack effects)

### Architecture: Brain + Hands + Muscle

```
┌─────────────────────────────────────────┐
│              Brain (Policy)             │
│  DeepSeek-R1-Distill-Qwen-14B (4-bit)  │
│  Input: task description                │
│  Output: Kore program (tool sequence)   │
│  Hardware: RTX 3090 Ti (24 GB)          │
└────────────────┬────────────────────────┘
                 │ generates
                 ▼
┌─────────────────────────────────────────┐
│         Hands (Execution)               │
│  Kore VM (korec serve)                  │
│  Executes generated programs            │
│  50,000 programs/sec throughput         │
│  Proof-checks before execution          │
│  Hardware: CPU                          │
└────────────────┬────────────────────────┘
                 │ results feed back
                 ▼
┌─────────────────────────────────────────┐
│         Muscle (Integration)            │
│  Konf tools: LLM, search, state, P2P   │
│  Sutra: YAML workflow orchestration     │
│  Kore-std: handles, crypto, schema      │
│  Hardware: CPU + Network                │
└─────────────────────────────────────────┘
```

### MCTS for Tool Composition

Monte Carlo Tree Search can explore the space of tool compositions:

**State:** Current stack contents + dictionary + goal description  
**Action:** Choose next tool to apply (from dictionary)  
**Transition:** Apply tool to stack → new stack state  
**Reward:** How close the new stack state is to the goal

```
            root (initial stack)
           /    |    \
        add    dup    mul
        / \     |     / \
     dup  sub  add  add  dup
     ...  ...  ...  ...  ...
```

At each node, MCTS:
1. **Select**: UCB1 formula picks which child to explore: $a^* = \arg\max_a \left[ Q(s,a) + c \sqrt{\frac{\ln N(s)}{N(s,a)}} \right]$
2. **Expand**: Try a new tool not yet explored
3. **Simulate**: Random rollout of tool applications
4. **Backpropagate**: Update value estimates

**Why this works for Kore:**
- The action space is finite (dictionary has ~200 tools)
- Transitions are deterministic (same tool + same stack = same result)
- Execution is fast (50K evaluations/sec via `korec serve`)
- Compositions can be cached (seen stack states → known outcomes)

**Expected MCTS budget:**
- 1,000 simulations per decision × 50K exec/sec = **20ms per decision**
- For a 10-tool composition: 200ms total search time

### Training Pipeline

```
Phase 1: SFT (DONE — 99.6% accuracy on L0-19)
    ↓ fine-tuned model generates programs
Phase 2: MCTS-guided exploration
    ↓ discovers new tool compositions
Phase 3: GRPO on MCTS-discovered programs
    ↓ reinforces good compositions
Phase 4: Self-play (write Kore → verify → improve)
    ↓ continuous improvement loop
```

### What We Can Expect vs. What We Can't

**Realistic expectations:**

| Capability | Expected Level | Why |
|------------|---------------|-----|
| Kore program generation | 95-99% correct (simple programs) | SFT-v3 already at 99.6% |
| Tool composition discovery | Can find 3-5 tool chains | MCTS with 1K simulations, 200 tools |
| Safety guarantees | Provable | Capability lattice + proof checker |
| Self-extension | Can define new words from existing tools | `def` tool already works |

**What we cannot expect:**

| Capability | Why Not |
|------------|---------|
| General-purpose coding (Python, JS, etc.) | Model is fine-tuned on Kore only. Not trained on general code. |
| Natural language understanding | 14B model is capable but not at GPT-4/Claude level |
| Multi-file project generation | No file system understanding trained |
| Web browsing / visual understanding | No multimodal capability |
| Beating Claude/Devin on SWE-bench | They use 100B+ models with specialized training |

---

## 9. Hardware Reality

### What We Have

| GPU | VRAM | Compute (fp16) | Role |
|-----|------|-----------------|------|
| RTX 2070 Super | 8 GB | 9.1 TFLOP/s | Inference of small models |
| RTX 3090 Ti | 24 GB | 40 TFLOP/s | Training + inference of 14B model |

### Memory Budget (14B model, 4-bit quantized)

| Component | Memory | Notes |
|-----------|--------|-------|
| Model weights (4-bit) | ~7 GB | 14B × 4 bits / 8 = 7 GB |
| KV cache (2K context) | ~2 GB | Depends on batch size |
| LoRA adapters (r=32) | ~0.5 GB | Trainable parameters |
| Optimizer states (GRPO) | ~1 GB | Adam moments for LoRA params |
| Activations + overhead | ~3 GB | Gradient checkpointing |
| **Total** | **~13.5 GB** | Fits in 24 GB with headroom |

### What This Means

- **Training (SFT/GRPO):** Must use the 3090 Ti. 4-bit QLoRA keeps memory under 14 GB. Training speed: ~50 examples/sec for SFT, ~5-10 examples/sec for GRPO.

- **Inference (generation):** Can use the 3090 Ti. At 4-bit quantization: ~30-50 tokens/sec for a single sequence.

- **MCTS:** Runs on CPU. Each Kore program evaluation takes ~20μs. 1,000 MCTS simulations = 20ms. This is not a bottleneck.

- **Training pipeline:** Generate programs on 3090 Ti → Evaluate on CPU → Compute rewards on CPU → Train on 3090 Ti. The bottleneck is generation speed (~30-50 tok/sec), not evaluation.

---

## 10. Honest Comparison with Existing Agents

### What Kore/KoreZero Brings That Others Don't

| Property | KoreZero | Claude/Cursor/Devin |
|----------|----------|---------------------|
| **Formal safety** | Capability lattice with mathematical proof of attenuation | Trust-based (prompt engineering, no formal guarantees) |
| **Composition verification** | Proof checker validates stack effects before execution | No — agents can generate broken code |
| **Self-extension** | Agent `def`s new tools, persists them, reuses them | Agents write code but don't extend their own toolset |
| **Deterministic composition** | Same tools + same input = same output (no randomness in execution) | LLM generation is inherently stochastic |
| **Execution speed** | 50K programs/sec | ~1 program/sec (LLM generation bottleneck) |
| **Auditable** | Every program is a flat list of ops, fully inspectable | LLM reasoning is opaque |

### What Others Have That KoreZero Doesn't

| Property | Claude/Cursor/Devin | KoreZero |
|----------|---------------------|----------|
| **Model size** | 100B-1T parameters | 14B parameters (4-bit) |
| **Training data** | Trillions of tokens | ~2,300 Kore examples |
| **General knowledge** | Broad world knowledge | Kore-specific only |
| **Multi-language** | Python, JS, Rust, etc. | Kore only |
| **IDE integration** | Full editor, file system, git | None (CLI only) |
| **Multi-modal** | Vision, code, documents | Text only |
| **SWE-bench score** | 49% (Devin), 72% (Claude) | Not applicable |

### Where KoreZero Has an Actual Advantage

1. **Tool composition search**: MCTS can explore 50,000 compositions/sec. No existing agent does systematic search over tool compositions — they rely on the LLM to "think of" the right composition. KoreZero can prove which compositions work.

2. **Safety-critical execution**: If you need to guarantee that an agent cannot access the network while processing sensitive data, Kore's capability lattice provides this with mathematical certainty. No other agent system has this.

3. **Incremental learning**: When the agent discovers a useful composition, it `def`s it, persists it, and reuses it. Over time, the dictionary grows. This is genuine cumulative learning — the agent gets permanently better, not just within a conversation context.

4. **Formal verification**: The proof checker catches errors before execution. If the agent generates `1 add` (stack underflow — needs 2 args), the proof checker rejects it. Other agents would execute the broken code and crash.

---

## 11. Risk Analysis & Failure Modes

### Technical Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| MCTS doesn't find useful compositions | Medium | High | Start with known-good compositions as seeds. Limit action space to ~50 most common tools. |
| 14B model forgets Kore syntax after GRPO | Low | High | Keep SFT replay buffer. Validate with held-out test set after each GRPO round. |
| Autodiff in Kore has numerical bugs | Medium | Medium | Compare Kore gradients vs PyTorch on 100 test cases. Fix any > 1e-6 discrepancy. |
| Konf integration breaks P4 (capability leak) | Low | Critical | Bridge must check Kore's capability bitmask before every Konf tool call. Add integration tests. |
| Training data contamination (model memorizes instead of generalizes) | Medium | Medium | Use held-out tasks not seen during training. Test on L20+ difficulty levels. |
| 3090 Ti runs out of memory during GRPO | Low | Medium | Already tested — QLoRA + gradient checkpointing keeps usage under 14 GB. |

### Project Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Scope creep (building too many things) | High | High | Follow the 5-phase plan. Each phase has concrete deliverables and acceptance criteria. |
| No users (cool tech, no adoption) | Medium | High | Focus on unique capabilities (safety, composition) rather than competing with general agents. |
| Single developer bottleneck | High | Medium | Document everything. Keep architecture simple. Avoid premature optimization. |

---

## 12. Implementation Roadmap

### Phase 0: VM Bug Fixes (1 week)

| Task | Description | Effort |
|------|-------------|--------|
| Per-call-frame locals | Fix global locals bug in `interpreter.rs` | 4 hours |
| Case opcode fix | Fix parser offset table for `case` in serve mode | 2 hours |
| `--caps` flag for serve | Add capability flag to serve mode CLI | 30 min |
| Scalar math tools | Add `exp`, `log`, `sqrt`, `pow`, `abs`, `floor`, `ceil` as core tools | 2 hours |
| Composition persistence | Add `dict-save` and `dict-load` tools | 1 day |

### Phase 1: Konf Integration (1-2 weeks)

| Task | Description | Effort |
|------|-------------|--------|
| Tier 1 bridge | Port 5 highest-value Konf tools as native Kore tools | 3 days |
| Capability check | Bridge enforces Kore's capability bitmask | 1 day |
| Integration tests | Test all postulates with integrated tools | 1 day |
| Sutra CLI wrapper | Python CLI for invoking Sutra workflows from Kore | 1 day |

### Phase 2: MCTS Engine (2-3 weeks)

| Task | Description | Effort |
|------|-------------|--------|
| State representation | Define MCTS state = (stack snapshot, goal embedding) | 2 days |
| Action enumeration | List available tools filtered by stack types | 1 day |
| UCB1 selection | Implement UCB1 with configurable exploration constant | 1 day |
| Simulation engine | Random rollouts with `korec serve` as oracle | 2 days |
| Value network | Small MLP (stack features → value) trained in Kore | 3 days |
| Composition caching | Hash stack states → cache known-good compositions | 2 days |

### Phase 3: Training Loop (2-3 weeks)

| Task | Description | Effort |
|------|-------------|--------|
| L20+ training data | Generate 5,000+ examples for MCTS-discovered compositions | 3 days |
| SFT-v4 | Train on expanded dataset | 1 day |
| GRPO-v3 | Reinforcement learning with MCTS rewards | 3 days |
| Self-play loop | Agent generates → verifies → trains on successful programs | 1 week |

### Phase 4: Evaluation & Hardening (1-2 weeks)

| Task | Description | Effort |
|------|-------------|--------|
| Benchmark suite | 500+ tasks across 10 difficulty levels | 3 days |
| Safety audit | Verify all capability paths, test privilege escalation | 2 days |
| Performance profiling | Identify and optimize bottlenecks | 2 days |
| Documentation | User guide, API reference, examples | 2 days |

### Total Timeline: 7-11 weeks

---

## 13. References

### Kore Language & VM

1. **Kore source code** — `kore/src/` in this workspace. 23,549 LoC, 527 tests.
2. **Postulates** — Defined in `kore/PREAMBLE.md`. Implemented in `src/tool.rs`, `src/executor.rs`, `src/interpreter.rs`.
3. **Training history** — SFT-v1 through v3 + GRPO-v1 through v2. Checkpoints in `experiments/agent-zero/checkpoints/`.

### MBRL & UniZero

4. Ye, W., et al. (2024). **"UniZero: Generalized and Efficient Planning with Scalable Latent World Models."** arXiv:2406.10667. Key insight: Transformer world model replaces MuZero's RNN.
5. Kamienny, P.-A., et al. (2024). **"ScaleZero: Scaling MuZero Planning with Value-Informed Tree Traversal."** Key insight: Value-informed traversal reduces MCTS simulations needed.
6. Xu, Z., et al. (2024). **"ReZero: Boosting MCTS-based Algorithms by Backward-view and Scalable Reanalysis."** Key insight: Reanalyze past experience buffers.
7. Huang, Y., et al. (2024). **"RobustZero: MuZero with Domain Randomization."** Key insight: Randomize dynamics for robust planning.
8. Niu, Y., et al. (2024). **"LightZero: A Unified Benchmark for Monte Carlo Tree Search in General Sequential Decision Scenarios."** Apache 2.0 framework implementing AlphaZero, MuZero, EfficientZero, UniZero, and others.

### Training Methods

9. Shao, Z., et al. (2024). **"DeepSeekMath: Pushing the Limits of Mathematical Reasoning in Open Language Models."** Introduces GRPO (Group Relative Policy Optimization). arXiv:2402.03300.
10. Dettmers, T., et al. (2023). **"QLoRA: Efficient Finetuning of Quantized Language Models."** 4-bit NormalFloat quantization + LoRA adapters. arXiv:2305.14314.

### Scaling Laws

11. Kaplan, J., et al. (2020). **"Scaling Laws for Neural Language Models."** OpenAI. Establishes that forward pass FLOP ≈ 6N per token for model with N parameters. arXiv:2001.08361. Used in our compute estimates.

### Capability Security

12. Dennis, J.B. & Van Horn, E.C. (1966). **"Programming Semantics for Multiprogrammed Computations."** Communications of the ACM. Original capability-based security model.
13. Miller, M.S. (2006). **"Robust Composition: Towards a Unified Approach to Access Control and Concurrency Control."** PhD thesis. Formalizes capability attenuation in distributed systems.

### Concatenative Languages

14. Kerby, B. (2007). **"Theory of Concatenative Combinators."** Formalizes the algebraic structure of stack-based languages as free monoids.
15. von Thun, M. (2001). **"Joy: Forth's Functional Cousin."** Establishes that concatenative languages have denotational semantics as function composition.

### Agent Systems (for comparison)

16. Jimenez, C.E., et al. (2024). **"SWE-bench: Can Language Models Resolve Real-World GitHub Issues?"** arXiv:2310.06770. Benchmark used to evaluate Devin, Claude, etc.
17. Yang, J., et al. (2024). **"SWE-agent: Agent Computer Interfaces Enable Software Engineering Language Models."** Princeton. arXiv:2405.15793.
18. Cognition AI (2024). **"Devin: The First AI Software Engineer."** Blog post. Reports 13.86% → 49% SWE-bench resolution.

### Autodiff Theory

19. Baydin, A.G., et al. (2018). **"Automatic Differentiation in Machine Learning: A Survey."** JMLR 18(153):1-43. arXiv:1502.05767. Covers reverse-mode AD as implemented in Kore's `tensor.rs`.

### MCTS Theory

20. Kocsis, L. & Szepesvári, C. (2006). **"Bandit-based Monte-Carlo Planning."** ECML 2006. Original UCB1-tree (UCT) algorithm. Proves $\lim_{n \to \infty} P(\text{UCT selects suboptimal}) = 0$ — MCTS converges to optimal with infinite simulations.
21. Silver, D., et al. (2016). **"Mastering the Game of Go with Deep Neural Networks and Tree Search."** Nature 529, 484-489. AlphaGo — first successful combination of neural networks + MCTS.

---

## Appendix A: Kore Program Examples

### A.1 Simple Arithmetic
```kore
3 4 add 2 mul    # (3 + 4) × 2 = 14
```

### A.2 Function Definition and Reuse
```kore
: square  dup mul ;
: sum-of-squares  swap square swap square add ;
3 4 sum-of-squares   # 9 + 16 = 25
```

### A.3 List Processing
```kore
[1 2 3 4 5] [dup mul] map    # [1, 4, 9, 16, 25]
[1 2 3 4 5] 0 [add] fold     # 15
```

### A.4 Neural Network Forward Pass (Pure Kore)
```kore
# Assume W1, b1, W2, b2 are tensors on the stack
# Input x is a tensor on the stack
x W1 tensor-matmul b1 tensor-add tensor-relu    # hidden = relu(W1·x + b1)
W2 tensor-matmul b2 tensor-add tensor-softmax   # output = softmax(W2·h + b2)
```

### A.5 Training Step (Pure Kore)
```kore
# Forward pass (with gradient tracking)
x requires-grad
x W1 tensor-matmul b1 tensor-add tensor-relu
W2 tensor-matmul b2 tensor-add tensor-softmax
y tensor-mse                # loss = MSE(pred, target)
backward                    # compute gradients
W1 "W1" grad-get 0.01 sgd-step  # W1 -= 0.01 * grad
W2 "W2" grad-get 0.01 sgd-step  # W2 -= 0.01 * grad
```

### A.6 Tool Composition with Persistence (Future)
```kore
# Agent discovers a useful composition
: analyze-paper
    "arxiv-search" konf-call     # search arXiv
    first                        # take top result
    "paper-details" konf-call    # get full details
    "abstract" map-get           # extract abstract
    "summarize this:" swap str-concat
    "ai/complete" konf-call      # summarize with LLM
;

# Save for future sessions
"learned-tools.kore-bin" dict-save

# Next session: load and reuse
"learned-tools.kore-bin" dict-load
"attention mechanism" analyze-paper
```

---

## Appendix B: Glossary

| Term | Definition |
|------|-----------|
| **Attenuation** | Reducing capabilities when delegating to a child. Formally: $\text{child} = \text{parent} \sqcap \text{request}$ |
| **Composed tool** | A tool whose body is `Vec<Op>` — a list of push/call instructions. Can be serialized. |
| **Dictionary** | `HashMap<String, Tool>` — the registry of all available tools in an execution context |
| **Free monoid** | An algebraic structure $(S^*, \cdot, \epsilon)$ where concatenation is the operation and the empty sequence is the identity. Programs in Kore form a free monoid. |
| **GRPO** | Group Relative Policy Optimization — RL algorithm that computes advantage relative to group mean reward (DeepSeekMath, 2024) |
| **Lattice** | A partially ordered set where every pair of elements has a meet (greatest lower bound) and join (least upper bound) |
| **Linear type** | A type that must be used exactly once — cannot be duplicated (`dup`) or discarded (`drop`) |
| **Native tool** | A tool whose body is a Rust closure `Arc<dyn Fn>` — cannot be serialized |
| **Op** | One of two instructions: `Push(Value)` or `Call(String)` |
| **Quote** | A Kore value containing a program (`Vec<Op>`). First-class: can be pushed to the stack, passed as arguments, called later. Written as `[dup mul]`. |
| **QLoRA** | Quantized Low-Rank Adaptation — 4-bit model weights + trainable low-rank matrices (Dettmers et al., 2023) |
| **SFT** | Supervised Fine-Tuning — training on (input, output) pairs |
| **Stack effect** | Type signature of a tool: `(inputs -- outputs)`. E.g., `add` has effect `(a b -- sum)` |
| **UCB1** | Upper Confidence Bound: $Q(s,a) + c\sqrt{\frac{\ln N(s)}{N(s,a)}}$. Balances exploration vs exploitation in MCTS. |
| **Value** | One of 13 types: Int, Float, Bool, Text, List, Map, Quote, Error, Handle, Nil, Tensor, Fiber, Linear |
