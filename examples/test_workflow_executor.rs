use kore::{Context, register_builtins, Op, Value, Stack, execute};
use kore::context::TenantId;

#[tokio::main]
async fn main() {
    let mut ctx = Context::new();
    register_builtins(&mut ctx).await;
    
    // Simple test: push 42, dup
    let ops = vec![Op::push(42), Op::call("dup")];
    
    // Execute op by op like the workflow executor does
    let mut stack = Stack::new();
    
    for (i, op) in ops.iter().enumerate() {
        println!("Executing op {}: {:?}", i, op);
        
        // Check context before execution
        {
            let dict = ctx.dict.read().await;
            let tools = dict.list(&TenantId::default());
            println!("  Context before: {} tools", tools.len());
        }
        
        let single_op = std::slice::from_ref(op);
        let taken_stack = std::mem::take(&mut stack);
        let taken_ctx = std::mem::take(&mut ctx);
        
        // Check taken context
        {
            let dict = taken_ctx.dict.read().await;
            let tools = dict.list(&TenantId::default());
            println!("  Taken context: {} tools", tools.len());
        }
        
        match execute(single_op, taken_stack, taken_ctx).await {
            Ok((new_stack, new_ctx)) => {
                stack = new_stack;
                ctx = new_ctx;
                println!("  Success! Stack: {:?}", stack.values());
            }
            Err(e) => {
                println!("  ERROR: {:?}", e);
                return;
            }
        }
    }
    
    println!("\nFinal stack: {:?}", stack.values());
}
