# Literature Survey: JVM-like OS for Agents

> Research on portable runtime abstractions for Kore OS

## Executive Summary

This document surveys prior art in portable runtime systems that abstract hardware resources, with focus on:
- How they handle resource abstraction (memory, compute, network, storage)
- Capability-based security models
- The principle of "everything does one thing"
- Layered abstraction patterns

**Goal**: Design Kore as a device-agnostic runtime where agents think in abstract resource units, and all resources (RAM, ROM, network, compute) are accessed through uniform, minimal primitives.

---

## 1. Prior Art Survey

### 1.1 Java Virtual Machine (JVM)

**Key Abstractions:**
- **Memory**: Garbage-collected heap, abstract from physical RAM
- **Types**: Platform-independent data types (32-bit int, 64-bit long regardless of host)
- **Bytecode**: Stack-based instruction set, portable across architectures
- **Class Loading**: Dynamic module loading with verification

**What Works:**
- "Write once, run anywhere" - true hardware abstraction
- Abstract types hide endianness, word size, alignment
- Security via bytecode verification before execution

**What Doesn't Work for Agents:**
- No explicit resource quotas (memory just grows until OOM)
- No capability-based I/O (any class can do anything)
- Complex, heavyweight (not minimal)

**Lesson for Kore:**
> Abstract resource *types* work, but agents need explicit *quotas* and *capabilities*.

---

### 1.2 Erlang BEAM VM

**Key Abstractions:**
- **Processes**: Lightweight, isolated processes (millions possible)
- **Memory**: Per-process heaps, isolated garbage collection
- **Messaging**: Async message passing between processes
- **Supervision**: Hierarchical process supervision trees

**What Works:**
- True process isolation (one crash doesn't kill system)
- Preemptive scheduling with reduction counting (fair compute)
- Hot code reloading (programs upgrade without restart)

**Unique Insight:**
> Processes are the abstraction unit, not objects or functions.

**Lesson for Kore:**
> Agents should be isolated processes with their own resource quotas. Supervision is essential for long-running agents.

---

### 1.3 Plan 9 from Bell Labs

**Key Abstractions:**
- **Everything is a file**: All resources exposed as files in a namespace
- **Per-process namespaces**: Each process has its own view of the filesystem
- **9P Protocol**: Single protocol for all resource access (local + remote)
- **/net virtual filesystem**: Network as files (`/net/tcp/clone`, `/net/tcp/0/data`)

**What Works:**
- Radical simplicity: only one interface (read/write files)
- Network is just another filesystem mount
- Sandboxing via namespace restriction

**Networking Example:**
```
# Open a TCP connection
echo "connect 10.0.0.1!80" > /net/tcp/clone
read < /net/tcp/0/data
echo "GET / HTTP/1.0\r\n" > /net/tcp/0/data
```

**Lesson for Kore:**
> The "everything is a file" model is powerful but maybe too low-level. The key insight is **uniform interface for all resources**.

---

### 1.4 Inferno OS + Dis VM

**Key Abstractions:**
- **Styx Protocol**: Like 9P, all resources are files
- **Dis VM**: Register-based VM (easier JIT than stack-based)
- **Limbo Language**: Type-safe, garbage-collected, channels
- **Namespaces**: Per-process namespace composition

**Portability:**
- Runs native OR hosted on Linux/Windows/Plan9
- Same bytecode runs everywhere
- 1MB minimum memory requirement

**Key Insight:**
> Designed for resource-constrained devices from day one. Agents on phones, embedded, servers all see the same interface.

**Lesson for Kore:**
> Kore should work on constrained devices. Abstract resource units enable graceful degradation.

---

### 1.5 WebAssembly + WASI

**Key Abstractions:**
- **Linear Memory**: Explicit, bounded memory regions
- **Capability-Based Security**: No ambient authority - all I/O requires explicit capabilities
- **WASI Interfaces**: Modular API packages (wasi:io, wasi:http, wasi:filesystem, etc.)
- **Component Model**: Composable modules with typed interfaces

**WASI Architecture:**
```
┌─────────────────────────────────────────────────────┐
│                   Your Program                      │
├─────────────────────────────────────────────────────┤
│  wasi:cli    wasi:http    wasi:filesystem   ...    │
├─────────────────────────────────────────────────────┤
│              WASI Interface Layer                   │
├─────────────────────────────────────────────────────┤
│              Host Runtime (Wasmtime)                │
└─────────────────────────────────────────────────────┘
```

**Capability Model:**
```rust
// Guest cannot open arbitrary files
// Host explicitly grants capabilities:
WasiCtxBuilder::new()
    .preopened_dir("/data", "data")    // Can access /data as "data"
    .inherit_stdout()                   // Can write stdout
    // NO inherit_network() = no network access
```

**WASI Packages (modular by design):**
- `wasi:io` - streams, polling
- `wasi:filesystem` - files and directories
- `wasi:sockets` - TCP/UDP networking
- `wasi:http` - HTTP client/server
- `wasi:cli` - command line, environment
- `wasi:random` - cryptographic random
- `wasi:clocks` - wall clock, monotonic

**Lesson for Kore:**
> WASI's modular capability model is exactly what agents need. Each capability is a separate interface. Kore should adopt this pattern.

---

### 1.6 Docker/Containers

**Key Abstractions:**
- **Cgroups**: Resource limits (CPU, memory, I/O bandwidth)
- **Namespaces**: Isolated view of system (PID, network, mount, user)
- **Layers**: Composable filesystem layers

**Resource Limits:**
```bash
docker run --memory=512m --cpus=0.5 myapp
```

**Lesson for Kore:**
> Explicit resource limits are essential for multi-tenant agent systems. Agents should not be able to exhaust host resources.

---

## 2. Key Design Patterns Identified

### 2.1 The Capability Pattern

**Core Idea:** No ambient authority. Access to resources requires explicit capability tokens.

| System | Capability Model |
|--------|-----------------|
| WASI | Preopened directories, explicitly granted interfaces |
| Plan 9 | Namespace construction (what's visible = what's accessible) |
| Capsicum | Capability file descriptors |
| CloudABI | Purely capability-based POSIX |

**For Kore:**
```
; Agent gets explicit capabilities at spawn time
; cap-fs-read "/workspace" → can read /workspace
; cap-net "10.0.0.0/8:80" → can connect to 10.x.x.x:80
; cap-mem 1000 → can use 1000 memory units
```

### 2.2 The Resource Quota Pattern

**Core Idea:** Abstract resources have explicit quotas that can be queried and limited.

| Resource | Abstract Unit | Host Mapping |
|----------|--------------|--------------|
| RAM | Memory units | 1 unit = 1KB or 1MB (configurable) |
| ROM | Storage units | 1 unit = 1KB (persisted) |
| Compute | Time units | 1 unit = 1ms or 1000 instructions |
| Network | Bandwidth units | 1 unit = 1KB transfer |

### 2.3 The Uniform Interface Pattern

**Core Idea:** All resources accessed through the same interface type.

| System | Uniform Interface |
|--------|------------------|
| Plan 9/Inferno | Everything is a file (read/write) |
| Unix | File descriptors |
| HTTP | Request/Response |
| WASI | Streams (input-stream, output-stream) |

**For Kore (Proposal):**
All external resources as **Channels**:
- `chan-open "resource-type" "address" caps` → channel-id
- `chan-read channel-id` → value
- `chan-write channel-id value`
- `chan-close channel-id`

---

## 3. Proposed Kore Architecture

### 3.1 Abstraction Layers

```
┌─────────────────────────────────────────────────────────────────┐
│                        AGENT LAYER                              │
│  Agent sees: X mem, Y rom, Z compute, capabilities list         │
├─────────────────────────────────────────────────────────────────┤
│                      PRIMITIVE LAYER                            │
│  mem-*, rom-*, compute-*, net-*, fs-*, cap-*                   │
├─────────────────────────────────────────────────────────────────┤
│                      RESOURCE LAYER                             │
│  ResourcePool { memory, storage, compute, network }             │
│  Each resource: (total, used, reserved)                         │
├─────────────────────────────────────────────────────────────────┤
│                     CAPABILITY LAYER                            │
│  What can this process access? (fs paths, net addrs, etc.)      │
├─────────────────────────────────────────────────────────────────┤
│                        HOST LAYER                               │
│  Linux/Windows/macOS/ARM/x86 - maps abstract to real           │
└─────────────────────────────────────────────────────────────────┘
```

### 3.2 Resource Model (Abstract Units)

```rust
pub struct Resources {
    // Memory: volatile, fast, session-scoped
    pub mem_total: u64,      // Units available
    pub mem_used: u64,       // Units consumed
    
    // Storage: persistent, slower, survives restart
    pub rom_total: u64,
    pub rom_used: u64,
    
    // Compute: execution budget
    pub compute_total: u64,   // Time/instruction units
    pub compute_used: u64,
    
    // Network: transfer budget
    pub net_total: u64,       // Transfer units
    pub net_used: u64,
}
```

### 3.3 Capability Model

```rust
pub struct Capabilities {
    // Filesystem capabilities
    pub fs_read: Vec<PathBuf>,    // Paths agent can read
    pub fs_write: Vec<PathBuf>,   // Paths agent can write
    
    // Network capabilities
    pub net_connect: Vec<NetCap>, // Addresses/ports allowed
    pub net_listen: Vec<u16>,     // Ports can listen on
    
    // Process capabilities
    pub can_spawn: bool,          // Can spawn sub-agents
    pub can_exec: bool,           // Can run shell commands
    
    // Resource capabilities
    pub max_mem: u64,             // Memory limit
    pub max_compute: u64,         // Compute limit
}

pub struct NetCap {
    pub host: IpNet,              // IP/CIDR pattern
    pub ports: Vec<u16>,          // Allowed ports
    pub protocol: Protocol,        // TCP/UDP/HTTP
}
```

---

## 4. Primitive Design (Layered, Single Purpose)

### Layer 0: Resource Query (Read-Only)
| Primitive | Purpose | Returns |
|-----------|---------|---------|
| `res-mem` | Query memory status | `{total: N, used: M, free: K}` |
| `res-rom` | Query storage status | `{total: N, used: M, free: K}` |
| `res-compute` | Query compute status | `{total: N, used: M, free: K}` |
| `res-net` | Query network status | `{total: N, used: M, free: K}` |
| `res-all` | Query all resources | Combined map |

### Layer 1: Capability Query (What Can I Do?)
| Primitive | Purpose | Returns |
|-----------|---------|---------|
| `cap-has` | Check single capability | Boolean |
| `cap-list` | List all capabilities | List of cap names |
| `cap-fs` | File paths accessible | Map of paths + permissions |
| `cap-net` | Network access allowed | List of net caps |

### Layer 2: Memory (Volatile RAM)
| Primitive | Purpose | Side Effect |
|-----------|---------|-------------|
| `mem-set` | Store value by key | Updates mem_used |
| `mem-get` | Retrieve value by key | None |
| `mem-del` | Remove value | Updates mem_used |
| `mem-has` | Check if key exists | None |
| `mem-keys` | List all keys | None |

### Layer 3: Storage (Persistent ROM)
| Primitive | Purpose | Side Effect |
|-----------|---------|-------------|
| `rom-set` | Store persistent value | Updates rom_used |
| `rom-get` | Retrieve persistent value | None |
| `rom-del` | Remove persistent value | Updates rom_used |
| `rom-has` | Check if key exists | None |
| `rom-keys` | List all keys | None |

### Layer 4: Network (Unified Model)
| Primitive | Purpose | Protocol |
|-----------|---------|----------|
| `net-resolve` | DNS resolution | DNS |
| `net-connect` | Open TCP connection | TCP |
| `net-send` | Send data on connection | TCP |
| `net-recv` | Receive data | TCP |
| `net-close` | Close connection | TCP |
| `net-listen` | Listen on port | TCP |
| `net-accept` | Accept connection | TCP |

### Layer 5: HTTP (Built on net, Higher Level)
Already have: `http-get`, `http-post`, `http-request`

### Layer 6: Channels (Unified Resource Access - Future)
| Primitive | Purpose |
|-----------|---------|
| `chan-open` | Open channel to resource (file/net/device) |
| `chan-read` | Read from channel |
| `chan-write` | Write to channel |
| `chan-close` | Close channel |

---

## 5. Implementation Plan

### Phase 1: Resource Accounting (Foundation)
**Goal:** Track all resource usage, enforce limits

```rust
// Add to Context
pub struct Context {
    pub dict: Arc<RwLock<Dictionary>>,
    pub resources: Arc<RwLock<Resources>>,
    pub capabilities: Capabilities,
}
```

Primitives: `res-mem`, `res-rom`, `res-compute`, `res-net`, `res-all`

### Phase 2: Session Memory (RAM)
**Goal:** Volatile key-value store with quota

Primitives: `mem-set`, `mem-get`, `mem-del`, `mem-has`, `mem-keys`

### Phase 3: Persistent Storage (ROM)
**Goal:** Survives restart, uses sled or similar

Primitives: `rom-set`, `rom-get`, `rom-del`, `rom-has`, `rom-keys`

### Phase 4: Capability System
**Goal:** Check permissions before all resource access

Primitives: `cap-has`, `cap-list`, `cap-fs`, `cap-net`

### Phase 5: Network Primitives
**Goal:** Low-level TCP/UDP with capability checks

Primitives: `net-resolve`, `net-connect`, `net-send`, `net-recv`, `net-close`

### Phase 6: Channels (Unified Model)
**Goal:** Abstract all I/O through channels

Primitives: `chan-open`, `chan-read`, `chan-write`, `chan-close`

---

## 6. Portability Targets

| Platform | Constraints | Resource Mapping |
|----------|-------------|------------------|
| Linux Server | 16+ cores, 64GB+ RAM | 1 mem unit = 10MB |
| Mac Laptop | 8 cores, 16GB RAM | 1 mem unit = 1MB |
| Raspberry Pi | 4 cores, 4GB RAM | 1 mem unit = 100KB |
| Phone (Android/iOS) | 4-8 cores, 4-8GB RAM | 1 mem unit = 100KB |
| WebAssembly | Browser limits | 1 mem unit = 10KB |

**Key:** Agent program is identical. Only resource unit size changes.

---

## 7. Design Principles Summary

### From Unix/Plan 9:
- Everything through uniform interface
- Composition of simple tools
- Text as universal interface

### From JVM/BEAM:
- Hardware abstraction
- Portable bytecode
- Garbage collection

### From WASI:
- Capability-based security
- Modular interfaces
- Explicit resource boundaries

### From Containers:
- Resource limits (cgroups)
- Namespace isolation
- Reproducibility

### Kore's Synthesis:
1. **Abstract Resources**: Memory, storage, compute, network as quotas
2. **Capabilities**: Explicit grants, no ambient authority
3. **Single Purpose**: Each primitive does one thing
4. **Uniform Interface**: Consistent patterns across resource types
5. **Portable Programs**: Same code runs on phone or server
6. **Graceful Limits**: Query resources, adapt behavior

---

## 8. References

1. Lindholm & Yellin, *The Java Virtual Machine Specification*
2. Armstrong, *A History of Erlang* (HOPL 2007)
3. Pike et al., *Plan 9 from Bell Labs* (Bell Labs 1995)
4. Dorward et al., *The Inferno Operating System* (Bell Labs 1997)
5. WebAssembly Community Group, *WASI Specification* (2024)
6. Dennis & Van Horn, *Programming Semantics for Multiprogrammed Computations* (1966) - Capability concept origin
7. Watson et al., *Capsicum: Practical Capabilities for UNIX* (USENIX Security 2010)

---

## Next Steps

1. ✅ Complete literature survey (this document)
2. 🔲 Implement `Resources` struct in Context
3. 🔲 Add `res-*` query primitives (5 primitives)
4. 🔲 Add `mem-*` RAM primitives (5 primitives)
5. 🔲 Add `cap-*` capability primitives (4 primitives)
6. 🔲 Add `rom-*` persistent storage (5 primitives)
7. 🔲 Add `net-*` network primitives (7 primitives)
8. 🔲 Add `chan-*` unified channels (4 primitives)

**Total new primitives: ~30**
**Current: 111 → Target: ~141**
