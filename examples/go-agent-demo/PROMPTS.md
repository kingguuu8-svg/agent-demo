# Go Agent 生成 Prompt

Rust Agent 在一个空目录中收到以下任务：

```text
在当前目录创建一个名为 go-agent-demo 的 Go 项目。

从零实现一个最小可用的 Coding Agent Runtime，不使用任何现有 Agent 框架，直接调用 DeepSeek OpenAI-compatible Chat API。

必须实现：
1. 基本 Agent Loop：用户输入、LLM 决策、工具执行、观察结果、继续 Loop 或最终回答。
2. Schema 注册工具，至少包括 shell、read_file、edit_file。
3. 工具执行结果必须返回模型，使模型能够继续判断下一步。
4. 最大 Loop 限制、基础错误处理和人类可读的执行 trace。
5. 基础 memory，并说明它的召回时机与 context 放置方式。
6. 自动化测试和 README，解释运行方式、架构和能力边界。

要求：
- 优先使用 Go 标准库。
- API Key 只从 DEEPSEEK_API_KEY 环境变量读取，不得写入提交文件。
- 保持实现简洁，不做 Web UI、沙箱、向量数据库或插件系统。
- 运行 gofmt、go vet 和 go test ./...。
- 不要只给方案，必须创建、构建并验证完整项目。
```

录屏展示了 Rust Agent 接收该任务、创建多文件 Go 项目、调用工具、构建并完成验证的过程。
