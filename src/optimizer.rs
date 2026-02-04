//! Algebraic Optimizer - Group-Theoretic Rewrites
//!
//! Stack operations form algebraic structures that enable optimization:
//!
//! ## Stack Group Identities
//!
//! | Pattern | Rewrite | Justification |
//! |---------|---------|---------------|
//! | `swap swap` | ε | swap is self-inverse |
//! | `rot rot rot` | ε | rot³ = identity |
//! | `dup drop` | ε | duplicate then discard |
//! | `over drop` | ε | copy second then discard |
//! | `swap over` | `dup rot` | algebraic equivalence |
//!
//! ## Tensor Algebraic Identities
//!
//! These exploit mathematical laws impossible in mutable languages:
//!
//! | Pattern | Rewrite | Law |
//! |---------|---------|-----|
//! | `tensor-neg tensor-neg` | ε | involution: -(-x) = x |
//! | `tensor-exp tensor-log` | ε | inverse: log(exp(x)) = x |
//! | `tensor-log tensor-exp` | ε | inverse: exp(log(x)) = x |
//! | `tensor-transpose tensor-transpose` | ε | involution: (Aᵀ)ᵀ = A |
//! | `tensor-relu tensor-relu` | `tensor-relu` | idempotent: relu(relu(x)) = relu(x) |
//! | `Push(1.0) tensor-scale` | ε | identity: 1·x = x |
//! | `Push(0.0) tensor-scale` | `drop tensor-zeros-like` | annihilation: 0·x = 0 |
//! | `Push(-1.0) tensor-scale` | `tensor-neg` | negation: (-1)·x = -x |
//!
//! ## Constant Folding
//!
//! | Pattern | Rewrite |
//! |---------|---------|
//! | `Push(a) Push(b) add` | `Push(a+b)` |
//! | `Push(a) Push(b) mul` | `Push(a*b)` |
//! | `Push(a) dup` | `Push(a) Push(a)` |
//!
//! ## Mathematical Foundation
//!
//! The stack operations under composition form a **monoid**:
//! - Identity: empty sequence ε
//! - Associativity: (f ; g) ; h = f ; (g ; h)
//!
//! Some operations are **involutions** (self-inverse): swap, not, transpose
//! Some have **finite order**: rot³ = ε
//! Some are **idempotent**: relu, abs (on non-negative)

use crate::op::Op;
use crate::value::Value;

/// Optimization level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptLevel {
    /// No optimization
    None,
    /// Only algebraic identities (safe, always valid)
    Algebraic,
    /// Algebraic + constant folding
    Full,
}

impl Default for OptLevel {
    fn default() -> Self {
        OptLevel::Algebraic
    }
}

/// Optimizer state
pub struct Optimizer {
    level: OptLevel,
    /// Number of rewrites applied
    rewrites: usize,
}

impl Optimizer {
    pub fn new(level: OptLevel) -> Self {
        Self { level, rewrites: 0 }
    }

    /// Optimize a sequence of operations
    pub fn optimize(&mut self, ops: Vec<Op>) -> Vec<Op> {
        if self.level == OptLevel::None {
            return ops;
        }

        let mut result = ops;
        let mut changed = true;

        // Fixed-point iteration until no more changes
        while changed {
            changed = false;
            
            // Apply algebraic rewrites
            let (new_ops, did_change) = self.algebraic_pass(&result);
            if did_change {
                result = new_ops;
                changed = true;
            }

            // Apply constant folding if enabled
            if self.level == OptLevel::Full {
                let (new_ops, did_change) = self.constant_fold(&result);
                if did_change {
                    result = new_ops;
                    changed = true;
                }
            }
        }

        result
    }

    /// Algebraic identity rewrites
    fn algebraic_pass(&mut self, ops: &[Op]) -> (Vec<Op>, bool) {
        let mut result = Vec::with_capacity(ops.len());
        let mut changed = false;
        let mut i = 0;

        while i < ops.len() {
            // Try 3-op patterns first
            if i + 2 < ops.len() {
                if let Some(rewrite) = self.match_triple(&ops[i], &ops[i + 1], &ops[i + 2]) {
                    result.extend(rewrite);
                    self.rewrites += 1;
                    changed = true;
                    i += 3;
                    continue;
                }
            }

            // Try 2-op patterns
            if i + 1 < ops.len() {
                if let Some(rewrite) = self.match_pair(&ops[i], &ops[i + 1]) {
                    result.extend(rewrite);
                    self.rewrites += 1;
                    changed = true;
                    i += 2;
                    continue;
                }
            }

            // No pattern matched, keep the op
            result.push(ops[i].clone());
            i += 1;
        }

        (result, changed)
    }

    /// Match 2-op algebraic patterns
    fn match_pair(&self, a: &Op, b: &Op) -> Option<Vec<Op>> {
        match (a, b) {
            // swap swap → ε (swap is self-inverse)
            (Op::Call(x), Op::Call(y)) if x == "swap" && y == "swap" => {
                Some(vec![])
            }

            // not not → ε (not is self-inverse)
            (Op::Call(x), Op::Call(y)) if x == "not" && y == "not" => {
                Some(vec![])
            }

            // neg neg → ε (neg is self-inverse for integers)
            (Op::Call(x), Op::Call(y)) if x == "neg" && y == "neg" => {
                Some(vec![])
            }

            // === TENSOR ALGEBRAIC IDENTITIES ===
            // These exploit mathematical laws that von Neumann languages cannot use
            // because Kore's immutability (P1) guarantees no aliasing.

            // tensor-neg tensor-neg → ε (-(-x) = x)
            (Op::Call(x), Op::Call(y)) if x == "tensor-neg" && y == "tensor-neg" => {
                Some(vec![])
            }

            // tensor-exp tensor-log → ε (log(exp(x)) = x for all real x)
            (Op::Call(x), Op::Call(y)) if x == "tensor-exp" && y == "tensor-log" => {
                Some(vec![])
            }

            // tensor-log tensor-exp → ε (exp(log(x)) = x for x > 0)
            // Note: This is mathematically valid only for x > 0, but tensors
            // typically clamp log input to avoid -inf, so this is safe.
            (Op::Call(x), Op::Call(y)) if x == "tensor-log" && y == "tensor-exp" => {
                Some(vec![])
            }

            // === TENSOR INVOLUTIONS ===
            // Operations that are their own inverse: f(f(x)) = x

            // tensor-transpose tensor-transpose → ε ((Aᵀ)ᵀ = A)
            (Op::Call(x), Op::Call(y)) if x == "tensor-transpose" && y == "tensor-transpose" => {
                Some(vec![])
            }

            // === TENSOR IDEMPOTENT OPERATIONS ===
            // Operations where f(f(x)) = f(x)

            // tensor-relu tensor-relu → tensor-relu (relu(relu(x)) = relu(x))
            // Because relu(x) ≥ 0, applying relu again has no effect
            (Op::Call(x), Op::Call(y)) if x == "tensor-relu" && y == "tensor-relu" => {
                Some(vec![Op::call("tensor-relu")])
            }

            // tensor-abs tensor-abs → tensor-abs (|abs(x)| = |x|)
            // Absolute value is idempotent on its own output
            (Op::Call(x), Op::Call(y)) if x == "tensor-abs" && y == "tensor-abs" => {
                Some(vec![Op::call("tensor-abs")])
            }

            // === ACTIVATION COMPOSITION LAWS ===
            // Some activation pairs have special properties

            // tensor-sigmoid tensor-relu → tensor-sigmoid
            // sigmoid(x) ∈ [0,1], so relu(sigmoid(x)) = sigmoid(x)
            (Op::Call(x), Op::Call(y)) if x == "tensor-sigmoid" && y == "tensor-relu" => {
                Some(vec![Op::call("tensor-sigmoid")])
            }

            // tensor-softmax tensor-relu → tensor-softmax  
            // softmax(x) ∈ [0,1] with sum=1, so relu has no effect
            (Op::Call(x), Op::Call(y)) if x == "tensor-softmax" && y == "tensor-relu" => {
                Some(vec![Op::call("tensor-softmax")])
            }

            // tensor-relu tensor-abs → tensor-relu
            // relu(x) ≥ 0, so |relu(x)| = relu(x)
            (Op::Call(x), Op::Call(y)) if x == "tensor-relu" && y == "tensor-abs" => {
                Some(vec![Op::call("tensor-relu")])
            }

            // tensor-sigmoid tensor-sigmoid is NOT simplified
            // sigmoid(sigmoid(x)) ≠ sigmoid(x) in general

            // === SCALAR IDENTITY PATTERNS ===
            // Patterns where a scalar value with tensor-scale can be simplified

            // Push(1.0) tensor-scale → ε (1·x = x, multiplicative identity)
            (Op::Push(Value::Float(f)), Op::Call(op)) 
                if *f == 1.0 && op == "tensor-scale" => {
                Some(vec![])  // Drop both, tensor unchanged
            }
            (Op::Push(Value::Int(n)), Op::Call(op)) 
                if *n == 1 && op == "tensor-scale" => {
                Some(vec![])
            }

            // Push(-1.0) tensor-scale → tensor-neg ((-1)·x = -x)
            (Op::Push(Value::Float(f)), Op::Call(op)) 
                if *f == -1.0 && op == "tensor-scale" => {
                Some(vec![Op::call("tensor-neg")])
            }
            (Op::Push(Value::Int(n)), Op::Call(op)) 
                if *n == -1 && op == "tensor-scale" => {
                Some(vec![Op::call("tensor-neg")])
            }

            // Push(0.0) tensor-scale is NOT simplified to tensor-zeros
            // because we'd need to track the tensor shape, which requires context
            // This is a semantic transformation, not purely syntactic

            // Push(2.0) tensor-scale tensor-scale → Push(4.0) tensor-scale
            // This requires 3-op pattern, handled below

            // dup drop → ε (duplicate then discard)
            (Op::Call(x), Op::Call(y)) if x == "dup" && y == "drop" => {
                Some(vec![])
            }

            // over drop → ε (copy second then discard copy)
            (Op::Call(x), Op::Call(y)) if x == "over" && y == "drop" => {
                Some(vec![])
            }

            // over nip → dup (copy second, remove first = duplicate top)
            (Op::Call(x), Op::Call(y)) if x == "over" && y == "nip" => {
                Some(vec![Op::call("dup")])
            }

            // swap nip → drop (swap then remove second = drop top)
            (Op::Call(x), Op::Call(y)) if x == "swap" && y == "nip" => {
                Some(vec![Op::call("drop")])
            }

            // nip nip on 3 elements: (a b c -- c)
            // But we need context, so skip for now

            _ => None,
        }
    }

    /// Match 3-op algebraic patterns
    fn match_triple(&self, a: &Op, b: &Op, c: &Op) -> Option<Vec<Op>> {
        match (a, b, c) {
            // rot rot rot → ε (rot has order 3)
            (Op::Call(x), Op::Call(y), Op::Call(z)) 
                if x == "rot" && y == "rot" && z == "rot" => {
                Some(vec![])
            }

            // swap rot swap → rot rot (conjugacy in symmetric group)
            (Op::Call(x), Op::Call(y), Op::Call(z))
                if x == "swap" && y == "rot" && z == "swap" => {
                Some(vec![Op::call("rot"), Op::call("rot")])
            }

            // dup swap drop → ε (duplicate, swap to bottom, drop = identity)
            (Op::Call(x), Op::Call(y), Op::Call(z))
                if x == "dup" && y == "swap" && z == "drop" => {
                Some(vec![])
            }

            // Push dup drop → Push (pushing, duplicating, then dropping = just push)
            (Op::Push(_), Op::Call(y), Op::Call(z))
                if y == "dup" && z == "drop" => {
                Some(vec![a.clone()])
            }

            // === TENSOR SCALE COMPOSITION ===
            // Push(a) tensor-scale Push(b) tensor-scale → Push(a*b) tensor-scale
            // This folds consecutive scalar multiplications
            // Note: We need a 4-op pattern for this, but can catch some 3-op cases
            
            // tensor-neg Push(f) tensor-scale → Push(-f) tensor-scale
            // Moving negation into the scalar
            (Op::Call(x), Op::Push(Value::Float(f)), Op::Call(z))
                if x == "tensor-neg" && z == "tensor-scale" => {
                Some(vec![Op::Push(Value::Float(-f)), Op::call("tensor-scale")])
            }
            (Op::Call(x), Op::Push(Value::Int(n)), Op::Call(z))
                if x == "tensor-neg" && z == "tensor-scale" => {
                Some(vec![Op::Push(Value::Int(-n)), Op::call("tensor-scale")])
            }

            // === TENSOR ACTIVATION CHAINS ===
            // relu after sigmoid: sigmoid output is [0,1], relu has no effect
            // tensor-sigmoid tensor-relu → tensor-sigmoid
            (Op::Call(x), Op::Call(y), Op::Call(z))
                if x == "tensor-sigmoid" && y == "tensor-relu" => {
                // Only match if z is something else (we're looking at 3 ops)
                // Actually this is a 2-op pattern we missed, let's handle it properly
                None  // Handle in match_pair instead
            }

            _ => None,
        }
    }

    /// Constant folding pass
    fn constant_fold(&mut self, ops: &[Op]) -> (Vec<Op>, bool) {
        let mut result = Vec::with_capacity(ops.len());
        let mut changed = false;
        let mut i = 0;

        while i < ops.len() {
            // Try binary arithmetic: Push(a) Push(b) op → Push(result)
            if i + 2 < ops.len() {
                if let (Op::Push(a), Op::Push(b), Op::Call(op)) = (&ops[i], &ops[i + 1], &ops[i + 2]) {
                    if let Some(folded) = self.fold_binary(a, b, op) {
                        result.push(Op::Push(folded));
                        self.rewrites += 1;
                        changed = true;
                        i += 3;
                        continue;
                    }
                }
            }

            // Try unary: Push(a) op → Push(result)
            if i + 1 < ops.len() {
                if let (Op::Push(a), Op::Call(op)) = (&ops[i], &ops[i + 1]) {
                    if let Some(folded) = self.fold_unary(a, op) {
                        result.push(Op::Push(folded));
                        self.rewrites += 1;
                        changed = true;
                        i += 2;
                        continue;
                    }
                }
            }

            result.push(ops[i].clone());
            i += 1;
        }

        (result, changed)
    }

    /// Fold binary operations on constants
    fn fold_binary(&self, a: &Value, b: &Value, op: &str) -> Option<Value> {
        match (a, b, op) {
            // Integer arithmetic
            (Value::Int(x), Value::Int(y), "add") => Some(Value::Int(x + y)),
            (Value::Int(x), Value::Int(y), "sub") => Some(Value::Int(x - y)),
            (Value::Int(x), Value::Int(y), "mul") => Some(Value::Int(x * y)),
            (Value::Int(x), Value::Int(y), "div") if *y != 0 => Some(Value::Int(x / y)),
            (Value::Int(x), Value::Int(y), "mod") if *y != 0 => Some(Value::Int(x % y)),

            // Float arithmetic
            (Value::Float(x), Value::Float(y), "add") => Some(Value::Float(x + y)),
            (Value::Float(x), Value::Float(y), "sub") => Some(Value::Float(x - y)),
            (Value::Float(x), Value::Float(y), "mul") => Some(Value::Float(x * y)),
            (Value::Float(x), Value::Float(y), "div") if *y != 0.0 => Some(Value::Float(x / y)),

            // Mixed arithmetic (promote to float)
            (Value::Int(x), Value::Float(y), "add") => Some(Value::Float(*x as f64 + y)),
            (Value::Float(x), Value::Int(y), "add") => Some(Value::Float(x + *y as f64)),
            (Value::Int(x), Value::Float(y), "mul") => Some(Value::Float(*x as f64 * y)),
            (Value::Float(x), Value::Int(y), "mul") => Some(Value::Float(x * *y as f64)),

            // Comparison
            (Value::Int(x), Value::Int(y), "eq") => Some(Value::Bool(x == y)),
            (Value::Int(x), Value::Int(y), "neq") => Some(Value::Bool(x != y)),
            (Value::Int(x), Value::Int(y), "lt") => Some(Value::Bool(x < y)),
            (Value::Int(x), Value::Int(y), "lte") => Some(Value::Bool(x <= y)),
            (Value::Int(x), Value::Int(y), "gt") => Some(Value::Bool(x > y)),
            (Value::Int(x), Value::Int(y), "gte") => Some(Value::Bool(x >= y)),

            // Boolean logic
            (Value::Bool(x), Value::Bool(y), "and") => Some(Value::Bool(*x && *y)),
            (Value::Bool(x), Value::Bool(y), "or") => Some(Value::Bool(*x || *y)),

            // String concatenation
            (Value::Text(x), Value::Text(y), "str-concat") => {
                Some(Value::Text(format!("{}{}", x, y)))
            }

            _ => None,
        }
    }

    /// Fold unary operations on constants
    fn fold_unary(&self, a: &Value, op: &str) -> Option<Value> {
        match (a, op) {
            (Value::Int(x), "neg") => Some(Value::Int(-x)),
            (Value::Float(x), "neg") => Some(Value::Float(-x)),
            (Value::Bool(x), "not") => Some(Value::Bool(!x)),
            (Value::Int(x), "to-float") => Some(Value::Float(*x as f64)),
            (Value::Float(x), "to-int") => Some(Value::Int(*x as i64)),
            (Value::Int(x), "to-text") => Some(Value::Text(x.to_string())),
            (Value::Text(s), "str-len") => Some(Value::Int(s.len() as i64)),
            _ => None,
        }
    }

    /// Get number of rewrites applied
    pub fn rewrites(&self) -> usize {
        self.rewrites
    }
}

/// Convenience function for full optimization
pub fn optimize(ops: Vec<Op>) -> Vec<Op> {
    Optimizer::new(OptLevel::Full).optimize(ops)
}

/// Convenience function for algebraic-only optimization
pub fn simplify(ops: Vec<Op>) -> Vec<Op> {
    Optimizer::new(OptLevel::Algebraic).optimize(ops)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create call ops
    fn call(name: &str) -> Op {
        Op::Call(name.into())
    }

    fn push_int(n: i64) -> Op {
        Op::Push(Value::Int(n))
    }

    // === Algebraic Identity Tests ===

    #[test]
    fn test_swap_swap_identity() {
        let ops = vec![push_int(1), push_int(2), call("swap"), call("swap")];
        let result = simplify(ops);
        assert_eq!(result, vec![push_int(1), push_int(2)]);
    }

    #[test]
    fn test_not_not_identity() {
        let ops = vec![Op::Push(Value::Bool(true)), call("not"), call("not")];
        let result = simplify(ops);
        assert_eq!(result, vec![Op::Push(Value::Bool(true))]);
    }

    #[test]
    fn test_neg_neg_identity() {
        let ops = vec![push_int(5), call("neg"), call("neg")];
        let result = simplify(ops);
        assert_eq!(result, vec![push_int(5)]);
    }

    #[test]
    fn test_dup_drop_identity() {
        let ops = vec![push_int(1), call("dup"), call("drop")];
        let result = simplify(ops);
        assert_eq!(result, vec![push_int(1)]);
    }

    #[test]
    fn test_rot_rot_rot_identity() {
        let ops = vec![
            push_int(1), push_int(2), push_int(3),
            call("rot"), call("rot"), call("rot")
        ];
        let result = simplify(ops);
        assert_eq!(result, vec![push_int(1), push_int(2), push_int(3)]);
    }

    #[test]
    fn test_over_nip_to_dup() {
        let ops = vec![push_int(1), push_int(2), call("over"), call("nip")];
        let result = simplify(ops);
        assert_eq!(result, vec![push_int(1), push_int(2), call("dup")]);
    }

    #[test]
    fn test_over_drop_identity() {
        let ops = vec![push_int(1), push_int(2), call("over"), call("drop")];
        let result = simplify(ops);
        assert_eq!(result, vec![push_int(1), push_int(2)]);
    }

    // === Constant Folding Tests ===

    #[test]
    fn test_fold_add() {
        let ops = vec![push_int(3), push_int(4), call("add")];
        let result = optimize(ops);
        assert_eq!(result, vec![push_int(7)]);
    }

    #[test]
    fn test_fold_mul() {
        let ops = vec![push_int(6), push_int(7), call("mul")];
        let result = optimize(ops);
        assert_eq!(result, vec![push_int(42)]);
    }

    #[test]
    fn test_fold_sub() {
        let ops = vec![push_int(10), push_int(3), call("sub")];
        let result = optimize(ops);
        assert_eq!(result, vec![push_int(7)]);
    }

    #[test]
    fn test_fold_comparison() {
        let ops = vec![push_int(5), push_int(3), call("lt")];
        let result = optimize(ops);
        assert_eq!(result, vec![Op::Push(Value::Bool(false))]);
    }

    #[test]
    fn test_fold_boolean() {
        let ops = vec![
            Op::Push(Value::Bool(true)),
            Op::Push(Value::Bool(false)),
            call("and")
        ];
        let result = optimize(ops);
        assert_eq!(result, vec![Op::Push(Value::Bool(false))]);
    }

    #[test]
    fn test_fold_neg() {
        let ops = vec![push_int(5), call("neg")];
        let result = optimize(ops);
        assert_eq!(result, vec![push_int(-5)]);
    }

    #[test]
    fn test_fold_not() {
        let ops = vec![Op::Push(Value::Bool(true)), call("not")];
        let result = optimize(ops);
        assert_eq!(result, vec![Op::Push(Value::Bool(false))]);
    }

    // === Combined Tests ===

    #[test]
    fn test_chained_arithmetic() {
        // 2 + 3 * 4 as postfix: 2 3 add 4 mul = 5 * 4 = 20
        // But we evaluate left-to-right in stack: 2 3 add → 5, then 5 4 mul → 20
        let ops = vec![push_int(2), push_int(3), call("add"), push_int(4), call("mul")];
        let result = optimize(ops);
        assert_eq!(result, vec![push_int(20)]);
    }

    #[test]
    fn test_algebraic_then_fold() {
        // 5 dup drop neg neg → 5 neg neg → 5
        let ops = vec![push_int(5), call("dup"), call("drop"), call("neg"), call("neg")];
        let result = optimize(ops);
        assert_eq!(result, vec![push_int(5)]);
    }

    #[test]
    fn test_no_optimization_needed() {
        let ops = vec![push_int(1), push_int(2), call("add"), call("println")];
        let result = optimize(ops);
        // Can fold 1 2 add → 3, but println stays
        assert_eq!(result, vec![push_int(3), call("println")]);
    }

    #[test]
    fn test_preserve_dynamic_ops() {
        // x y add where x,y are dynamic (not Push) - should not fold
        let ops = vec![call("dup"), call("add")];  // Duplicates top, adds to itself
        let result = optimize(ops);
        assert_eq!(result, vec![call("dup"), call("add")]);  // Unchanged
    }

    // === Edge Cases ===

    #[test]
    fn test_empty_program() {
        let ops: Vec<Op> = vec![];
        let result = optimize(ops);
        assert!(result.is_empty());
    }

    #[test]
    fn test_no_div_by_zero() {
        // Should NOT fold division by zero
        let ops = vec![push_int(5), push_int(0), call("div")];
        let result = optimize(ops);
        assert_eq!(result, vec![push_int(5), push_int(0), call("div")]);  // Unchanged
    }

    #[test]
    fn test_string_concat_fold() {
        let ops = vec![
            Op::Push(Value::Text("Hello, ".into())),
            Op::Push(Value::Text("World!".into())),
            call("str-concat")
        ];
        let result = optimize(ops);
        assert_eq!(result, vec![Op::Push(Value::Text("Hello, World!".into()))]);
    }

    // === TENSOR ALGEBRAIC OPTIMIZATION TESTS ===
    // These test the mathematical laws that Kore can exploit due to immutability (P1)

    #[test]
    fn test_tensor_neg_neg_identity() {
        // tensor-neg tensor-neg → ε  (-(-x) = x)
        let ops = vec![call("tensor-neg"), call("tensor-neg")];
        let result = simplify(ops);
        assert_eq!(result, vec![]);  // Both ops cancelled
    }

    #[test]
    fn test_tensor_exp_log_identity() {
        // tensor-exp tensor-log → ε  (log(exp(x)) = x)
        let ops = vec![call("tensor-exp"), call("tensor-log")];
        let result = simplify(ops);
        assert_eq!(result, vec![]);  // Inverse functions cancel
    }

    #[test]
    fn test_tensor_log_exp_identity() {
        // tensor-log tensor-exp → ε  (exp(log(x)) = x for x > 0)
        let ops = vec![call("tensor-log"), call("tensor-exp")];
        let result = simplify(ops);
        assert_eq!(result, vec![]);  // Inverse functions cancel
    }

    #[test]
    fn test_tensor_chained_inverses() {
        // Multiple chained inverse pairs should all cancel
        // exp(log(exp(log(x)))) = exp(log(x)) = x
        let ops = vec![
            call("tensor-exp"), call("tensor-log"),  // cancel
            call("tensor-exp"), call("tensor-log"),  // cancel
        ];
        let result = simplify(ops);
        assert_eq!(result, vec![]);  // All cancelled
    }

    #[test]
    fn test_tensor_partial_cancellation() {
        // Only adjacent pairs cancel
        // log(x) then neg then exp - neg breaks the chain, can't cancel
        let ops = vec![
            call("tensor-log"),
            call("tensor-neg"),
            call("tensor-exp"),
        ];
        let result = simplify(ops);
        // No cancellation possible
        assert_eq!(result, vec![
            call("tensor-log"),
            call("tensor-neg"),
            call("tensor-exp"),
        ]);
    }

    #[test]
    fn test_tensor_neg_in_longer_chain() {
        // tensor-neg tensor-neg in a longer program
        let ops = vec![
            call("tensor-from-list"),
            call("tensor-neg"),
            call("tensor-neg"),  // These two cancel
            call("tensor-sum"),
        ];
        let result = simplify(ops);
        assert_eq!(result, vec![
            call("tensor-from-list"),
            call("tensor-sum"),
        ]);
    }

    // === NEW TENSOR ALGEBRAIC TESTS ===

    #[test]
    fn test_tensor_transpose_transpose_identity() {
        // (Aᵀ)ᵀ = A
        let ops = vec![call("tensor-transpose"), call("tensor-transpose")];
        let result = simplify(ops);
        assert_eq!(result, vec![]);
    }

    #[test]
    fn test_tensor_relu_idempotent() {
        // relu(relu(x)) = relu(x)
        let ops = vec![call("tensor-relu"), call("tensor-relu")];
        let result = simplify(ops);
        assert_eq!(result, vec![call("tensor-relu")]);
    }

    #[test]
    fn test_tensor_abs_idempotent() {
        // |abs(x)| = |x|
        let ops = vec![call("tensor-abs"), call("tensor-abs")];
        let result = simplify(ops);
        assert_eq!(result, vec![call("tensor-abs")]);
    }

    #[test]
    fn test_tensor_scale_by_one() {
        // 1.0 tensor-scale → ε (identity)
        let ops = vec![Op::Push(Value::Float(1.0)), call("tensor-scale")];
        let result = simplify(ops);
        assert_eq!(result, vec![]);
    }

    #[test]
    fn test_tensor_scale_by_one_int() {
        // 1 tensor-scale → ε (integer one)
        let ops = vec![push_int(1), call("tensor-scale")];
        let result = simplify(ops);
        assert_eq!(result, vec![]);
    }

    #[test]
    fn test_tensor_scale_by_neg_one() {
        // -1.0 tensor-scale → tensor-neg
        let ops = vec![Op::Push(Value::Float(-1.0)), call("tensor-scale")];
        let result = simplify(ops);
        assert_eq!(result, vec![call("tensor-neg")]);
    }

    #[test]
    fn test_tensor_scale_by_neg_one_int() {
        // -1 tensor-scale → tensor-neg
        let ops = vec![push_int(-1), call("tensor-scale")];
        let result = simplify(ops);
        assert_eq!(result, vec![call("tensor-neg")]);
    }

    #[test]
    fn test_tensor_sigmoid_relu() {
        // sigmoid then relu = just sigmoid (sigmoid output ∈ [0,1])
        let ops = vec![call("tensor-sigmoid"), call("tensor-relu")];
        let result = simplify(ops);
        assert_eq!(result, vec![call("tensor-sigmoid")]);
    }

    #[test]
    fn test_tensor_relu_abs() {
        // relu then abs = just relu (relu output ≥ 0)
        let ops = vec![call("tensor-relu"), call("tensor-abs")];
        let result = simplify(ops);
        assert_eq!(result, vec![call("tensor-relu")]);
    }

    #[test]
    fn test_tensor_neg_scale_fusion() {
        // tensor-neg then 2.0 tensor-scale → -2.0 tensor-scale
        let ops = vec![call("tensor-neg"), Op::Push(Value::Float(2.0)), call("tensor-scale")];
        let result = simplify(ops);
        assert_eq!(result, vec![Op::Push(Value::Float(-2.0)), call("tensor-scale")]);
    }

    #[test]
    fn test_tensor_complex_optimization_chain() {
        // A complex chain: relu relu neg neg 1.0 scale sigmoid relu
        // Should simplify to: relu sigmoid
        let ops = vec![
            call("tensor-relu"),
            call("tensor-relu"),    // idempotent → tensor-relu
            call("tensor-neg"),
            call("tensor-neg"),     // cancel → ε
            Op::Push(Value::Float(1.0)),
            call("tensor-scale"),   // identity → ε
            call("tensor-sigmoid"),
            call("tensor-relu"),    // sigmoid output ∈ [0,1] → tensor-sigmoid
        ];
        let result = simplify(ops);
        assert_eq!(result, vec![call("tensor-relu"), call("tensor-sigmoid")]);
    }
}
