use crate::tools::registry::ToolHandler;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// 动态工具管理器
pub struct DynamicToolManager {
    registry: Arc<Mutex<HashMap<String, Arc<dyn ToolHandler>>>>,
}

impl DynamicToolManager {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 注册动态工具
    pub fn register(&self, name: String, handler: Arc<dyn ToolHandler>) {
        let mut registry = match self.registry.lock() {
            Ok(registry) => registry,
            Err(poisoned) => {
                tracing::error!("dynamic tool registry lock poisoned during register; recovering");
                poisoned.into_inner()
            }
        };
        registry.insert(name, handler);
    }

    /// 列出所有已注册的工具
    pub fn list_tools(&self) -> Vec<String> {
        let registry = match self.registry.lock() {
            Ok(registry) => registry,
            Err(poisoned) => {
                tracing::error!("dynamic tool registry lock poisoned during list; recovering");
                poisoned.into_inner()
            }
        };
        registry.keys().cloned().collect()
    }

    /// 移除工具
    pub fn unregister(&self, name: &str) -> Option<Arc<dyn ToolHandler>> {
        let mut registry = match self.registry.lock() {
            Ok(registry) => registry,
            Err(poisoned) => {
                tracing::error!(
                    "dynamic tool registry lock poisoned during unregister; recovering"
                );
                poisoned.into_inner()
            }
        };
        registry.remove(name)
    }
}

impl Default for DynamicToolManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::DynamicToolManager;
    use crate::tools::context::{FunctionCallOutputBody, ToolInvocation, ToolOutput};
    use crate::tools::error::FunctionCallError;
    use crate::tools::registry::{ToolHandler, ToolKind};
    use std::sync::Arc;

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

    #[test]
    fn poisoned_lock_does_not_panic_on_list_or_register() {
        let manager = DynamicToolManager::new();
        let registry = Arc::clone(&manager.registry);
        let _ = std::thread::spawn(move || {
            let _guard = registry.lock().expect("lock should be acquired");
            panic!("poison lock");
        })
        .join();

        assert!(manager.list_tools().is_empty());
        manager.register("noop".to_string(), Arc::new(NoopHandler));
        assert_eq!(manager.list_tools(), vec!["noop".to_string()]);
    }
}
