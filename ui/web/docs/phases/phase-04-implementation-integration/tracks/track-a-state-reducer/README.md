# Track A - State & Reducer

## 目标
- 让状态层可表达 ToolStep 结构化数据并支持幂等更新。

## 任务范围
- 扩展 chat types（ToolStep 与 message payload）。
- 调整 event reducer 的 step 聚合逻辑。
- 保持旧消息回退不受影响。

## 交付物
- 类型定义变更说明。
- reducer 映射逻辑说明。
