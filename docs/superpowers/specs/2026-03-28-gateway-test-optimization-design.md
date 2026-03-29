# openjax-gateway 测试优化设计

日期：2026-03-28

## 1. 背景

`openjax-gateway` 当前已经有一定测试基础，但测试执行入口、CI 门禁和测试文件组织仍不够收敛，导致两类问题同时存在：

- 日常开发缺少明确的“快线”入口，文档、Makefile 和 CI 仍倾向直接使用 `cargo test -p openjax-gateway` 或更大范围的 workspace 全量测试。
- 集成测试虽然不再只有一个 target，但核心网关 API 测试仍集中在单个大文件中，后续新增用例容易继续堆积，维护和精准执行成本会上升。

基于当前代码状态，`openjax-gateway/tests` 现有目标大致分为：

- `gateway_api.rs`
- `policy_api_suite.rs`
- `m1_assistant_message_compat_only.rs`
- crate 内 unit tests

其中 `gateway_api.rs` 已接近千行，已明显超过“继续集中承载新增 gateway 行为测试”的合理规模。

## 2. 现状观察

### 2.1 当前测试组织

当前 `openjax-gateway` 处于“部分拆分但未收敛完成”的状态：

- 已有多个 test target，不再是单一集成测试文件。
- 但 gateway 主 API 行为仍集中在 `tests/gateway_api.rs`。
- `policy_api_suite.rs` 已具备相对清晰的主题边界。
- README 尚将 `cargo test -p openjax-gateway` 作为默认本地测试命令。
- Makefile 仅提供 `openjax-core` 的分层测试入口，没有 gateway 对应入口。
- CI 仍主要依赖 workspace 级 Rust job，没有 gateway 专用快慢线。

### 2.2 当前耗时结论

基于现状实测，结论如下：

- `cargo test -p openjax-gateway --lib --tests` 热态很快，可作为日常默认入口。
- 单独跑 gateway 各 test target 也都较快，说明测试逻辑本身不是主要瓶颈。
- `cargo test -p openjax-gateway --doc` 即使当前 0 个 doctest，也会带来明显额外成本。
- `cargo test -p openjax-gateway` 的慢主要来自完整命令路径中的 doc 阶段与批量调度成本，而不是业务断言执行本身。

因此，当前最短路径不是继续怀疑测试逻辑太慢，而是：

1. 将 gateway 的日常命令切换为快线。
2. 将 doc 阶段从默认高频路径中剥离。
3. 将大文件测试结构整理为可持续维护的 suite 形态。

## 3. 目标

本次设计目标分为两层，主次明确。

### 3.1 主目标

降低 `openjax-gateway` 日常开发测试反馈时间。

具体体现为：

- 日常开发不再默认使用完整 `cargo test -p openjax-gateway`。
- 建立 gateway 专用测试入口，明确快线、慢线和完整校验路径。
- 为 gateway 提供可复现、本地和 CI 共用的执行编排。

### 3.2 次目标

将 gateway 主 API 集成测试重组为与仓库约定一致的 `suite + 子目录用例` 结构。

具体体现为：

- 停止继续向 `gateway_api.rs` 追加新用例。
- 通过业务域拆分，支持按 target 和用例域精准执行。
- 保持测试断言语义不降级，避免“为了拆分而删逻辑”。

## 4. 非目标

本次设计明确不覆盖以下内容：

- 不重构整个 workspace 的测试门禁与 Rust CI 拓扑。
- 不修改 `openjax-gateway` 生产代码行为，除非测试可测性确有必要且调整最小。
- 不为追求“更细文件粒度”而进行机械拆分。
- 不设计兼容性补丁式流程，不新增与当前团队习惯冲突的并行测试规范。

## 5. 设计原则

### 5.1 单一默认入口

开发者、Makefile、README、CI 应围绕同一套 gateway 测试入口命名和职责工作，避免“本地一套、流水线一套、文档再一套”。

### 5.2 快慢线分离

高频反馈和完整覆盖不是同一条路径。快线优先服务开发回路，慢线承担完整性保障。

### 5.3 结构整理服务于执行效率

测试拆分不是为了文件更碎，而是为了：

- 精准运行
- 清晰定位失败域
- 降低单文件膨胀
- 对齐仓库既有测试约定

### 5.4 与现有模式对齐

优先复用 `openjax-core` 已有测试入口模式，尤其是脚本驱动、Makefile 薄封装、`*_suite.rs` 收编子用例的组织方式。

## 6. 目标结构

### 6.1 测试执行入口

为 `openjax-gateway` 提供统一脚本入口：

- `gateway-smoke`
- `gateway-fast`
- `gateway-doc`
- `gateway-full`
- `gateway-baseline`

这些入口将通过专用脚本实现，再由 Makefile 和 CI 进行薄封装。

### 6.2 测试组织终态

建议目标结构如下：

```text
openjax-gateway/tests/
├── gateway_api_suite.rs
├── gateway_api/
│   ├── m1_auth.rs
│   ├── m2_session_lifecycle.rs
│   ├── m3_slash_and_compact.rs
│   ├── m4_approval.rs
│   ├── m5_stream_and_timeline.rs
│   ├── m6_provider.rs
│   └── m7_policy_level.rs
├── policy_api_suite.rs
├── policy_api/
│   ├── m1_publish.rs
│   ├── m2_rules_crud.rs
│   ├── m3_validation.rs
│   └── m4_session_overlay.rs
└── m1_assistant_message_compat_only.rs
```

说明如下：

- `gateway_api.rs` 被收敛为 `gateway_api_suite.rs + gateway_api/`。
- `policy_api_suite.rs` 保持独立主题，不强行并入主 gateway suite。
- `m1_assistant_message_compat_only.rs` 作为独立兼容性目标继续保留，除非后续确认也需要收编。

## 7. 执行模型设计

### 7.1 gateway-smoke

用途：

- 用于最短反馈
- 验证最关键、高价值链路仍然正常

特点：

- 不追求覆盖面
- 不承担完整回归职责
- 适合开发中频繁运行

建议内容：

- 从 gateway 主 API suite 中选择极少数代表性场景
- 只覆盖最容易暴露整体回归的问题路径

### 7.2 gateway-fast

用途：

- 作为日常开发默认测试命令

职责：

- 运行 gateway 的 unit tests 与 integration tests
- 不包含 doc tests

建议实现：

- 基于 `cargo test -p openjax-gateway --lib --tests --locked`
- 在脚本中保持日志前缀和目标说明一致

这是本次设计中的一号高频入口。

### 7.3 gateway-doc

用途：

- 单独验证 doc tests 路径

职责：

- 将 doctest 路径与快线分离
- 保障完整性而不阻塞高频反馈

建议实现：

- 基于 `cargo test -p openjax-gateway --doc --locked`

即使当前 doctest 数为 0，也保留该入口，避免未来新增文档测试后又回到入口混乱状态。

### 7.4 gateway-full

用途：

- 在合并前或较重校验时使用

职责：

- 代表 gateway 的完整测试流程
- 由 `fast + doc` 组合而成

原则：

- `full` 不应成为高频默认本地命令
- 但应作为完整验证基线保留

### 7.5 gateway-baseline

用途：

- 统计冷/热耗时
- 发现慢项
- 判断优化是否真实有效

职责：

- 明确区分冷态与热态
- 输出可对比的时间统计
- 为后续治理提供证据而不是猜测

原则：

- 这是治理工具，不是日常开发入口
- 输出中需避免把冷态构建成本误判为测试逻辑耗时

## 8. 测试组织设计

### 8.1 gateway_api_suite 的职责

`gateway_api_suite.rs` 作为主 gateway API 行为的 suite 入口，只负责通过 `#[path = "..."] mod ...;` 收编子目录中的业务域测试文件。

它不应继续承载大量测试逻辑实现。

### 8.2 子目录拆分边界

建议按接口域与行为边界拆分，而不是按代码行数硬拆：

- `m1_auth.rs`
- `m2_session_lifecycle.rs`
- `m3_slash_and_compact.rs`
- `m4_approval.rs`
- `m5_stream_and_timeline.rs`
- `m6_provider.rs`
- `m7_policy_level.rs`

每个文件只聚焦一个清晰主题：

- 鉴权相关行为归为一组
- session 创建、关闭、重建归为一组
- slash/compact/clear 归为一组
- 审批相关行为归为一组
- SSE、timeline、message persistence 归为一组
- provider CRUD 归为一组
- session policy level 归为一组

### 8.3 policy_api_suite 的处理

`policy_api_suite.rs` 当前边界已较清晰，应保留独立 suite 目标，只在必要时进一步收编为 `policy_api/` 子目录结构。

也就是说：

- 不为了形式统一而强制合并
- 只在主题内部继续整理
- 保持“target 名称即主题”的可读性

### 8.4 共享 helper 原则

允许为 suite 抽取公共 helper，但必须保持克制。

允许收敛的内容：

- `app_with_api_key`
- `auth_header`
- `response_json`
- `login`
- `create_session_for_test`

不建议过度抽象的内容：

- 将不同 API 行为包装成隐藏断言语义的复杂 DSL
- 为少量重复代码引入层层 helper

原则是：

- 抽 setup，不抽测试意图
- 测试文件打开后应能快速看懂验证目的

## 9. 脚本、Makefile、CI、README 的协同设计

### 9.1 脚本职责

新增 `scripts/test/gateway.sh` 作为唯一编排入口。

其职责包括：

- 接收 `gateway-smoke`、`gateway-fast`、`gateway-doc`、`gateway-full`、`gateway-baseline`
- 输出统一命令日志
- 发现并执行 suite
- 在 baseline 中统计时间

Makefile 和 CI 不重复实现这些逻辑。

### 9.2 Makefile 职责

Makefile 只负责暴露稳定命令名：

- `make gateway-smoke`
- `make gateway-fast`
- `make gateway-doc`
- `make gateway-full`
- `make gateway-baseline`

Makefile 不直接堆叠复杂 gateway 测试逻辑，避免与脚本分叉。

### 9.3 README 职责

`openjax-gateway/README.md` 需要同步更新本地推荐命令。

核心变化：

- 不再将 `cargo test -p openjax-gateway` 写为默认日常命令
- 明确说明何时跑 `gateway-fast`
- 明确说明何时跑 `gateway-full`
- 明确说明 `gateway-doc` 是独立慢线

### 9.4 CI 职责

CI 中为 gateway 增加独立 job，但不重构整个 workspace Rust job。

建议策略：

- PR 高频反馈：跑 `gateway-fast`
- 慢线独立 job：跑 `gateway-doc`
- 更完整校验时机：跑 `gateway-full`

这样做的意义是：

- 保持本次优化边界聚焦在 gateway
- 不把项目扩大成整个 monorepo 的测试门禁治理
- 让 gateway 改动拥有更短、更针对性的反馈路径

## 10. 验收标准

### 10.1 正确性

必须满足：

- `gateway-fast` 全绿
- `gateway-doc` 全绿
- `gateway-full` 全绿
- 结构迁移前后测试行为和关键断言保持等价

### 10.2 效率

必须满足：

- 热态下 gateway 日常入口保持秒级反馈
- gateway PR 主路径不再默认包含 doc 阶段
- baseline 输出能稳定区分冷态与热态

### 10.3 可维护性

必须满足：

- gateway 新增测试遵循 `suite + 子目录用例` 约定
- README、Makefile、CI 使用统一命名
- `gateway_api.rs` 不再作为新增测试堆积入口

## 11. 风险与控制

### 11.1 helper 过度抽象

风险：

- 测试可读性下降
- 失败定位反而更难

控制：

- 仅抽取公共 setup
- 不抽象断言语义

### 11.2 suite 与脚本发现逻辑不一致

风险：

- 本地与 CI 跑到的目标不一致
- 迁移后可能漏跑部分 suite

控制：

- 脚本作为唯一编排入口
- Makefile 和 CI 都只调用脚本

### 11.3 冷态数据误导

风险：

- 将构建/链接成本误判为测试逻辑慢

控制：

- baseline 强制区分冷态和热态
- 报告中分别展示而不混合结论

## 12. 实施顺序

建议实施顺序如下：

1. 新增 `scripts/test/gateway.sh` 设计对应的命令分层。
2. 在 Makefile 中暴露 gateway 测试入口。
3. 更新 `openjax-gateway/README.md` 的推荐命令。
4. 在 CI 中增加 gateway 独立快慢线 job。
5. 将 `gateway_api.rs` 迁移为 `gateway_api_suite.rs + gateway_api/`。
6. 视需要将 `policy_api_suite.rs` 收敛到其主题子目录。
7. 补齐 `gateway-baseline` 的统计与慢项输出。

## 13. 决策总结

本次设计选择的是“入口优化 + 结构整理”的合并方案，但主次分明：

- 第一优先级是把 gateway 日常测试入口切换到快线，并让 CI 与文档同步。
- 第二优先级是把 `gateway_api.rs` 重组为可持续维护的 suite 结构。

这样既能立即降低开发反馈时间，也能阻止测试代码继续向单一大文件聚集。
