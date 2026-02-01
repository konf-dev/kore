//! Tool - The single entity type in Konf

use crate::context::Context;
use crate::effect::Effect;
use crate::error::Result;
use crate::op::Op;
use crate::stack::Stack;
use async_trait::async_trait;
use std::fmt;
use std::sync::Arc;

/// A tool is a named function: (Stack, Context) -> (Stack, Context)
#[derive(Clone)]
pub struct Tool {
    /// Tool name (e.g., "math/double")
    pub name: String,

    /// Documentation
    pub doc: Option<String>,

    /// Type signature (optional but recommended)
    pub effect: Option<Effect>,

    /// The tool's implementation
    pub body: ToolBody,
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
        effect_str: &str,
        f: impl NativeFn + 'static,
    ) -> Self {
        let effect = if effect_str.is_empty() {
            None
        } else {
            Effect::parse(effect_str).ok()
        };
        Self {
            name: name.into(),
            doc: None,
            effect,
            body: ToolBody::Native(Arc::new(f)),
        }
    }

    /// Create a new native tool with optional effect
    pub fn native_opt(
        name: impl Into<String>,
        effect: Option<Effect>,
        f: impl NativeFn + 'static,
    ) -> Self {
        Self {
            name: name.into(),
            doc: None,
            effect,
            body: ToolBody::Native(Arc::new(f)),
        }
    }

    /// Create a new composed tool
    pub fn composed(name: impl Into<String>, effect: Option<Effect>, ops: Vec<Op>) -> Self {
        Self {
            name: name.into(),
            doc: None,
            effect,
            body: ToolBody::Ops(ops),
        }
    }

    /// Add documentation
    pub fn with_doc(mut self, doc: impl Into<String>) -> Self {
        self.doc = Some(doc.into());
        self
    }

    /// Check if this is a native tool
    pub fn is_native(&self) -> bool {
        matches!(self.body, ToolBody::Native(_))
    }
}

impl fmt::Debug for Tool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tool")
            .field("name", &self.name)
            .field("doc", &self.doc)
            .field("effect", &self.effect)
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
    use crate::effect::Effect;

    #[test]
    fn test_tool_creation() {
        let tool = Tool::composed(
            "math/double",
            Some(Effect::parse("(n:Num -- result:Num)").unwrap()),
            vec![Op::call("dup"), Op::call("add")],
        )
        .with_doc("Double a number");

        assert_eq!(tool.name, "math/double");
        assert!(tool.doc.is_some());
        assert!(tool.effect.is_some());
        assert!(!tool.is_native());
    }
}
