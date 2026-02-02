# Kore v0.2 Architecture Plan

> Device-agnostic OS for Agents with Abstract Resources

## Core Insight

From the literature survey, the key pattern is:

```
┌─────────────────────────────────────────────────────────────┐
│  AGENT SEES:                                                │
│  - I have 100 memory units (not "16GB RAM")                │
│  - I have 50 storage units (not "1TB SSD")                 │
│  - I have 1000 compute units (not "3.2GHz CPU")            │
│  - I can access these 3 network endpoints                  │
│  - I can read/write these 2 filesystem paths               │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  KORE RUNTIME:                                              │
│  - Maps abstract units to real resources                   │
│  - Enforces capability boundaries                          │
│  - Tracks usage against quotas                             │
│  - Provides uniform primitives                             │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  HARDWARE:                                                  │
│  Phone | Laptop | Server | Embedded | Browser (WASM)       │
└─────────────────────────────────────────────────────────────┘
```

---

## Design Principles (Applied)

### 1. Divide Into Smallest Pieces

**Bad:** `connect-and-send "http://api.com" data`
**Good:**
```
"http://api.com" net-resolve     ; → IP address
80 net-connect                    ; → connection-id  
request-bytes net-send           ; → bytes sent
net-recv                         ; → response bytes
net-close                        ; → closed
```

### 2. Each Piece Does One Thing

| Primitive | ONLY Does |
|-----------|-----------|
| `res-mem` | Returns memory quota/usage numbers |
| `mem-set` | Stores one key-value pair |
| `cap-has` | Returns true/false for one capability |
| `net-connect` | Opens one connection, returns ID |

### 3. Explicit Over Implicit

**Bad:** `mem-set` silently fails when quota exceeded
**Good:** `mem-set` returns Error with "quota exceeded" message

**Bad:** `net-connect` works on any host
**Good:** `net-connect` checks capabilities, returns Error if not allowed

### 4. Reuse Existing Pieces

New primitives should compose with existing ones:
```
; Check before using
"can-write" cap-has [ data mem-set ] [ "no write cap" println ] if

; Retry with backoff (uses existing primitives)
[ net-connect ] try is-error [ 1000 sleep net-connect ] when
```

### 5. Verify Everything

Every primitive that could fail:
- Returns `Error` type on failure
- Error includes human-readable reason
- Agent can `try` and handle gracefully

---

## Layer Architecture

### Layer 0: Resource Accounting (No I/O)
Pure queries about what resources exist.

```
┌────────────────────────────────────────────────────┐
│  res-mem   res-rom   res-compute   res-net        │
│                      res-all                       │
│  Output: { total: N, used: M, free: K }           │
└────────────────────────────────────────────────────┘
```

Implementation:
```rust
struct Resources {
    mem_total: u64,
    mem_used: u64,
    rom_total: u64,
    rom_used: u64,
    compute_total: u64,  // Optional: instruction counting
    compute_used: u64,
    net_total: u64,      // Optional: bandwidth metering
    net_used: u64,
}
```

### Layer 1: Capability System (Security Boundary)
What is this agent allowed to do?

```
┌────────────────────────────────────────────────────┐
│  cap-has       Check single capability (→ bool)   │
│  cap-list      All capability names (→ list)      │
│  cap-fs        File access allowed (→ map)        │
│  cap-net       Network access allowed (→ list)    │
└────────────────────────────────────────────────────┘
```

Implementation:
```rust
struct Capabilities {
    // File system
    read_paths: HashSet<PathBuf>,
    write_paths: HashSet<PathBuf>,
    
    // Network
    connect_allowed: Vec<NetPattern>,  // host:port patterns
    listen_allowed: Vec<u16>,          // ports
    
    // Process
    can_exec: bool,
    can_spawn: bool,
    
    // Features
    features: HashSet<String>,  // arbitrary capability names
}
```

### Layer 2: Memory (Volatile Session State)
Key-value store that lives in RAM, cleared on restart.

```
┌────────────────────────────────────────────────────┐
│  mem-set   key value → () or Error                │
│  mem-get   key → value or Error                   │
│  mem-del   key → () or Error                      │
│  mem-has   key → bool                             │
│  mem-keys  () → [keys...]                         │
└────────────────────────────────────────────────────┘
```

Quota enforcement:
- Each value's size counted against `mem_used`
- `mem-set` returns Error if `mem_used + size > mem_total`

### Layer 3: Storage (Persistent ROM)
Key-value store that survives restarts.

```
┌────────────────────────────────────────────────────┐
│  rom-set   key value → () or Error                │
│  rom-get   key → value or Error                   │
│  rom-del   key → () or Error                      │
│  rom-has   key → bool                             │
│  rom-keys  () → [keys...]                         │
└────────────────────────────────────────────────────┘
```

Implementation options:
- sled (embedded key-value store)
- SQLite
- Simple JSON file (for small deployments)

### Layer 4: Low-Level Network
Connection-oriented networking.

```
┌────────────────────────────────────────────────────┐
│  net-resolve   hostname → ip or Error             │
│  net-connect   ip port → conn-id or Error         │
│  net-send      conn-id bytes → count or Error     │
│  net-recv      conn-id → bytes or Error           │
│  net-close     conn-id → ()                       │
│  net-listen    port → listener-id or Error        │
│  net-accept    listener-id → conn-id or Error     │
└────────────────────────────────────────────────────┘
```

All network ops check capabilities first:
```rust
fn net_connect(ctx, addr, port) -> Result<Value> {
    // Check capability
    if !ctx.capabilities.can_connect(addr, port) {
        return Err(Error::new(format!(
            "no capability to connect to {}:{}",
            addr, port
        )));
    }
    // Actually connect...
}
```

### Layer 5: HTTP (Built on Layer 4)
Already exists: `http-get`, `http-post`, `http-request`
These should be updated to check `cap-net` and update `net_used`.

### Layer 6: Channels (Unified Abstraction - Future)
All I/O through channels (like Plan 9 / Inferno).

```
┌────────────────────────────────────────────────────┐
│  chan-open   type addr [caps] → chan-id or Error  │
│  chan-read   chan-id → value or Error             │
│  chan-write  chan-id value → () or Error          │
│  chan-close  chan-id → ()                         │
│  chan-poll   [chan-ids...] timeout → ready-ids    │
└────────────────────────────────────────────────────┘
```

Channel types (discriminated by first argument):
- `"file" "/path/to/file"` → file channel
- `"tcp" "host:port"` → TCP connection
- `"udp" "host:port"` → UDP socket
- `"http" "https://api.com"` → HTTP client
- `"mem" "key"` → memory access as channel
- `"rom" "key"` → storage access as channel

This is the ultimate abstraction but can be added later.

---

## Context Structure

```rust
pub struct Context {
    // Existing
    pub dict: Arc<RwLock<Dictionary>>,
    
    // Resource quotas
    pub resources: Arc<RwLock<Resources>>,
    
    // Session memory (RAM)
    pub memory: Arc<RwLock<HashMap<String, Value>>>,
    
    // Persistent storage handle
    pub storage: Option<Arc<RwLock<Storage>>>,
    
    // Capabilities (what this agent can do)
    pub capabilities: Capabilities,
    
    // Active connections (for net-* primitives)
    pub connections: Arc<RwLock<ConnectionPool>>,
}

pub struct Resources {
    pub mem_total: u64,
    pub mem_used: u64,
    pub rom_total: u64,
    pub rom_used: u64,
    pub compute_total: u64,
    pub compute_used: u64,
    pub net_total: u64,
    pub net_used: u64,
}

pub struct Capabilities {
    pub fs_read: HashSet<PathBuf>,
    pub fs_write: HashSet<PathBuf>,
    pub net_connect: Vec<NetCap>,
    pub net_listen: Vec<u16>,
    pub can_exec: bool,
    pub can_spawn: bool,
    pub features: HashSet<String>,
}
```

---

## Implementation Phases

### Phase 1: Resource Foundation
Add Resources struct, context integration, query primitives.

**Files to modify:**
- `src/context.rs` (new) - Context with resources
- `src/builtins.rs` - Add res-* primitives

**Primitives (5):**
- `res-mem` → `{total: N, used: M, free: K}`
- `res-rom` → `{total: N, used: M, free: K}`
- `res-compute` → `{total: N, used: M, free: K}`
- `res-net` → `{total: N, used: M, free: K}`
- `res-all` → combined map

### Phase 2: Memory Layer
Session-scoped key-value store with quota enforcement.

**Files to modify:**
- `src/builtins.rs` - Add mem-* primitives

**Primitives (5):**
- `mem-set` - Store with quota check
- `mem-get` - Retrieve
- `mem-del` - Remove with quota update
- `mem-has` - Existence check
- `mem-keys` - List keys

### Phase 3: Capability Foundation
Capability checking infrastructure.

**Files to modify:**
- `src/context.rs` - Add Capabilities struct
- `src/builtins.rs` - Add cap-* primitives
- Update existing fs-* and http-* to check capabilities

**Primitives (4):**
- `cap-has` - Check single capability
- `cap-list` - List all capabilities
- `cap-fs` - File capabilities
- `cap-net` - Network capabilities

### Phase 4: Storage Layer
Persistent key-value store (sled).

**New dependencies:**
- sled = "0.34"

**Files to modify:**
- `src/storage.rs` (new) - Storage abstraction
- `src/builtins.rs` - Add rom-* primitives

**Primitives (5):**
- `rom-set` - Store persistent
- `rom-get` - Retrieve persistent
- `rom-del` - Remove persistent
- `rom-has` - Existence check
- `rom-keys` - List keys

### Phase 5: Network Layer
Low-level TCP with capability enforcement.

**Files to modify:**
- `src/connections.rs` (new) - Connection pool
- `src/builtins.rs` - Add net-* primitives

**Primitives (7):**
- `net-resolve` - DNS lookup
- `net-connect` - Open TCP connection
- `net-send` - Send bytes
- `net-recv` - Receive bytes
- `net-close` - Close connection
- `net-listen` - Listen on port
- `net-accept` - Accept connection

### Phase 6: Retrofit Existing Primitives
Update existing primitives to use new infrastructure.

**Primitives to update:**
- `fs-read`, `fs-write`, etc. → Check `cap-fs`
- `http-get`, etc. → Check `cap-net`, update `net_used`
- `exec` → Check `can_exec`

---

## Configuration

Resources and capabilities set at startup:

```toml
# kore.toml
[resources]
mem_total = 1000      # 1000 memory units
rom_total = 500       # 500 storage units
compute_total = 10000 # 10000 compute units
net_total = 1000      # 1000 network units

[capabilities.fs]
read = ["/workspace", "/data"]
write = ["/workspace/output"]

[capabilities.net]
connect = ["*:80", "*:443", "10.0.0.0/8:*"]
listen = [8080]

[capabilities.process]
exec = true
spawn = false
```

Or via environment/CLI:
```bash
KORE_MEM_TOTAL=1000 \
KORE_CAP_FS_READ="/workspace:/data" \
KORE_CAP_NET_CONNECT="*:80:*:443" \
kore run program.kore
```

---

## Example Agent Code (After Implementation)

```forth
; Check what resources I have
res-all println
; → {mem: {total: 1000, used: 50, free: 950}, ...}

; Check my capabilities
cap-list println
; → ["fs-read", "fs-write", "net-connect", "exec"]

; Store something in session memory
"session-data" {counter: 0 items: []} mem-set

; Store something persistent
"user-prefs" {theme: "dark"} rom-set

; Network with capability check
"api-allowed" cap-has [
    "api.example.com" net-resolve
    80 net-connect
    ; ... use connection
] [
    "No API access" println
] if

; Graceful resource handling
res-mem "free" map-get 100 lt [
    ; Low memory - cleanup old data
    mem-keys each [ drop mem-del ]
] when
```

---

## Primitive Count Projection

| Category | Current | After v0.2 |
|----------|---------|------------|
| Core | 111 | 111 |
| Resource | 0 | 5 |
| Memory | 0 | 5 |
| Capability | 0 | 4 |
| Storage | 0 | 5 |
| Network | 0 | 7 |
| **Total** | **111** | **137** |

All following the philosophy: each does one thing, built on layers, explicit behavior.

---

## Summary

Kore becomes a **device-agnostic runtime** where:

1. **Resources are abstract** - Agents think in units, not bytes/MHz
2. **Capabilities are explicit** - No ambient authority
3. **Layers are clean** - Query → Check → Use
4. **Primitives are atomic** - Each does exactly one thing
5. **Errors are informative** - Know why it failed
6. **Programs are portable** - Same code, any device
