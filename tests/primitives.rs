//! Tests for the 75 built-in primitives
//!
//! Each primitive does ONE thing. Predictable. No magic.
//! LLMs are first-class users.

use kore::context::Context;
use kore::executor::execute;
use kore::op::Op;
use kore::stack::Stack;
use kore::value::Value;
use kore::builtins::register_builtins;

async fn setup() -> Context {
    let mut ctx = Context::new();
    register_builtins(&mut ctx).await;
    ctx
}

async fn run(ops: Vec<Op>) -> Vec<Value> {
    let ctx = setup().await;
    let stack = Stack::new();
    let (result, _) = execute(&ops, stack, ctx).await.unwrap();
    result.values().to_vec()
}

async fn run_err(ops: Vec<Op>) -> String {
    let ctx = setup().await;
    let stack = Stack::new();
    match execute(&ops, stack, ctx).await {
        Ok(_) => panic!("expected error"),
        Err(e) => e.to_string(),
    }
}

// === Arithmetic ===

#[tokio::test]
async fn add_integers() {
    let result = run(vec![Op::push(3), Op::push(4), Op::call("add")]).await;
    assert_eq!(result, vec![Value::Int(7)]);
}

#[tokio::test]
async fn add_floats() {
    let result = run(vec![Op::Push(Value::Float(1.5)), Op::Push(Value::Float(2.5)), Op::call("add")]).await;
    assert_eq!(result, vec![Value::Float(4.0)]);
}

#[tokio::test]
async fn add_mixed() {
    let result = run(vec![Op::push(2), Op::Push(Value::Float(3.5)), Op::call("add")]).await;
    assert_eq!(result, vec![Value::Float(5.5)]);
}

#[tokio::test]
async fn sub_integers() {
    let result = run(vec![Op::push(10), Op::push(3), Op::call("sub")]).await;
    assert_eq!(result, vec![Value::Int(7)]);
}

#[tokio::test]
async fn mul_integers() {
    let result = run(vec![Op::push(6), Op::push(7), Op::call("mul")]).await;
    assert_eq!(result, vec![Value::Int(42)]);
}

#[tokio::test]
async fn div_integers() {
    let result = run(vec![Op::push(20), Op::push(4), Op::call("div")]).await;
    assert_eq!(result, vec![Value::Int(5)]);
}

#[tokio::test]
async fn div_by_zero() {
    let err = run_err(vec![Op::push(10), Op::push(0), Op::call("div")]).await;
    assert!(err.contains("zero"));
}

#[tokio::test]
async fn mod_integers() {
    let result = run(vec![Op::push(17), Op::push(5), Op::call("mod")]).await;
    assert_eq!(result, vec![Value::Int(2)]);
}

#[tokio::test]
async fn neg_integer() {
    let result = run(vec![Op::push(42), Op::call("neg")]).await;
    assert_eq!(result, vec![Value::Int(-42)]);
}

#[tokio::test]
async fn neg_float() {
    let result = run(vec![Op::Push(Value::Float(3.14)), Op::call("neg")]).await;
    assert_eq!(result, vec![Value::Float(-3.14)]);
}

// === Comparison ===

#[tokio::test]
async fn eq_true() {
    let result = run(vec![Op::push(5), Op::push(5), Op::call("eq")]).await;
    assert_eq!(result, vec![Value::Bool(true)]);
}

#[tokio::test]
async fn eq_false() {
    let result = run(vec![Op::push(5), Op::push(6), Op::call("eq")]).await;
    assert_eq!(result, vec![Value::Bool(false)]);
}

#[tokio::test]
async fn neq_true() {
    let result = run(vec![Op::push(5), Op::push(6), Op::call("neq")]).await;
    assert_eq!(result, vec![Value::Bool(true)]);
}

#[tokio::test]
async fn lt_true() {
    let result = run(vec![Op::push(3), Op::push(5), Op::call("lt")]).await;
    assert_eq!(result, vec![Value::Bool(true)]);
}

#[tokio::test]
async fn gt_true() {
    let result = run(vec![Op::push(5), Op::push(3), Op::call("gt")]).await;
    assert_eq!(result, vec![Value::Bool(true)]);
}

#[tokio::test]
async fn le_equal() {
    let result = run(vec![Op::push(5), Op::push(5), Op::call("le")]).await;
    assert_eq!(result, vec![Value::Bool(true)]);
}

#[tokio::test]
async fn ge_equal() {
    let result = run(vec![Op::push(5), Op::push(5), Op::call("ge")]).await;
    assert_eq!(result, vec![Value::Bool(true)]);
}

// === Logic ===

#[tokio::test]
async fn and_true() {
    let result = run(vec![Op::Push(Value::Bool(true)), Op::Push(Value::Bool(true)), Op::call("and")]).await;
    assert_eq!(result, vec![Value::Bool(true)]);
}

#[tokio::test]
async fn and_false() {
    let result = run(vec![Op::Push(Value::Bool(true)), Op::Push(Value::Bool(false)), Op::call("and")]).await;
    assert_eq!(result, vec![Value::Bool(false)]);
}

#[tokio::test]
async fn or_true() {
    let result = run(vec![Op::Push(Value::Bool(false)), Op::Push(Value::Bool(true)), Op::call("or")]).await;
    assert_eq!(result, vec![Value::Bool(true)]);
}

#[tokio::test]
async fn not_true() {
    let result = run(vec![Op::Push(Value::Bool(false)), Op::call("not")]).await;
    assert_eq!(result, vec![Value::Bool(true)]);
}

// === String ===

#[tokio::test]
async fn str_len() {
    let result = run(vec![Op::Push(Value::Text("hello".into())), Op::call("str-len")]).await;
    assert_eq!(result, vec![Value::Int(5)]);
}

#[tokio::test]
async fn str_len_unicode() {
    let result = run(vec![Op::Push(Value::Text("こんにちは".into())), Op::call("str-len")]).await;
    assert_eq!(result, vec![Value::Int(5)]); // 5 characters, not bytes
}

#[tokio::test]
async fn str_get() {
    let result = run(vec![Op::Push(Value::Text("hello".into())), Op::push(1), Op::call("str-get")]).await;
    assert_eq!(result, vec![Value::Text("e".into())]);
}

#[tokio::test]
async fn str_slice() {
    let result = run(vec![
        Op::Push(Value::Text("hello world".into())),
        Op::push(0),
        Op::push(5),
        Op::call("str-slice"),
    ]).await;
    assert_eq!(result, vec![Value::Text("hello".into())]);
}

#[tokio::test]
async fn str_split() {
    let result = run(vec![
        Op::Push(Value::Text("a,b,c".into())),
        Op::Push(Value::Text(",".into())),
        Op::call("str-split"),
    ]).await;
    assert_eq!(result, vec![Value::List(vec![
        Value::Text("a".into()),
        Value::Text("b".into()),
        Value::Text("c".into()),
    ])]);
}

#[tokio::test]
async fn str_join() {
    let result = run(vec![
        Op::Push(Value::List(vec![
            Value::Text("a".into()),
            Value::Text("b".into()),
            Value::Text("c".into()),
        ])),
        Op::Push(Value::Text("-".into())),
        Op::call("str-join"),
    ]).await;
    assert_eq!(result, vec![Value::Text("a-b-c".into())]);
}

#[tokio::test]
async fn str_concat() {
    let result = run(vec![
        Op::Push(Value::Text("hello".into())),
        Op::Push(Value::Text(" world".into())),
        Op::call("str-concat"),
    ]).await;
    assert_eq!(result, vec![Value::Text("hello world".into())]);
}

#[tokio::test]
async fn str_trim() {
    let result = run(vec![Op::Push(Value::Text("  hello  ".into())), Op::call("str-trim")]).await;
    assert_eq!(result, vec![Value::Text("hello".into())]);
}

#[tokio::test]
async fn str_find_found() {
    let result = run(vec![
        Op::Push(Value::Text("hello world".into())),
        Op::Push(Value::Text("world".into())),
        Op::call("str-find"),
    ]).await;
    assert_eq!(result, vec![Value::Int(6)]);
}

#[tokio::test]
async fn str_find_not_found() {
    let result = run(vec![
        Op::Push(Value::Text("hello".into())),
        Op::Push(Value::Text("xyz".into())),
        Op::call("str-find"),
    ]).await;
    assert_eq!(result, vec![Value::Int(-1)]);
}

#[tokio::test]
async fn str_starts() {
    let result = run(vec![
        Op::Push(Value::Text("hello world".into())),
        Op::Push(Value::Text("hello".into())),
        Op::call("str-starts"),
    ]).await;
    assert_eq!(result, vec![Value::Bool(true)]);
}

#[tokio::test]
async fn str_ends() {
    let result = run(vec![
        Op::Push(Value::Text("hello world".into())),
        Op::Push(Value::Text("world".into())),
        Op::call("str-ends"),
    ]).await;
    assert_eq!(result, vec![Value::Bool(true)]);
}

#[tokio::test]
async fn str_replace() {
    let result = run(vec![
        Op::Push(Value::Text("hello world".into())),
        Op::Push(Value::Text("world".into())),
        Op::Push(Value::Text("kore".into())),
        Op::call("str-replace"),
    ]).await;
    assert_eq!(result, vec![Value::Text("hello kore".into())]);
}

#[tokio::test]
async fn char_code() {
    let result = run(vec![Op::Push(Value::Text("A".into())), Op::call("char-code")]).await;
    assert_eq!(result, vec![Value::Int(65)]);
}

#[tokio::test]
async fn code_char() {
    let result = run(vec![Op::push(65), Op::call("code-char")]).await;
    assert_eq!(result, vec![Value::Text("A".into())]);
}

// === List ===

#[tokio::test]
async fn list_len() {
    let result = run(vec![
        Op::Push(Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)])),
        Op::call("list-len"),
    ]).await;
    assert_eq!(result, vec![Value::Int(3)]);
}

#[tokio::test]
async fn list_get() {
    let result = run(vec![
        Op::Push(Value::List(vec![Value::Int(10), Value::Int(20), Value::Int(30)])),
        Op::push(1),
        Op::call("list-get"),
    ]).await;
    assert_eq!(result, vec![Value::Int(20)]);
}

#[tokio::test]
async fn list_set() {
    let result = run(vec![
        Op::Push(Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)])),
        Op::push(1),
        Op::push(99),
        Op::call("list-set"),
    ]).await;
    assert_eq!(result, vec![Value::List(vec![Value::Int(1), Value::Int(99), Value::Int(3)])]);
}

#[tokio::test]
async fn list_push() {
    let result = run(vec![
        Op::Push(Value::List(vec![Value::Int(1), Value::Int(2)])),
        Op::push(3),
        Op::call("list-push"),
    ]).await;
    assert_eq!(result, vec![Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)])]);
}

#[tokio::test]
async fn list_pop() {
    let result = run(vec![
        Op::Push(Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)])),
        Op::call("list-pop"),
    ]).await;
    assert_eq!(result, vec![
        Value::List(vec![Value::Int(1), Value::Int(2)]),
        Value::Int(3),
    ]);
}

#[tokio::test]
async fn list_slice() {
    let result = run(vec![
        Op::Push(Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3), Value::Int(4)])),
        Op::push(1),
        Op::push(3),
        Op::call("list-slice"),
    ]).await;
    assert_eq!(result, vec![Value::List(vec![Value::Int(2), Value::Int(3)])]);
}

#[tokio::test]
async fn list_concat() {
    let result = run(vec![
        Op::Push(Value::List(vec![Value::Int(1), Value::Int(2)])),
        Op::Push(Value::List(vec![Value::Int(3), Value::Int(4)])),
        Op::call("list-concat"),
    ]).await;
    assert_eq!(result, vec![Value::List(vec![
        Value::Int(1), Value::Int(2), Value::Int(3), Value::Int(4),
    ])]);
}

#[tokio::test]
async fn list_reverse() {
    let result = run(vec![
        Op::Push(Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)])),
        Op::call("list-reverse"),
    ]).await;
    assert_eq!(result, vec![Value::List(vec![Value::Int(3), Value::Int(2), Value::Int(1)])]);
}

#[tokio::test]
async fn list_empty() {
    let result = run(vec![Op::call("list-empty")]).await;
    assert_eq!(result, vec![Value::List(vec![])]);
}

// === Map ===

#[tokio::test]
async fn map_set_get() {
    let result = run(vec![
        Op::call("map-empty"),
        Op::Push(Value::Text("name".into())),
        Op::Push(Value::Text("kore".into())),
        Op::call("map-set"),
        Op::Push(Value::Text("name".into())),
        Op::call("map-get"),
    ]).await;
    assert_eq!(result, vec![Value::Text("kore".into())]);
}

#[tokio::test]
async fn map_has() {
    let result = run(vec![
        Op::call("map-empty"),
        Op::Push(Value::Text("key".into())),
        Op::push(42),
        Op::call("map-set"),
        Op::Push(Value::Text("key".into())),
        Op::call("map-has"),
    ]).await;
    assert_eq!(result, vec![Value::Bool(true)]);
}

#[tokio::test]
async fn map_del() {
    let result = run(vec![
        Op::call("map-empty"),
        Op::Push(Value::Text("key".into())),
        Op::push(42),
        Op::call("map-set"),
        Op::Push(Value::Text("key".into())),
        Op::call("map-del"),
        Op::Push(Value::Text("key".into())),
        Op::call("map-has"),
    ]).await;
    assert_eq!(result, vec![Value::Bool(false)]);
}

#[tokio::test]
async fn map_keys() {
    let result = run(vec![
        Op::call("map-empty"),
        Op::Push(Value::Text("a".into())),
        Op::push(1),
        Op::call("map-set"),
        Op::Push(Value::Text("b".into())),
        Op::push(2),
        Op::call("map-set"),
        Op::call("map-keys"),
    ]).await;
    assert_eq!(result, vec![Value::List(vec![
        Value::Text("a".into()),
        Value::Text("b".into()),
    ])]);
}

#[tokio::test]
async fn map_vals() {
    let result = run(vec![
        Op::call("map-empty"),
        Op::Push(Value::Text("a".into())),
        Op::push(1),
        Op::call("map-set"),
        Op::Push(Value::Text("b".into())),
        Op::push(2),
        Op::call("map-set"),
        Op::call("map-vals"),
    ]).await;
    assert_eq!(result, vec![Value::List(vec![Value::Int(1), Value::Int(2)])]);
}

// === Type ===

#[tokio::test]
async fn type_of() {
    let result = run(vec![Op::push(42), Op::call("type-of")]).await;
    assert_eq!(result, vec![Value::Text("Int".into())]);
}

#[tokio::test]
async fn is_null() {
    let result = run(vec![Op::Push(Value::Null), Op::call("is-null")]).await;
    assert_eq!(result, vec![Value::Bool(true)]);
}

#[tokio::test]
async fn is_int() {
    let result = run(vec![Op::push(42), Op::call("is-int")]).await;
    assert_eq!(result, vec![Value::Bool(true)]);
}

#[tokio::test]
async fn is_text() {
    let result = run(vec![Op::Push(Value::Text("hi".into())), Op::call("is-text")]).await;
    assert_eq!(result, vec![Value::Bool(true)]);
}

#[tokio::test]
async fn is_list() {
    let result = run(vec![Op::Push(Value::List(vec![])), Op::call("is-list")]).await;
    assert_eq!(result, vec![Value::Bool(true)]);
}

#[tokio::test]
async fn is_quote() {
    let result = run(vec![Op::quote(vec![Op::push(1)]), Op::call("is-quote")]).await;
    assert_eq!(result, vec![Value::Bool(true)]);
}

// === Conversion ===

#[tokio::test]
async fn to_int_from_float() {
    let result = run(vec![Op::Push(Value::Float(3.7)), Op::call("to-int")]).await;
    assert_eq!(result, vec![Value::Int(3)]);
}

#[tokio::test]
async fn to_int_from_text() {
    let result = run(vec![Op::Push(Value::Text("42".into())), Op::call("to-int")]).await;
    assert_eq!(result, vec![Value::Int(42)]);
}

#[tokio::test]
async fn to_float_from_int() {
    let result = run(vec![Op::push(5), Op::call("to-float")]).await;
    assert_eq!(result, vec![Value::Float(5.0)]);
}

#[tokio::test]
async fn to_text_from_int() {
    let result = run(vec![Op::push(42), Op::call("to-text")]).await;
    assert_eq!(result, vec![Value::Text("42".into())]);
}

#[tokio::test]
async fn to_bool_truthy() {
    let result = run(vec![Op::push(1), Op::call("to-bool")]).await;
    assert_eq!(result, vec![Value::Bool(true)]);
}

#[tokio::test]
async fn to_list_wrap() {
    let result = run(vec![Op::push(42), Op::call("to-list")]).await;
    assert_eq!(result, vec![Value::List(vec![Value::Int(42)])]);
}

// === Combinators ===

#[tokio::test]
async fn map_combinator() {
    let result = run(vec![
        Op::Push(Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)])),
        Op::quote(vec![Op::push(10), Op::call("mul")]),
        Op::call("map"),
    ]).await;
    assert_eq!(result, vec![Value::List(vec![
        Value::Int(10), Value::Int(20), Value::Int(30),
    ])]);
}

#[tokio::test]
async fn filter_combinator() {
    let result = run(vec![
        Op::Push(Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3), Value::Int(4)])),
        Op::quote(vec![Op::push(2), Op::call("mod"), Op::push(0), Op::call("eq")]),
        Op::call("filter"),
    ]).await;
    assert_eq!(result, vec![Value::List(vec![Value::Int(2), Value::Int(4)])]);
}

#[tokio::test]
async fn fold_sum() {
    let result = run(vec![
        Op::Push(Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3), Value::Int(4)])),
        Op::push(0),
        Op::quote(vec![Op::call("add")]),
        Op::call("fold"),
    ]).await;
    assert_eq!(result, vec![Value::Int(10)]);
}

#[tokio::test]
async fn times_combinator() {
    let result = run(vec![
        Op::push(3),
        Op::quote(vec![Op::push(1), Op::call("add")]), // Push i, add 1
        Op::call("times"),
    ]).await;
    // times pushes 0, runs quote (0+1=1), pushes 1, runs (1+1=2), pushes 2, runs (2+1=3)
    assert_eq!(result, vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
}

#[tokio::test]
async fn each_combinator() {
    // each doesn't leave results on stack
    let result = run(vec![
        Op::Push(Value::List(vec![Value::Int(1), Value::Int(2)])),
        Op::quote(vec![Op::call("drop")]),
        Op::call("each"),
    ]).await;
    assert_eq!(result, vec![]);
}

#[tokio::test]
async fn while_combinator() {
    // Count down from 3 to 0
    let result = run(vec![
        Op::push(3), // Start with 3
        Op::quote(vec![Op::call("dup"), Op::push(0), Op::call("gt")]), // while > 0
        Op::quote(vec![Op::push(1), Op::call("sub")]), // subtract 1
        Op::call("while"),
    ]).await;
    assert_eq!(result, vec![Value::Int(0)]);
}
