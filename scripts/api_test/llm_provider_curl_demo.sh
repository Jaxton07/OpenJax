curl -X POST "https://open.bigmodel.cn/api/coding/paas/v4/chat/completions" \
-H "Content-Type: application/json" \
-H "Authorization: Bearer 6b53f844f47f4cd1a38538aa1ae5569e.pc8qhkqvn0Z8Z7Q9" \
-d '{
    "model": "glm-4.7",
    "messages": [
        {
            "role": "system",
            "content": "你是一个有用的AI助手。"
        },
        {
            "role": "user",
            "content": "你好，请介绍一下自己。"
        }
    ],
    "temperature": 1.0,
    "stream": true
}'




curl -X POST "https://newapi.stonefancyx.com/v1/chat/completions" \
-H "Content-Type: application/json" \
-H "Authorization: Bearer sk-ORVwcvjOW2ulitELUY5HrmskxudZkxvD8S5e2NYzGMwPXX46" \
-d '{
    "model": "gpt-5.3-codex",
    "messages": [
        {
            "role": "system",
            "content": "你是一个有用的AI助手。"
        },
        {
            "role": "user",
            "content": "你好，请介绍一下自己。"
        }
    ],
    "temperature": 1.0,
    "stream": true
}'