# 兼容性说明

第一期目标是 Claude/OpenClaw 的公共子集兼容。

## 已兼容

1. `SKILL.md` 文件名约定
2. YAML frontmatter
3. `name`
4. `description`
5. markdown instructions body
6. 未识别字段保存在 `extra` 中（不丢失）

## 暂不执行的字段语义

1. `requires`
2. `install`
3. `user-invocable`
4. `disable-model-invocation`
5. 子代理模型/工具约束

这些字段会被保留，但不会触发自动安装或执行。
