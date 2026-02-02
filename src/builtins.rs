//! Built-in tools - the minimal set that makes kore a complete language
//!
//! These are language primitives, not library functions.
//! Each tool does exactly one thing. LLMs are first-class citizens.
//!
//! ## Execution (7)
//! - `call`: Run a quote
//! - `try`: Run a quote, capture errors as Error values
//! - `if`: Conditional execution
//! - `loop`: Repeat until false on stack
//! - `def`: Define a new tool from a quote
//! - `words`: List all tool names
//! - `describe`: Get tool signature
//!
//! ## Error inspection (4)
//! - `is-error`: Check if a value is an Error
//! - `unwrap`: Extract value, or stop if Error
//! - `assert`: Fail with message if condition is false
//! - `panic`: Intentionally fail with message
//!
//! ## Stack manipulation (6)
//! - `dup`: Duplicate top value
//! - `drop`: Remove top value
//! - `swap`: Swap top two values
//! - `over`: Copy second value to top
//! - `rot`: Rotate top three values
//! - `depth`: Get stack depth
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
//! ## OS: File System (7)
//! - `fs-read`, `fs-write`, `fs-append`, `fs-exists`, `fs-list`, `fs-rm`, `fs-mkdir`
//!
//! ## OS: Process (1)
//! - `exec`: Run shell command
//!
//! ## OS: I/O (4)
//! - `print`, `println`, `read-line`, `log`
//!
//! ## OS: Time (2)
//! - `now`, `sleep`
//!
//! ## OS: Misc (2)
//! - `uuid`: Generate UUID v4
//! - `random`: Random float 0.0-1.0
//!
//! ## OS: System Info (5)
//! - `pid`, `cwd`, `args`, `exit`, `version`
//!
//! ## OS: Module Loading (1)
//! - `load`: Execute a .kore file
//!
//! ## OS: Environment (2)
//! - `env-get`, `env-set`
//!
//! ## OS: HTTP (3)
//! - `http-get`, `http-post`, `http-request`
//!
//! ## Data: JSON (2)
//! - `json-parse`, `json-encode`
//!
//! ## Resources (5)
//! - `res-mem`, `res-rom`, `res-compute`, `res-net`, `res-all`
//!
//! ## Capabilities (4)
//! - `cap-has`, `cap-list`, `cap-fs`, `cap-net`
//!
//! ## Session Memory (5)
//! - `mem-set`, `mem-get`, `mem-del`, `mem-has`, `mem-keys`
//!
//! ## Persistent Storage (5)
//! - `rom-set`, `rom-get`, `rom-del`, `rom-has`, `rom-keys`
//!
//! ## Introspection (8)
//! - `meta`: Get metadata for any tool
//! - `meta!`: Set metadata field
//! - `calls`: Get immediate dependencies
//! - `graph`: Get full call tree
//! - `tag`: Add tag to a tool
//! - `find-tag`: Find tools by tag
//! - `health`: Get tools with warnings/errors
//! - `stats`: Get call statistics
//!
//! ## Persistence (5)
//! - `persist`: Save tool to ROM only
//! - `register`: Define AND persist a tool
//! - `load-tools`: Load persisted tools into dictionary
//! - `unregister`: Remove from dictionary and ROM
//! - `list-persisted`: List persisted tool names
//!
//! **Total: 143 primitives**

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

    // assert: (condition message -- ) - fail with message if condition is false
    dict.register(Tool::native("assert", "(cond:Bool msg:Text -- )", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let message = stack.pop()?.into_text()?;
            let condition = stack.pop()?;
            if !condition.is_truthy() {
                return Err(crate::error::Error::Runtime(format!("Assertion failed: {}", message)));
            }
            Ok((stack, ctx))
        })
    }));

    // panic: (message -- ) - intentionally fail with message
    dict.register(Tool::native("panic", "(msg:Text -- )", |mut stack: Stack, _ctx: Context| {
        Box::pin(async move {
            let message = stack.pop()?.into_text()?;
            Err(crate::error::Error::Runtime(format!("Panic: {}", message)))
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

    // depth: ( -- n) - get current stack depth
    dict.register(Tool::native("depth", "( -- n:Int)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let d = stack.depth() as i64;
            stack.push(Value::Int(d))?;
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

    // words: ( -- list) - list all defined tool names
    dict.register(Tool::native("words", "( -- names:List)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let dict = ctx.dict.read().await;
            let names: Vec<Value> = dict.names()
                .map(|s| Value::Text(s.to_string()))
                .collect();
            drop(dict);
            stack.push(Value::List(names))?;
            Ok((stack, ctx))
        })
    }));

    // describe: (name -- signature) - get the signature of a tool
    dict.register(Tool::native("describe", "(name:Text -- sig:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let name = stack.pop()?.into_text()?;
            let dict = ctx.dict.read().await;
            let tool = dict.get(&name)?;
            drop(dict);
            
            let sig = match tool.sig() {
                Some(s) => format!("{} {}", name, s),
                None => format!("{} (unknown signature)", name),
            };
            stack.push(Value::Text(sig))?;
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
                .ok_or(crate::error::Error::KeyNotFound(k.clone()))?
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

    // === OS: File System (6) ===

    // fs-read: (path -- text) - read file contents
    dict.register(Tool::native("fs-read", "(path:Text -- contents:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let path = stack.pop()?.into_text()?;
            // Check capability
            if !ctx.caps.can_read_path(std::path::Path::new(&path)) {
                return Err(crate::error::Error::CapabilityDenied { 
                    capability: format!("fs:read:{}", path), 
                    tool: "fs-read".to_string() 
                });
            }
            match tokio::fs::read_to_string(&path).await {
                Ok(contents) => stack.push(Value::Text(contents))?,
                Err(e) => return Err(crate::error::Error::io(format!("fs-read '{}': {}", path, e))),
            }
            Ok((stack, ctx))
        })
    }));

    // fs-write: (path text -- ) - write text to file
    dict.register(Tool::native("fs-write", "(path:Text contents:Text -- )", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let contents = stack.pop()?.into_text()?;
            let path = stack.pop()?.into_text()?;
            // Check capability
            if !ctx.caps.can_write_path(std::path::Path::new(&path)) {
                return Err(crate::error::Error::CapabilityDenied { 
                    capability: format!("fs:write:{}", path), 
                    tool: "fs-write".to_string() 
                });
            }
            match tokio::fs::write(&path, &contents).await {
                Ok(_) => {},
                Err(e) => return Err(crate::error::Error::io(format!("fs-write '{}': {}", path, e))),
            }
            Ok((stack, ctx))
        })
    }));

    // fs-append: (path text -- ) - append text to file
    dict.register(Tool::native("fs-append", "(path:Text contents:Text -- )", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let contents = stack.pop()?.into_text()?;
            let path = stack.pop()?.into_text()?;
            // Check capability
            if !ctx.caps.can_write_path(std::path::Path::new(&path)) {
                return Err(crate::error::Error::CapabilityDenied { 
                    capability: format!("fs:write:{}", path), 
                    tool: "fs-append".to_string() 
                });
            }
            use tokio::io::AsyncWriteExt;
            let mut file = match tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .await {
                    Ok(f) => f,
                    Err(e) => return Err(crate::error::Error::io(format!("fs-append '{}': {}", path, e))),
                };
            match file.write_all(contents.as_bytes()).await {
                Ok(_) => {},
                Err(e) => return Err(crate::error::Error::io(format!("fs-append '{}': {}", path, e))),
            }
            Ok((stack, ctx))
        })
    }));

    // fs-exists: (path -- bool) - check if path exists
    dict.register(Tool::native("fs-exists", "(path:Text -- exists:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let path = stack.pop()?.into_text()?;
            // Check capability (need read to check existence)
            if !ctx.caps.can_read_path(std::path::Path::new(&path)) {
                return Err(crate::error::Error::CapabilityDenied { 
                    capability: format!("fs:read:{}", path), 
                    tool: "fs-exists".to_string() 
                });
            }
            let exists = tokio::fs::metadata(&path).await.is_ok();
            stack.push(Value::Bool(exists))?;
            Ok((stack, ctx))
        })
    }));

    // fs-list: (path -- list) - list directory contents
    dict.register(Tool::native("fs-list", "(path:Text -- entries:List)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let path = stack.pop()?.into_text()?;
            // Check capability
            if !ctx.caps.can_read_path(std::path::Path::new(&path)) {
                return Err(crate::error::Error::CapabilityDenied { 
                    capability: format!("fs:read:{}", path), 
                    tool: "fs-list".to_string() 
                });
            }
            let mut entries = Vec::new();
            let mut dir = match tokio::fs::read_dir(&path).await {
                Ok(d) => d,
                Err(e) => return Err(crate::error::Error::io(format!("fs-list '{}': {}", path, e))),
            };
            while let Some(entry) = dir.next_entry().await.map_err(|e| 
                crate::error::Error::io(format!("fs-list '{}': {}", path, e)))? {
                let name = entry.file_name().to_string_lossy().to_string();
                let is_dir = entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false);
                let display = if is_dir { format!("{}/", name) } else { name };
                entries.push(Value::Text(display));
            }
            stack.push(Value::List(entries))?;
            Ok((stack, ctx))
        })
    }));

    // fs-rm: (path -- ) - remove file or directory
    dict.register(Tool::native("fs-rm", "(path:Text -- )", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let path = stack.pop()?.into_text()?;
            // Check capability (need write to delete)
            if !ctx.caps.can_write_path(std::path::Path::new(&path)) {
                return Err(crate::error::Error::CapabilityDenied { 
                    capability: format!("fs:write:{}", path), 
                    tool: "fs-rm".to_string() 
                });
            }
            // Try as file first, then as directory
            if tokio::fs::remove_file(&path).await.is_err() {
                if let Err(e) = tokio::fs::remove_dir_all(&path).await {
                    return Err(crate::error::Error::io(format!("fs-rm '{}': {}", path, e)));
                }
            }
            Ok((stack, ctx))
        })
    }));

    // fs-mkdir: (path -- ) - create directory (and parents)
    dict.register(Tool::native("fs-mkdir", "(path:Text -- )", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let path = stack.pop()?.into_text()?;
            // Check capability
            if !ctx.caps.can_write_path(std::path::Path::new(&path)) {
                return Err(crate::error::Error::CapabilityDenied { 
                    capability: format!("fs:write:{}", path), 
                    tool: "fs-mkdir".to_string() 
                });
            }
            match tokio::fs::create_dir_all(&path).await {
                Ok(_) => {},
                Err(e) => return Err(crate::error::Error::io(format!("fs-mkdir '{}': {}", path, e))),
            }
            Ok((stack, ctx))
        })
    }));

    // === OS: Process (1) ===

    // exec: (cmd -- output) - run shell command, return stdout
    dict.register(Tool::native("exec", "(cmd:Text -- output:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let cmd = stack.pop()?.into_text()?;
            // Check capability
            if !ctx.caps.can_exec() {
                return Err(crate::error::Error::CapabilityDenied { 
                    capability: "exec".to_string(), 
                    tool: "exec".to_string() 
                });
            }
            let output = tokio::process::Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .output()
                .await
                .map_err(|e| crate::error::Error::io(format!("exec '{}': {}", cmd, e)))?;
            
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            
            if !output.status.success() {
                return Err(crate::error::Error::io(format!("exec '{}' failed: {}", cmd, stderr)));
            }
            
            stack.push(Value::Text(stdout))?;
            Ok((stack, ctx))
        })
    }));

    // === OS: I/O (2) ===

    // print: (text -- ) - print to stdout (no newline)
    dict.register(Tool::native("print", "(text:Text -- )", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let text = stack.pop()?.into_text()?;
            use std::io::Write;
            print!("{}", text);
            std::io::stdout().flush().ok();
            Ok((stack, ctx))
        })
    }));

    // println: (text -- ) - print with newline
    dict.register(Tool::native("println", "(text:Text -- )", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let text = stack.pop()?.into_text()?;
            println!("{}", text);
            Ok((stack, ctx))
        })
    }));

    // log: (level message -- ) - write log to stderr with timestamp
    // level is one of: "debug", "info", "warn", "error"
    dict.register(Tool::native("log", "(level:Text message:Text -- )", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let message = stack.pop()?.into_text()?;
            let level = stack.pop()?.into_text()?;
            
            // Get current time as ISO-8601
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            
            // Format: [LEVEL] timestamp message
            let level_upper = level.to_uppercase();
            eprintln!("[{}] {} {}", level_upper, now, message);
            
            Ok((stack, ctx))
        })
    }));

    // === OS: Time (2) ===

    // now: ( -- ms) - current unix timestamp in milliseconds
    dict.register(Tool::native("now", "( -- ms:Int)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64;
            stack.push(Value::Int(ms))?;
            Ok((stack, ctx))
        })
    }));

    // sleep: (ms -- ) - sleep for milliseconds
    dict.register(Tool::native("sleep", "(ms:Int -- )", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let ms = stack.pop()?.as_int()?;
            tokio::time::sleep(tokio::time::Duration::from_millis(ms as u64)).await;
            Ok((stack, ctx))
        })
    }));

    // === OS: Misc (1) ===

    // uuid: ( -- uuid) - generate a new UUID v4
    dict.register(Tool::native("uuid", "( -- id:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let id = uuid::Uuid::new_v4().to_string();
            stack.push(Value::Text(id))?;
            Ok((stack, ctx))
        })
    }));

    // random: ( -- n) - generate random float between 0.0 and 1.0
    dict.register(Tool::native("random", "( -- n:Float)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            // Use UUID as source of randomness (simple approach without adding rand crate)
            let uuid = uuid::Uuid::new_v4();
            let bytes = uuid.as_bytes();
            // Convert first 8 bytes to u64, then normalize to 0.0-1.0
            let mut arr = [0u8; 8];
            arr.copy_from_slice(&bytes[0..8]);
            let raw = u64::from_le_bytes(arr);
            let normalized = (raw as f64) / (u64::MAX as f64);
            stack.push(Value::Float(normalized))?;
            Ok((stack, ctx))
        })
    }));

    // === OS: System Info (3) ===

    // pid: ( -- id) - get current process ID
    dict.register(Tool::native("pid", "( -- id:Int)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let pid = std::process::id() as i64;
            stack.push(Value::Int(pid))?;
            Ok((stack, ctx))
        })
    }));

    // cwd: ( -- path) - get current working directory
    dict.register(Tool::native("cwd", "( -- path:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let cwd = std::env::current_dir()
                .map_err(|e| crate::error::Error::io(format!("cwd: {}", e)))?;
            stack.push(Value::Text(cwd.to_string_lossy().to_string()))?;
            Ok((stack, ctx))
        })
    }));

    // args: ( -- list) - get command line arguments
    dict.register(Tool::native("args", "( -- args:List)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let args: Vec<Value> = std::env::args()
                .map(Value::Text)
                .collect();
            stack.push(Value::List(args))?;
            Ok((stack, ctx))
        })
    }));

    // exit: (code -- ) - exit the process with given exit code
    dict.register(Tool::native("exit", "(code:Int -- )", |mut stack: Stack, _ctx: Context| {
        Box::pin(async move {
            let code = stack.pop()?.as_int()?;
            std::process::exit(code as i32);
        })
    }));

    // version: ( -- text) - get Kore version string
    dict.register(Tool::native("version", "( -- v:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            stack.push(Value::Text("0.1.0".to_string()))?;
            Ok((stack, ctx))
        })
    }));

    // === OS: Module Loading (1) ===

    // load: (path -- ) - execute a .kore file
    dict.register(Tool::native("load", "(path:Text -- )", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let path = stack.pop()?.into_text()?;
            let source = tokio::fs::read_to_string(&path).await
                .map_err(|e| crate::error::Error::io(format!("load '{}': {}", path, e)))?;
            let ops = crate::op::Op::parse(&source)?;
            let (new_stack, new_ctx) = execute(&ops, stack, ctx).await?;
            Ok((new_stack, new_ctx))
        })
    }));

    // === OS: Environment (2) ===

    // env-get: (name -- value-or-null) - get environment variable
    dict.register(Tool::native("env-get", "(name:Text -- value:Text|Null)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let name = stack.pop()?.into_text()?;
            // Check capability
            if !ctx.caps.can_env_read() {
                return Err(crate::error::Error::CapabilityDenied { 
                    capability: "env:read".to_string(), 
                    tool: "env-get".to_string() 
                });
            }
            match std::env::var(&name) {
                Ok(val) => stack.push(Value::Text(val))?,
                Err(_) => stack.push(Value::Null)?,
            }
            Ok((stack, ctx))
        })
    }));

    // env-set: (name value -- ) - set environment variable
    dict.register(Tool::native("env-set", "(name:Text value:Text -- )", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let value = stack.pop()?.into_text()?;
            let name = stack.pop()?.into_text()?;
            // Check capability
            if !ctx.caps.can_env_write() {
                return Err(crate::error::Error::CapabilityDenied { 
                    capability: "env:write".to_string(), 
                    tool: "env-set".to_string() 
                });
            }
            std::env::set_var(&name, &value);
            Ok((stack, ctx))
        })
    }));

    // === OS: Stdin (1) ===

    // read-line: ( -- text) - read a line from stdin
    dict.register(Tool::native("read-line", "( -- line:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let mut line = String::new();
            std::io::stdin().read_line(&mut line)
                .map_err(|e| crate::error::Error::io(format!("read-line: {}", e)))?;
            // Remove trailing newline
            if line.ends_with('\n') {
                line.pop();
                if line.ends_with('\r') {
                    line.pop();
                }
            }
            stack.push(Value::Text(line))?;
            Ok((stack, ctx))
        })
    }));

    // === Data: JSON (2) ===

    // json-parse: (text -- value) - parse JSON text into a value
    dict.register(Tool::native("json-parse", "(json:Text -- value:Any)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let text = stack.pop()?.into_text()?;
            let json: serde_json::Value = serde_json::from_str(&text)
                .map_err(|e| crate::error::Error::io(format!("json-parse: {}", e)))?;
            let value = json_to_value(json);
            stack.push(value)?;
            Ok((stack, ctx))
        })
    }));

    // json-encode: (value -- text) - encode a value as JSON text
    dict.register(Tool::native("json-encode", "(value:Any -- json:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let value = stack.pop()?;
            let json = value_to_json(&value);
            let text = serde_json::to_string(&json)
                .map_err(|e| crate::error::Error::io(format!("json-encode: {}", e)))?;
            stack.push(Value::Text(text))?;
            Ok((stack, ctx))
        })
    }));

    // === OS: HTTP (3) ===

    // http-get: (url -- response) - HTTP GET request, returns response map
    dict.register(Tool::native("http-get", "(url:Text -- response:Map)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let url = stack.pop()?.into_text()?;
            
            let client = reqwest::Client::new();
            let response = client.get(&url)
                .send()
                .await
                .map_err(|e| crate::error::Error::io(format!("http-get: {}", e)))?;
            
            let status = response.status().as_u16() as i64;
            let headers = response.headers().clone();
            let body = response.text().await
                .map_err(|e| crate::error::Error::io(format!("http-get body: {}", e)))?;
            
            // Build response map
            let mut result = indexmap::IndexMap::new();
            result.insert("status".to_string(), Value::Int(status));
            result.insert("body".to_string(), Value::Text(body));
            
            // Headers as a map
            let mut header_map = indexmap::IndexMap::new();
            for (k, v) in headers.iter() {
                if let Ok(v_str) = v.to_str() {
                    header_map.insert(k.to_string(), Value::Text(v_str.to_string()));
                }
            }
            result.insert("headers".to_string(), Value::Map(header_map));
            
            stack.push(Value::Map(result))?;
            Ok((stack, ctx))
        })
    }));

    // http-post: (url body headers -- response) - HTTP POST with body and headers
    dict.register(Tool::native("http-post", "(url:Text body:Text headers:Map -- response:Map)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let headers_map = stack.pop()?.into_map()?;
            let body = stack.pop()?.into_text()?;
            let url = stack.pop()?.into_text()?;
            
            let client = reqwest::Client::new();
            let mut request = client.post(&url).body(body);
            
            // Add headers
            for (k, v) in headers_map.iter() {
                if let Value::Text(v_str) = v {
                    request = request.header(k.as_str(), v_str.as_str());
                }
            }
            
            let response = request.send()
                .await
                .map_err(|e| crate::error::Error::io(format!("http-post: {}", e)))?;
            
            let status = response.status().as_u16() as i64;
            let resp_headers = response.headers().clone();
            let resp_body = response.text().await
                .map_err(|e| crate::error::Error::io(format!("http-post body: {}", e)))?;
            
            // Build response map
            let mut result = indexmap::IndexMap::new();
            result.insert("status".to_string(), Value::Int(status));
            result.insert("body".to_string(), Value::Text(resp_body));
            
            // Headers as a map
            let mut header_map = indexmap::IndexMap::new();
            for (k, v) in resp_headers.iter() {
                if let Ok(v_str) = v.to_str() {
                    header_map.insert(k.to_string(), Value::Text(v_str.to_string()));
                }
            }
            result.insert("headers".to_string(), Value::Map(header_map));
            
            stack.push(Value::Map(result))?;
            Ok((stack, ctx))
        })
    }));

    // http-request: (method url body headers -- response) - generic HTTP request
    dict.register(Tool::native("http-request", "(method:Text url:Text body:Text headers:Map -- response:Map)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let headers_map = stack.pop()?.into_map()?;
            let body = stack.pop()?.into_text()?;
            let url = stack.pop()?.into_text()?;
            let method = stack.pop()?.into_text()?;
            
            let client = reqwest::Client::new();
            let method_enum = match method.to_uppercase().as_str() {
                "GET" => reqwest::Method::GET,
                "POST" => reqwest::Method::POST,
                "PUT" => reqwest::Method::PUT,
                "DELETE" => reqwest::Method::DELETE,
                "PATCH" => reqwest::Method::PATCH,
                "HEAD" => reqwest::Method::HEAD,
                "OPTIONS" => reqwest::Method::OPTIONS,
                _ => return Err(crate::error::Error::Runtime(format!("Unknown HTTP method: {}", method))),
            };
            
            let mut request = client.request(method_enum, &url).body(body);
            
            // Add headers
            for (k, v) in headers_map.iter() {
                if let Value::Text(v_str) = v {
                    request = request.header(k.as_str(), v_str.as_str());
                }
            }
            
            let response = request.send()
                .await
                .map_err(|e| crate::error::Error::io(format!("http-request: {}", e)))?;
            
            let status = response.status().as_u16() as i64;
            let resp_headers = response.headers().clone();
            let resp_body = response.text().await
                .map_err(|e| crate::error::Error::io(format!("http-request body: {}", e)))?;
            
            // Build response map
            let mut result = indexmap::IndexMap::new();
            result.insert("status".to_string(), Value::Int(status));
            result.insert("body".to_string(), Value::Text(resp_body));
            
            // Headers as a map
            let mut header_map = indexmap::IndexMap::new();
            for (k, v) in resp_headers.iter() {
                if let Ok(v_str) = v.to_str() {
                    header_map.insert(k.to_string(), Value::Text(v_str.to_string()));
                }
            }
            result.insert("headers".to_string(), Value::Map(header_map));
            
            stack.push(Value::Map(result))?;
            Ok((stack, ctx))
        })
    }));

    // === Resources ===

    // res-mem: ( -- map) - get memory quota info
    dict.register(Tool::native("res-mem", "( -- info:Map)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let info = {
                let res = ctx.resources.read().await;
                let quota = &res.mem;
                let mut info = indexmap::IndexMap::new();
                info.insert("total".to_string(), Value::Int(quota.total as i64));
                info.insert("used".to_string(), Value::Int(quota.used as i64));
                info.insert("reserved".to_string(), Value::Int(quota.reserved as i64));
                info.insert("free".to_string(), Value::Int(quota.free() as i64));
                info
            };
            stack.push(Value::Map(info))?;
            Ok((stack, ctx))
        })
    }));

    // res-rom: ( -- map) - get storage quota info
    dict.register(Tool::native("res-rom", "( -- info:Map)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let info = {
                let res = ctx.resources.read().await;
                let quota = &res.rom;
                let mut info = indexmap::IndexMap::new();
                info.insert("total".to_string(), Value::Int(quota.total as i64));
                info.insert("used".to_string(), Value::Int(quota.used as i64));
                info.insert("reserved".to_string(), Value::Int(quota.reserved as i64));
                info.insert("free".to_string(), Value::Int(quota.free() as i64));
                info
            };
            stack.push(Value::Map(info))?;
            Ok((stack, ctx))
        })
    }));

    // res-compute: ( -- map) - get compute quota info
    dict.register(Tool::native("res-compute", "( -- info:Map)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let info = {
                let res = ctx.resources.read().await;
                let quota = &res.compute;
                let mut info = indexmap::IndexMap::new();
                info.insert("total".to_string(), Value::Int(quota.total as i64));
                info.insert("used".to_string(), Value::Int(quota.used as i64));
                info.insert("reserved".to_string(), Value::Int(quota.reserved as i64));
                info.insert("free".to_string(), Value::Int(quota.free() as i64));
                info
            };
            stack.push(Value::Map(info))?;
            Ok((stack, ctx))
        })
    }));

    // res-net: ( -- map) - get network quota info
    dict.register(Tool::native("res-net", "( -- info:Map)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let info = {
                let res = ctx.resources.read().await;
                let quota = &res.net;
                let mut info = indexmap::IndexMap::new();
                info.insert("total".to_string(), Value::Int(quota.total as i64));
                info.insert("used".to_string(), Value::Int(quota.used as i64));
                info.insert("reserved".to_string(), Value::Int(quota.reserved as i64));
                info.insert("free".to_string(), Value::Int(quota.free() as i64));
                info
            };
            stack.push(Value::Map(info))?;
            Ok((stack, ctx))
        })
    }));

    // res-all: ( -- map) - get all resource quotas
    dict.register(Tool::native("res-all", "( -- info:Map)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let all = {
                let res = ctx.resources.read().await;

                let make_quota = |q: &crate::resources::ResourceQuota| {
                    let mut m = indexmap::IndexMap::new();
                    m.insert("total".to_string(), Value::Int(q.total as i64));
                    m.insert("used".to_string(), Value::Int(q.used as i64));
                    m.insert("reserved".to_string(), Value::Int(q.reserved as i64));
                    m.insert("free".to_string(), Value::Int(q.free() as i64));
                    Value::Map(m)
                };

                let mut all = indexmap::IndexMap::new();
                all.insert("mem".to_string(), make_quota(&res.mem));
                all.insert("rom".to_string(), make_quota(&res.rom));
                all.insert("compute".to_string(), make_quota(&res.compute));
                all.insert("net".to_string(), make_quota(&res.net));
                all
            };
            stack.push(Value::Map(all))?;
            Ok((stack, ctx))
        })
    }));

    // === Capabilities ===

    // cap-has: (cap -- bool) - check if capability is granted
    dict.register(Tool::native("cap-has", "(cap:Text -- result:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let cap = stack.pop()?.into_text()?;
            let has = ctx.caps.has(&cap);
            stack.push(Value::Bool(has))?;
            Ok((stack, ctx))
        })
    }));

    // cap-list: ( -- list) - list all granted capabilities
    dict.register(Tool::native("cap-list", "( -- caps:List)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let caps = ctx.caps.list();
            let list: Vec<Value> = caps.into_iter().map(Value::Text).collect();
            stack.push(Value::List(list))?;
            Ok((stack, ctx))
        })
    }));

    // cap-fs: (path mode -- bool) - check fs capability (mode: "read" or "write")
    dict.register(Tool::native("cap-fs", "(path:Text mode:Text -- result:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let mode = stack.pop()?.into_text()?;
            let path = stack.pop()?.into_text()?;
            let path = std::path::Path::new(&path);

            let can = match mode.as_str() {
                "read" => ctx.caps.can_read_path(path),
                "write" => ctx.caps.can_write_path(path),
                _ => false,
            };
            stack.push(Value::Bool(can))?;
            Ok((stack, ctx))
        })
    }));

    // cap-net: (host port mode -- bool) - check net capability (mode: "connect" or "listen")
    dict.register(Tool::native("cap-net", "(host:Text port:Int mode:Text -- result:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let mode = stack.pop()?.into_text()?;
            let port = stack.pop()?.into_int()? as u16;
            let host = stack.pop()?.into_text()?;

            let can = match mode.as_str() {
                "connect" => ctx.caps.can_connect(&host, port),
                "listen" => ctx.caps.can_listen(port),
                _ => false,
            };
            stack.push(Value::Bool(can))?;
            Ok((stack, ctx))
        })
    }));

    // === Session Memory ===

    // mem-set: (key value -- ) - store value in session memory
    dict.register(Tool::native("mem-set", "(key:Text value:Any -- )", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let value = stack.pop()?;
            let key = stack.pop()?.into_text()?;

            {
                let mut res = ctx.resources.write().await;
                ctx.memory.set(key, value, &mut res.mem).await
                    .map_err(|e| crate::error::Error::Runtime(format!("mem-set: {}", e)))?;
            }

            Ok((stack, ctx))
        })
    }));

    // mem-get: (key -- value) - get value from session memory
    dict.register(Tool::native("mem-get", "(key:Text -- value:Any)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let key = stack.pop()?.into_text()?;
            let value = ctx.memory.get(&key).await.unwrap_or(Value::Null);
            stack.push(value)?;
            Ok((stack, ctx))
        })
    }));

    // mem-del: (key -- ) - remove key from session memory
    dict.register(Tool::native("mem-del", "(key:Text -- )", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let key = stack.pop()?.into_text()?;
            {
                let mut res = ctx.resources.write().await;
                ctx.memory.del(&key, &mut res.mem).await;
            }
            Ok((stack, ctx))
        })
    }));

    // mem-has: (key -- bool) - check if key exists in session memory
    dict.register(Tool::native("mem-has", "(key:Text -- exists:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let key = stack.pop()?.into_text()?;
            let has = ctx.memory.has(&key).await;
            stack.push(Value::Bool(has))?;
            Ok((stack, ctx))
        })
    }));

    // mem-keys: ( -- list) - list all keys in session memory
    dict.register(Tool::native("mem-keys", "( -- keys:List)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let keys = ctx.memory.keys().await;
            let list: Vec<Value> = keys.into_iter().map(Value::Text).collect();
            stack.push(Value::List(list))?;
            Ok((stack, ctx))
        })
    }));

    // === Persistent Storage (ROM) ===

    // rom-set: (key value -- ) - store value in persistent storage
    dict.register(Tool::native("rom-set", "(key:Text value:Any -- )", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let value = stack.pop()?;
            let key = stack.pop()?.into_text()?;

            let storage = ctx.storage.as_ref()
                .ok_or_else(|| crate::error::Error::Runtime("rom-set: storage not available".to_string()))?;

            {
                let mut res = ctx.resources.write().await;
                storage.set(&key, &value, &mut res.rom)
                    .map_err(|e| crate::error::Error::Runtime(format!("rom-set: {}", e)))?;
            }

            Ok((stack, ctx))
        })
    }));

    // rom-get: (key -- value) - get value from persistent storage
    dict.register(Tool::native("rom-get", "(key:Text -- value:Any)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let key = stack.pop()?.into_text()?;

            let storage = ctx.storage.as_ref()
                .ok_or_else(|| crate::error::Error::Runtime("rom-get: storage not available".to_string()))?;

            let value = storage.get(&key)
                .map_err(|e| crate::error::Error::Runtime(format!("rom-get: {}", e)))?
                .unwrap_or(Value::Null);

            stack.push(value)?;
            Ok((stack, ctx))
        })
    }));

    // rom-del: (key -- ) - remove key from persistent storage
    dict.register(Tool::native("rom-del", "(key:Text -- )", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let key = stack.pop()?.into_text()?;

            let storage = ctx.storage.as_ref()
                .ok_or_else(|| crate::error::Error::Runtime("rom-del: storage not available".to_string()))?;

            {
                let mut res = ctx.resources.write().await;
                storage.del(&key, &mut res.rom)
                    .map_err(|e| crate::error::Error::Runtime(format!("rom-del: {}", e)))?;
            }

            Ok((stack, ctx))
        })
    }));

    // rom-has: (key -- bool) - check if key exists in persistent storage
    dict.register(Tool::native("rom-has", "(key:Text -- exists:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let key = stack.pop()?.into_text()?;

            let storage = ctx.storage.as_ref()
                .ok_or_else(|| crate::error::Error::Runtime("rom-has: storage not available".to_string()))?;

            let has = storage.has(&key);
            stack.push(Value::Bool(has))?;
            Ok((stack, ctx))
        })
    }));

    // rom-keys: ( -- list) - list all keys in persistent storage
    dict.register(Tool::native("rom-keys", "( -- keys:List)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let storage = ctx.storage.as_ref()
                .ok_or_else(|| crate::error::Error::Runtime("rom-keys: storage not available".to_string()))?;

            let keys = storage.keys()
                .map_err(|e| crate::error::Error::Runtime(format!("rom-keys: {}", e)))?;
            let list: Vec<Value> = keys.into_iter().map(Value::Text).collect();
            stack.push(Value::List(list))?;
            Ok((stack, ctx))
        })
    }));

    // Release lock before calling other registration functions
    drop(dict);

    // Register introspection tools
    register_introspection(ctx).await;

    // Register persistence tools  
    register_persistence(ctx).await;
}

// Helper: Convert serde_json::Value to kore Value
fn json_to_value(json: serde_json::Value) -> Value {
    match json {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Null
            }
        }
        serde_json::Value::String(s) => Value::Text(s),
        serde_json::Value::Array(arr) => {
            Value::List(arr.into_iter().map(json_to_value).collect())
        }
        serde_json::Value::Object(obj) => {
            let mut map = indexmap::IndexMap::new();
            for (k, v) in obj {
                map.insert(k, json_to_value(v));
            }
            Value::Map(map)
        }
    }
}

// Helper: Convert kore Value to serde_json::Value
fn value_to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Null => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Int(i) => serde_json::Value::Number((*i).into()),
        Value::Float(f) => {
            serde_json::Number::from_f64(*f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null)
        }
        Value::Text(s) => serde_json::Value::String(s.clone()),
        Value::List(arr) => {
            serde_json::Value::Array(arr.iter().map(value_to_json).collect())
        }
        Value::Map(map) => {
            let obj: serde_json::Map<String, serde_json::Value> = map
                .iter()
                .map(|(k, v)| (k.clone(), value_to_json(v)))
                .collect();
            serde_json::Value::Object(obj)
        }
        Value::Quote(_) => serde_json::Value::String("<quote>".to_string()),
        Value::Handle(h) => serde_json::Value::String(format!("<handle:{:?}>", h)),
        Value::Error(e) => serde_json::Value::String(format!("<error:{}>", e.message)),
    }
}

/// Register introspection tools
/// Everything is introspectable. Same interface for all.
pub async fn register_introspection(ctx: &mut Context) {
    let mut dict = ctx.dict.write().await;

    // meta: (name -- map) - get metadata for a tool
    dict.register(Tool::native("meta", "(name:Text -- meta:Map)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let name = stack.pop()?.into_text()?;
            let dict = ctx.dict.read().await;
            let tool = dict.get(&name)?;
            drop(dict);
            
            stack.push(tool.meta.to_value())?;
            Ok((stack, ctx))
        })
    }).with_doc("Get metadata for any tool. Returns map with name, sig, doc, status, stats, etc."));

    // meta!: (name key value -- ) - set metadata field on a tool
    dict.register(Tool::native("meta!", "(name:Text key:Text val:Text -- )", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let value = stack.pop()?.into_text()?;
            let key = stack.pop()?.into_text()?;
            let name = stack.pop()?.into_text()?;
            
            let mut dict = ctx.dict.write().await;
            if let Ok(mut tool) = dict.get(&name) {
                match key.as_str() {
                    "doc" => tool.meta = tool.meta.clone().with_doc(&value),
                    "status" => tool.meta.status = value,
                    _ => tool.meta.set(&key, &value),
                }
                dict.register(tool);
            }
            drop(dict);
            
            Ok((stack, ctx))
        })
    }).with_doc("Set a metadata field on a tool. Supported keys: doc, status, or any custom field."));

    // calls: (name -- list) - get immediate dependencies of a tool
    dict.register(Tool::native("calls", "(name:Text -- deps:List)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let name = stack.pop()?.into_text()?;
            let dict = ctx.dict.read().await;
            let tool = dict.get(&name)?;
            drop(dict);
            
            let deps: Vec<Value> = tool.calls().into_iter()
                .map(Value::Text)
                .collect();
            
            stack.push(Value::List(deps))?;
            Ok((stack, ctx))
        })
    }).with_doc("Get immediate dependencies (tools called) by a composed tool."));

    // graph: (name -- tree) - get full call tree
    dict.register(Tool::native("graph", "(name:Text -- tree:Map)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let name = stack.pop()?.into_text()?;
            let dict = ctx.dict.read().await;
            let tree = build_call_tree(&name, &dict, 10)?;
            drop(dict);
            
            stack.push(tree)?;
            Ok((stack, ctx))
        })
    }).with_doc("Get full call tree for a tool (recursive dependencies)."));

    // tag: (name tag -- ) - add a tag to a tool
    dict.register(Tool::native("tag", "(name:Text tag:Text -- )", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let tag = stack.pop()?.into_text()?;
            let name = stack.pop()?.into_text()?;
            
            let mut dict = ctx.dict.write().await;
            if let Ok(mut tool) = dict.get(&name) {
                tool.meta.tags.push(tag);
                dict.register(tool);
            }
            drop(dict);
            
            Ok((stack, ctx))
        })
    }).with_doc("Add a tag to a tool for discovery."));

    // find-tag: (tag -- list) - find all tools with a tag
    dict.register(Tool::native("find-tag", "(tag:Text -- names:List)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let tag = stack.pop()?.into_text()?;
            let dict = ctx.dict.read().await;
            
            let matches: Vec<Value> = dict.names()
                .filter(|name| {
                    if let Ok(tool) = dict.get(name) {
                        tool.meta.tags.contains(&tag)
                    } else {
                        false
                    }
                })
                .map(|s| Value::Text(s.to_string()))
                .collect();
            drop(dict);
            
            stack.push(Value::List(matches))?;
            Ok((stack, ctx))
        })
    }).with_doc("Find all tools with a specific tag."));

    // health: ( -- list) - get tools with warnings or errors
    dict.register(Tool::native("health", "( -- report:List)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let dict = ctx.dict.read().await;
            
            let unhealthy: Vec<Value> = dict.names()
                .filter_map(|name| {
                    if let Ok(tool) = dict.get(name) {
                        if tool.meta.status != "ok" {
                            let mut map = indexmap::IndexMap::new();
                            map.insert("name".to_string(), Value::Text(name.to_string()));
                            map.insert("status".to_string(), Value::Text(tool.meta.status.clone()));
                            if !tool.meta.warnings.is_empty() {
                                map.insert("warnings".to_string(), Value::List(
                                    tool.meta.warnings.iter().map(|w: &String| Value::Text(w.clone())).collect()
                                ));
                            }
                            if !tool.meta.errors.is_empty() {
                                map.insert("errors".to_string(), Value::List(
                                    tool.meta.errors.iter().map(|e: &String| Value::Text(e.clone())).collect()
                                ));
                            }
                            Some(Value::Map(map))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect();
            drop(dict);
            
            stack.push(Value::List(unhealthy))?;
            Ok((stack, ctx))
        })
    }).with_doc("Get all tools with warnings or errors. Returns list of {name, status, warnings, errors}."));

    // stats: (name -- map) - get just the stats for a tool
    dict.register(Tool::native("stats", "(name:Text -- stats:Map)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let name = stack.pop()?.into_text()?;
            let dict = ctx.dict.read().await;
            let tool = dict.get(&name)?;
            drop(dict);
            
            let mut map = indexmap::IndexMap::new();
            map.insert("calls".to_string(), Value::Int(tool.meta.stats.calls as i64));
            map.insert("failures".to_string(), Value::Int(tool.meta.stats.failures as i64));
            map.insert("time_ms".to_string(), Value::Int(tool.meta.stats.time_ms as i64));
            if let Some(last) = tool.meta.stats.last_call {
                map.insert("last_call".to_string(), Value::Int(last as i64));
            }
            
            stack.push(Value::Map(map))?;
            Ok((stack, ctx))
        })
    }).with_doc("Get call statistics for a tool."));
}

/// Helper: Build call tree recursively
fn build_call_tree(name: &str, dict: &crate::context::Dictionary, max_depth: usize) -> crate::error::Result<Value> {
    use crate::context::Dictionary;
    
    fn build_tree_inner(name: &str, dict: &Dictionary, depth: usize, max_depth: usize, visited: &mut std::collections::HashSet<String>) -> Value {
        if depth > max_depth || visited.contains(name) {
            return Value::Text(format!("{}...", name));
        }
        visited.insert(name.to_string());
        
        let mut map = indexmap::IndexMap::new();
        map.insert("name".to_string(), Value::Text(name.to_string()));
        
        if let Ok(tool) = dict.get(name) {
            let calls = tool.calls();
            if !calls.is_empty() {
                let children: Vec<Value> = calls.iter()
                    .map(|child| build_tree_inner(child, dict, depth + 1, max_depth, visited))
                    .collect();
                map.insert("calls".to_string(), Value::List(children));
            }
        }
        
        Value::Map(map)
    }
    
    let mut visited = std::collections::HashSet::new();
    Ok(build_tree_inner(name, dict, 0, max_depth, &mut visited))
}

/// Register persistence tools
/// Tools can be saved to ROM and loaded back.
pub async fn register_persistence(ctx: &mut Context) {
    let mut dict = ctx.dict.write().await;

    // persist: (name quote -- ) - save a tool to ROM only (not dictionary)
    dict.register(Tool::native("persist", "(name:Text body:Quote -- )", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let body = stack.pop()?.into_quote()?;
            let name = stack.pop()?.into_text()?;
            
            // Check if storage is available
            let storage = ctx.storage.as_ref()
                .ok_or_else(|| crate::error::Error::Runtime("No storage available".into()))?;
            
            // Create a serializable representation
            let tool = Tool::composed(&name, None, body);
            let tool_value = serialize_tool(&tool);
            
            // Save to ROM under "tools/" prefix
            let key = format!("tools/{}", name);
            {
                let mut resources = ctx.resources.write().await;
                storage.set(&key, &tool_value, &mut resources.rom)
                    .map_err(|e| crate::error::Error::Runtime(e.to_string()))?;
            }
            
            Ok((stack, ctx))
        })
    }).with_doc("Save a tool to persistent storage (ROM only, not loaded into dictionary)."));

    // register: (name quote -- ) - define AND persist a tool
    dict.register(Tool::native("register", "(name:Text body:Quote -- )", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let body = stack.pop()?.into_quote()?;
            let name = stack.pop()?.into_text()?;
            
            // Create tool
            let tool = Tool::composed(&name, None, body.clone());
            
            // Register in dictionary
            {
                let mut dict = ctx.dict.write().await;
                dict.register(tool.clone());
            }
            
            // Persist to ROM if storage available
            if let Some(ref storage) = ctx.storage {
                let tool_value = serialize_tool(&tool);
                let key = format!("tools/{}", name);
                let mut resources = ctx.resources.write().await;
                let _ = storage.set(&key, &tool_value, &mut resources.rom);
                drop(resources);
            }
            
            Ok((stack, ctx))
        })
    }).with_doc("Define a tool AND save it to persistent storage."));

    // load-tools: ( -- n) - load all persisted tools into dictionary
    dict.register(Tool::native("load-tools", "( -- count:Int)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let storage = ctx.storage.as_ref()
                .ok_or_else(|| crate::error::Error::Runtime("No storage available".into()))?;
            
            let keys = storage.keys()
                .map_err(|e| crate::error::Error::Runtime(e.to_string()))?;
            
            let mut count = 0;
            for key in keys {
                if key.starts_with("tools/") {
                    // Load tool
                    if let Ok(Some(value)) = storage.get(&key) {
                        if let Some(tool) = deserialize_tool(&value) {
                            let mut dict = ctx.dict.write().await;
                            dict.register(tool);
                            count += 1;
                        }
                    }
                }
            }
            
            stack.push(Value::Int(count))?;
            Ok((stack, ctx))
        })
    }).with_doc("Load all persisted tools from ROM into the dictionary."));

    // unregister: (name -- ) - remove from dictionary AND ROM
    dict.register(Tool::native("unregister", "(name:Text -- )", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let name = stack.pop()?.into_text()?;
            
            // Remove from dictionary
            {
                let mut dict = ctx.dict.write().await;
                dict.remove(&name);
            }
            
            // Remove from ROM if storage available
            if let Some(ref storage) = ctx.storage {
                let key = format!("tools/{}", name);
                let mut resources = ctx.resources.write().await;
                let _ = storage.del(&key, &mut resources.rom);
            }
            
            Ok((stack, ctx))
        })
    }).with_doc("Remove a tool from dictionary and persistent storage."));

    // list-persisted: ( -- list) - list all persisted tool names
    dict.register(Tool::native("list-persisted", "( -- names:List)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let storage = ctx.storage.as_ref()
                .ok_or_else(|| crate::error::Error::Runtime("No storage available".into()))?;
            
            let keys = storage.keys()
                .map_err(|e| crate::error::Error::Runtime(e.to_string()))?;
            
            let names: Vec<Value> = keys.into_iter()
                .filter_map(|key| {
                    if key.starts_with("tools/") {
                        Some(Value::Text(key.strip_prefix("tools/")?.to_string()))
                    } else {
                        None
                    }
                })
                .collect();
            
            stack.push(Value::List(names))?;
            Ok((stack, ctx))
        })
    }).with_doc("List all tool names saved in persistent storage."));
}

/// Serialize a tool to a Value for storage
fn serialize_tool(tool: &Tool) -> Value {
    let mut map = indexmap::IndexMap::new();
    map.insert("name".to_string(), Value::Text(tool.name.clone()));
    
    if let Some(doc) = tool.doc() {
        map.insert("doc".to_string(), Value::Text(doc.to_string()));
    }
    
    if let Some(sig) = tool.sig() {
        map.insert("sig".to_string(), Value::Text(sig.to_string()));
    }
    
    // Serialize the body (ops)
    if let crate::tool::ToolBody::Ops(ref ops) = tool.body {
        map.insert("body".to_string(), Value::Quote(ops.clone()));
    }
    
    // Serialize meta
    map.insert("meta".to_string(), tool.meta.to_value());
    
    Value::Map(map)
}

/// Deserialize a tool from a Value
fn deserialize_tool(value: &Value) -> Option<Tool> {
    let map = value.as_map().ok()?;
    
    let name = map.get("name")?.as_text().ok()?;
    let body = map.get("body")?.as_quote().ok()?;
    
    let mut tool = Tool::composed(name, None, body.clone());
    
    if let Some(doc) = map.get("doc").and_then(|v| v.text_opt()) {
        tool = tool.with_doc(doc);
    }
    
    // TODO: Restore full meta from stored value
    
    Some(tool)
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

    // === Introspection Tests ===

    #[tokio::test]
    async fn test_meta() {
        let ctx = setup().await;
        let stack = Stack::new();

        // Get metadata for "dup"
        let ops = vec![
            Op::Push(Value::Text("dup".into())),
            Op::call("meta"),
        ];
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        
        // Should return a map with name, sig, etc
        let meta = result.values()[0].as_map().unwrap();
        assert_eq!(meta.get("name").unwrap().as_text().unwrap(), "dup");
        assert!(meta.contains_key("sig"));
        assert!(meta.contains_key("stats"));
    }

    #[tokio::test]
    async fn test_calls() {
        let ctx = setup().await;
        let stack = Stack::new();

        // First define a composed tool
        let ops = vec![
            Op::Push(Value::Text("square".into())),
            Op::Quote(vec![Op::call("dup"), Op::call("mul")]),
            Op::call("def"),
            // Now get its dependencies
            Op::Push(Value::Text("square".into())),
            Op::call("calls"),
        ];
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        
        let deps = result.values()[0].as_list().unwrap();
        assert_eq!(deps.len(), 2);
        assert_eq!(deps[0].as_text().unwrap(), "dup");
        assert_eq!(deps[1].as_text().unwrap(), "mul");
    }

    #[tokio::test]
    async fn test_graph() {
        let ctx = setup().await;
        let stack = Stack::new();

        // Define a composed tool
        let ops = vec![
            Op::Push(Value::Text("square".into())),
            Op::Quote(vec![Op::call("dup"), Op::call("mul")]),
            Op::call("def"),
            // Get call graph
            Op::Push(Value::Text("square".into())),
            Op::call("graph"),
        ];
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        
        let tree = result.values()[0].as_map().unwrap();
        assert_eq!(tree.get("name").unwrap().as_text().unwrap(), "square");
        assert!(tree.contains_key("calls"));
    }

    #[tokio::test]
    async fn test_tag_and_find_tag() {
        let ctx = setup().await;
        let stack = Stack::new();

        // Define a tool
        let ops = vec![
            Op::Push(Value::Text("square".into())),
            Op::Quote(vec![Op::call("dup"), Op::call("mul")]),
            Op::call("def"),
            // Add a tag
            Op::Push(Value::Text("square".into())),
            Op::Push(Value::Text("math".into())),
            Op::call("tag"),
            // Find by tag
            Op::Push(Value::Text("math".into())),
            Op::call("find-tag"),
        ];
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        
        let matches = result.values()[0].as_list().unwrap();
        assert!(matches.iter().any(|v| v.as_text().unwrap() == "square"));
    }

    #[tokio::test]
    async fn test_health() {
        let ctx = setup().await;
        let stack = Stack::new();

        // Health should return list (possibly empty)
        let ops = vec![Op::call("health")];
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        
        assert!(result.values()[0].as_list().is_ok());
    }

    #[tokio::test]
    async fn test_stats() {
        let ctx = setup().await;
        let stack = Stack::new();

        // Get stats for "dup"
        let ops = vec![
            Op::Push(Value::Text("dup".into())),
            Op::call("stats"),
        ];
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        
        let stats = result.values()[0].as_map().unwrap();
        assert!(stats.contains_key("calls"));
        assert!(stats.contains_key("failures"));
        assert!(stats.contains_key("time_ms"));
    }

    #[tokio::test]
    async fn test_meta_set() {
        let ctx = setup().await;
        let stack = Stack::new();

        // Define a tool
        let ops = vec![
            Op::Push(Value::Text("square".into())),
            Op::Quote(vec![Op::call("dup"), Op::call("mul")]),
            Op::call("def"),
            // Set doc
            Op::Push(Value::Text("square".into())),
            Op::Push(Value::Text("doc".into())),
            Op::Push(Value::Text("Squares a number".into())),
            Op::call("meta!"),
            // Get meta to verify
            Op::Push(Value::Text("square".into())),
            Op::call("meta"),
        ];
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        
        let meta = result.values()[0].as_map().unwrap();
        assert_eq!(meta.get("doc").unwrap().as_text().unwrap(), "Squares a number");
    }

    // === Registration Tests ===

    #[tokio::test]
    async fn test_register_and_call() {
        let ctx = setup().await;
        let stack = Stack::new();

        // Register a tool (def + persist)
        let ops = vec![
            Op::Push(Value::Text("double".into())),
            Op::Quote(vec![Op::push(2), Op::call("mul")]),
            Op::call("register"),
            // Call it
            Op::push(5),
            Op::call("double"),
        ];
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        
        assert_eq!(result.values()[0].as_int().unwrap(), 10);
    }

    #[tokio::test]
    async fn test_stats_are_tracked() {
        let ctx = setup().await;
        let stack = Stack::new();

        // Call dup a few times
        let ops = vec![
            Op::push(5),
            Op::call("dup"),
            Op::call("dup"),
            Op::call("dup"),
            Op::call("drop"),
            Op::call("drop"),
            Op::call("drop"),
            Op::call("drop"),
            // Now check stats for dup
            Op::Push(Value::Text("dup".into())),
            Op::call("stats"),
        ];
        let (result, _) = execute(&ops, stack, ctx).await.unwrap();
        
        let stats = result.values()[0].as_map().unwrap();
        let calls = stats.get("calls").unwrap().as_int().unwrap();
        // Should have at least 3 calls to dup
        assert!(calls >= 3, "Expected at least 3 calls, got {}", calls);
    }
}
