use kore::{Context, register_builtins, Op, Stack, execute};
use kore::context::TenantId;

#[tokio::main]
async fn main() {
    let mut ctx = Context::new();
    register_builtins(&mut ctx).await;
    
    // Check if dup exists
    let dict = ctx.dict.read().await;
    let tools = dict.list(&TenantId::default());
    println!("Registered tools: {:?}", tools);
    drop(dict);
    
    // Try to execute
    let ops = vec![Op::push(42), Op::call("dup")];
    let stack = Stack::new();
    
    match execute(&ops, stack, ctx).await {
        Ok((s, _)) => println!("Success! Stack: {:?}", s.values()),
        Err(e) => println!("Error: {:?}", e),
    }
}
