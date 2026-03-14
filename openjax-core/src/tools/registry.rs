use crate::tools::context::{ToolInvocation, ToolOutput, ToolPayload};
use crate::tools::error::FunctionCallError;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// 工具类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolKind {
    Function,
    Mcp,
}

/// 工具处理器 trait
#[async_trait::async_trait]
pub trait ToolHandler: Send + Sync {
    /// 返回工具类型
    fn kind(&self) -> ToolKind;

    /// 检查是否匹配载荷类型
    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(
            (self.kind(), payload),
            (ToolKind::Function, ToolPayload::Function { .. })
                | (ToolKind::Mcp, ToolPayload::Mcp { .. })
        )
    }

    /// 返回 true 如果工具调用可能修改用户环境
    async fn is_mutating(&self, _invocation: &ToolInvocation) -> bool {
        false
    }

    /// 执行工具调用并返回输出
    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError>;
}

/// 工具注册表
pub struct ToolRegistry {
    handlers: RwLock<HashMap<String, Arc<dyn ToolHandler>>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
        }
    }

    /// 注册工具处理器
    pub fn register(&self, name: impl Into<String>, handler: Arc<dyn ToolHandler>) {
        let name = name.into();
        let mut handlers = match self.handlers.write() {
            Ok(handlers) => handlers,
            Err(poisoned) => {
                tracing::error!("tool registry write lock poisoned; recovering to continue");
                poisoned.into_inner()
            }
        };
        if handlers.contains_key(&name) {
            tracing::warn!("overwriting handler for tool {}", name);
        }
        handlers.insert(name, handler);
    }

    /// 获取工具处理器
    pub fn handler(&self, name: &str) -> Option<Arc<dyn ToolHandler>> {
        let handlers = match self.handlers.read() {
            Ok(handlers) => handlers,
            Err(poisoned) => {
                tracing::error!("tool registry read lock poisoned; recovering to continue");
                poisoned.into_inner()
            }
        };
        handlers.get(name).cloned()
    }

    /// 分发工具调用
    pub async fn dispatch(
        &self,
        invocation: ToolInvocation,
    ) -> Result<ToolOutput, FunctionCallError> {
        let tool_name = invocation.tool_name.clone();
        let handler = self
            .handler(&tool_name)
            .ok_or_else(|| FunctionCallError::ToolNotFound(tool_name.clone()))?;

        if !handler.matches_kind(&invocation.payload) {
            return Err(FunctionCallError::InvalidPayload(format!(
                "tool {} does not support payload type",
                tool_name
            )));
        }

        handler.handle(invocation).await
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::context::{FunctionCallOutputBody, ToolPayload, ToolTurnContext};
    use tokio::task;

    struct NoopHandler;

    #[async_trait::async_trait]
    impl ToolHandler for NoopHandler {
        fn kind(&self) -> ToolKind {
            ToolKind::Function
        }

        async fn handle(
            &self,
            _invocation: ToolInvocation,
        ) -> Result<ToolOutput, FunctionCallError> {
            Ok(ToolOutput::Function {
                body: FunctionCallOutputBody::Text("ok".to_string()),
                success: Some(true),
            })
        }
    }

    fn make_invocation(tool_name: &str) -> ToolInvocation {
        ToolInvocation {
            tool_name: tool_name.to_string(),
            call_id: "call-1".to_string(),
            payload: ToolPayload::Function {
                arguments: "{}".to_string(),
            },
            turn: ToolTurnContext::default(),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn register_and_dispatch_can_run_concurrently() {
        let registry = Arc::new(ToolRegistry::new());
        registry.register("noop", Arc::new(NoopHandler));

        let writer_registry = Arc::clone(&registry);
        let writer = task::spawn(async move {
            for idx in 0..100 {
                writer_registry.register(format!("noop-{idx}"), Arc::new(NoopHandler));
                writer_registry.register("noop", Arc::new(NoopHandler));
            }
        });

        let reader_registry = Arc::clone(&registry);
        let reader = task::spawn(async move {
            for _ in 0..100 {
                let result = reader_registry.dispatch(make_invocation("noop")).await;
                assert!(result.is_ok());
            }
        });

        writer.await.expect("writer task should finish");
        reader.await.expect("reader task should finish");
    }
}
