# openjax_sdk

Python async SDK for `openjaxd` (JSONL over stdio).

## MVP 能力

1. Daemon 生命周期管理（启动/停止）
2. 会话操作：`start_session` / `shutdown_session`
3. turn 操作：`submit_turn`
4. 事件订阅：`stream_events` + `next_event` / `iter_events`
5. 审批回传：`resolve_approval`
6. `AssistantDelta` 聚合缓存：`assistant_text_for_turn`

## 快速示例

```python
import asyncio
from openjax_sdk import OpenJaxAsyncClient


async def main():
    client = OpenJaxAsyncClient()
    await client.start()
    try:
        session_id = await client.start_session()
        print("session:", session_id)
        await client.stream_events()
        turn_id = await client.submit_turn("tool:list_dir dir_path=.")
        events = await client.collect_turn_events(turn_id)
        print("events:", [e.event_type for e in events])
    finally:
        if client.session_id:
            await client.shutdown_session()
        await client.stop()


asyncio.run(main())
```
