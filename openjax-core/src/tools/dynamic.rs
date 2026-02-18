use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::tools::registry::ToolHandler;

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
        let mut registry = self.registry.lock().unwrap();
        registry.insert(name, handler);
    }

    /// 列出所有已注册的工具
    pub fn list_tools(&self) -> Vec<String> {
        let registry = self.registry.lock().unwrap();
        registry.keys().cloned().collect()
    }

    /// 移除工具
    pub fn unregister(&self, name: &str) -> Option<Arc<dyn ToolHandler>> {
        let mut registry = self.registry.lock().unwrap();
        registry.remove(name)
    }
}

impl Default for DynamicToolManager {
    fn default() -> Self {
        Self::new()
    }
}
