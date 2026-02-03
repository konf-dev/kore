//! Verification Primitives - The Trusted Mathematical Kernel
//!
//! These primitives expose the effect algebra to Kore programs.
//! They are the ONLY trusted Rust code for verification.
//! Everything else (analyzer, checker) can be written in Kore.
//!
//! | Tool | Signature | Description |
//! |------|-----------|-------------|
//! | effect-compose | (e1 e2 -- e3) | Compose two effects |
//! | effect-parse | (sig -- effect) | Parse effect from signature |
//! | effect-net | (effect -- n) | Get net stack change |
//! | effect-valid? | (effect depth -- bool) | Check if effect valid at depth |
//! | effect-infer | (quote -- analysis) | Static analysis of quote |
//! | io-effects | (quote -- list) | Get IO effects of quote |
//! | pure? | (quote -- bool) | Check if quote is pure |
//! | optimize | (quote -- quote') | Algebraically optimize code |
//! | simplify | (quote -- quote') | Apply only algebraic identities |

use crate::analyzer;
use crate::optimizer;
use crate::context::{Context, Dictionary};
use crate::error::Error;
use crate::stack::Stack;
use crate::tool::Tool;
use crate::types::Effect;
use crate::value::Value;
use indexmap::IndexMap;

/// Register verification primitives
pub fn register(dict: &mut Dictionary) {
    // effect-compose: The mathematical core
    dict.register(Tool::native(
        "effect-compose",
        "(e1:Map e2:Map -- e3:Map)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let e2 = effect_from_value(stack.pop()?)?;
                let e1 = effect_from_value(stack.pop()?)?;
                
                let e3 = e1.compose(e2);
                
                stack.push(effect_to_value(e3))?;
                Ok((stack, ctx))
            })
        },
    ).with_doc("Compose two stack effects using the formula: compose((a,b), (c,d)) = if b >= c then (a, b-c+d) else (a+c-b, d)"));

    // effect-parse: Parse signature string
    dict.register(Tool::native(
        "effect-parse",
        "(sig:Text -- effect:Map)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let v = stack.pop()?;
                let sig = v.as_text()?;
                let effect = Effect::parse(sig)
                    .map_err(|e| Error::Runtime(format!("effect-parse: {}", e)))?;
                
                stack.push(effect_to_value(effect))?;
                Ok((stack, ctx))
            })
        },
    ).with_doc("Parse effect signature like '(a b -- sum)' into effect map"));

    // effect-net: Get net stack change
    dict.register(Tool::native(
        "effect-net",
        "(effect:Map -- n:Int)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let effect = effect_from_value(stack.pop()?)?;
                stack.push(Value::Int(effect.net() as i64))?;
                Ok((stack, ctx))
            })
        },
    ).with_doc("Get net stack change: produces - consumes"));

    // effect-valid?: Check if effect is valid at given depth
    dict.register(Tool::native(
        "effect-valid?",
        "(effect:Map depth:Int -- valid:Bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let depth = stack.pop()?.as_int()? as u32;
                let effect = effect_from_value(stack.pop()?)?;
                
                stack.push(Value::Bool(effect.is_valid_at(depth)))?;
                Ok((stack, ctx))
            })
        },
    ).with_doc("Check if effect can execute with given stack depth"));

    // effect-new: Create effect from consumes/produces
    dict.register(Tool::native(
        "effect-new",
        "(consumes:Int produces:Int -- effect:Map)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let produces = stack.pop()?.as_int()? as u32;
                let consumes = stack.pop()?.as_int()? as u32;
                
                let effect = Effect::new(consumes, produces);
                stack.push(effect_to_value(effect))?;
                Ok((stack, ctx))
            })
        },
    ).with_doc("Create effect from consume/produce counts"));

    // effect-infer: Static analysis of a quote
    dict.register(Tool::native(
        "effect-infer",
        "(code:Quote -- analysis:Map)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let ops = stack.pop()?.into_quote()?;
                let analysis = analyzer::analyze(&ops);
                
                // Build result map
                let mut result = IndexMap::new();
                result.insert("effect".into(), effect_to_value(analysis.effect));
                result.insert("io".into(), Value::List(
                    analysis.io_effects.to_list().iter()
                        .map(|s| Value::Text(s.to_string()))
                        .collect()
                ));
                result.insert("pure".into(), Value::Bool(analysis.is_pure()));
                result.insert("errors".into(), Value::List(
                    analysis.errors.iter()
                        .map(|e| Value::Text(e.message.clone()))
                        .collect()
                ));
                result.insert("warnings".into(), Value::List(
                    analysis.warnings.iter()
                        .map(|w| Value::Text(w.message.clone()))
                        .collect()
                ));
                result.insert("safe".into(), Value::Bool(!analysis.has_errors()));
                
                stack.push(Value::Map(result))?;
                Ok((stack, ctx))
            })
        },
    ).with_doc("Statically analyze a quote: stack effect, IO effects, safety"));

    // io-effects: Get list of IO effects for a quote
    dict.register(Tool::native(
        "io-effects",
        "(code:Quote -- effects:List)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let ops = stack.pop()?.into_quote()?;
                let analysis = analyzer::analyze(&ops);
                
                let effects: Vec<Value> = analysis.io_effects.to_list().iter()
                    .map(|s| Value::Text(s.to_string()))
                    .collect();
                
                stack.push(Value::List(effects))?;
                Ok((stack, ctx))
            })
        },
    ).with_doc("Get IO effects as a list: fs, net, spawn, time, io, env, exec, mem"));

    // pure?: Check if a quote is pure (no IO effects)
    dict.register(Tool::native(
        "pure?",
        "(code:Quote -- is_pure:Bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let ops = stack.pop()?.into_quote()?;
                let analysis = analyzer::analyze(&ops);
                
                stack.push(Value::Bool(analysis.is_pure()))?;
                Ok((stack, ctx))
            })
        },
    ).with_doc("Check if code is pure (no IO effects)"));

    // optimize: Full algebraic optimization + constant folding
    dict.register(Tool::native(
        "optimize",
        "(code:Quote -- optimized:Quote)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let ops = stack.pop()?.into_quote()?;
                let optimized = optimizer::optimize(ops);
                
                stack.push(Value::Quote(optimized))?;
                Ok((stack, ctx))
            })
        },
    ).with_doc("Algebraically optimize code: constant folding, identity elimination"));

    // simplify: Only algebraic identities (no constant folding)
    dict.register(Tool::native(
        "simplify",
        "(code:Quote -- simplified:Quote)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let ops = stack.pop()?.into_quote()?;
                let simplified = optimizer::simplify(ops);
                
                stack.push(Value::Quote(simplified))?;
                Ok((stack, ctx))
            })
        },
    ).with_doc("Apply algebraic identities only: swap swap → ε, rot rot rot → ε"));
}

/// Convert Value (Map) to Effect
fn effect_from_value(v: Value) -> Result<Effect, Error> {
    match v {
        Value::Map(m) => {
            let consumes = m.get("consumes")
                .and_then(|v| v.as_int().ok())
                .ok_or_else(|| Error::Runtime("effect missing 'consumes' field".into()))?;
            let produces = m.get("produces")
                .and_then(|v| v.as_int().ok())
                .ok_or_else(|| Error::Runtime("effect missing 'produces' field".into()))?;
            Ok(Effect::new(consumes as u32, produces as u32))
        }
        _ => Err(Error::Runtime(format!("expected effect map, got {:?}", v))),
    }
}

/// Convert Effect to Value (Map)
fn effect_to_value(e: Effect) -> Value {
    let mut m = IndexMap::new();
    m.insert("consumes".into(), Value::Int(e.consumes as i64));
    m.insert("produces".into(), Value::Int(e.produces as i64));
    m.insert("net".into(), Value::Int(e.net() as i64));
    Value::Map(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effect_roundtrip() {
        let e = Effect::new(2, 3);
        let v = effect_to_value(e);
        let e2 = effect_from_value(v).unwrap();
        assert_eq!(e, e2);
    }
}
