//! Algebraic capability and resource tools (10)
//!
//! These tools implement the formal algebra from algebra.rs:
//! - Capabilities form a bounded lattice with meet/join/attenuate
//! - Resources form a conservation-preserving monoid with split/add
//!
//! | Tool | Signature | Description |
//! |------|-----------|-------------|
//! | cap-list | ( -- list) | List current capabilities |
//! | cap-has | (cap -- bool) | Check if capability granted |
//! | cap-fs | (path mode -- bool) | Check fs capability |
//! | cap-net | (host port mode -- bool) | Check net capability |
//! | cap-leq | (caps1 caps2 -- bool) | Lattice ordering check |
//! | cap-meet | (caps1 caps2 -- caps) | Greatest lower bound |
//! | cap-join | (caps1 caps2 -- caps) | Least upper bound |
//! | cap-attenuate | (caps mask -- caps) | Attenuate capabilities |
//! | res-split | (m r c n ratio -- child parent) | Split resources by ratio |
//! | res-add | (m1 r1 c1 n1 m2 r2 c2 n2 -- m r c n) | Add resource bundles |
//! | res-has | (avail required -- bool) | Check resource sufficiency |

use crate::algebra;
use crate::context::{Context, Dictionary};
use crate::stack::Stack;
use crate::tool::Tool;
use crate::value::Value;

/// Register algebra tools (11)
pub fn register(dict: &mut Dictionary) {
    // cap-list: ( -- list) - list all granted capabilities
    dict.register(Tool::native(
        "cap-list",
        "( -- caps:List)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let caps = ctx.caps.list();
                let list: Vec<Value> = caps.into_iter().map(Value::Text).collect();
                stack.push(Value::List(list))?;
                Ok((stack, ctx))
            })
        },
    ));

    // cap-has: (cap -- bool) - check if specific capability is granted
    dict.register(Tool::native(
        "cap-has",
        "(cap:Text -- result:Bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let cap = stack.pop()?.into_text()?;
                let has = ctx.caps.has(&cap);
                stack.push(Value::Bool(has))?;
                Ok((stack, ctx))
            })
        },
    ));

    // cap-fs: (path mode -- bool) - check fs capability
    dict.register(Tool::native(
        "cap-fs",
        "(path:Text mode:Text -- result:Bool)",
        |mut stack: Stack, ctx: Context| {
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
        },
    ));

    // cap-net: (host port mode -- bool) - check net capability
    dict.register(Tool::native(
        "cap-net",
        "(host:Text port:Int mode:Text -- result:Bool)",
        |mut stack: Stack, ctx: Context| {
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
        },
    ));

    // cap-leq: (caps1 caps2 -- bool) - lattice ordering check
    dict.register(Tool::native(
        "cap-leq",
        "(caps1:List caps2:List -- result:Bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let caps2_list = stack.pop()?.into_list()?;
                let caps1_list = stack.pop()?.into_list()?;

                let caps1_str: String = caps1_list
                    .iter()
                    .filter_map(|v| v.as_text().ok())
                    .collect::<Vec<_>>()
                    .join(",");
                let caps2_str: String = caps2_list
                    .iter()
                    .filter_map(|v| v.as_text().ok())
                    .collect::<Vec<_>>()
                    .join(",");

                let a = algebra::CapSet::parse(&caps1_str);
                let b = algebra::CapSet::parse(&caps2_str);

                stack.push(Value::Bool(a.leq(&b)))?;
                Ok((stack, ctx))
            })
        },
    ));

    // cap-meet: (caps1 caps2 -- caps) - greatest lower bound (intersection)
    dict.register(Tool::native(
        "cap-meet",
        "(caps1:List caps2:List -- result:List)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let caps2_list = stack.pop()?.into_list()?;
                let caps1_list = stack.pop()?.into_list()?;

                let caps1_str: String = caps1_list
                    .iter()
                    .filter_map(|v| v.as_text().ok())
                    .collect::<Vec<_>>()
                    .join(",");
                let caps2_str: String = caps2_list
                    .iter()
                    .filter_map(|v| v.as_text().ok())
                    .collect::<Vec<_>>()
                    .join(",");

                let a = algebra::CapSet::parse(&caps1_str);
                let b = algebra::CapSet::parse(&caps2_str);
                let meet = a.meet(&b);

                let result: Vec<Value> = meet
                    .list()
                    .iter()
                    .map(|c| Value::Text(c.as_str().to_string()))
                    .collect();
                stack.push(Value::List(result))?;
                Ok((stack, ctx))
            })
        },
    ));

    // cap-join: (caps1 caps2 -- caps) - least upper bound (union)
    dict.register(Tool::native(
        "cap-join",
        "(caps1:List caps2:List -- result:List)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let caps2_list = stack.pop()?.into_list()?;
                let caps1_list = stack.pop()?.into_list()?;

                let caps1_str: String = caps1_list
                    .iter()
                    .filter_map(|v| v.as_text().ok())
                    .collect::<Vec<_>>()
                    .join(",");
                let caps2_str: String = caps2_list
                    .iter()
                    .filter_map(|v| v.as_text().ok())
                    .collect::<Vec<_>>()
                    .join(",");

                let a = algebra::CapSet::parse(&caps1_str);
                let b = algebra::CapSet::parse(&caps2_str);
                let join = a.join(&b);

                let result: Vec<Value> = join
                    .list()
                    .iter()
                    .map(|c| Value::Text(c.as_str().to_string()))
                    .collect();
                stack.push(Value::List(result))?;
                Ok((stack, ctx))
            })
        },
    ));

    // cap-attenuate: (caps mask -- caps') - attenuate (can only decrease)
    dict.register(Tool::native(
        "cap-attenuate",
        "(caps:List mask:List -- result:List)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let mask_list = stack.pop()?.into_list()?;
                let caps_list = stack.pop()?.into_list()?;

                let caps_str: String = caps_list
                    .iter()
                    .filter_map(|v| v.as_text().ok())
                    .collect::<Vec<_>>()
                    .join(",");
                let mask_str: String = mask_list
                    .iter()
                    .filter_map(|v| v.as_text().ok())
                    .collect::<Vec<_>>()
                    .join(",");

                let caps = algebra::CapSet::parse(&caps_str);
                let mask = algebra::CapSet::parse(&mask_str);
                let attenuated = caps.attenuate(&mask);

                let result: Vec<Value> = attenuated
                    .list()
                    .iter()
                    .map(|c| Value::Text(c.as_str().to_string()))
                    .collect();
                stack.push(Value::List(result))?;
                Ok((stack, ctx))
            })
        },
    ));

    // res-split: (mem rom compute net ratio -- child parent)
    // Splits resources by ratio, GUARANTEES conservation: child + parent = original
    dict.register(Tool::native(
        "res-split",
        "(mem:Int rom:Int compute:Int net:Int ratio:Float -- cm:Int cr:Int cc:Int cn:Int pm:Int pr:Int pc:Int pn:Int)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let ratio = stack.pop()?.as_float()?;
                let net = stack.pop()?.as_int()? as u64;
                let compute = stack.pop()?.as_int()? as u64;
                let rom = stack.pop()?.as_int()? as u64;
                let mem = stack.pop()?.as_int()? as u64;

                let res = algebra::Res::new(mem, rom, compute, net);
                let (child, parent) = res.split(ratio);

                // Push child resources
                stack.push(Value::Int(child.mem as i64))?;
                stack.push(Value::Int(child.rom as i64))?;
                stack.push(Value::Int(child.compute as i64))?;
                stack.push(Value::Int(child.net as i64))?;
                // Push parent (remaining) resources
                stack.push(Value::Int(parent.mem as i64))?;
                stack.push(Value::Int(parent.rom as i64))?;
                stack.push(Value::Int(parent.compute as i64))?;
                stack.push(Value::Int(parent.net as i64))?;

                Ok((stack, ctx))
            })
        },
    ));

    // res-add: (m1 r1 c1 n1 m2 r2 c2 n2 -- m r c n) - add two resource bundles
    dict.register(Tool::native(
        "res-add",
        "(m1:Int r1:Int c1:Int n1:Int m2:Int r2:Int c2:Int n2:Int -- m:Int r:Int c:Int n:Int)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let n2 = stack.pop()?.as_int()? as u64;
                let c2 = stack.pop()?.as_int()? as u64;
                let r2 = stack.pop()?.as_int()? as u64;
                let m2 = stack.pop()?.as_int()? as u64;
                let n1 = stack.pop()?.as_int()? as u64;
                let c1 = stack.pop()?.as_int()? as u64;
                let r1 = stack.pop()?.as_int()? as u64;
                let m1 = stack.pop()?.as_int()? as u64;

                let res1 = algebra::Res::new(m1, r1, c1, n1);
                let res2 = algebra::Res::new(m2, r2, c2, n2);
                let sum = res1.add(&res2);

                stack.push(Value::Int(sum.mem as i64))?;
                stack.push(Value::Int(sum.rom as i64))?;
                stack.push(Value::Int(sum.compute as i64))?;
                stack.push(Value::Int(sum.net as i64))?;

                Ok((stack, ctx))
            })
        },
    ));

    // res-has: (avail required -- bool) - check resource sufficiency
    dict.register(Tool::native(
        "res-has",
        "(am:Int ar:Int ac:Int an:Int rm:Int rr:Int rc:Int rn:Int -- result:Bool)",
        |mut stack: Stack, ctx: Context| {
            Box::pin(async move {
                let rn = stack.pop()?.as_int()? as u64;
                let rc = stack.pop()?.as_int()? as u64;
                let rr = stack.pop()?.as_int()? as u64;
                let rm = stack.pop()?.as_int()? as u64;
                let an = stack.pop()?.as_int()? as u64;
                let ac = stack.pop()?.as_int()? as u64;
                let ar = stack.pop()?.as_int()? as u64;
                let am = stack.pop()?.as_int()? as u64;

                let avail = algebra::Res::new(am, ar, ac, an);
                let required = algebra::Res::new(rm, rr, rc, rn);

                stack.push(Value::Bool(avail.has(&required)))?;
                Ok((stack, ctx))
            })
        },
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Context;
    use crate::executor::execute;
    use crate::op::Op;

    async fn setup() -> Context {
        let ctx = Context::new();
        let mut dict = ctx.dict.write().await;
        register(&mut dict);
        drop(dict);
        ctx
    }

    #[tokio::test]
    async fn test_cap_leq() {
        let ctx = setup().await;
        // ["fs:read"] ["fs:read", "fs:write"] cap-leq -> true
        let ops = vec![
            Op::Push(Value::List(vec![Value::Text("fs:read".into())])),
            Op::Push(Value::List(vec![
                Value::Text("fs:read".into()),
                Value::Text("fs:write".into()),
            ])),
            Op::call("cap-leq"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert!(result.values()[0].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_res_add() {
        let ctx = setup().await;
        // 100 50 200 75 + 50 25 100 25 -> 150 75 300 100
        let ops = vec![
            Op::push(100), Op::push(50), Op::push(200), Op::push(75),
            Op::push(50), Op::push(25), Op::push(100), Op::push(25),
            Op::call("res-add"),
        ];
        let (result, _) = execute(&ops, Stack::new(), ctx).await.unwrap();
        assert_eq!(result.values()[0].as_int().unwrap(), 150);
        assert_eq!(result.values()[1].as_int().unwrap(), 75);
        assert_eq!(result.values()[2].as_int().unwrap(), 300);
        assert_eq!(result.values()[3].as_int().unwrap(), 100);
    }
}
