//! Tool - The single entity type in Konf
//!
//! Every Tool is a Thing with:
//! - A body (the actual code)
//! - Metadata (documentation, stats, health)
//!
//! Same structure for everything. Compose for power.

use crate::context::Context;
use crate::error::Result;
use crate::meta::Meta;
use crate::op::Op;
use crate::stack::Stack;
use async_trait::async_trait;
use std::fmt;
use std::sync::Arc;

/// A tool is a named function: (Stack, Context) -> (Stack, Context)
/// Every tool carries its own metadata.
#[derive(Clone)]
pub struct Tool {
    /// Tool name (e.g., "math/double")
    pub name: String,

    /// The tool's implementation
    pub body: ToolBody,

    /// Metadata: sig, doc, stats, health, custom fields
    pub meta: Meta,
}

/// How a tool is implemented
#[derive(Clone)]
pub enum ToolBody {
    /// Native Rust function
    Native(Arc<dyn NativeFn>),

    /// Sequence of operations (interpreted)
    Ops(Vec<Op>),
}

/// Trait for native tool implementations
#[async_trait]
pub trait NativeFn: Send + Sync {
    async fn call(&self, stack: Stack, ctx: Context) -> Result<(Stack, Context)>;
}

// Allow closures to be used as NativeFn
#[async_trait]
impl<F, Fut> NativeFn for F
where
    F: Fn(Stack, Context) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<(Stack, Context)>> + Send,
{
    async fn call(&self, stack: Stack, ctx: Context) -> Result<(Stack, Context)> {
        self(stack, ctx).await
    }
}

impl Tool {
    /// Create a new native tool with effect string
    pub fn native(
        name: impl Into<String>,
        sig: &str,
        f: impl NativeFn + 'static,
    ) -> Self {
        let name = name.into();
        let mut meta = Meta::new(&name);
        if !sig.is_empty() {
            meta = meta.with_sig(sig);
        }
        Self {
            name,
            body: ToolBody::Native(Arc::new(f)),
            meta,
        }
    }

    /// Create a new composed tool
    pub fn composed(name: impl Into<String>, sig: Option<&str>, ops: Vec<Op>) -> Self {
        let name = name.into();
        let mut meta = Meta::new(&name);
        if let Some(s) = sig {
            meta = meta.with_sig(s);
        }
        Self {
            name,
            body: ToolBody::Ops(ops),
            meta,
        }
    }

    /// Add documentation
    pub fn with_doc(mut self, doc: impl Into<String>) -> Self {
        self.meta = self.meta.with_doc(doc);
        self
    }

    /// Add a tag
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.meta = self.meta.with_tag(tag);
        self
    }

    /// Get signature
    pub fn sig(&self) -> Option<&str> {
        self.meta.sig.as_deref()
    }

    /// Get documentation
    pub fn doc(&self) -> Option<&str> {
        self.meta.doc.as_deref()
    }

    /// Check if this is a native tool
    pub fn is_native(&self) -> bool {
        matches!(self.body, ToolBody::Native(_))
    }

    /// Get dependencies (calls) for composed tools
    pub fn calls(&self) -> Vec<String> {
        match &self.body {
            ToolBody::Native(_) => vec![],
            ToolBody::Ops(ops) => {
                ops.iter()
                    .filter_map(|op| {
                        if let Op::Call(name) = op {
                            Some(name.clone())
                        } else {
                            None
                        }
                    })
                    .collect()
            }
        }
    }
}

impl fmt::Debug for Tool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tool")
            .field("name", &self.name)
            .field("sig", &self.meta.sig)
            .field("doc", &self.meta.doc)
            .field(
                "body",
                match &self.body {
                    ToolBody::Native(_) => &"Native(...)",
                    ToolBody::Ops(ops) => ops,
                },
            )
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_creation() {
        let tool = Tool::composed(
            "math/double",
            Some("(n:Num -- result:Num)"),
            vec![Op::call("dup"), Op::call("add")],
        )
        .with_doc("Double a number");

        assert_eq!(tool.name, "math/double");
        assert!(tool.doc().is_some());
        assert!(tool.sig().is_some());
        assert!(!tool.is_native());
    }

    #[test]
    fn test_tool_calls() {
        let tool = Tool::composed(
            "square",
            Some("(n -- n)"),
            vec![Op::call("dup"), Op::call("mul")],
        );
        
        let deps = tool.calls();
        assert_eq!(deps, vec!["dup", "mul"]);
    }
}
