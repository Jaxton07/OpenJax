# 04 tool_calls 与调度设计

## 目标
将当前单次工具调用执行模型升级为“批量声明 + 依赖调度 + 并发执行 + 结构化回注”。

## Model Decision v2 草案
```json
{
  "action": "tool_batch",
  "tool_calls": [
    {
      "tool_call_id": "call_1",
      "tool_name": "Read",
      "arguments": {"file_path": "..."},
      "depends_on": [],
      "concurrency_group": "g1"
    }
  ]
}
```

## Tool Result v2 草案
```json
{
  "tool_results": [
    {
      "tool_call_id": "call_1",
      "ok": true,
      "output_text": "...",
      "output_json": null,
      "error_code": null
    }
  ]
}
```

## 调度规则
- 无依赖节点可并发执行。
- 有依赖节点等待前置完成后再执行。
- 同 `concurrency_group` 可并发，不同组按依赖拓扑推进。
- 任一调用失败时根据策略选择：继续、局部重试、批次终止。

## 与审批的关系
- 批次内任一调用触发审批时，挂起该调用并保留其余可执行调用策略可配置。
- 审批结果必须绑定 `tool_call_id` 并回写事件流。

## 安全策略
- 高风险工具必须在执行前完成审批决策，不允许绕过。
- 批次内每个调用独立继承沙箱与权限边界，不共享越权上下文。
- 失败输出按安全策略做内容裁剪，避免敏感信息回注模型。

## 回注策略
- 批次完成后统一回注 `tool_results`。
- 模型据此续写文本或发起下一批工具调用。
