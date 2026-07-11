# 系统设计

## 范围

Mini Coding Agent 是单进程 CLI Runtime。模型根据 JSON Schema 自主决定直接回复或调用工具；Runtime 负责验证、权限、执行、持久化、预算和 trace。项目没有关键词路由，也没有使用 Agent 框架。

## CLI 产品层

安装后的命令为 `agent-demo`。无参数启动会创建持久 REPL session。`/new`、`/resume`、`/sessions`、`/history`、`/permission`、`/paste`、`/trace`、`/status`、`/config` 和 `/exit` 由确定性命令解析器拦截，不会发送给 LLM。

恢复 session 时显示压缩摘要和最近的用户/assistant 消息，但不展示 reasoning 与原始工具 payload。终端编辑器把多行粘贴保持为一个请求。执行期间的 Esc 监听器可取消 HTTP 或工具 Future；Shell 子进程启用 kill-on-drop。Runtime 会补齐被中断的对话轮次，使 session 仍可继续使用。`agent-demo run` 提供单次自动化和 JSON 输出。

普通配置保存在平台配置目录的 JSON 文件中。API Key 优先读取 `DEEPSEEK_API_KEY`，否则从操作系统凭据管理器读取，永远不会序列化进普通配置。

## 状态机

```text
用户输入 → 持久化 → 构造 context → DeepSeek
  最终文本 → 持久化 → 返回
  工具调用 → 原样持久化 assistant 协议消息
           → 解析并验证全部调用
           → 批准或生成拒绝结果
           → 只读并行 / 写操作串行
           → 每个 call ID 持久化一个结果 → 继续 Loop
```

可恢复错误会作为观察交还模型，而不是让 Runtime 崩溃，包括无效 JSON、未知工具、Schema 验证失败、Shell 非零退出、编辑不匹配、todo 不存在和用户拒绝。

## 工具契约

每个 `Tool` 提供名称、描述、JSON Schema、类型验证、影响分类和异步执行方法。`ToolOutput` 统一为 `{ok:true,data:...}` 或 `{ok:false,error:{code,message}}`。注册表负责向 DeepSeek提供定义，并按精确名称解析调用。

## 顺序与幂等

DeepSeek 一次可以返回多个工具调用。如果所有已验证调用都是只读操作，`join_all` 会并行执行并保持输入顺序；只要包含写操作，整个批次就按模型顺序串行执行。

`tool_runs` 对 `(user_id, session_id, call_id)` 建立唯一约束。重复 call ID 会复用已保存结果，不会重复执行副作用。

## Session Memory

SQLite 表包括 `sessions`、`messages`、`todos` 和 `tool_runs`。消息序号通过 immediate transaction 分配。即使旧消息前缀已在模型 context 中被摘要替代，完整消息行仍保留用于审计。

Session 使用短 ID，标题来自首条用户消息，并保存最近活动时间。空 session 会隐藏，并在退出或切换时删除。`/resume` 只切换当前 `(user_id, session_id)`，同一个 Runtime 和数据库继续服务 REPL。

### 召回时机与放置方式

每次调用 LLM 前，Runtime 读取当前 session 的摘要与 `compacted_through` 之后的活跃消息。System Prompt 放在最前；若摘要非空，作为第二条 system memory 消息；随后按数据库序号追加用户、assistant 和工具消息。工具结果在执行完成后立即作为 `role=tool` 消息写入，并通过 `tool_call_id` 与 assistant 调用对应。

## Context 压缩

估算器按消息序列化后的 Unicode 字符数保守计数。只有接近配置阈值时才压缩。Runtime 按用户轮次分组，只总结已经完成的旧轮次，最近轮次保持原样。压缩调用与普通决策共用逻辑 LLM 预算。

如果当前 context 已超过极限但没有安全可压缩的完整轮次，Runtime 会明确返回压缩错误，不会删除活跃工具链或伪造摘要。

## 权限与信任边界

权限模式为直接执行或对标准化工具批次请求一次批准。批准发生在解析与验证之后。它不是沙箱：Shell 和绝对路径工具能够使用当前进程权限访问主机。这个限制是有意的，并在 CLI 和 README 中明确展示。
