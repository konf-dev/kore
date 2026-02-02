//! Math tools: add, sub, mul, div, eq, lt, gt, concat

use kore::{Context, Stack, Tool, Value};

/// add: (a b -- sum)
pub fn add_tool() -> Tool {
    Tool::native("add", "(a:Int b:Int -- sum:Int)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?.as_int()?;
            let a = stack.pop()?.as_int()?;
            stack.push(Value::Int(a + b))?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Add two integers.")
}

/// sub: (a b -- diff)
pub fn sub_tool() -> Tool {
    Tool::native("sub", "(a:Int b:Int -- diff:Int)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?.as_int()?;
            let a = stack.pop()?.as_int()?;
            stack.push(Value::Int(a - b))?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Subtract b from a.")
}

/// mul: (a b -- product)
pub fn mul_tool() -> Tool {
    Tool::native("mul", "(a:Int b:Int -- product:Int)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?.as_int()?;
            let a = stack.pop()?.as_int()?;
            stack.push(Value::Int(a * b))?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Multiply two integers.")
}

/// div: (a b -- quotient)
pub fn div_tool() -> Tool {
    Tool::native("div", "(a:Int b:Int -- quotient:Int)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?.as_int()?;
            let a = stack.pop()?.as_int()?;
            if b == 0 {
                return Err(kore::Error::Runtime("Division by zero".into()));
            }
            stack.push(Value::Int(a / b))?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Divide a by b.")
}

/// eq: (a b -- bool)
pub fn eq_tool() -> Tool {
    Tool::native("eq", "(a:Any b:Any -- result:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?;
            let a = stack.pop()?;
            let equal = match (&a, &b) {
                (Value::Int(a), Value::Int(b)) => a == b,
                (Value::Float(a), Value::Float(b)) => a == b,
                (Value::Text(a), Value::Text(b)) => a == b,
                (Value::Bool(a), Value::Bool(b)) => a == b,
                (Value::Null, Value::Null) => true,
                _ => false,
            };
            stack.push(Value::Bool(equal))?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Check if two values are equal.")
}

/// lt: (a b -- bool)
pub fn lt_tool() -> Tool {
    Tool::native("lt", "(a:Int b:Int -- result:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?.as_int()?;
            let a = stack.pop()?.as_int()?;
            stack.push(Value::Bool(a < b))?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Check if a is less than b.")
}

/// gt: (a b -- bool)
pub fn gt_tool() -> Tool {
    Tool::native("gt", "(a:Int b:Int -- result:Bool)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?.as_int()?;
            let a = stack.pop()?.as_int()?;
            stack.push(Value::Bool(a > b))?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Check if a is greater than b.")
}

/// concat: (a b -- ab)
pub fn concat_tool() -> Tool {
    Tool::native("concat", "(a:Text b:Text -- result:Text)", |mut stack: Stack, ctx: Context| {
        Box::pin(async move {
            let b = stack.pop()?.as_text()?.to_string();
            let a = stack.pop()?.as_text()?.to_string();
            stack.push(Value::Text(format!("{}{}", a, b)))?;
            Ok((stack, ctx))
        })
    })
    .with_doc("Concatenate two strings.")
}
