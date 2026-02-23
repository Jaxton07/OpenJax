# openjax_sdk

基于 `openjaxd`（JSONL over stdio）的 Python 异步 SDK，提供会话、turn、事件流与审批回传能力。

## 项目结构

```
python/openjax_sdk/
├── README.md                   # 项目文档
├── pyproject.toml             # Python 包配置
├── pyrightconfig.json         # 类型检查配置
├── src/
│   └── openjax_sdk/           # 主包源代码
│       ├── __init__.py        # 包入口（导出核心类型和客户端）
│       ├── client.py          # 异步客户端（daemon 生命周期、请求响应、事件流）
│       ├── models.py          # 协议数据模型（Event/Response/Error）
│       └── exceptions.py      # SDK 异常定义
└── tests/                     # 测试套件
    ├── test_client_io.py      # I/O 与读循环行为测试
    └── test_integration.py    # 与 openjaxd 的集成测试
```

## 各模块功能介绍

### 核心模块

| 模块 | 功能描述 |
|------|----------|
| `client.py` | `OpenJaxAsyncClient`：管理 daemon 启停、请求发送、响应匹配、事件订阅、审批回传和会话关闭 |
| `models.py` | 协议数据类型：`ErrorBody`、`ResponseEnvelope`、`EventEnvelope`，负责从字典反序列化 |
| `exceptions.py` | 异常类型：`OpenJaxProtocolError`（协议/流错误）与 `OpenJaxResponseError`（业务错误响应） |
| `__init__.py` | 对外导出 SDK 公共 API，统一 import 入口 |

## MVP 能力

1. daemon 生命周期管理（`start` / `stop`）
2. 会话操作（`start_session` / `shutdown_session`）
3. turn 提交（`submit_turn`）
4. 事件订阅与消费（`stream_events`、`next_event`、`iter_events`、`collect_turn_events`）
5. 审批回传（`resolve_approval`）
6. Assistant Delta 文本聚合（`assistant_text_for_turn`）

## 快速示例

```python
import asyncio
from openjax_sdk import OpenJaxAsyncClient


async def main() -> None:
    client = OpenJaxAsyncClient()
    await client.start()
    try:
        session_id = await client.start_session()
        print("session:", session_id)

        await client.stream_events()
        turn_id = await client.submit_turn("tool:list_dir dir_path=.")
        events = await client.collect_turn_events(turn_id)
        print("events:", [e.event_type for e in events])
        print("assistant:", client.assistant_text_for_turn(turn_id))
    finally:
        if client.session_id:
            await client.shutdown_session()
        await client.stop()


asyncio.run(main())
```

## 使用说明

- 默认 daemon 命令为：`cargo run -q -p openjaxd`
- 若已构建二进制，可通过 `OpenJaxAsyncClient(daemon_cmd=["target/debug/openjaxd"])` 指定命令
- 推荐在仓库根目录运行，并设置：

```bash
PYTHONPATH=python/openjax_sdk/src
```

## 测试

运行全部 SDK 测试：

```bash
PYTHONPATH=python/openjax_sdk/src \
python3 -m unittest discover -s python/openjax_sdk/tests -v
```

运行单个测试文件：

```bash
PYTHONPATH=python/openjax_sdk/src \
python3 -m unittest python/openjax_sdk/tests/test_client_io.py -v
```

## 架构特点

- **异步优先**：基于 `asyncio` 的非阻塞请求/事件模型
- **协议类型化**：使用数据类封装响应与事件，减少字典散落访问
- **流式事件消费**：支持逐条拉取和按 turn 聚合两种模式
- **错误分层清晰**：协议错误与业务错误分离，便于上层处理
