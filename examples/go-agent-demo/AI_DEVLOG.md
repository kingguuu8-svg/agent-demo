# Go Agent AI 开发记录

## 生成 Prompt

```text
在当前目录创建 go-agent-demo。使用 Go 从零实现一个最小可用 Coding Agent Runtime，不使用现有 Agent 框架，直接调用 DeepSeek OpenAI-compatible Chat API。

实现用户输入、LLM 决策、工具执行、观察回传和最终回答的基本 Loop；通过 Schema 注册 shell、read_file、edit_file 等工具；加入最大 Loop、基础错误处理、trace、memory、测试和 README。优先使用标准库，API Key 只从 DEEPSEEK_API_KEY 读取。实际运行 gofmt、go vet 和 go test，不要只给方案，必须创建、构建并验证完整项目。
```

## 一条 Prompt 生成

Rust `agent-demo` 在新目录中收到上述任务。它检查 Go 环境，选择以标准库为主的目录结构，创建 API Client、Agent Loop、工具注册表、八个工具、容量受限 memory 和交互式 CLI，并完成构建。操作录屏保留了这个过程。

## 提交整理

Go Runtime 的价值是证明“主 Agent 能根据一条任务自主创建另一个 Agent”，因此没有把它继续重写成第二套生产实现。后续只做了必要的安全和验证整理：

- 排除开发时的明文 Key 和 Windows exe；
- 将 `DEEPSEEK_API_KEY` 作为唯一 Key 来源；
- 为生成的 calculator、容量受限日志和两次 LLM 调用 Loop 增加最小确定性测试；
- 增加真实 DeepSeek 端到端工具 Loop 测试；
- 在 README 中如实记录生成版本较小的 memory 语义。

## 验证结果

离线测试使用 `httptest` 模拟两次模型响应：第一次返回 calculator 工具调用，Runtime 执行后写回观察，第二次返回最终答案。真实测试使用 DeepSeek 完成同样闭环，模型自主调用 calculator 计算 `123 * 456`，最终返回 `56088`。

## 已知限制

这个证明版本使用进程内 memory，新目标会重置消息历史；工具调用串行执行；没有批准界面、session 持久化、context 压缩或取消机制。这些能力由根目录 Rust Runtime 实现。Go 项目证明的是 Rust Agent 能自主创建、构建和验证一个结构完整的多文件 Agent，而不是声称两个 Runtime 功能完全相同。
