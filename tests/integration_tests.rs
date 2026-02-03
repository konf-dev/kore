//! # Integration Tests
//!
//! Tests for the complete Kore system, verifying that all components
//! work together correctly. Follows the Three Postulates.

use kore::{Context, Stack, Op, Value, execute, register_builtins};
use kore::stdlib::load_prelude;

/// Helper to run a program and get the final stack
async fn run(ops: Vec<Op>) -> Vec<Value> {
    let mut ctx = Context::new();
    register_builtins(&mut ctx).await;
    load_prelude(&ctx).await.expect("prelude should load");
    let stack = Stack::new();
    let (result, _) = execute(&ops, stack, ctx).await.unwrap();
    result.values().to_vec()
}

/// Helper that expects an error
async fn run_expect_error(ops: Vec<Op>) -> String {
    let mut ctx = Context::new();
    register_builtins(&mut ctx).await;
    load_prelude(&ctx).await.expect("prelude should load");
    let stack = Stack::new();
    match execute(&ops, stack, ctx).await {
        Err(e) => e.to_string(),
        Ok(_) => panic!("Expected error but got success"),
    }
}

// ============================================================================
// PARSER INTEGRATION
// ============================================================================

#[tokio::test]
async fn parser_handles_all_types() {
    let ops = Op::parse("42 3.14 true false null \"hello\" 'world'").unwrap();
    let result = run(ops).await;
    
    assert_eq!(result.len(), 7);
    assert_eq!(result[0].as_int().unwrap(), 42);
    assert!((result[1].as_float().unwrap() - 3.14).abs() < 0.001);
    assert_eq!(result[2].as_bool().unwrap(), true);
    assert_eq!(result[3].as_bool().unwrap(), false);
    assert!(result[4].is_null());
    assert_eq!(result[5].as_text().unwrap(), "hello");
    assert_eq!(result[6].as_text().unwrap(), "world");
}

#[tokio::test]
async fn parser_handles_nested_quotes() {
    let ops = Op::parse("[ [ [ 42 ] ] ]").unwrap();
    let result = run(ops).await;
    
    assert_eq!(result.len(), 1);
    let outer = result[0].as_quote().unwrap();
    assert_eq!(outer.len(), 1);
}

#[tokio::test]
async fn parser_handles_comments() {
    let ops = Op::parse("1 # comment\n2 add").unwrap();
    let result = run(ops).await;
    
    assert_eq!(result, vec![Value::Int(3)]);
}

// ============================================================================
// CONDITIONAL INTEGRATION (if as a tool)
// ============================================================================

#[tokio::test]
async fn if_tool_true_branch() {
    let result = run(vec![
        Op::push(true),
        Op::quote(vec![Op::push("yes")]),
        Op::quote(vec![Op::push("no")]),
        Op::call("if"),
    ]).await;
    
    assert_eq!(result, vec![Value::Text("yes".into())]);
}

#[tokio::test]
async fn if_tool_false_branch() {
    let result = run(vec![
        Op::push(false),
        Op::quote(vec![Op::push("yes")]),
        Op::quote(vec![Op::push("no")]),
        Op::call("if"),
    ]).await;
    
    assert_eq!(result, vec![Value::Text("no".into())]);
}

#[tokio::test]
async fn if_tool_with_computation() {
    // true [5 dup add] [10] if should give 10
    let result = run(vec![
        Op::push(true),
        Op::quote(vec![Op::push(5), Op::call("dup"), Op::call("add")]),
        Op::quote(vec![Op::push(10)]),
        Op::call("if"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(10)]);
}

#[tokio::test]
async fn nested_if_tools() {
    // Nested conditionals using if as a tool
    let result = run(vec![
        Op::push(true),
        Op::quote(vec![
            Op::push(false),
            Op::quote(vec![Op::push(1)]),
            Op::quote(vec![Op::push(2)]),
            Op::call("if"),
        ]),
        Op::quote(vec![Op::push(3)]),
        Op::call("if"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(2)]);
}

// ============================================================================
// ERROR HANDLING INTEGRATION
// ============================================================================

#[tokio::test]
async fn try_catch_pattern() {
    // try with catch using if tool
    let result = run(vec![
        Op::quote(vec![Op::call("nonexistent")]),
        Op::call("try"),
        Op::call("dup"),
        Op::call("is-error"),
        Op::quote(vec![Op::call("drop"), Op::push("caught")]),
        Op::quote(vec![]),
        Op::call("if"),
    ]).await;
    
    assert_eq!(result, vec![Value::Text("caught".into())]);
}

#[tokio::test]
async fn try_success_passes_through() {
    let result = run(vec![
        Op::quote(vec![Op::push(42)]),
        Op::call("try"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(42)]);
}

// ============================================================================
// TOOL DEFINITION INTEGRATION
// ============================================================================

#[tokio::test]
async fn def_and_call() {
    // [ body ] "name" def
    let result = run(vec![
        Op::quote(vec![Op::call("dup"), Op::call("mul")]),
        Op::push("square"),
        Op::call("def"),
        
        Op::push(7),
        Op::call("square"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(49)]);
}

#[tokio::test]
async fn def_with_composed_tools() {
    // Define a tool that uses other defined tools
    // [ body ] "name" def
    let result = run(vec![
        // Define double
        Op::quote(vec![Op::call("dup"), Op::call("add")]),
        Op::push("double"),
        Op::call("def"),
        
        // Define quadruple using double
        Op::quote(vec![Op::call("double"), Op::call("double")]),
        Op::push("quadruple"),
        Op::call("def"),
        
        // Test it
        Op::push(5),
        Op::call("quadruple"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(20)]);
}

// ============================================================================
// LIST INTEGRATION
// ============================================================================

#[tokio::test]
async fn list_operations_chain() {
    // Create, map, filter, fold
    let result = run(vec![
        // Create [1, 2, 3, 4, 5] using collect
        Op::push(1),
        Op::push(2),
        Op::push(3),
        Op::push(4),
        Op::push(5),
        Op::push(5),
        Op::call("collect"),  // collect n items into list
        
        // Double each: [2, 4, 6, 8, 10]
        Op::quote(vec![Op::call("dup"), Op::call("add")]),
        Op::call("map"),
        
        // Keep > 5: [6, 8, 10]
        Op::quote(vec![Op::push(5), Op::call("gt")]),
        Op::call("filter"),
        
        // Sum: 24 (using fold, not reduce)
        Op::push(0),
        Op::quote(vec![Op::call("add")]),
        Op::call("fold"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(24)]);
}

// ============================================================================
// RECURSION PATTERN
// ============================================================================

#[tokio::test]
async fn recursive_factorial() {
    // Factorial using recursion with if tool
    // [ body ] "name" def
    let result = run(vec![
        // Define factorial
        Op::quote(vec![
            // (n -- n!)
            Op::call("dup"),
            Op::push(1),
            Op::call("le"),  // n <= 1?
            Op::quote(vec![Op::call("drop"), Op::push(1)]),  // base case
            Op::quote(vec![
                Op::call("dup"),
                Op::push(1),
                Op::call("sub"),
                Op::call("factorial"),
                Op::call("mul"),
            ]),  // recursive case
            Op::call("if"),
        ]),
        Op::push("factorial"),
        Op::call("def"),
        
        // Test: 5!
        Op::push(5),
        Op::call("factorial"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(120)]);
}

// ============================================================================
// LOOP INTEGRATION
// ============================================================================

#[tokio::test]
async fn times_loop() {
    // times creates a fresh stack with index for each iteration
    // and pushes results back to main stack
    let result = run(vec![
        Op::push(5),  // 5 times
        Op::quote(vec![Op::call("dup"), Op::call("add")]),  // double the index
        Op::call("times"),
    ]).await;
    
    // 0*2=0, 1*2=2, 2*2=4, 3*2=6, 4*2=8
    assert_eq!(result, vec![Value::Int(0), Value::Int(2), Value::Int(4), Value::Int(6), Value::Int(8)]);
}

// ============================================================================
// STRING OPERATIONS
// ============================================================================

#[tokio::test]
async fn string_operations() {
    // str-concat
    let result = run(vec![
        Op::push("hello"),
        Op::push(" world"),
        Op::call("str-concat"),
    ]).await;
    assert_eq!(result[0].as_text().unwrap(), "hello world");
    
    // str-len
    let result = run(vec![
        Op::push("hello"),
        Op::call("str-len"),
    ]).await;
    assert_eq!(result[0].as_int().unwrap(), 5);
}

// ============================================================================
// MAP/RECORD OPERATIONS
// ============================================================================

#[tokio::test]
async fn map_operations() {
    let result = run(vec![
        // Create empty map and fill it
        Op::call("map-empty"),
        Op::push("name"),
        Op::push("Alice"),
        Op::call("map-set"),
        Op::push("age"),
        Op::push(30),
        Op::call("map-set"),
        
        // Get value
        Op::call("dup"),
        Op::push("name"),
        Op::call("map-get"),
    ]).await;
    
    assert_eq!(result.len(), 2);
    assert_eq!(result[1].as_text().unwrap(), "Alice");
}

// ============================================================================
// INTROSPECTION
// ============================================================================

#[tokio::test]
async fn meta_introspection() {
    let result = run(vec![
        Op::push("dup"),
        Op::call("meta"),
    ]).await;
    
    assert_eq!(result.len(), 1);
    let meta = result[0].as_map().unwrap();
    assert!(meta.contains_key("name") || meta.contains_key("doc"));
}

#[tokio::test]
async fn words_lists_tools() {
    let result = run(vec![
        Op::call("words"),  // words lists all tool names
    ]).await;
    
    assert_eq!(result.len(), 1);
    let tools = result[0].as_list().unwrap();
    
    // Should include basic tools
    let tool_names: Vec<_> = tools.iter()
        .map(|v| v.as_text().unwrap())
        .collect();
    
    assert!(tool_names.contains(&"dup"));
    assert!(tool_names.contains(&"add"));
    assert!(tool_names.contains(&"if"));  // if is a tool!
}

// ============================================================================
// COMPLETE PROGRAMS
// ============================================================================

#[tokio::test]
async fn fibonacci_sequence() {
    // Generate fibonacci numbers using a while loop instead of times
    // (times creates fresh stacks, while works on the current stack)
    let result = run(vec![
        // Define fib-next: (a b -- b a+b)
        // over gives us (a b -- a b a), then add gives (a, b+a)
        // We need swap at the end: (a c -- c a) where c = b+a... 
        // Actually: swap, over, add = (a b -- b a) -> (b a b) -> (b a+b)
        // [ body ] "name" def
        Op::quote(vec![
            Op::call("swap"),  // a b -> b a
            Op::call("over"),  // b a -> b a b
            Op::call("add"),   // b a b -> b a+b
        ]),
        Op::push("fib-next"),
        Op::call("def"),
        
        // Start with 0 1, and a counter
        Op::push(0),
        Op::push(1),
        Op::push(5),  // counter
        
        // While counter > 0: fib-next, decrement counter
        Op::quote(vec![Op::call("dup"), Op::push(0), Op::call("gt")]),  // check counter > 0
        Op::quote(vec![
            Op::push(1), Op::call("sub"),  // decrement counter
            Op::call("rot"), Op::call("rot"),  // bring counter to bottom
            Op::call("fib-next"),  // do fib step
            Op::call("rot"),  // bring counter back to top
        ]),
        Op::call("while"),
        
        // Drop counter
        Op::call("drop"),
    ]).await;
    
    // 0 1 -> 1 1 -> 1 2 -> 2 3 -> 3 5 -> 5 8
    // After 5 iterations: [5, 8]
    assert_eq!(result, vec![Value::Int(5), Value::Int(8)]);
}

#[tokio::test]
async fn stack_based_calculator() {
    // (3 + 4) * (10 - 5) = 35
    let result = run(vec![
        Op::push(3),
        Op::push(4),
        Op::call("add"),     // 7
        Op::push(10),
        Op::push(5),
        Op::call("sub"),     // 5
        Op::call("mul"),     // 35
    ]).await;
    
    assert_eq!(result, vec![Value::Int(35)]);
}

// ============================================================================
// LINEAR TYPE ENFORCEMENT
// ============================================================================

#[tokio::test]
async fn linear_value_cannot_be_duplicated() {
    // Create a linear value and try to dup it - should fail
    let linear = Value::linear(Value::Text("unique-resource".into()));
    
    let mut ctx = Context::new();
    register_builtins(&mut ctx).await;
    
    let mut stack = Stack::new();
    stack.push(linear).unwrap();
    
    let ops = vec![Op::call("dup")];
    let result = execute(&ops, stack, ctx).await;
    
    match result {
        Ok(_) => panic!("Expected error but got success"),
        Err(err) => {
            assert!(err.to_string().contains("cannot be duplicated") || 
                    err.code() == "E_LINEAR_DUP",
                    "Unexpected error: {}", err);
        }
    }
}

#[tokio::test]
async fn linear_value_cannot_be_discarded() {
    // Create a linear value and try to drop it - should fail
    let linear = Value::linear(Value::Text("unique-resource".into()));
    
    let mut ctx = Context::new();
    register_builtins(&mut ctx).await;
    
    let mut stack = Stack::new();
    stack.push(linear).unwrap();
    
    let ops = vec![Op::call("drop")];
    let result = execute(&ops, stack, ctx).await;
    
    match result {
        Ok(_) => panic!("Expected error but got success"),
        Err(err) => {
            assert!(err.to_string().contains("cannot be discarded") || 
                    err.code() == "E_LINEAR_DROP",
                    "Unexpected error: {}", err);
        }
    }
}

#[tokio::test]
async fn regular_values_can_still_be_duplicated() {
    // Normal values should still dup fine
    let result = run(vec![
        Op::push(42),
        Op::call("dup"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(42), Value::Int(42)]);
}
