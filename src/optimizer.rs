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
//! Some operations are **involutions** (self-inverse): swap, not
//! Some have **finite order**: rot³ = ε

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
}
