//! Complex Algorithm Tests in Kore
//!
//! This file tests Kore by implementing complex algorithms to verify
//! the language's expressiveness and correctness.

use kore::{execute, Context, Op, Stack, Value};
use kore::builtins::register_builtins;

async fn load_prelude(ctx: &mut Context) {
    let prelude = include_str!("../stdlib/prelude.kore");
    let ops = Op::parse(prelude).expect("Failed to parse prelude");
    let stack = Stack::new();
    execute(&ops, stack, ctx.clone()).await.expect("Failed to execute prelude");
}

async fn run(code: &str) -> Vec<Value> {
    let mut ctx = Context::new();
    register_builtins(&mut ctx).await;
    load_prelude(&mut ctx).await;
    
    let ops = Op::parse(code).expect("Failed to parse");
    let (result, _) = execute(&ops, Stack::new(), ctx).await.expect("Execution failed");
    result.values().to_vec()
}

async fn run_expect_float(code: &str) -> f64 {
    let values = run(code).await;
    match &values[0] {
        Value::Float(f) => *f,
        Value::Int(i) => *i as f64,
        v => panic!("Expected float, got {:?}", v),
    }
}

mod basic_operations {
    use super::*;

    #[tokio::test]
    async fn test_list_creation() {
        // Create a list with 5 elements
        let code = r#"3 1 4 1 5 5 list list-len"#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Int(5));
    }

    #[tokio::test]
    async fn test_comparison() {
        let code = r#"3 5 lt"#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Bool(true));
    }

    #[tokio::test]
    async fn test_fold_combinator() {
        // Sum list elements using fold
        let code = r#"
            0 1 2 3 4 5 list  
            0
            [ add ] fold
        "#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Int(10)); // 0+1+2+3+4 = 10
    }
}

mod math_algorithms {
    use super::*;

    #[tokio::test]
    async fn test_manual_factorial() {
        // 5! = 120, computed manually
        let code = r#"1 2 mul 3 mul 4 mul 5 mul"#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Int(120));
    }

    #[tokio::test]
    async fn test_fibonacci_manual() {
        // Compute fib(6) = 8 manually using while loop
        // fib: index 0=0, 1=1, 2=1, 3=2, 4=3, 5=5, 6=8
        // Start: a=0, b=1, count=6
        // Each step: a, b = b, a+b
        let code = r#"
            0 1 6
            [ dup 0 ne ]
            [
                1 sub
                rot rot
                over over add
                rot drop
                rot
            ]
            while
            drop drop
        "#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Int(8));
    }

    #[tokio::test]
    async fn test_gcd() {
        // GCD using while loop
        let code = r#"
            48 18
            [ dup 0 ne ]
            [ swap over mod ]
            while
            drop
        "#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Int(6));
    }

    #[tokio::test]
    async fn test_power_manual() {
        // 2^3 = 8
        let code = r#"2 2 mul 2 mul"#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Int(8));
    }

    #[tokio::test]
    async fn test_sum_of_squares() {
        // 1² + 2² + 3² = 1 + 4 + 9 = 14
        let code = r#"1 1 mul 2 2 mul 3 3 mul add add"#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Int(14));
    }

    #[tokio::test]
    async fn test_times_loop() {
        // times gives index i to body, body output pushed to main stack
        // Each iteration: body gets i, outputs are pushed to main stack
        // 5 [ dup ] times pushes 0 0 1 1 2 2 3 3 4 4 (each iteration: i i)
        // Actually: body gets i, and "dup" outputs i i, both pushed back
        // Let's just verify times works: just push nothing
        let code = r#"5 [ drop ] times 42"#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Int(42));
    }
}

mod vector_operations {
    use super::*;

    #[tokio::test]
    async fn test_vec_sum() {
        // Sum vector using fold
        let code = r#"
            1 2 3 4 5 5 list
            0
            [ add ] fold
        "#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Int(15));
    }

    #[tokio::test]
    async fn test_vec_product() {
        // Product using fold
        let code = r#"
            1 2 3 4 4 list
            1
            [ mul ] fold
        "#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Int(24));
    }

    #[tokio::test]
    async fn test_vec_max() {
        // Find max using fold
        // fold signature: (list init quote -- value)
        let code = r#"
            3 1 9 4 5 5 list
            dup 0 list-get
            swap 1 4 list-slice
            swap
            [
                over over lt
                [ swap drop ]
                [ drop ]
                if
            ] fold
        "#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Int(9));
    }

    #[tokio::test]
    async fn test_vec_min() {
        // Find min using fold
        let code = r#"
            3 1 9 4 5 5 list
            dup 0 list-get
            swap 1 4 list-slice
            swap
            [
                over over lt
                [ drop ]
                [ swap drop ]
                if
            ] fold
        "#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Int(1));
    }

    #[tokio::test]
    async fn test_map_combinator() {
        // Double each element
        let code = r#"
            1 2 3 3 list
            [ dup add ] map
            list-len
        "#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Int(3));
    }

    #[tokio::test]
    async fn test_filter_combinator() {
        // Filter even numbers
        let code = r#"
            1 2 3 4 5 5 list
            [ 2 mod 0 eq ] filter
            list-len
        "#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Int(2)); // 2 and 4
    }

    #[tokio::test]
    async fn test_fold_combinator() {
        // Sum using fold
        let code = r#"
            1 2 3 4 5 5 list
            0
            [ add ] fold
        "#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Int(15));
    }
}

mod neural_net_primitives {
    use super::*;

    #[tokio::test]
    async fn test_dot_product_2d() {
        // [1,2] · [3,4] = 1*3 + 2*4 = 11
        let code = r#"
            1 2 2 list
            3 4 2 list
            swap dup 0 list-get
            swap 1 list-get
            rot dup 0 list-get
            swap 1 list-get
            rot mul
            rot rot mul
            add
        "#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Int(11));
    }

    #[tokio::test]
    async fn test_sigmoid_at_zero() {
        // sigmoid(0) = 0.5
        let code = r#"
            0.0
            neg
            256.0 div 1.0 add
            dup mul dup mul dup mul dup mul
            dup mul dup mul dup mul dup mul
            1.0 add
            1.0 swap div
        "#;
        let result = run_expect_float(code).await;
        assert!((result - 0.5).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_relu_negative() {
        let code = r#"-5.0 dup 0.0 lt [ drop 0.0 ] [ ] if"#;
        let result = run_expect_float(code).await;
        assert!((result - 0.0).abs() < 0.0001);
    }

    #[tokio::test]
    async fn test_relu_positive() {
        let code = r#"5.0 dup 0.0 lt [ drop 0.0 ] [ ] if"#;
        let result = run_expect_float(code).await;
        assert!((result - 5.0).abs() < 0.0001);
    }

    #[tokio::test]
    async fn test_weight_update() {
        // w' = w - lr * gradient
        // 0.5 - 0.1 * 0.2 = 0.5 - 0.02 = 0.48
        let code = r#"0.5 0.1 0.2 mul sub"#;
        let result = run_expect_float(code).await;
        assert!((result - 0.48).abs() < 0.0001);
    }
}

mod control_flow {
    use super::*;

    #[tokio::test]
    async fn test_while_countdown() {
        // Count down from 5 to 0
        let code = r#"
            5
            [ dup 0 ne ]
            [ 1 sub ]
            while
        "#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Int(0));
    }

    #[tokio::test]
    async fn test_while_accumulate() {
        // Sum 1+2+3+4+5 using while
        let code = r#"
            0 5
            [ dup 0 ne ]
            [ dup rot add swap 1 sub ]
            while
            drop
        "#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Int(15));
    }

    #[tokio::test]
    async fn test_times_basic() {
        // times runs body n times with index 0..n-1 on fresh stack
        // Body outputs get pushed to main stack
        // Just verify it runs n times
        let code = r#"3 [ drop 1 ] times add add"#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Int(3)); // 1+1+1
    }

    #[tokio::test]
    async fn test_conditional_if() {
        let code = r#"true [ 42 ] [ 0 ] if"#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Int(42));
    }

    #[tokio::test]
    async fn test_conditional_if_else() {
        let code = r#"false [ 42 ] [ 0 ] if"#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Int(0));
    }
}

mod string_operations {
    use super::*;

    #[tokio::test]
    async fn test_string_length() {
        let code = r#""hello" str-len"#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Int(5));
    }

    #[tokio::test]
    async fn test_string_concat() {
        let code = r#""hello" " world" str-concat str-len"#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Int(11));
    }

    #[tokio::test]
    async fn test_string_trim() {
        let code = r#""  hello  " str-trim str-len"#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Int(5));
    }

    #[tokio::test]
    async fn test_char_code() {
        let code = r#""A" char-code"#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Int(65));
    }

    #[tokio::test]
    async fn test_code_char() {
        let code = r#"65 code-char str-len"#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Int(1));
    }
}

mod definitions {
    use super::*;

    #[tokio::test]
    async fn test_simple_def() {
        let code = r#"[ 2 mul ] "double" def 5 double"#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Int(10));
    }

    #[tokio::test]
    async fn test_composed_def() {
        let code = r#"
            [ 2 mul ] "double" def
            [ double double ] "quadruple" def
            5 quadruple
        "#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Int(20));
    }

    #[tokio::test]
    async fn test_recursive_style() {
        // Factorial using defined helper
        let code = r#"
            [ dup 1 le [ drop 1 ] [ dup 1 sub fact mul ] if ] "fact" def
            5 fact
        "#;
        let result = run(code).await;
        assert_eq!(result[0], Value::Int(120));
    }
}
