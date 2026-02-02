//! # Language Semantics Tests
//!
//! These tests verify that Kore is a theoretically and practically sound
//! programming language. They test fundamental properties that any correct
//! implementation must satisfy.
//!
//! Following the Three Postulates:
//! - P1: Everything is a Tool (including `if`, which is a tool not a primitive)
//! - P2: Tools Transform Stacks
//! - P3: Composition is Concatenation

use kore::{Context, Stack, Op, Value, execute, register_builtins, Tool};

/// Helper to run a program and get the final stack
async fn run(ops: Vec<Op>) -> Vec<Value> {
    let mut ctx = Context::new();
    register_builtins(&mut ctx).await;
    let stack = Stack::new();
    let (result, _) = execute(&ops, stack, ctx).await.unwrap();
    result.values().to_vec()
}

/// Helper to run a program with additional tools
#[allow(dead_code)]
async fn run_with_tools(ops: Vec<Op>, tools: Vec<Tool>) -> Vec<Value> {
    let mut ctx = Context::new();
    register_builtins(&mut ctx).await;
    for tool in tools {
        ctx.dict.write().await.register(tool);
    }
    let stack = Stack::new();
    let (result, _) = execute(&ops, stack, ctx).await.unwrap();
    result.values().to_vec()
}

/// Helper that expects an error
async fn run_expect_error(ops: Vec<Op>) -> String {
    let mut ctx = Context::new();
    register_builtins(&mut ctx).await;
    let stack = Stack::new();
    match execute(&ops, stack, ctx).await {
        Err(e) => e.to_string(),
        Ok(_) => panic!("Expected error but got success"),
    }
}

// ============================================================================
// STACK INVARIANTS
// ============================================================================

#[tokio::test]
async fn stack_is_lifo() {
    // Push 1, 2, 3 - should be [1, 2, 3] with 3 on top
    let result = run(vec![
        Op::push(1),
        Op::push(2),
        Op::push(3),
    ]).await;
    
    // Values are stored bottom-to-top
    assert_eq!(result, vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
}

#[tokio::test]
async fn empty_program_produces_empty_stack() {
    let result = run(vec![]).await;
    assert!(result.is_empty());
}

// ============================================================================
// THE TWO OPERATIONS
// ============================================================================

#[tokio::test]
async fn push_adds_to_stack() {
    // Push is one of only two operations
    let result = run(vec![
        Op::push(42),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(42)]);
}

#[tokio::test]
async fn call_invokes_tool() {
    // Call is one of only two operations
    let result = run(vec![
        Op::push(5),
        Op::push(3),
        Op::call("add"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(8)]);
}

#[tokio::test]
async fn quote_is_just_push() {
    // Quote is Push(Value::Quote(...)), not a special operation
    // This proves Postulate 1: no special treatment
    let result = run(vec![
        Op::quote(vec![Op::push(42)]),
    ]).await;
    
    assert_eq!(result.len(), 1);
    assert!(result[0].as_quote().is_ok());
}

// ============================================================================
// QUOTE/CALL DUALITY
// ============================================================================

#[tokio::test]
async fn quote_defers_execution() {
    // Quoted code doesn't execute immediately
    let result = run(vec![
        Op::quote(vec![Op::push(1), Op::push(2)]),
    ]).await;
    
    // Only the quote is on stack, not 1 and 2
    assert_eq!(result.len(), 1);
    assert!(result[0].as_quote().is_ok());
}

#[tokio::test]
async fn call_executes_quote() {
    // call should execute a quote's contents
    let result = run(vec![
        Op::quote(vec![Op::push(42)]),
        Op::call("call"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(42)]);
}

// ============================================================================
// CONDITIONAL SEMANTICS (if is a TOOL, not a primitive!)
// ============================================================================

#[tokio::test]
async fn if_is_a_tool() {
    // `if` is just a tool - it's not special!
    // Signature: (condition:Bool then_quote:Quote else_quote:Quote -- result)
    let result = run(vec![
        Op::push(true),
        Op::quote(vec![Op::push(1)]),  // then
        Op::quote(vec![Op::push(2)]),  // else
        Op::call("if"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(1)]);
}

#[tokio::test]
async fn if_false_executes_else() {
    let result = run(vec![
        Op::push(false),
        Op::quote(vec![Op::push(1)]),  // then
        Op::quote(vec![Op::push(2)]),  // else
        Op::call("if"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(2)]);
}

#[tokio::test]
async fn if_consumes_condition_and_quotes() {
    // `if` consumes all three of its inputs
    let result = run(vec![
        Op::push(999),
        Op::push(true),
        Op::quote(vec![]),  // then (empty)
        Op::quote(vec![]),  // else (empty)
        Op::call("if"),
    ]).await;
    
    // Only 999 remains
    assert_eq!(result, vec![Value::Int(999)]);
}

#[tokio::test]
async fn truthy_values_with_if_tool() {
    // Int 1 is truthy
    let result = run(vec![
        Op::push(1),
        Op::quote(vec![Op::push("yes")]),
        Op::quote(vec![Op::push("no")]),
        Op::call("if"),
    ]).await;
    assert_eq!(result, vec![Value::Text("yes".into())]);
    
    // Int 0 is falsy
    let result = run(vec![
        Op::push(0),
        Op::quote(vec![Op::push("yes")]),
        Op::quote(vec![Op::push("no")]),
        Op::call("if"),
    ]).await;
    assert_eq!(result, vec![Value::Text("no".into())]);
}

// ============================================================================
// ERROR HANDLING SEMANTICS
// ============================================================================

#[tokio::test]
async fn try_captures_error() {
    // try should capture errors as Error values
    let result = run(vec![
        Op::quote(vec![Op::call("nonexistent")]),
        Op::call("try"),
    ]).await;
    
    assert_eq!(result.len(), 1);
    assert!(result[0].as_error().is_ok());
}

#[tokio::test]
async fn try_passes_through_success() {
    // try should pass through successful results
    let result = run(vec![
        Op::quote(vec![Op::push(42)]),
        Op::call("try"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(42)]);
}

#[tokio::test]
async fn is_error_detects_errors() {
    // is-error returns true for Error values
    let result = run(vec![
        Op::quote(vec![Op::call("nonexistent")]),
        Op::call("try"),
        Op::call("is-error"),
    ]).await;
    
    assert_eq!(result, vec![Value::Bool(true)]);
}

#[tokio::test]
async fn is_error_false_for_normal_values() {
    let result = run(vec![
        Op::push(42),
        Op::call("is-error"),
    ]).await;
    
    assert_eq!(result, vec![Value::Bool(false)]);
}

#[tokio::test]
async fn unwrap_passes_through_normal() {
    let result = run(vec![
        Op::push(42),
        Op::call("unwrap"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(42)]);
}

#[tokio::test]
async fn unwrap_stops_on_error() {
    let err = run_expect_error(vec![
        Op::quote(vec![Op::call("nonexistent")]),
        Op::call("try"),
        Op::call("unwrap"),
    ]).await;
    
    assert!(err.contains("Tool not found") || err.contains("nonexistent"));
}

#[tokio::test]
async fn errors_are_inspectable() {
    // Full error handling pattern using if as a tool
    let result = run(vec![
        // Try something that fails
        Op::quote(vec![Op::call("nonexistent")]),
        Op::call("try"),
        
        // Check if error
        Op::call("dup"),
        Op::call("is-error"),
        
        // Branch based on error using if tool
        Op::quote(vec![Op::call("drop"), Op::push("recovered")]),
        Op::quote(vec![Op::call("unwrap")]),
        Op::call("if"),
    ]).await;
    
    assert_eq!(result, vec![Value::Text("recovered".into())]);
}

// ============================================================================
// STACK MANIPULATION
// ============================================================================

#[tokio::test]
async fn dup_duplicates() {
    let result = run(vec![
        Op::push(42),
        Op::call("dup"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(42), Value::Int(42)]);
}

#[tokio::test]
async fn drop_removes() {
    let result = run(vec![
        Op::push(1),
        Op::push(2),
        Op::call("drop"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(1)]);
}

#[tokio::test]
async fn swap_swaps() {
    let result = run(vec![
        Op::push(1),
        Op::push(2),
        Op::call("swap"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(2), Value::Int(1)]);
}

#[tokio::test]
async fn over_copies_second() {
    let result = run(vec![
        Op::push(1),
        Op::push(2),
        Op::call("over"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(1), Value::Int(2), Value::Int(1)]);
}

#[tokio::test]
async fn rot_rotates() {
    let result = run(vec![
        Op::push(1),
        Op::push(2),
        Op::push(3),
        Op::call("rot"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(2), Value::Int(3), Value::Int(1)]);
}

// ============================================================================
// ARITHMETIC
// ============================================================================

#[tokio::test]
async fn add_integers() {
    let result = run(vec![
        Op::push(2),
        Op::push(3),
        Op::call("add"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(5)]);
}

#[tokio::test]
async fn mul_integers() {
    let result = run(vec![
        Op::push(4),
        Op::push(5),
        Op::call("mul"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(20)]);
}

#[tokio::test]
async fn div_integers() {
    let result = run(vec![
        Op::push(10),
        Op::push(3),
        Op::call("div"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(3)]);
}

#[tokio::test]
async fn div_by_zero_errors() {
    let err = run_expect_error(vec![
        Op::push(5),
        Op::push(0),
        Op::call("div"),
    ]).await;
    
    assert!(err.to_lowercase().contains("zero") || err.to_lowercase().contains("division"));
}

// ============================================================================
// COMPARISON
// ============================================================================

#[tokio::test]
async fn lt_comparison() {
    let result = run(vec![
        Op::push(2),
        Op::push(3),
        Op::call("lt"),
    ]).await;
    assert_eq!(result, vec![Value::Bool(true)]);
    
    let result = run(vec![
        Op::push(5),
        Op::push(3),
        Op::call("lt"),
    ]).await;
    assert_eq!(result, vec![Value::Bool(false)]);
}

#[tokio::test]
async fn eq_comparison() {
    let result = run(vec![
        Op::push(5),
        Op::push(5),
        Op::call("eq"),
    ]).await;
    assert_eq!(result, vec![Value::Bool(true)]);
    
    let result = run(vec![
        Op::push(5),
        Op::push(6),
        Op::call("eq"),
    ]).await;
    assert_eq!(result, vec![Value::Bool(false)]);
}

// ============================================================================
// COMPOSITION (Postulate 3: Composition is Concatenation)
// ============================================================================

#[tokio::test]
async fn composition_is_concatenation() {
    // f then g = [f g]
    // This is Postulate 3: composition is just putting programs together
    
    // Manual composition: dup then add
    let result = run(vec![
        Op::push(5),
        Op::call("dup"),
        Op::call("add"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(10)]);
    
    // Same thing as a composed quote
    let result = run(vec![
        Op::push(5),
        Op::quote(vec![Op::call("dup"), Op::call("add")]),
        Op::call("call"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(10)]);
}

#[tokio::test]
async fn def_creates_tool() {
    // def creates a tool from a quote
    let result = run(vec![
        Op::push("double"),
        Op::quote(vec![Op::call("dup"), Op::call("add")]),
        Op::call("def"),
        
        Op::push(21),
        Op::call("double"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(42)]);
}

// ============================================================================
// LIST OPERATIONS
// ============================================================================

#[tokio::test]
async fn list_creation() {
    let result = run(vec![
        Op::push(1),
        Op::push(2),
        Op::push(3),
        Op::push(3),
        Op::call("collect"),  // collect n items into list
    ]).await;
    
    assert_eq!(result.len(), 1);
    let list = result[0].as_list().unwrap();
    assert_eq!(list.len(), 3);
}

#[tokio::test]
async fn list_map() {
    // map applies a function to each element
    let result = run(vec![
        Op::push(1),
        Op::push(2),
        Op::push(3),
        Op::push(3),
        Op::call("collect"),  // collect n items into list
        Op::quote(vec![Op::call("dup"), Op::call("add")]),  // double
        Op::call("map"),
    ]).await;
    
    let list = result[0].as_list().unwrap();
    assert_eq!(list[0].as_int().unwrap(), 2);
    assert_eq!(list[1].as_int().unwrap(), 4);
    assert_eq!(list[2].as_int().unwrap(), 6);
}

#[tokio::test]
async fn list_filter() {
    // filter keeps elements where predicate is truthy
    let result = run(vec![
        Op::push(1),
        Op::push(2),
        Op::push(3),
        Op::push(4),
        Op::push(4),
        Op::call("collect"),  // collect n items into list
        Op::quote(vec![Op::push(2), Op::call("gt")]),  // > 2
        Op::call("filter"),
    ]).await;
    
    let list = result[0].as_list().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].as_int().unwrap(), 3);
    assert_eq!(list[1].as_int().unwrap(), 4);
}

#[tokio::test]
async fn list_fold() {
    // fold accumulates with a function
    let result = run(vec![
        Op::push(1),
        Op::push(2),
        Op::push(3),
        Op::push(3),
        Op::call("collect"),  // collect n items into list
        Op::push(0),  // initial accumulator
        Op::quote(vec![Op::call("add")]),
        Op::call("fold"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(6)]);
}

// ============================================================================
// LOOP CONSTRUCTS (loops are tools, not primitives!)
// ============================================================================

#[tokio::test]
async fn times_repeats() {
    // times creates fresh stack with index each iteration
    // and pushes results back to main stack
    let result = run(vec![
        Op::push(3),  // repeat 3 times
        Op::quote(vec![Op::call("dup"), Op::call("add")]),  // double the index
        Op::call("times"),
    ]).await;
    
    // 0*2=0, 1*2=2, 2*2=4
    assert_eq!(result, vec![Value::Int(0), Value::Int(2), Value::Int(4)]);
}

// ============================================================================
// TYPE CHECKING
// ============================================================================

#[tokio::test]
async fn is_int_check() {
    let result = run(vec![
        Op::push(42),
        Op::call("is-int"),
    ]).await;
    assert_eq!(result, vec![Value::Bool(true)]);
    
    let result = run(vec![
        Op::push("hello"),
        Op::call("is-int"),
    ]).await;
    assert_eq!(result, vec![Value::Bool(false)]);
}

#[tokio::test]
async fn is_text_check() {
    let result = run(vec![
        Op::push("hello"),
        Op::call("is-text"),
    ]).await;
    assert_eq!(result, vec![Value::Bool(true)]);
    
    let result = run(vec![
        Op::push(42),
        Op::call("is-text"),
    ]).await;
    assert_eq!(result, vec![Value::Bool(false)]);
}

// ============================================================================
// PARSING
// ============================================================================

#[tokio::test]
async fn parse_simple_program() {
    // Parse and execute a string program
    let ops = Op::parse("1 2 add").unwrap();
    assert_eq!(ops.len(), 3);
    
    let result = run(ops).await;
    assert_eq!(result, vec![Value::Int(3)]);
}

#[tokio::test]
async fn parse_with_quote() {
    // Parse a program with a quote
    let ops = Op::parse("5 [ dup add ] call").unwrap();
    
    let result = run(ops).await;
    assert_eq!(result, vec![Value::Int(10)]);
}

#[tokio::test]
async fn parse_with_if() {
    // Parse a program using if as a tool
    let ops = Op::parse("true [ 1 ] [ 2 ] if").unwrap();
    
    let result = run(ops).await;
    assert_eq!(result, vec![Value::Int(1)]);
}

// ============================================================================
// POSTULATE VERIFICATION
// ============================================================================

#[tokio::test]
async fn postulate_1_everything_is_tool() {
    // Verify that if, while, etc. are tools, not primitives
    // They can be looked up and have metadata
    let result = run(vec![
        Op::push("if"),
        Op::call("meta"),
    ]).await;
    
    assert_eq!(result.len(), 1);
    assert!(result[0].as_map().is_ok(), "if should be a tool with metadata");
}

#[tokio::test]
async fn postulate_2_tools_transform_stacks() {
    // Every tool takes a stack and returns a stack
    // This is verified by the fact that our entire test suite works!
    let result = run(vec![
        Op::push(1),
        Op::push(2),
        Op::call("add"),
    ]).await;
    
    // The tool transformed (1, 2) -> (3)
    assert_eq!(result, vec![Value::Int(3)]);
}

#[tokio::test]
async fn postulate_3_composition_is_concatenation() {
    // f ; g = concatenate(f, g)
    // This is the simplest composition model possible
    
    // Composition IS concatenation - we just put operations together
    // There's no special compose operator needed, you just... compose
    
    // Define double = [dup add]
    let result = run(vec![
        Op::push("double"),
        Op::quote(vec![Op::call("dup"), Op::call("add")]),
        Op::call("def"),
        
        // Define quadruple = [double double] (composition!)
        Op::push("quadruple"),
        Op::quote(vec![Op::call("double"), Op::call("double")]),
        Op::call("def"),
        
        // Test: 5 quadruple = 20
        Op::push(5),
        Op::call("quadruple"),
    ]).await;
    
    assert_eq!(result, vec![Value::Int(20)]);
}
