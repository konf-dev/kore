use kore::{Context, register_builtins, Op, Stack, Value};
use kore::context::TenantId;
use std::mem;

#[tokio::main]
async fn main() {
    let mut ctx = Context::new();
    register_builtins(&mut ctx).await;
    
    // Check tools before take
    {
        let dict = ctx.dict.read().await;
        let tools = dict.list(&TenantId::default());
        println!("Before take: {:?}", tools);
    }
    
    // Simulate what execute_workflow does
    let ctx_taken = mem::take(&mut ctx);
    
    // Check original after take
    {
        let dict = ctx.dict.read().await;
        let tools = dict.list(&TenantId::default());
        println!("Original after take: {:?}", tools);
    }
    
    // Check taken context
    {
        let dict = ctx_taken.dict.read().await;
        let tools = dict.list(&TenantId::default());
        println!("Taken context: {:?}", tools);
    }
}
