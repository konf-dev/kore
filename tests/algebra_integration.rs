//! Integration Tests for Algebraic Tools
//!
//! These tests verify that the algebra-based tools work correctly
//! when executed through the full Kore runtime.

use kore::{execute, register_builtins, Context, Op, Stack, Value};

async fn setup() -> Context {
    let mut ctx = Context::trusted();
    register_builtins(&mut ctx).await;
    ctx
}

// ============================================================================
// CAPABILITY LATTICE TOOLS
// ============================================================================

mod cap_tools {
    use super::*;

    #[tokio::test]
    async fn test_cap_leq_subset() {
        let ctx = setup().await;
        let stack = Stack::new();

        // ["fs:read"] ≤ ["fs:read", "net:connect"] should be true
        let ops = vec![
            Op::Push(Value::List(vec![Value::Text("fs:read".into())])),
            Op::Push(Value::List(vec![
                Value::Text("fs:read".into()),
                Value::Text("net:connect".into()),
            ])),
            Op::call("cap-leq"),
        ];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.values()[0].as_bool().unwrap(), true);
    }

    #[tokio::test]
    async fn test_cap_leq_not_subset() {
        let ctx = setup().await;
        let stack = Stack::new();

        // ["fs:read", "net:connect"] ≤ ["fs:read"] should be false
        let ops = vec![
            Op::Push(Value::List(vec![
                Value::Text("fs:read".into()),
                Value::Text("net:connect".into()),
            ])),
            Op::Push(Value::List(vec![Value::Text("fs:read".into())])),
            Op::call("cap-leq"),
        ];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.values()[0].as_bool().unwrap(), false);
    }

    #[tokio::test]
    async fn test_cap_meet_intersection() {
        let ctx = setup().await;
        let stack = Stack::new();

        // meet(["fs:read", "net:connect"], ["fs:read", "exec"]) = ["fs:read"]
        let ops = vec![
            Op::Push(Value::List(vec![
                Value::Text("fs:read".into()),
                Value::Text("net:connect".into()),
            ])),
            Op::Push(Value::List(vec![
                Value::Text("fs:read".into()),
                Value::Text("exec".into()),
            ])),
            Op::call("cap-meet"),
        ];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        let meet = result.values()[0].as_list().unwrap();
        
        // Should contain fs:read
        let caps: Vec<&str> = meet.iter().filter_map(|v| v.as_text().ok()).collect();
        assert!(caps.iter().any(|c| *c == "fs:read"), "meet should contain fs:read");
        assert_eq!(caps.len(), 1, "meet should have exactly 1 element");
    }

    #[tokio::test]
    async fn test_cap_join_union() {
        let ctx = setup().await;
        let stack = Stack::new();

        // join(["fs:read"], ["net:connect"]) = ["fs:read", "net:connect"]
        let ops = vec![
            Op::Push(Value::List(vec![Value::Text("fs:read".into())])),
            Op::Push(Value::List(vec![Value::Text("net:connect".into())])),
            Op::call("cap-join"),
        ];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        let join = result.values()[0].as_list().unwrap();
        
        let caps: Vec<&str> = join.iter().filter_map(|v| v.as_text().ok()).collect();
        assert!(caps.iter().any(|c| *c == "fs:read"), "join should contain fs:read");
        assert!(caps.iter().any(|c| *c == "net:connect"), "join should contain net:connect");
    }

    #[tokio::test]
    async fn test_cap_attenuate_reduces() {
        let ctx = setup().await;
        let stack = Stack::new();

        // attenuate(["fs:read", "net:connect", "exec"], ["fs:read"]) = ["fs:read"]
        let ops = vec![
            Op::Push(Value::List(vec![
                Value::Text("fs:read".into()),
                Value::Text("net:connect".into()),
                Value::Text("exec".into()),
            ])),
            Op::Push(Value::List(vec![Value::Text("fs:read".into())])),
            Op::call("cap-attenuate"),
        ];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        let attenuated = result.values()[0].as_list().unwrap();
        
        let caps: Vec<&str> = attenuated.iter().filter_map(|v| v.as_text().ok()).collect();
        assert!(caps.iter().any(|c| *c == "fs:read"), "should contain requested fs:read");
        assert!(!caps.iter().any(|c| *c == "net:connect"), "should NOT contain net:connect");
        assert!(!caps.iter().any(|c| *c == "exec"), "should NOT contain exec");
    }

    #[tokio::test]
    async fn test_cap_attenuate_cannot_escalate() {
        let ctx = setup().await;
        let stack = Stack::new();

        // attenuate(["fs:read"], ["fs:read", "exec"]) = ["fs:read"]
        // Even though we request exec, parent doesn't have it, so child doesn't get it
        let ops = vec![
            Op::Push(Value::List(vec![Value::Text("fs:read".into())])),
            Op::Push(Value::List(vec![
                Value::Text("fs:read".into()),
                Value::Text("exec".into()), // parent doesn't have this
            ])),
            Op::call("cap-attenuate"),
        ];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        let attenuated = result.values()[0].as_list().unwrap();
        
        let caps: Vec<&str> = attenuated.iter().filter_map(|v| v.as_text().ok()).collect();
        assert!(!caps.iter().any(|c| *c == "exec"), "SECURITY: cannot escalate capabilities");
    }
}

// ============================================================================
// RESOURCE MONOID TOOLS
// ============================================================================

mod res_tools {
    use super::*;

    #[tokio::test]
    async fn test_res_split_conservation() {
        let ctx = setup().await;
        let stack = Stack::new();

        // Split 1000 mem, 500 rom, 2000 compute, 100 net by 0.3
        // Verify child + parent = original
        let ops = vec![
            Op::push(1000), // mem
            Op::push(500),  // rom
            Op::push(2000), // compute
            Op::push(100),  // net
            Op::Push(Value::Float(0.3)), // ratio
            Op::call("res-split"),
            // Stack now has: child-mem child-rom child-compute child-net parent-mem parent-rom parent-compute parent-net
            // We need to verify child + parent = original for mem
        ];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        let values = result.values();
        
        // Stack order (bottom to top): child-mem child-rom child-compute child-net parent-mem parent-rom parent-compute parent-net
        let child_mem = values[0].as_int().unwrap();
        let child_rom = values[1].as_int().unwrap();
        let child_compute = values[2].as_int().unwrap();
        let child_net = values[3].as_int().unwrap();
        let parent_mem = values[4].as_int().unwrap();
        let parent_rom = values[5].as_int().unwrap();
        let parent_compute = values[6].as_int().unwrap();
        let parent_net = values[7].as_int().unwrap();

        // Conservation law: child + parent = original
        assert_eq!(child_mem + parent_mem, 1000, "CONSERVATION: mem");
        assert_eq!(child_rom + parent_rom, 500, "CONSERVATION: rom");
        assert_eq!(child_compute + parent_compute, 2000, "CONSERVATION: compute");
        assert_eq!(child_net + parent_net, 100, "CONSERVATION: net");
    }

    #[tokio::test]
    async fn test_res_add_monoid() {
        let ctx = setup().await;
        let stack = Stack::new();

        // Add (100, 50, 200, 10) + (30, 20, 50, 5) = (130, 70, 250, 15)
        let ops = vec![
            Op::push(100), Op::push(50), Op::push(200), Op::push(10), // first bundle
            Op::push(30), Op::push(20), Op::push(50), Op::push(5),    // second bundle
            Op::call("res-add"),
        ];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        let values = result.values();
        
        // Stack order (bottom to top): mem, rom, compute, net
        assert_eq!(values[0].as_int().unwrap(), 130, "mem: 100 + 30 = 130");
        assert_eq!(values[1].as_int().unwrap(), 70, "rom: 50 + 20 = 70");
        assert_eq!(values[2].as_int().unwrap(), 250, "compute: 200 + 50 = 250");
        assert_eq!(values[3].as_int().unwrap(), 15, "net: 10 + 5 = 15");
    }

    #[tokio::test]
    async fn test_res_has_sufficient() {
        let ctx = setup().await;
        let stack = Stack::new();

        // Check if (100, 50, 200, 10) has (30, 20, 50, 5) - should be true
        let ops = vec![
            Op::push(100), Op::push(50), Op::push(200), Op::push(10), // available
            Op::push(30), Op::push(20), Op::push(50), Op::push(5),    // required
            Op::call("res-has"),
        ];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.values()[0].as_bool().unwrap(), true);
    }

    #[tokio::test]
    async fn test_res_has_insufficient() {
        let ctx = setup().await;
        let stack = Stack::new();

        // Check if (100, 50, 200, 10) has (200, 20, 50, 5) - should be false (mem insufficient)
        let ops = vec![
            Op::push(100), Op::push(50), Op::push(200), Op::push(10), // available
            Op::push(200), Op::push(20), Op::push(50), Op::push(5),   // required (200 mem > 100)
            Op::call("res-has"),
        ];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.values()[0].as_bool().unwrap(), false);
    }
}

// ============================================================================
// SPAWN TOOL
// ============================================================================

mod spawn_tool {
    use super::*;

    #[tokio::test]
    async fn test_spawn_basic() {
        let ctx = setup().await;
        let stack = Stack::new();

        // Spawn a child that pushes 42
        let ops = vec![
            Op::quote(vec![Op::push(42)]),  // quote to execute
            Op::Push(Value::List(vec![])),   // empty caps (no restriction)
            Op::Push(Value::Float(0.5)),     // 50% resources
            Op::call("spawn"),
        ];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        let child_result = result.values()[0].as_list().unwrap();
        
        assert_eq!(child_result.len(), 1);
        assert_eq!(child_result[0].as_int().unwrap(), 42);
    }

    #[tokio::test]
    async fn test_spawn_computation() {
        let ctx = setup().await;
        let stack = Stack::new();

        // Spawn a child that computes 3 + 4
        let ops = vec![
            Op::quote(vec![Op::push(3), Op::push(4), Op::call("add")]),
            Op::Push(Value::List(vec![])),
            Op::Push(Value::Float(0.5)),
            Op::call("spawn"),
        ];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        let child_result = result.values()[0].as_list().unwrap();
        
        assert_eq!(child_result[0].as_int().unwrap(), 7);
    }

    #[tokio::test]
    async fn test_spawn_isolation() {
        let ctx = setup().await;
        let stack = Stack::new();

        // Push something on parent stack, spawn child, verify parent stack unchanged
        let ops = vec![
            Op::push(100),  // This is on parent stack
            Op::quote(vec![Op::push(999)]),  // Child pushes something else
            Op::Push(Value::List(vec![])),
            Op::Push(Value::Float(0.5)),
            Op::call("spawn"),
            // Stack should now have: 100, [999]
        ];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        let values = result.values();
        
        // Bottom to top: original 100, then child result list
        assert_eq!(values[0].as_int().unwrap(), 100, "parent stack preserved");
        let child_result = values[1].as_list().unwrap();
        assert_eq!(child_result[0].as_int().unwrap(), 999);
    }

    #[tokio::test]
    async fn test_spawn_with_capability_restriction() {
        let ctx = setup().await;
        let stack = Stack::new();

        // Spawn with specific capabilities
        let ops = vec![
            Op::quote(vec![
                Op::call("cap-list"),  // Get child's capabilities
            ]),
            Op::Push(Value::List(vec![Value::Text("fs:read:/tmp".into())])),
            Op::Push(Value::Float(0.5)),
            Op::call("spawn"),
        ];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        let child_result = result.values()[0].as_list().unwrap();
        
        // Child should have cap-list result
        assert!(!child_result.is_empty());
    }
}

// ============================================================================
// TRACE TOOLS
// ============================================================================

mod trace_tools {
    use super::*;

    #[tokio::test]
    async fn test_trace_step() {
        let ctx = setup().await;
        let stack = Stack::new();

        // Just verify trace-step doesn't fail
        let ops = vec![
            Op::Push(Value::Text("test-op".into())),
            Op::call("trace-step"),
        ];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert!(result.is_empty(), "trace-step consumes its argument");
    }

    #[tokio::test]
    async fn test_trace_fingerprint() {
        let ctx = setup().await;
        let stack = Stack::new();

        // Get a fingerprint
        let ops = vec![
            Op::call("trace-fingerprint"),
        ];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        let fp = result.values()[0].as_int().unwrap();
        
        // Should be a non-zero hash
        assert!(fp != 0, "fingerprint should be non-zero");
    }
}

// ============================================================================
// COMBINED ALGEBRAIC WORKFLOWS
// ============================================================================

mod workflows {
    use super::*;

    #[tokio::test]
    async fn test_spawn_with_attenuated_caps() {
        // This is the core security workflow:
        // 1. Parent has broad caps
        // 2. Parent attenuates before spawning
        // 3. Child only gets attenuated caps
        
        let ctx = setup().await;
        let stack = Stack::new();

        let ops = vec![
            // Simulate attenuating parent caps before spawn
            Op::Push(Value::List(vec![
                Value::Text("fs:read".into()),
                Value::Text("net:connect".into()),
                Value::Text("exec".into()),
            ])),
            // Attenuate to just fs:read
            Op::Push(Value::List(vec![Value::Text("fs:read".into())])),
            Op::call("cap-attenuate"),
            // Now use attenuated caps for spawn
            // Store for later
            Op::quote(vec![Op::push(42)]),
            Op::call("swap"),  // Put quote under caps
            Op::Push(Value::Float(0.5)),
            Op::call("spawn"),
        ];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert!(result.values()[0].as_list().is_ok(), "spawn succeeded with attenuated caps");
    }

    #[tokio::test]
    async fn test_resource_budget_tracking() {
        // Verify that split resources add back to original
        let ctx = setup().await;
        let stack = Stack::new();

        // Original resources
        let original_mem = 1000;
        let original_rom = 500;
        let original_compute = 2000;
        let original_net = 100;
        let ratio = 0.4;

        let ops = vec![
            Op::push(original_mem),
            Op::push(original_rom),
            Op::push(original_compute),
            Op::push(original_net),
            Op::Push(Value::Float(ratio)),
            Op::call("res-split"),
            // Now add them back together
            Op::call("res-add"),
        ];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        let values = result.values();
        
        // Should get original values back (bottom to top: mem, rom, compute, net)
        assert_eq!(values[0].as_int().unwrap(), original_mem, "mem conserved");
        assert_eq!(values[1].as_int().unwrap(), original_rom, "rom conserved");
        assert_eq!(values[2].as_int().unwrap(), original_compute, "compute conserved");
        assert_eq!(values[3].as_int().unwrap(), original_net, "net conserved");
    }
}
