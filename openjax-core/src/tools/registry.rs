use crate::tools::context::{ToolInvocation, ToolOutput, ToolPayload};
use crate::tools::error::FunctionCallError;
use std::collections::HashMap;
use std::sync::Arc;

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
    handlers: HashMap<String, Arc<dyn ToolHandler>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// 注册工具处理器
    pub fn register(&mut self, name: impl Into<String>, handler: Arc<dyn ToolHandler>) {
        let name = name.into();
        if self.handlers.contains_key(&name) {
            tracing::warn!("overwriting handler for tool {}", name);
        }
        self.handlers.insert(name, handler);
    }

    /// 获取工具处理器
    pub fn handler(&self, name: &str) -> Option<Arc<dyn ToolHandler>> {
        self.handlers.get(name).cloned()
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
