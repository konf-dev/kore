//! Capability-Requiring Tools
//!
//! These tools require capabilities to execute because they access
//! external resources: filesystem, network, process, etc.
//!
//! ## Philosophy
//!
//! Core primitives are pure computations. Capability tools are effects.
//! The separation enforces:
//! 1. Clear distinction between computation and effect
//! 2. Explicit capability requirements
//! 3. Easier testing (mock capabilities)
//! 4. Security by default (no capabilities = no effects)
//!
//! ## Modules
//!
//! | Module  | Count | Capabilities Required |
//! |---------|-------|----------------------|
//! | algebra | 11    | (lattice/monoid ops) |
//! | control | 2     | (stack control)      |

//! | fs      | 7     | fs:read, fs:write    |
//! | http    | 3     | net:*                |
//! | io      | 4     | io:stdout, io:stdin  |
//! | process | 5     | process:*            |
//! | time    | 2     | time:read, time:sleep|
//! | env     | 2     | env:read, env:write  |
//! | json    | 2     | (none - pure)        |
//! | trace   | 4     | trace:*              |
//! | mem     | 5     | mem:*                |
//! | rom     | 5     | storage:*            |
//! | introspect | 3  | (meta)               |
//!
//! ## Usage
//!
//! ```ignore
//! use kore::cap::register_cap;
//! register_cap(&mut ctx).await;
//! ```

mod algebra;
mod control;
mod fs;
mod http;
mod io;
mod process;
mod time;
mod env;
mod json;
mod trace;
mod mem;
mod rom;
mod introspect;

use crate::context::Context;

/// Total count of capability tools
pub const CAP_COUNT: usize = 55;

/// Register all capability-requiring tools
pub async fn register_cap(ctx: &mut Context) {
    let mut dict = ctx.dict.write().await;
    
    // Algebra (11) - capability/resource lattice operations
    algebra::register(&mut dict);
    
    // Control flow (2) - times, while with special stack semantics
    control::register(&mut dict);
    
    // File system (7)
    fs::register(&mut dict);
    
    // HTTP (3)
    http::register(&mut dict);
    
    // Console I/O (4)
    io::register(&mut dict);
    
    // Process control (5)
    process::register(&mut dict);
    
    // Time (2)
    time::register(&mut dict);
    
    // Environment (2)
    env::register(&mut dict);
    
    // JSON (2) - pure but commonly needed
    json::register(&mut dict);
    
    // Tracing (4)
    trace::register(&mut dict);
    
    // Session memory (5)
    mem::register(&mut dict);
    
    // Persistent storage (5)
    rom::register(&mut dict);
    
    // Introspection (3)
    introspect::register(&mut dict);
}
