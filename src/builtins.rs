//! Built-in tools - the minimal set that makes kore a complete language
//!
//! These are language primitives, not library functions.
//! Each tool does exactly one thing. LLMs are first-class citizens.
//!
//! ## Execution (5)
//! - `call`: Run a quote
//! - `try`: Run a quote, capture errors as Error values
//! - `if`: Conditional execution
//! - `loop`: Repeat until false on stack
//! - `def`: Define a new tool from a quote
//!
//! ## Error inspection (2)
//! - `is-error`: Check if a value is an Error
//! - `unwrap`: Extract value, or stop if Error
//!
//! ## Stack manipulation (5)
//! - `dup`: Duplicate top value
//! - `drop`: Remove top value
//! - `swap`: Swap top two values
//! - `over`: Copy second value to top
//! - `rot`: Rotate top three values
//!
//! ## Arithmetic (6)
//! - `add`, `sub`, `mul`, `div`, `mod`, `neg`
//!
//! ## Comparison (6)
//! - `eq`, `neq`, `lt`, `gt`, `le`, `ge`
//!
//! ## Logic (3)
//! - `and`, `or`, `not`
//!
//! ## String (13)
//! - `str-len`, `str-get`, `str-slice`, `str-split`, `str-join`
//! - `str-concat`, `str-trim`, `str-find`, `str-starts`, `str-ends`
//! - `str-replace`, `char-code`, `code-char`
//!
//! ## List (10)
//! - `list-len`, `list-get`, `list-set`, `list-push`, `list-pop`
//! - `list-slice`, `list-concat`, `list-reverse`, `list-empty`, `collect`
//!
//! ## Map (7)
//! - `map-get`, `map-set`, `map-has`, `map-del`
//! - `map-keys`, `map-vals`, `map-empty`
//!
//! ## Type (9)
//! - `type-of`, `is-null`, `is-bool`, `is-int`, `is-float`
//! - `is-text`, `is-list`, `is-map`, `is-quote`
//!
//! ## Conversion (5)
//! - `to-int`, `to-float`, `to-text`, `to-bool`, `to-list`
//!
//! ## Combinators (6)
//! - `map`, `filter`, `fold`, `each`, `times`, `while`
//!
//! **Total: 77 primitives**

use crate::context::Context;
use crate::executor::execute;
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::{ErrorValue, Value};

/// Register all built-in tools
pub async fn register_builtins(ctx: &mut Context) {
    let mut dict = ctx.dict.write().await;

    // === Execution ===

    // call: (quote -- ...) - run a quote
    dict.register(Tool::native("call", "(q:Quote -- ...)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let quote = stack.pop()?.into_quote()?;
            execute(&quote, stack, ctx).await
        })
    }));

    // try: (quote -- value-or-error) - run a quote, capture errors
    dict.register(Tool::native("try", "(q:Quote -- result:Any)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let quote = stack.pop()?.into_quote()?;
            
            match execute(&quote, stack.clone(), ctx.clone()).await {
                Ok((result_stack, new_ctx)) => Ok((result_stack, new_ctx)),
                Err(error) => {
                    // Convert error to Error value
                    stack.push(Value::Error(Box::new(ErrorValue {
                        code: error.code().to_string(),
                        message: error.to_string(),
                    })))?;
                    Ok((stack, ctx))
                }
            }
        })
    }));

    // === Error inspection ===

    // is-error: (value -- bool) - check if value is an Error
    dict.register(Tool::native("is-error", "(v:Any -- result:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let value = stack.pop()?;
            let is_error = matches!(value, Value::Error(_));
            stack.push(Value::Bool(is_error))?;
            Ok((stack, ctx))
        })
    }));

    // unwrap: (value-or-error -- value) - extract value, or stop if Error
    dict.register(Tool::native("unwrap", "(v:Any -- result:Any)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let value = stack.pop()?;
            match value {
                Value::Error(e) => Err(crate::error::Error::Custom {
                    code: e.code,
                    message: e.message,
                }),
                other => {
                    stack.push(other)?;
                    Ok((stack, ctx))
                }
            }
        })
    }));

    // === Stack manipulation ===

    // dup: (a -- a a) - duplicate top value
    dict.register(Tool::native("dup", "(a:Any -- a:Any a:Any)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let value = stack.pop()?;
            stack.push(value.clone())?;
            stack.push(value)?;
            Ok((stack, ctx))
        })
    }));

    // drop: (a -- ) - remove top value
    dict.register(Tool::native("drop", "(a:Any -- )", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            stack.pop()?;
            Ok((stack, ctx))
        })
    }));

    // swap: (a b -- b a) - swap top two values
    dict.register(Tool::native("swap", "(a:Any b:Any -- b:Any a:Any)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?;
            let a = stack.pop()?;
            stack.push(b)?;
            stack.push(a)?;
            Ok((stack, ctx))
        })
    }));

    // over: (a b -- a b a) - copy second value to top
    dict.register(Tool::native("over", "(a:Any b:Any -- a:Any b:Any a:Any)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?;
            let a = stack.pop()?;
            stack.push(a.clone())?;
            stack.push(b)?;
            stack.push(a)?;
            Ok((stack, ctx))
        })
    }));

    // rot: (a b c -- b c a) - rotate top three values
    dict.register(Tool::native("rot", "(a:Any b:Any c:Any -- b:Any c:Any a:Any)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let c = stack.pop()?;
            let b = stack.pop()?;
            let a = stack.pop()?;
            stack.push(b)?;
            stack.push(c)?;
            stack.push(a)?;
            Ok((stack, ctx))
        })
    }));

    // === Control Flow ===

    // if: (condition then-quote else-quote -- ...) - conditional execution
    dict.register(Tool::native("if", "(cond:Bool then:Quote else:Quote -- ...)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let else_quote = stack.pop()?.into_quote()?;
            let then_quote = stack.pop()?.into_quote()?;
            let condition = stack.pop()?;
            
            let ops = if condition.is_truthy() {
                then_quote
            } else {
                else_quote
            };
            execute(&ops, stack, ctx).await
        })
    }));

    // loop: (quote -- ...) - repeat until false on stack
    dict.register(Tool::native("loop", "(body:Quote -- ...)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let quote = stack.pop()?.into_quote()?;
            
            loop {
                let (new_stack, _) = execute(&quote, stack, ctx.clone()).await?;
                stack = new_stack;
                
                // Check condition on top of stack
                let condition = stack.pop()?;
                if !condition.is_truthy() {
                    break;
                }
            }
            Ok((stack, ctx))
        })
    }));

    // def: (name quote -- ) - define a new tool
    dict.register(Tool::native("def", "(name:Text body:Quote -- )", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let body = stack.pop()?.into_quote()?;
            let name = stack.pop()?.into_text()?;
            
            // Create composed tool from quote
            let tool = Tool::composed(&name, None, body);
            
            // Register in dictionary
            let mut dict = ctx.dict.write().await;
            dict.register(tool);
            drop(dict);
            
            Ok((stack, ctx))
        })
    }));

    // === Arithmetic ===
    // Each operates on numbers (Int or Float). Mixed types promote to Float.

    // add: (a b -- a+b)
    dict.register(Tool::native("add", "(a:Num b:Num -- c:Num)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?;
            let a = stack.pop()?;
            let result = match (&a, &b) {
                (Value::Int(x), Value::Int(y)) => Value::Int(x.wrapping_add(*y)),
                (Value::Float(x), Value::Float(y)) => Value::Float(x + y),
                (Value::Int(x), Value::Float(y)) => Value::Float(*x as f64 + y),
                (Value::Float(x), Value::Int(y)) => Value::Float(x + *y as f64),
                _ => return Err(crate::error::Error::type_error("Num", &a)),
            };
            stack.push(result)?;
            Ok((stack, ctx))
        })
    }));

    // sub: (a b -- a-b)
    dict.register(Tool::native("sub", "(a:Num b:Num -- c:Num)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?;
            let a = stack.pop()?;
            let result = match (&a, &b) {
                (Value::Int(x), Value::Int(y)) => Value::Int(x.wrapping_sub(*y)),
                (Value::Float(x), Value::Float(y)) => Value::Float(x - y),
                (Value::Int(x), Value::Float(y)) => Value::Float(*x as f64 - y),
                (Value::Float(x), Value::Int(y)) => Value::Float(x - *y as f64),
                _ => return Err(crate::error::Error::type_error("Num", &a)),
            };
            stack.push(result)?;
            Ok((stack, ctx))
        })
    }));

    // mul: (a b -- a*b)
    dict.register(Tool::native("mul", "(a:Num b:Num -- c:Num)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?;
            let a = stack.pop()?;
            let result = match (&a, &b) {
                (Value::Int(x), Value::Int(y)) => Value::Int(x.wrapping_mul(*y)),
                (Value::Float(x), Value::Float(y)) => Value::Float(x * y),
                (Value::Int(x), Value::Float(y)) => Value::Float(*x as f64 * y),
                (Value::Float(x), Value::Int(y)) => Value::Float(x * *y as f64),
                _ => return Err(crate::error::Error::type_error("Num", &a)),
            };
            stack.push(result)?;
            Ok((stack, ctx))
        })
    }));

    // div: (a b -- a/b)
    dict.register(Tool::native("div", "(a:Num b:Num -- c:Num)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?;
            let a = stack.pop()?;
            let result = match (&a, &b) {
                (Value::Int(_), Value::Int(0)) => return Err(crate::error::Error::DivisionByZero),
                (Value::Int(x), Value::Int(y)) => Value::Int(x / y),
                (Value::Float(x), Value::Float(y)) => Value::Float(x / y),
                (Value::Int(x), Value::Float(y)) => Value::Float(*x as f64 / y),
                (Value::Float(x), Value::Int(y)) => Value::Float(x / *y as f64),
                _ => return Err(crate::error::Error::type_error("Num", &a)),
            };
            stack.push(result)?;
            Ok((stack, ctx))
        })
    }));

    // mod: (a b -- a%b)
    dict.register(Tool::native("mod", "(a:Int b:Int -- c:Int)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?.into_int()?;
            let a = stack.pop()?.into_int()?;
            if b == 0 {
                return Err(crate::error::Error::DivisionByZero);
            }
            stack.push(Value::Int(a % b))?;
            Ok((stack, ctx))
        })
    }));

    // neg: (a -- -a)
    dict.register(Tool::native("neg", "(a:Num -- b:Num)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let a = stack.pop()?;
            let result = match a {
                Value::Int(x) => Value::Int(-x),
                Value::Float(x) => Value::Float(-x),
                _ => return Err(crate::error::Error::type_error("Num", &a)),
            };
            stack.push(result)?;
            Ok((stack, ctx))
        })
    }));

    // === Comparison ===
    // Returns Bool. Works on any comparable types.

    // eq: (a b -- a==b)
    dict.register(Tool::native("eq", "(a:Any b:Any -- c:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?;
            let a = stack.pop()?;
            stack.push(Value::Bool(a == b))?;
            Ok((stack, ctx))
        })
    }));

    // neq: (a b -- a!=b)
    dict.register(Tool::native("neq", "(a:Any b:Any -- c:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?;
            let a = stack.pop()?;
            stack.push(Value::Bool(a != b))?;
            Ok((stack, ctx))
        })
    }));

    // lt: (a b -- a<b)
    dict.register(Tool::native("lt", "(a:Num b:Num -- c:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?.as_num()?;
            let a = stack.pop()?.as_num()?;
            stack.push(Value::Bool(a < b))?;
            Ok((stack, ctx))
        })
    }));

    // gt: (a b -- a>b)
    dict.register(Tool::native("gt", "(a:Num b:Num -- c:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?.as_num()?;
            let a = stack.pop()?.as_num()?;
            stack.push(Value::Bool(a > b))?;
            Ok((stack, ctx))
        })
    }));

    // le: (a b -- a<=b)
    dict.register(Tool::native("le", "(a:Num b:Num -- c:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?.as_num()?;
            let a = stack.pop()?.as_num()?;
            stack.push(Value::Bool(a <= b))?;
            Ok((stack, ctx))
        })
    }));

    // ge: (a b -- a>=b)
    dict.register(Tool::native("ge", "(a:Num b:Num -- c:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?.as_num()?;
            let a = stack.pop()?.as_num()?;
            stack.push(Value::Bool(a >= b))?;
            Ok((stack, ctx))
        })
    }));

    // === Logic ===

    // and: (a b -- a&&b)
    dict.register(Tool::native("and", "(a:Bool b:Bool -- c:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?.into_bool()?;
            let a = stack.pop()?.into_bool()?;
            stack.push(Value::Bool(a && b))?;
            Ok((stack, ctx))
        })
    }));

    // or: (a b -- a||b)
    dict.register(Tool::native("or", "(a:Bool b:Bool -- c:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?.into_bool()?;
            let a = stack.pop()?.into_bool()?;
            stack.push(Value::Bool(a || b))?;
            Ok((stack, ctx))
        })
    }));

    // not: (a -- !a)
    dict.register(Tool::native("not", "(a:Bool -- b:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let a = stack.pop()?.into_bool()?;
            stack.push(Value::Bool(!a))?;
            Ok((stack, ctx))
        })
    }));

    // === String ===
    // LLMs work with text. These are first-class.

    // str-len: (s -- n)
    dict.register(Tool::native("str-len", "(s:Text -- n:Int)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let s = stack.pop()?.into_text()?;
            stack.push(Value::Int(s.chars().count() as i64))?;
            Ok((stack, ctx))
        })
    }));

    // str-get: (s i -- c)
    dict.register(Tool::native("str-get", "(s:Text i:Int -- c:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let i = stack.pop()?.into_int()?;
            let s = stack.pop()?.into_text()?;
            if i < 0 {
                return Err(crate::error::Error::IndexOutOfBounds { index: i, length: s.chars().count() });
            }
            let c = s.chars().nth(i as usize)
                .ok_or_else(|| crate::error::Error::IndexOutOfBounds { index: i, length: s.chars().count() })?;
            stack.push(Value::Text(c.to_string()))?;
            Ok((stack, ctx))
        })
    }));

    // str-slice: (s start end -- sub)
    dict.register(Tool::native("str-slice", "(s:Text start:Int end:Int -- sub:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let end = stack.pop()?.into_int()? as usize;
            let start = stack.pop()?.into_int()? as usize;
            let s = stack.pop()?.into_text()?;
            let sub: String = s.chars().skip(start).take(end.saturating_sub(start)).collect();
            stack.push(Value::Text(sub))?;
            Ok((stack, ctx))
        })
    }));

    // str-split: (s delim -- list)
    dict.register(Tool::native("str-split", "(s:Text delim:Text -- parts:List)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let delim = stack.pop()?.into_text()?;
            let s = stack.pop()?.into_text()?;
            let parts: Vec<Value> = s.split(&delim).map(|p| Value::Text(p.to_string())).collect();
            stack.push(Value::List(parts))?;
            Ok((stack, ctx))
        })
    }));

    // str-join: (list delim -- s)
    dict.register(Tool::native("str-join", "(parts:List delim:Text -- s:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let delim = stack.pop()?.into_text()?;
            let list = stack.pop()?.into_list()?;
            let parts: Result<Vec<String>, _> = list.into_iter().map(|v| v.into_text()).collect();
            stack.push(Value::Text(parts?.join(&delim)))?;
            Ok((stack, ctx))
        })
    }));

    // str-concat: (a b -- ab)
    dict.register(Tool::native("str-concat", "(a:Text b:Text -- c:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?.into_text()?;
            let a = stack.pop()?.into_text()?;
            stack.push(Value::Text(a + &b))?;
            Ok((stack, ctx))
        })
    }));

    // str-trim: (s -- s)
    dict.register(Tool::native("str-trim", "(s:Text -- trimmed:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let s = stack.pop()?.into_text()?;
            stack.push(Value::Text(s.trim().to_string()))?;
            Ok((stack, ctx))
        })
    }));

    // str-find: (s pattern -- i) returns -1 if not found
    dict.register(Tool::native("str-find", "(s:Text pattern:Text -- index:Int)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let pattern = stack.pop()?.into_text()?;
            let s = stack.pop()?.into_text()?;
            let index = s.find(&pattern).map(|i| i as i64).unwrap_or(-1);
            stack.push(Value::Int(index))?;
            Ok((stack, ctx))
        })
    }));

    // str-starts: (s prefix -- bool)
    dict.register(Tool::native("str-starts", "(s:Text prefix:Text -- result:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let prefix = stack.pop()?.into_text()?;
            let s = stack.pop()?.into_text()?;
            stack.push(Value::Bool(s.starts_with(&prefix)))?;
            Ok((stack, ctx))
        })
    }));

    // str-ends: (s suffix -- bool)
    dict.register(Tool::native("str-ends", "(s:Text suffix:Text -- result:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let suffix = stack.pop()?.into_text()?;
            let s = stack.pop()?.into_text()?;
            stack.push(Value::Bool(s.ends_with(&suffix)))?;
            Ok((stack, ctx))
        })
    }));

    // str-replace: (s old new -- s)
    dict.register(Tool::native("str-replace", "(s:Text old:Text new:Text -- result:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let new = stack.pop()?.into_text()?;
            let old = stack.pop()?.into_text()?;
            let s = stack.pop()?.into_text()?;
            stack.push(Value::Text(s.replace(&old, &new)))?;
            Ok((stack, ctx))
        })
    }));

    // char-code: (c -- n)
    dict.register(Tool::native("char-code", "(c:Text -- code:Int)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let c = stack.pop()?.into_text()?;
            let ch = c.chars().next()
                .ok_or_else(|| crate::error::Error::Runtime("empty string".into()))?;
            stack.push(Value::Int(ch as i64))?;
            Ok((stack, ctx))
        })
    }));

    // code-char: (n -- c)
    dict.register(Tool::native("code-char", "(code:Int -- c:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let code = stack.pop()?.into_int()?;
            let ch = char::from_u32(code as u32)
                .ok_or_else(|| crate::error::Error::Runtime(format!("invalid char code: {}", code)))?;
            stack.push(Value::Text(ch.to_string()))?;
            Ok((stack, ctx))
        })
    }));

    // === List ===
    // Ordered sequences. Core data structure.

    // list-len: (l -- n)
    dict.register(Tool::native("list-len", "(l:List -- n:Int)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let list = stack.pop()?.into_list()?;
            stack.push(Value::Int(list.len() as i64))?;
            Ok((stack, ctx))
        })
    }));

    // list-get: (l i -- v)
    dict.register(Tool::native("list-get", "(l:List i:Int -- v:Any)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let i = stack.pop()?.into_int()?;
            let list = stack.pop()?.into_list()?;
            if i < 0 || i as usize >= list.len() {
                return Err(crate::error::Error::IndexOutOfBounds { index: i, length: list.len() });
            }
            stack.push(list[i as usize].clone())?;
            Ok((stack, ctx))
        })
    }));

    // list-set: (l i v -- l)
    dict.register(Tool::native("list-set", "(l:List i:Int v:Any -- l:List)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let v = stack.pop()?;
            let i = stack.pop()?.into_int()?;
            let mut list = stack.pop()?.into_list()?;
            if i < 0 || i as usize >= list.len() {
                return Err(crate::error::Error::IndexOutOfBounds { index: i, length: list.len() });
            }
            list[i as usize] = v;
            stack.push(Value::List(list))?;
            Ok((stack, ctx))
        })
    }));

    // list-push: (l v -- l)
    dict.register(Tool::native("list-push", "(l:List v:Any -- l:List)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let v = stack.pop()?;
            let mut list = stack.pop()?.into_list()?;
            list.push(v);
            stack.push(Value::List(list))?;
            Ok((stack, ctx))
        })
    }));

    // list-pop: (l -- l v)
    dict.register(Tool::native("list-pop", "(l:List -- l:List v:Any)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let mut list = stack.pop()?.into_list()?;
            let v = list.pop()
                .ok_or_else(|| crate::error::Error::Runtime("cannot pop from empty list".into()))?;
            stack.push(Value::List(list))?;
            stack.push(v)?;
            Ok((stack, ctx))
        })
    }));

    // list-slice: (l start end -- l)
    dict.register(Tool::native("list-slice", "(l:List start:Int end:Int -- sub:List)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let end = stack.pop()?.into_int()? as usize;
            let start = stack.pop()?.into_int()? as usize;
            let list = stack.pop()?.into_list()?;
            let end = end.min(list.len());
            let start = start.min(end);
            stack.push(Value::List(list[start..end].to_vec()))?;
            Ok((stack, ctx))
        })
    }));

    // list-concat: (a b -- c)
    dict.register(Tool::native("list-concat", "(a:List b:List -- c:List)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?.into_list()?;
            let mut a = stack.pop()?.into_list()?;
            a.extend(b);
            stack.push(Value::List(a))?;
            Ok((stack, ctx))
        })
    }));

    // list-reverse: (l -- l)
    dict.register(Tool::native("list-reverse", "(l:List -- reversed:List)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let mut list = stack.pop()?.into_list()?;
            list.reverse();
            stack.push(Value::List(list))?;
            Ok((stack, ctx))
        })
    }));

    // list-empty: ( -- l)
    dict.register(Tool::native("list-empty", "( -- l:List)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            stack.push(Value::List(vec![]))?;
            Ok((stack, ctx))
        })
    }));

    // collect: (n -- l) - collect n items from stack into list
    dict.register(Tool::native("collect", "(n:Int -- l:List)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let n = stack.pop()?.into_int()?;
            if n < 0 {
                return Err(crate::error::Error::Runtime("collect count must be non-negative".into()));
            }
            let mut items = Vec::with_capacity(n as usize);
            for _ in 0..n {
                items.push(stack.pop()?);
            }
            items.reverse(); // Stack order is reversed
            stack.push(Value::List(items))?;
            Ok((stack, ctx))
        })
    }));

    // === Map ===
    // Key-value stores. String keys only.

    // map-get: (m k -- v)
    dict.register(Tool::native("map-get", "(m:Map k:Text -- v:Any)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let k = stack.pop()?.into_text()?;
            let map = stack.pop()?.into_map()?;
            let v = map.get(&k)
                .ok_or_else(|| crate::error::Error::KeyNotFound(k))?
                .clone();
            stack.push(v)?;
            Ok((stack, ctx))
        })
    }));

    // map-set: (m k v -- m)
    dict.register(Tool::native("map-set", "(m:Map k:Text v:Any -- m:Map)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let v = stack.pop()?;
            let k = stack.pop()?.into_text()?;
            let mut map = stack.pop()?.into_map()?;
            map.insert(k, v);
            stack.push(Value::Map(map))?;
            Ok((stack, ctx))
        })
    }));

    // map-has: (m k -- bool)
    dict.register(Tool::native("map-has", "(m:Map k:Text -- result:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let k = stack.pop()?.into_text()?;
            let map = stack.pop()?.into_map()?;
            stack.push(Value::Bool(map.contains_key(&k)))?;
            Ok((stack, ctx))
        })
    }));

    // map-del: (m k -- m)
    dict.register(Tool::native("map-del", "(m:Map k:Text -- m:Map)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let k = stack.pop()?.into_text()?;
            let mut map = stack.pop()?.into_map()?;
            map.shift_remove(&k);
            stack.push(Value::Map(map))?;
            Ok((stack, ctx))
        })
    }));

    // map-keys: (m -- l)
    dict.register(Tool::native("map-keys", "(m:Map -- keys:List)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let map = stack.pop()?.into_map()?;
            let keys: Vec<Value> = map.keys().map(|k| Value::Text(k.clone())).collect();
            stack.push(Value::List(keys))?;
            Ok((stack, ctx))
        })
    }));

    // map-vals: (m -- l)
    dict.register(Tool::native("map-vals", "(m:Map -- vals:List)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let map = stack.pop()?.into_map()?;
            let vals: Vec<Value> = map.values().cloned().collect();
            stack.push(Value::List(vals))?;
            Ok((stack, ctx))
        })
    }));

    // map-empty: ( -- m)
    dict.register(Tool::native("map-empty", "( -- m:Map)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            stack.push(Value::Map(indexmap::IndexMap::new()))?;
            Ok((stack, ctx))
        })
    }));

    // === Type ===
    // Introspection for LLMs to understand what they're working with.

    // type-of: (v -- type-name)
    dict.register(Tool::native("type-of", "(v:Any -- t:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let v = stack.pop()?;
            stack.push(Value::Text(v.type_name().to_string()))?;
            Ok((stack, ctx))
        })
    }));

    // is-null: (v -- bool)
    dict.register(Tool::native("is-null", "(v:Any -- result:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let v = stack.pop()?;
            stack.push(Value::Bool(v.is_null()))?;
            Ok((stack, ctx))
        })
    }));

    // is-bool: (v -- bool)
    dict.register(Tool::native("is-bool", "(v:Any -- result:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let v = stack.pop()?;
            stack.push(Value::Bool(matches!(v, Value::Bool(_))))?;
            Ok((stack, ctx))
        })
    }));

    // is-int: (v -- bool)
    dict.register(Tool::native("is-int", "(v:Any -- result:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let v = stack.pop()?;
            stack.push(Value::Bool(matches!(v, Value::Int(_))))?;
            Ok((stack, ctx))
        })
    }));

    // is-float: (v -- bool)
    dict.register(Tool::native("is-float", "(v:Any -- result:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let v = stack.pop()?;
            stack.push(Value::Bool(matches!(v, Value::Float(_))))?;
            Ok((stack, ctx))
        })
    }));

    // is-text: (v -- bool)
    dict.register(Tool::native("is-text", "(v:Any -- result:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let v = stack.pop()?;
            stack.push(Value::Bool(matches!(v, Value::Text(_))))?;
            Ok((stack, ctx))
        })
    }));

    // is-list: (v -- bool)
    dict.register(Tool::native("is-list", "(v:Any -- result:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let v = stack.pop()?;
            stack.push(Value::Bool(matches!(v, Value::List(_))))?;
            Ok((stack, ctx))
        })
    }));

    // is-map: (v -- bool)
    dict.register(Tool::native("is-map", "(v:Any -- result:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let v = stack.pop()?;
            stack.push(Value::Bool(matches!(v, Value::Map(_))))?;
            Ok((stack, ctx))
        })
    }));

    // is-quote: (v -- bool)
    dict.register(Tool::native("is-quote", "(v:Any -- result:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let v = stack.pop()?;
            stack.push(Value::Bool(matches!(v, Value::Quote(_))))?;
            Ok((stack, ctx))
        })
    }));

    // === Conversion ===
    // Type coercion when LLMs need to convert between types.

    // to-int: (v -- n)
    dict.register(Tool::native("to-int", "(v:Any -- n:Int)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let v = stack.pop()?;
            let result = match v {
                Value::Int(n) => n,
                Value::Float(f) => f as i64,
                Value::Text(s) => s.parse::<i64>()
                    .map_err(|_| crate::error::Error::Runtime(format!("cannot parse '{}' as int", s)))?,
                Value::Bool(b) => if b { 1 } else { 0 },
                _ => return Err(crate::error::Error::type_error("Int|Float|Text|Bool", &v)),
            };
            stack.push(Value::Int(result))?;
            Ok((stack, ctx))
        })
    }));

    // to-float: (v -- f)
    dict.register(Tool::native("to-float", "(v:Any -- f:Float)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let v = stack.pop()?;
            let result = match v {
                Value::Float(f) => f,
                Value::Int(n) => n as f64,
                Value::Text(s) => s.parse::<f64>()
                    .map_err(|_| crate::error::Error::Runtime(format!("cannot parse '{}' as float", s)))?,
                _ => return Err(crate::error::Error::type_error("Int|Float|Text", &v)),
            };
            stack.push(Value::Float(result))?;
            Ok((stack, ctx))
        })
    }));

    // to-text: (v -- s)
    dict.register(Tool::native("to-text", "(v:Any -- s:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let v = stack.pop()?;
            let result = match v {
                Value::Text(s) => s,
                Value::Int(n) => n.to_string(),
                Value::Float(f) => f.to_string(),
                Value::Bool(b) => b.to_string(),
                Value::Null => "null".to_string(),
                _ => return Err(crate::error::Error::type_error("Int|Float|Text|Bool|Null", &v)),
            };
            stack.push(Value::Text(result))?;
            Ok((stack, ctx))
        })
    }));

    // to-bool: (v -- b)
    dict.register(Tool::native("to-bool", "(v:Any -- b:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let v = stack.pop()?;
            stack.push(Value::Bool(v.is_truthy()))?;
            Ok((stack, ctx))
        })
    }));

    // to-list: (v -- l) - wrap in list, or identity if already list
    dict.register(Tool::native("to-list", "(v:Any -- l:List)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let v = stack.pop()?;
            let result = match v {
                Value::List(l) => l,
                other => vec![other],
            };
            stack.push(Value::List(result))?;
            Ok((stack, ctx))
        })
    }));

    // === Combinators ===
    // Higher-order tools for LLMs to compose behavior.

    // map: (list quote -- list)
    dict.register(Tool::native("map", "(l:List f:Quote -- result:List)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let quote = stack.pop()?.into_quote()?;
            let list = stack.pop()?.into_list()?;
            let mut result = Vec::with_capacity(list.len());
            
            for item in list {
                let mut item_stack = Stack::new();
                item_stack.push(item)?;
                let (result_stack, _) = execute(&quote, item_stack, ctx.clone()).await?;
                if let Some(v) = result_stack.values().last() {
                    result.push(v.clone());
                }
            }
            
            stack.push(Value::List(result))?;
            Ok((stack, ctx))
        })
    }));

    // filter: (list quote -- list)
    dict.register(Tool::native("filter", "(l:List pred:Quote -- result:List)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let quote = stack.pop()?.into_quote()?;
            let list = stack.pop()?.into_list()?;
            let mut result = Vec::new();
            
            for item in list {
                let mut item_stack = Stack::new();
                item_stack.push(item.clone())?;
                let (result_stack, _) = execute(&quote, item_stack, ctx.clone()).await?;
                if let Some(v) = result_stack.values().last() {
                    if v.is_truthy() {
                        result.push(item);
                    }
                }
            }
            
            stack.push(Value::List(result))?;
            Ok((stack, ctx))
        })
    }));

    // fold: (list init quote -- value) - quote receives (acc item -- acc)
    dict.register(Tool::native("fold", "(l:List init:Any f:Quote -- result:Any)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let quote = stack.pop()?.into_quote()?;
            let init = stack.pop()?;
            let list = stack.pop()?.into_list()?;
            
            let mut acc = init;
            for item in list {
                let mut fold_stack = Stack::new();
                fold_stack.push(acc)?;
                fold_stack.push(item)?;
                let (result_stack, _) = execute(&quote, fold_stack, ctx.clone()).await?;
                acc = result_stack.values().last()
                    .ok_or_else(|| crate::error::Error::Runtime("fold quote must return a value".into()))?
                    .clone();
            }
            
            stack.push(acc)?;
            Ok((stack, ctx))
        })
    }));

    // each: (list quote -- ) - execute quote for each item, discard results
    dict.register(Tool::native("each", "(l:List f:Quote -- )", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let quote = stack.pop()?.into_quote()?;
            let list = stack.pop()?.into_list()?;
            
            for item in list {
                let mut item_stack = Stack::new();
                item_stack.push(item)?;
                execute(&quote, item_stack, ctx.clone()).await?;
            }
            
            Ok((stack, ctx))
        })
    }));

    // times: (n quote -- ) - execute quote n times
    dict.register(Tool::native("times", "(n:Int f:Quote -- )", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let quote = stack.pop()?.into_quote()?;
            let n = stack.pop()?.into_int()?;
            
            for i in 0..n {
                let mut iter_stack = Stack::new();
                iter_stack.push(Value::Int(i))?;
                let (result_stack, _) = execute(&quote, iter_stack, ctx.clone()).await?;
                // Pass through results to main stack
                for v in result_stack.values() {
                    stack.push(v.clone())?;
                }
            }
            
            Ok((stack, ctx))
        })
    }));

    // while: (cond-quote body-quote -- ) - while cond returns true, execute body
    // The condition quote should push a bool. That bool is consumed.
    dict.register(Tool::native("while", "(cond:Quote body:Quote -- )", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let body = stack.pop()?.into_quote()?;
            let cond = stack.pop()?.into_quote()?;
            
            loop {
                // Evaluate condition on current stack
                let (mut cond_stack, _) = execute(&cond, stack, ctx.clone()).await?;
                
                // Pop the condition result
                let should_continue = cond_stack.pop()?.is_truthy();
                stack = cond_stack;
                
                if !should_continue {
                    break;
                }
                
                // Execute body
                let (new_stack, _) = execute(&body, stack, ctx.clone()).await?;
                stack = new_stack;
            }
            
            Ok((stack, ctx))
        })
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::op::Op;

    async fn setup() -> Context {
        let mut ctx = Context::new();
        register_builtins(&mut ctx).await;
        ctx
    }

    #[tokio::test]
    async fn test_call() {
        let ctx = setup().await;
        let stack = Stack::new();

        // 5 [dup] call -> 5 5
        let ops = vec![
            Op::push(5),
            Op::Quote(vec![Op::call("dup")]),
            Op::call("call"),
        ];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.depth(), 2);
        assert_eq!(result.values()[0].as_int().unwrap(), 5);
        assert_eq!(result.values()[1].as_int().unwrap(), 5);
    }

    #[tokio::test]
    async fn test_try_success() {
        let ctx = setup().await;
        let stack = Stack::new();

        // [5] try -> 5
        let ops = vec![
            Op::Quote(vec![Op::push(5)]),
            Op::call("try"),
        ];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.depth(), 1);
        assert_eq!(result.values()[0].as_int().unwrap(), 5);
    }

    #[tokio::test]
    async fn test_try_error() {
        let ctx = setup().await;
        let stack = Stack::new();

        // [drop] try -> Error (stack underflow)
        let ops = vec![
            Op::Quote(vec![Op::call("drop")]),
            Op::call("try"),
        ];

        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.depth(), 1);
        assert!(matches!(result.values()[0], Value::Error(_)));
    }

    #[tokio::test]
    async fn test_is_error() {
        let ctx = setup().await;
        let stack = Stack::new();

        // 5 is-error -> false
        let ops = vec![Op::push(5), Op::call("is-error")];
        let (result, _) = execute(&ops, stack, ctx.clone()).await.unwrap();
        assert_eq!(result.values()[0].as_bool().unwrap(), false);

        // [drop] try is-error -> true
        let stack = Stack::new();
        let ops = vec![
            Op::Quote(vec![Op::call("drop")]),
            Op::call("try"),
            Op::call("is-error"),
        ];
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.values()[0].as_bool().unwrap(), true);
    }

    #[tokio::test]
    async fn test_unwrap_value() {
        let ctx = setup().await;
        let stack = Stack::new();

        // 5 unwrap -> 5
        let ops = vec![Op::push(5), Op::call("unwrap")];
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), 5);
    }

    #[tokio::test]
    async fn test_unwrap_error() {
        let ctx = setup().await;
        let stack = Stack::new();

        // [drop] try unwrap -> stops with error
        let ops = vec![
            Op::Quote(vec![Op::call("drop")]),
            Op::call("try"),
            Op::call("unwrap"),
        ];
        let result = execute(&ops, stack, ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_dup() {
        let ctx = setup().await;
        let stack = Stack::new();

        let ops = vec![Op::push(42), Op::call("dup")];
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.depth(), 2);
        assert_eq!(result.values()[0].as_int().unwrap(), 42);
        assert_eq!(result.values()[1].as_int().unwrap(), 42);
    }

    #[tokio::test]
    async fn test_drop() {
        let ctx = setup().await;
        let stack = Stack::new();

        let ops = vec![Op::push(1), Op::push(2), Op::call("drop")];
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.depth(), 1);
        assert_eq!(result.values()[0].as_int().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_swap() {
        let ctx = setup().await;
        let stack = Stack::new();

        let ops = vec![Op::push(1), Op::push(2), Op::call("swap")];
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), 2);
        assert_eq!(result.values()[1].as_int().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_over() {
        let ctx = setup().await;
        let stack = Stack::new();

        let ops = vec![Op::push(1), Op::push(2), Op::call("over")];
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.depth(), 3);
        assert_eq!(result.values()[0].as_int().unwrap(), 1);
        assert_eq!(result.values()[1].as_int().unwrap(), 2);
        assert_eq!(result.values()[2].as_int().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_rot() {
        let ctx = setup().await;
        let stack = Stack::new();

        // 1 2 3 rot -> 2 3 1
        let ops = vec![
            Op::push(1),
            Op::push(2),
            Op::push(3),
            Op::call("rot"),
        ];
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), 2);
        assert_eq!(result.values()[1].as_int().unwrap(), 3);
        assert_eq!(result.values()[2].as_int().unwrap(), 1);
    }
}
