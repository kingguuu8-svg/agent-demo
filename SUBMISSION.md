# 面试提交清单

## 代码

- 仓库：[github.com/kingguuu8-svg/agent-demo](https://github.com/kingguuu8-svg/agent-demo)
- 主实现：`src/` 下的 Rust Runtime
- 自举证明：`examples/go-agent-demo/` 下的 Go Runtime

两个实现均未使用 Agent 框架，均直接调用 DeepSeek OpenAI-compatible API。API Key 仅在运行时提供，不会提交到仓库。

## 运行与验证

```text
agent-demo
cargo test
```

Windows 发布包包含 `agent-demo.exe` 与 `install.cmd`。Go 证明项目的验证方式：

```text
cd examples/go-agent-demo
set DEEPSEEK_API_KEY=your-key
go test ./...
go test -run TestLiveDeepSeekAgentLoop -v
go run .
```

## 操作录屏

录屏文件随面试交付包提交。视频展示 Rust Agent 接收 Go 项目任务、检查工作区、创建多个文件、调用工具、构建测试、处理问题并完成另一个 Agent。

## 设计与 Memory 证据

- 根目录 [README](README.md)：安装、命令、工具、session 和 context 策略
- [系统设计](docs/DESIGN.md)：Loop、持久化、调度、权限、context 压缩和 trace
- [Go README](examples/go-agent-demo/README.md)：生成版 Runtime 的 memory 放置方式与能力边界

Rust Runtime 在每次 LLM 调用前召回 SQLite 中的 session 摘要和未压缩消息；工具观察紧跟对应工具调用写入。只有接近 context 阈值时才压缩，并保留最近的完整轮次。Go 证明在单次活跃 Loop 中保持消息与工具观察顺序，但不重复实现跨任务持久召回，相关限制已在其 README 中明确说明。

## AI Prompt 与问题解决记录

- 主项目 [Prompt](docs/PROMPTS.md) 与 [开发记录](docs/AI_DEVLOG.md)
- Go [生成 Prompt](examples/go-agent-demo/PROMPTS.md)
- Go [生成与整理记录](examples/go-agent-demo/AI_DEVLOG.md)

## 测试证据

常规测试均为确定性离线测试。显式 ignored/gated 测试会调用真实 DeepSeek API 和操作系统凭据管理器。最终版本已分别使用 Rust 与 Go 对真实 DeepSeek 完成回归。
