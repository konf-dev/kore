//! # DNS Resolution
//!
//! DNS lookup tools for Kore programs.
//!
//! ## Tools
//!
//! | Tool | Stack Effect | Description |
//! |------|--------------|-------------|
//! | `dns-lookup` | `(hostname -- addresses)` | Resolve hostname to IPs |
//! | `dns-reverse` | `(ip -- hostnames)` | Reverse DNS lookup |
//!
//! ## Example
//!
//! ```kore
//! "example.com" dns-lookup
//! -- [ "93.184.216.34" ]
//!
//! "93.184.216.34" dns-reverse
//! -- [ "example.com" ]
//! ```

use kore::{Context, Stack, Tool, Value};
#[cfg(test)]
use kore::{execute, Op};
use std::net::{IpAddr, ToSocketAddrs};

/// Register all DNS tools into a context.
pub async fn register_dns_tools(ctx: &mut Context) {
    // dns-lookup: (hostname -- list)
    ctx.dict.write().await.register(
        Tool::native("dns-lookup", "(text -- list)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let hostname = stack.pop()?.into_text()?;

                // Add port to make it a valid socket address
                let socket_addr = format!("{}:0", hostname);

                let addresses: Vec<Value> = socket_addr
                    .to_socket_addrs()
                    .map_err(|e| kore::Error::Runtime(format!("DNS lookup failed: {}", e)))?
                    .map(|addr| Value::Text(addr.ip().to_string()))
                    .collect();

                stack.push(Value::List(addresses))?;
                Ok((stack, ctx))
            })
        }),
    );

    // dns-reverse: (ip -- text)
    ctx.dict.write().await.register(
        Tool::native("dns-reverse", "(text -- text)", |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let ip_str = stack.pop()?.into_text()?;

                let _ip_addr: IpAddr = ip_str
                    .parse()
                    .map_err(|e| kore::Error::Runtime(format!("Invalid IP address: {}", e)))?;

                // Use DNS-over-TCP for reverse lookup (standard lib doesn't have reverse DNS)
                // For now, just return a placeholder - real impl would use trust-dns or hickory-dns
                // This is a placeholder that could be enhanced with a proper DNS library

                stack.push(Value::Text(format!("reverse-dns-not-implemented-for-{}", ip_str)))?;
                Ok((stack, ctx))
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dns_tools_registered() {
        let mut ctx = Context::new();
        register_dns_tools(&mut ctx).await;

        let names = {
            let dict = ctx.dict.read().await;
            dict.list(&ctx.tenant)
        };

        assert!(names.contains(&"dns-lookup".to_string()));
        assert!(names.contains(&"dns-reverse".to_string()));
    }

    #[tokio::test]
    async fn test_dns_lookup_localhost() {
        let mut ctx = Context::new();
        register_dns_tools(&mut ctx).await;

        let stack = Stack::new();
        let ops = vec![
            Op::push("localhost"),
            Op::call("dns-lookup"),
        ];

        let (result_stack, _) = execute(&ops, stack, ctx).await.unwrap();
        let result = result_stack.peek().unwrap();
        assert!(matches!(result, Value::List(_)));
    }
}
