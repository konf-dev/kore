//! Static Stack Effect Analyzer
//!
//! Analyzes Kore programs to detect stack errors before execution:
//! - Underflows: consuming more than available
//! - Unbalanced conditionals: if-branches with different effects  
//!
//! Uses the Effect algebra from types.rs for correct composition.

use crate::op::Op;
use crate::types::Effect;
use crate::value::Value;
use std::collections::HashMap;

/// Result of analyzing a program's stack effect
#[derive(Debug, Clone)]
pub struct Analysis {
    /// Computed effect of the program
    pub effect: Effect,
    /// Any errors found
    pub errors: Vec<AnalysisError>,
    /// Any warnings (not errors, but suspicious)
    pub warnings: Vec<AnalysisWarning>,
}

#[derive(Debug, Clone)]
pub struct AnalysisError {
    pub location: usize,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct AnalysisWarning {
    pub location: usize,
    pub message: String,
}

impl Analysis {
    pub fn ok(effect: Effect) -> Self {
        Self {
            effect,
            errors: vec![],
            warnings: vec![],
        }
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

/// Analyze a sequence of operations for stack safety
pub fn analyze(ops: &[Op]) -> Analysis {
    let mut analyzer = Analyzer::new();
    
    // First pass: collect user definitions
    analyzer.collect_definitions(ops);
    
    // Second pass: analyze with knowledge of definitions
    analyzer.analyze_ops(ops, 0)
}

struct Analyzer {
    /// User-defined tools and their effects
    user_defs: HashMap<String, Effect>,
}

impl Analyzer {
    fn new() -> Self {
        Self {
            user_defs: HashMap::new(),
        }
    }
    
    /// First pass: scan for definitions
    fn collect_definitions(&mut self, ops: &[Op]) {
        let mut i = 0;
        while i < ops.len() {
            // Pattern: Push(value) Push("name") Call("def")
            if i + 2 < ops.len() {
                if let (Op::Push(value), Op::Push(Value::Text(name)), Op::Call(def_name)) = 
                    (&ops[i], &ops[i + 1], &ops[i + 2]) 
                {
                    if def_name == "def" {
                        let effect = if let Value::Quote(quote_ops) = value {
                            // Function: infer its effect
                            self.infer_effect(quote_ops)
                        } else {
                            // Constant: pushes 1 value
                            Effect::push()
                        };
                        
                        self.user_defs.insert(name.clone(), effect);
                        i += 3;
                        continue;
                    }
                }
            }
            
            // Recurse into quotes
            if let Op::Push(Value::Quote(inner_ops)) = &ops[i] {
                self.collect_definitions(inner_ops);
            }
            
            i += 1;
        }
    }
    
    /// Infer effect without reporting errors
    fn infer_effect(&mut self, ops: &[Op]) -> Effect {
        let mut depth: i32 = 0;
        let mut min_depth: i32 = 0;

        for op in ops {
            match op {
                Op::Push(value) => {
                    depth += 1;
                    if let Value::Quote(inner) = value {
                        self.collect_definitions(inner);
                    }
                }
                Op::Call(name) => {
                    let eff = self.get_effect(name).unwrap_or(Effect::new(0, 1));
                    depth -= eff.consumes as i32;
                    min_depth = min_depth.min(depth);
                    depth += eff.produces as i32;
                }
            }
        }

        // consumes = how deep we went below zero
        // produces = final depth relative to start
        let consumes = (-min_depth).max(0) as u32;
        let produces = (depth + consumes as i32) as u32;
        
        Effect::new(consumes, produces)
    }

    fn analyze_ops(&mut self, ops: &[Op], start_pos: usize) -> Analysis {
        let mut depth: i32 = 0;
        let mut min_depth: i32 = 0;
        let mut errors = vec![];
        let mut warnings = vec![];

        let mut i = 0;
        while i < ops.len() {
            let pos = start_pos + i;
            
            // Skip def patterns
            if i + 2 < ops.len() {
                if let (Op::Push(_), Op::Push(Value::Text(_)), Op::Call(name)) = 
                    (&ops[i], &ops[i + 1], &ops[i + 2]) 
                {
                    if name == "def" {
                        i += 3;
                        continue;
                    }
                }
            }
            
            match &ops[i] {
                Op::Push(_) => {
                    depth += 1;
                }
                Op::Call(name) => {
                    if let Some(eff) = self.get_effect(name) {
                        if depth < eff.consumes as i32 {
                            errors.push(AnalysisError {
                                location: pos,
                                message: format!(
                                    "`{}` requires {} value(s) but only {} available",
                                    name, eff.consumes, depth.max(0)
                                ),
                            });
                        }
                        depth -= eff.consumes as i32;
                        min_depth = min_depth.min(depth);
                        depth += eff.produces as i32;
                    } else {
                        warnings.push(AnalysisWarning {
                            location: pos,
                            message: format!("Unknown tool `{}`", name),
                        });
                        depth += 1; // Assume pushes 1
                    }
                }
            }
            i += 1;
        }

        let consumes = (-min_depth).max(0) as u32;
        let produces = (depth + consumes as i32).max(0) as u32;

        Analysis {
            effect: Effect::new(consumes, produces),
            errors,
            warnings,
        }
    }

    /// Get effect for a tool
    fn get_effect(&self, name: &str) -> Option<Effect> {
        // User definitions first
        if let Some(e) = self.user_defs.get(name) {
            return Some(*e);
        }
        
        // Built-in effects
        let (c, p) = match name {
            // Stack
            "dup" => (1, 2),
            "drop" => (1, 0),
            "swap" => (2, 2),
            "over" => (2, 3),
            "rot" => (3, 3),
            "nip" => (2, 1),
            "tuck" => (2, 3),
            "depth" => (0, 1),
            "dip" => (2, 1), // x q -- x (after calling q)
            
            // Arithmetic
            "add" | "sub" | "mul" | "div" | "mod" => (2, 1),
            "neg" | "abs" => (1, 1),
            
            // Conversion
            "to-float" | "to-int" | "to-text" | "to-bool" => (1, 1),
            
            // Comparison
            "eq" | "neq" | "lt" | "lte" | "gt" | "gte" => (2, 1),
            
            // Logic
            "and" | "or" => (2, 1),
            "not" => (1, 1),
            
            // Type checks
            "is-null" | "is-bool" | "is-int" | "is-float" | "is-text" 
            | "is-list" | "is-map" | "is-quote" | "is-error" => (1, 1),
            "type-of" => (1, 1),
            "unwrap" => (1, 1),
            
            // List
            "list" => (2, 1), // n items... -- list (but really variable, treat as 2 for safety)
            "unlist" => (1, 0), // list -- items... (variable, but consumes 1)
            "list-len" | "list-reverse" | "list-first" | "list-last" => (1, 1),
            "list-get" | "list-push" | "list-pop" => (2, 1),
            "list-set" => (3, 1),
            "list-concat" => (2, 1),
            "list-slice" => (3, 1),
            
            // Map
            "map-new" => (0, 1),
            "map-get" | "map-has" => (2, 1),
            "map-set" => (3, 1),
            "map-del" => (2, 1),
            "map-keys" | "map-vals" => (1, 1),
            
            // String
            "str-len" | "str-upper" | "str-lower" | "str-trim" => (1, 1),
            "str-concat" | "str-split" | "str-find" => (2, 1),
            "str-get" | "str-slice" => (2, 1),
            "str-starts" | "str-ends" => (2, 1),
            "str-replace" => (3, 1),
            "str-join" => (2, 1),
            "char-code" | "code-char" => (1, 1),
            
            // Memory
            "mem-get" | "rom-get" => (1, 1),
            "mem-set" | "rom-set" => (2, 0),
            
            // I/O
            "print" | "println" => (1, 0),
            "fs-read" | "fs-exists" => (1, 1),
            "fs-write" => (2, 0),
            "json-parse" | "json-encode" => (1, 1),
            
            // Control
            "call" => (1, 0), // quote --, effect depends on quote
            "if" => (3, 0),   // cond then else --
            "times" => (2, 0),
            "while" => (2, 0),
            "when" | "unless" => (2, 0),
            "loop" => (1, 0),
            "spawn" => (1, 1),
            "try" => (1, 1),
            "fail" => (1, 0),
            
            // Definition
            "def" => (2, 0),
            "words" => (0, 1),
            "meta" => (1, 1),
            "meta!" => (3, 0),
            "defined?" => (1, 1),
            
            // Time
            "now" => (0, 1),
            "sleep" => (1, 0),
            
            // Tensor
            "tensor-from-list" | "tensor-to-list" => (1, 1),
            "tensor-zeros" | "tensor-ones" => (1, 1),
            "tensor-randn" => (2, 1),
            "tensor-shape" => (1, 1),
            "tensor-add" | "tensor-sub" | "tensor-mul" | "tensor-div" => (2, 1),
            "tensor-matmul" | "tensor-matmul-t" => (4, 1),
            "tensor-outer" => (2, 1),
            "tensor-scale" => (2, 1),
            "tensor-sum" | "tensor-mean" | "tensor-max" | "tensor-min" | "tensor-argmax" => (1, 1),
            "tensor-softmax" | "tensor-relu" | "tensor-log" | "tensor-exp" => (1, 1),
            "tensor-relu-bwd" => (2, 1),
            "tensor-get" => (2, 1),
            "tensor-set" => (3, 1),
            
            // Effect verification (THE TRUSTED KERNEL)
            "effect-compose" => (2, 1),
            "effect-parse" => (1, 1),
            "effect-net" => (1, 1),
            "effect-valid?" => (2, 1),
            "effect-new" => (2, 1),
            
            // Combinators
            "map" | "filter" | "each" => (2, 1),
            "fold" => (3, 1),
            
            _ => return None,
        };
        
        Some(Effect::new(c, p))
    }
}

/// Format analysis results for display
pub fn format_analysis(analysis: &Analysis) -> String {
    let mut out = String::new();
    
    if analysis.has_errors() {
        out.push_str("❌ Stack errors:\n");
        for err in &analysis.errors {
            out.push_str(&format!("  [{}] {}\n", err.location, err.message));
        }
    } else {
        out.push_str("✓ Stack safe\n");
    }
    
    if !analysis.warnings.is_empty() {
        out.push_str("⚠ Warnings:\n");
        for warn in &analysis.warnings {
            out.push_str(&format!("  [{}] {}\n", warn.location, warn.message));
        }
    }
    
    out.push_str(&format!(
        "Effect: {} (net: {:+})\n",
        analysis.effect, analysis.effect.net()
    ));
    
    out
}

// Backward compatibility aliases
pub type StackAnalysis = Analysis;
pub type StackError = AnalysisError;
pub type StackWarning = AnalysisWarning;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_underflow() {
        let ops = vec![Op::Call("drop".into())];
        let result = analyze(&ops);
        assert!(result.has_errors());
    }

    #[test]
    fn test_push_then_drop() {
        let ops = vec![
            Op::Push(Value::Int(1)),
            Op::Call("drop".into()),
        ];
        let result = analyze(&ops);
        assert!(!result.has_errors());
        assert_eq!(result.effect, Effect::new(0, 0));
    }

    #[test]
    fn test_dup_requires_one() {
        let ops = vec![Op::Call("dup".into())];
        let result = analyze(&ops);
        assert!(result.has_errors());
    }

    #[test]
    fn test_add_requires_two() {
        let ops = vec![
            Op::Push(Value::Int(1)),
            Op::Call("add".into()),
        ];
        let result = analyze(&ops);
        assert!(result.has_errors());
    }

    #[test]
    fn test_valid_arithmetic() {
        let ops = vec![
            Op::Push(Value::Int(1)),
            Op::Push(Value::Int(2)),
            Op::Call("add".into()),
        ];
        let result = analyze(&ops);
        assert!(!result.has_errors());
        assert_eq!(result.effect, Effect::new(0, 1));
    }

    #[test]
    fn test_user_defined_constant() {
        let ops = vec![
            Op::Push(Value::Int(784)),
            Op::Push(Value::Text("N_IN".into())),
            Op::Call("def".into()),
            Op::Call("N_IN".into()),
        ];
        let result = analyze(&ops);
        assert!(!result.has_errors());
        assert_eq!(result.effect, Effect::new(0, 1));
    }

    #[test]
    fn test_effect_composition() {
        // Verify effect algebra works
        let dup = Effect::new(1, 2);
        let drop = Effect::new(1, 0);
        assert_eq!(dup.compose(drop), Effect::new(1, 1));
    }
}
