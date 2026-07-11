# Go Agent Demo

这是 Rust `agent-demo` 根据一条自然语言任务生成的精简 Go Coding Agent Runtime。它作为执行能力证明：Rust Agent 在不使用 Agent 框架的情况下，自主创建了第二个多文件 Agent，运行 Go 工具链并完成验证。

## 运行

需要 Go 1.21+ 和真实 DeepSeek Key。Key 只从环境变量读取：

```text
set DEEPSEEK_API_KEY=your-key
go run .
```

交互命令包括 `tools`、`memory`、`history`、`clear` 和 `exit`。验证命令：

```text
gofmt -w .
go vet ./...
go test ./...
go test -run TestLiveDeepSeekAgentLoop -v
```

## Runtime 设计

`LLMAgent.Run` 构造 OpenAI-compatible 请求，包含系统提示词、已注册工具的 JSON Schema 和当前目标。DeepSeek 可以返回文本或工具调用。Runtime 执行工具，把观察追加到活跃消息链，再次调用模型，直到获得最终答案或触发 20 步保护。

八个注册工具为 `calculator`、`read_file`、`write_file`、`edit_file`、`list_dir`、`shell`、mock `search` 和 `finish`。工具使用当前进程的主机权限；这个证明项目刻意没有沙箱或批准界面。

## Memory 放置方式

`Memory` 是一个由互斥锁保护、最多保存 200 条 goal、LLM 和工具观察的内存环。目标和模型文本在产生后记录，工具结果在执行后立即记录。`memory` 命令用于查看，`clear` 用于清空。

单次 `Run` 的权威 context 是 `LLMAgent.messages`：每个工具观察都会紧跟匹配的 assistant tool call，并在下一次 LLM 请求前放入消息链。

这个一次生成版本会在每个新目标开始时重置 `LLMAgent.messages`，不会把观察日志召回到后续目标。因此它证明的是活跃 Loop 内的 context 放置，而不是持久 memory。持久 session、历史恢复、SQLite 召回和 context 压缩由仓库根目录的完整 Rust Runtime 实现。

## 验证材料

- `agent_test.go`：确定性 Agent Loop、calculator、容量限制和配置测试
- `live_test.go`：可选的真实 DeepSeek 工具执行 Loop 回归
- `PROMPTS.md`：生成任务 Prompt
- `AI_DEVLOG.md`：生成过程与问题记录
