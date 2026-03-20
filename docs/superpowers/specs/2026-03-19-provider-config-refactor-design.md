# Provider 配置体系重构设计文档

**日期**：2026-03-19
**状态**：待实现
**背景**：为上下文压缩功能打基础，压缩逻辑需要知道当前模型的 context_window_size，该信息必须来自 provider 配置。

---

## 1. 目标

1. DB 作为单一配置来源（已部分完成，本次补全 `context_window_size`）
2. 系统内置主流 LLM provider 目录，用户只需填写 API Key 即可使用
3. 前端 Provider List 支持混合模式：已配置列表 + 可添加目录区
4. 内置 provider 卡片支持模型切换下拉，自定义 provider 保持原有全字段编辑

---

## 2. 设计决策

| 决策项 | 结论 |
|--------|------|
| 目录数据存放位置 | `openjax-core` 里的 Rust 静态数组，通过 `GET /api/v1/catalog` 暴露 |
| 列表布局模式 | 方案 A：上方已配置列表 + 下方虚线分隔的可添加目录区 |
| 激活 provider 位置 | 始终置顶显示 |
| 模型切换保存时机 | 即时自动保存（下拉 onChange 直接调用更新接口） |
| context_window_size 展示 | 卡片只读展示；自定义 provider 表单可编辑；内置 provider 由目录锁定 |
| DB 新增字段 | `llm_providers`：`provider_type`、`context_window_size`；`llm_runtime_settings`：`context_window_size` |
| 是否引入 catalog_id | 否。前端从目录缓存中获取 context_window_size，写入时直接存 DB，运行时无需反查目录 |
| provider_type 可变性 | 一经创建不可变更，update 接口忽略该字段 |

---

## 3. 架构概览

```
React WebUI
  │  GET /api/v1/catalog  ──────────────────────────────────┐
  │  GET /api/v1/providers                                   │
  │  POST/PUT /api/v1/providers                              ▼
  │                                               openjax-core
  ▼                                               BUILTIN_CATALOG (静态数组)
openjax-gateway
  │  读写 llm_providers（含两个新列）
  │  读写 llm_runtime_settings（含一个新列）
  ▼
SQLite (openjax-store)
  │  llm_providers.context_window_size
  │  llm_runtime_settings.context_window_size（激活快照）
  ▼
Agent Loop (build_config_from_providers)
  └─ 从 ActiveProviderRecord 直接读取 context_window_size
     → 透传到 ProviderModelConfig，零额外查询
```

---

## 4. DB 变更（openjax-store）

### 4.1 Schema Migration

```sql
-- llm_providers 新增两列
ALTER TABLE llm_providers ADD COLUMN provider_type TEXT NOT NULL DEFAULT 'custom';
ALTER TABLE llm_providers ADD COLUMN context_window_size INTEGER NOT NULL DEFAULT 0;

-- llm_runtime_settings 新增一列（激活快照同步用）
ALTER TABLE llm_runtime_settings ADD COLUMN context_window_size INTEGER NOT NULL DEFAULT 0;
```

迁移策略：`SqliteStore::open()` 时逐列检测是否存在，不存在则执行对应 `ALTER TABLE`。现有行迁移后 `provider_type='custom'`、`context_window_size=0`，不破坏已有功能。

`llm_runtime_settings` 快照中的 `context_window_size` 在以下时机同步写入：
- `set_active_provider(provider_id)` 被调用时，从 `llm_providers` 读取该行的 `context_window_size` 写入快照
- `update_provider(provider_id, ...)` 被调用且该 provider 为当前激活项时，同步更新快照列

### 4.2 ProviderRecord

```rust
pub struct ProviderRecord {
    pub provider_id: String,
    pub provider_name: String,
    pub base_url: String,
    pub model_name: String,
    pub api_key: String,
    pub provider_type: String,        // 新增："built_in" | "custom"
    pub context_window_size: u32,     // 新增
    pub created_at: String,
    pub updated_at: String,
}
```

### 4.3 ActiveProviderRecord

```rust
pub struct ActiveProviderRecord {
    pub provider_id: String,
    pub model_name: String,
    pub context_window_size: u32,     // 新增，从 llm_runtime_settings 快照读取
    pub updated_at: String,
}
```

`get_active_provider()` 直接读 `llm_runtime_settings` 快照列，无需 JOIN `llm_providers`（快照在激活/更新时已同步）。

### 4.4 ProviderRepository trait 签名变更

```rust
fn create_provider(
    &self,
    name: &str,
    base_url: &str,
    model_name: &str,
    api_key: &str,
    provider_type: &str,          // 新增："built_in" | "custom"
    context_window_size: u32,     // 新增
) -> Result<ProviderRecord>;

fn update_provider(
    &self,
    provider_id: &str,
    name: &str,
    base_url: &str,
    model_name: &str,
    api_key: Option<&str>,
    context_window_size: u32,     // 新增
    // 注意：provider_type 不在 update 签名中，一经创建不可变更
) -> Result<Option<ProviderRecord>>;
```

### 4.5 ProviderModelConfig（openjax-core/src/config.rs）

`ProviderModelConfig` 新增 `context_window_size` 字段，供 Agent 运行时消费：

```rust
pub struct ProviderModelConfig {
    // 已有字段不变...
    pub context_window_size: Option<u32>,  // 新增
}
```

`build_config_from_providers()` 在构建 `ProviderModelConfig` 时透传该值：

```rust
ProviderModelConfig {
    // 已有字段赋值不变...
    context_window_size: Some(provider.context_window_size),
}
```

---

## 5. BuiltinCatalog（openjax-core）

### 5.1 新增文件

`openjax-core/src/builtin_catalog.rs`

### 5.2 数据结构

```rust
pub struct CatalogModel {
    pub model_id: &'static str,
    pub display_name: &'static str,
    pub context_window: u32,
}

pub struct CatalogProvider {
    pub catalog_key: &'static str,
    pub display_name: &'static str,
    pub base_url: &'static str,
    pub protocol: &'static str,       // "chat_completions" | "anthropic_messages"
    pub default_model: &'static str,
    pub models: &'static [CatalogModel],
}
```

### 5.3 内置目录数据

| catalog_key | display_name | base_url | protocol | default_model |
|---|---|---|---|---|
| openai | OpenAI | https://api.openai.com/v1 | chat_completions | gpt-5.3-codex |
| anthropic | Claude (Anthropic) | https://api.anthropic.com | anthropic_messages | claude-sonnet-4-6 |
| glm_coding | GLM Coding | https://open.bigmodel.cn/api/coding/paas/v4 | chat_completions | glm-4.7 |
| kimi_coding | Kimi Coding | https://api.kimi.com/coding | chat_completions | k2.5 |
| minimax_coding | MiniMax Coding | https://api.minimaxi.com/v1 | chat_completions | MiniMax-M2.7 |

**OpenAI 模型列表**：
- gpt-5.3-codex (200k) — 默认
- gpt-5.4 (200k)
- gpt-4o (128k)
- gpt-4o-mini (128k)
- gpt-4.1 (1047576)
- gpt-4.1-mini (1047576)

**Claude 模型列表**：
- claude-opus-4-6 (200k)
- claude-sonnet-4-6 (200k) — 默认
- claude-haiku-4-5 (200k)

**GLM Coding**：glm-4.7 (200k)
**Kimi Coding**：k2.5 (256k)
**MiniMax Coding**：MiniMax-M2.7 (200k)

### 5.4 导出

`openjax-core/src/lib.rs` 导出 `builtin_catalog::{BUILTIN_CATALOG, CatalogProvider, CatalogModel}`。

---

## 6. API 变更（openjax-gateway）

### 6.1 新增接口

**`GET /api/v1/catalog`**（无需鉴权）

从 `BUILTIN_CATALOG` 静态数组序列化返回，不查 DB。

```json
{
  "providers": [
    {
      "catalog_key": "openai",
      "display_name": "OpenAI",
      "base_url": "https://api.openai.com/v1",
      "protocol": "chat_completions",
      "default_model": "gpt-5.3-codex",
      "models": [
        { "model_id": "gpt-5.3-codex", "display_name": "GPT-5.3 Codex", "context_window": 200000 }
      ]
    }
  ]
}
```

### 6.2 修改：POST /api/v1/providers

请求 body 新增：

```json
{
  "provider_name": "OpenAI",
  "base_url": "https://api.openai.com/v1",
  "model_name": "gpt-5.3-codex",
  "api_key": "sk-...",
  "provider_type": "built_in",
  "context_window_size": 200000
}
```

`provider_type` 默认 `"custom"`，`context_window_size` 默认 `0`。

### 6.3 修改：PUT /api/v1/providers/:id

请求 body 新增 `context_window_size`。此接口同时承担模型切换职责（前端切换模型时传新 `model_name` + 对应 `context_window_size`）。

**注意**：即使请求 body 中包含 `provider_type` 字段，gateway handler 也应静默忽略，不写入 DB。类型一经创建不可变更。

### 6.4 修改：active provider 响应

`GET /api/v1/providers/active` 和激活接口的响应新增 `context_window_size`：

```json
{
  "active_provider": {
    "provider_id": "prov_xxx",
    "model_name": "gpt-5.3-codex",
    "context_window_size": 200000
  }
}
```

### 6.5 路由注册

`GET /api/v1/catalog` 注册在 `lib.rs` 中 `build_app()` 的顶层 Router（与 `/healthz`、`/readyz` 同级），不经过 `access_token_middleware`。具体实现：在组装 `protected` Router 之前，先将 catalog 路由合并进 `app`：

```rust
let app = Router::new()
    .route("/healthz", get(healthz))
    .route("/readyz", get(readyz))
    .route("/api/v1/catalog", get(handlers::get_catalog))  // 公开，无需鉴权
    .merge(protected_router);  // 已有受保护路由
```

---

## 7. 前端变更（ui/web）

### 7.1 类型扩展（types/gateway.ts）

```typescript
interface CatalogModel {
  model_id: string;
  display_name: string;
  context_window: number;
}

interface CatalogProvider {
  catalog_key: string;
  display_name: string;
  base_url: string;
  protocol: string;
  default_model: string;
  models: CatalogModel[];
}

interface LlmProvider {
  // 已有字段不变
  provider_type: "built_in" | "custom";  // 新增
  context_window_size: number;            // 新增
}
```

### 7.2 客户端（lib/gatewayClient.ts）

新增 `fetchCatalog(): Promise<CatalogProvider[]>`，调用 `GET /api/v1/catalog`。

### 7.3 ProviderForm（components/settings/ProviderForm.tsx）

`ProviderFormValue` 新增：
```typescript
interface ProviderFormValue {
  providerName: string;
  baseUrl: string;
  modelName: string;
  apiKey: string;
  providerType: "built_in" | "custom";  // 新增
  contextWindowSize: number;             // 新增
  catalogModels?: CatalogModel[];        // 新增，仅内置模式使用
}
```

**内置模式**（`providerType = "built_in"`）：
- 名称、Base URL 只读展示
- 模型名称改为 `<select>` 下拉，选中时同步更新 `contextWindowSize`
- 上下文窗口大小只读展示（格式：`200,000 tokens`）
- 仅 API Key 可输入

**自定义模式**（`providerType = "custom"`）：
- 保持现有全部字段可编辑
- 新增 `contextWindowSize` 数字输入框（必填，placeholder：`如 128000`）

### 7.4 ProviderListPanel（components/settings/ProviderListPanel.tsx）

新增 props：
```typescript
interface ProviderListPanelProps {
  // 已有 props 不变
  catalog: CatalogProvider[];
  onSwitchModel: (providerId: string, modelId: string, contextWindow: number) => Promise<void>;
}
```

**已配置区**：
- 激活的 provider 排第一（按 `activeProviderId` 排序）
- 内置 provider 卡片显示：模型下拉 + 上下文窗口大小（只读）
- 下拉 `onChange` 调用 `onSwitchModel`，即时保存，卡片短暂显示"已切换"提示（2s 后消失）

**可添加区**（下方虚线分隔）：

匹配逻辑：目录条目是否已配置，通过 `base_url` 匹配，比较前双端去除末尾 `/`：

```typescript
const normalize = (url: string) => url.replace(/\/+$/, "");
const configured = catalog.filter(entry =>
  !providers.some(
    p => p.provider_type === "built_in" &&
         normalize(p.base_url) === normalize(entry.base_url)
  )
);
```

未配置的目录条目显示在下方区域，每条展示品牌名、默认模型名、默认 context_window，右侧「+ 配置」按钮。点击「+ 配置」→ 打开右侧表单，以内置模式预填（名称、base_url、默认模型、context_window_size 锁定，仅 API Key 可填）。

### 7.5 SettingsModal（components/SettingsModal.tsx）

- 新增 `onFetchCatalog` prop
- 打开 Provider tab 时并行请求 `onListProviders()` 和 `onFetchCatalog()`
- 新增 `handleSwitchModel`：复用 `onUpdateProvider`，传入新 `model_name` + `context_window_size`，静默保存

**重构建议**：目录加载逻辑和模型切换逻辑下沉到 `ProviderListPanel` 内部自管理（接收 `gatewayClient` 或 fetch 回调），减少 `SettingsModal` props 数量，避免继续膨胀。

---

## 8. 数据流总结

### 打开设置页
1. 前端并行请求 `GET /api/v1/catalog` 和 `GET /api/v1/providers`
2. Gateway：`/catalog` 直接从静态数组返回，`/providers` 查 DB
3. 前端对比两个列表，已配置显示在上方，未配置的内置条目显示在下方

### 添加内置 Provider
1. 用户点「+ 配置」→ 表单预填（名称/URL/模型/窗口大小锁定）
2. 用户填入 API Key 提交 → `POST /api/v1/providers`（含 `provider_type="built_in"` 和 `context_window_size`）
3. 新 provider 出现在上方列表，底部目录区移除该条目

### 切换模型
1. 用户操作内置 provider 卡片上的下拉
2. 前端从本地 catalog 缓存取出新模型的 `context_window_size`
3. 立即发送 `PUT /api/v1/providers/:id`（含新 `model_name` + `context_window_size`）
4. DB 更新 `llm_providers` 行；若为激活 provider，同步更新 `llm_runtime_settings` 快照
5. 卡片短暂显示"已切换"提示

### Agent 运行时
1. `get_active_provider()` 直接从 `llm_runtime_settings` 快照读取（含 `context_window_size`），零 JOIN
2. `build_config_from_providers()` 将 `context_window_size` 透传到 `ProviderModelConfig`
3. 上下文压缩逻辑从 `ProviderModelConfig.context_window_size` 读取窗口大小

---

## 9. 文件变更清单

| 文件 | 变更类型 |
|------|----------|
| `openjax-store/src/types.rs` | 修改：ProviderRecord / ActiveProviderRecord 新增字段 |
| `openjax-store/src/repository.rs` | 修改：trait 方法签名 |
| `openjax-store/src/sqlite.rs` | 修改：migration（含 llm_runtime_settings）+ CRUD SQL + 快照同步 + 测试 |
| `openjax-core/src/config.rs` | 修改：ProviderModelConfig 新增 `context_window_size: Option<u32>` |
| `openjax-core/src/builtin_catalog.rs` | 新增：静态目录数据 |
| `openjax-core/src/lib.rs` | 修改：导出新模块 |
| `openjax-core/src/provider_store.rs` | 修改：`build_config_from_providers` 透传 `context_window_size` |
| `openjax-gateway/src/handlers.rs` | 修改：provider CRUD handler（忽略 provider_type on update）+ 新增 catalog handler |
| `openjax-gateway/src/state.rs` | 修改：provider 方法传参 |
| `openjax-gateway/src/lib.rs` | 修改：顶层注册 catalog 公开路由（与 /healthz 同级） |
| `ui/web/src/types/gateway.ts` | 修改：类型扩展 |
| `ui/web/src/lib/gatewayClient.ts` | 修改：新增 fetchCatalog |
| `ui/web/src/components/settings/ProviderForm.tsx` | 修改：双模式表单 |
| `ui/web/src/components/settings/ProviderListPanel.tsx` | 修改：模型下拉 + 可添加区 + URL normalize 匹配 |
| `ui/web/src/components/settings/ProviderEditorPanel.tsx` | 修改：传入 catalogModels |
| `ui/web/src/components/SettingsModal.tsx` | 修改：catalog 加载 + 模型切换回调 |
| `ui/web/src/styles/settings.provider.css` | 修改：新样式（下拉、窗口大小展示、目录区） |

---

## 10. 测试要点

- `openjax-store`：migration 不破坏已有数据；`llm_runtime_settings` 快照在激活和更新时正确同步 `context_window_size`；新增字段 CRUD 正确
- `openjax-gateway`：`GET /api/v1/catalog` 无需鉴权可访问；PUT handler 静默忽略 `provider_type`；provider 创建/更新含新字段正确写入
- `openjax-core`：`build_config_from_providers` 将 `context_window_size` 正确透传到 `ProviderModelConfig`
- 前端：目录加载与 provider 列表对比逻辑（含 URL normalize）；模型切换即时保存；自定义模式表单验证

---

## 11. 不在本次范围内

- 上下文压缩的具体实现（本次只是打基础，`context_window_size` 进入 `ProviderModelConfig` 即完成）
- TUI 侧的 provider 配置 UI
- Provider 连接测试功能增强
- 内置目录数据的远程更新机制（目录目前为编译期静态数组，更新需重新发布；动态更新机制为后续工作）
