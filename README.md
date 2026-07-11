# Agent Demo

一个使用 Rust 从零实现、具备真实执行能力的 Coding Agent Runtime。项目直接调用 DeepSeek Chat Completions API，不依赖 LangGraph、OpenHands、OpenClaw 或其他 Agent 框架。

代码仓库：[github.com/kingguuu8-svg/agent-demo](https://github.com/kingguuu8-svg/agent-demo)

## Go 自举示例

为了验证 Runtime 的多文件开发、工具调用、执行验证和纠错闭环能力，本 Agent 根据一条自然语言任务创建了一个 [Go Agent Demo](examples/go-agent-demo/README.md)。Go 版包含独立 Runtime、八个工具、离线测试和真实 DeepSeek 回归，并刻意保持较小规模；根目录 Rust 项目是功能完整的主实现。

## 安装与启动

Windows 用户把 `install.cmd` 与 `agent-demo.exe` 放在同一目录，双击 `install.cmd` 或从任意终端运行。安装脚本会写入当前用户 PATH 并启动首次配置。重新打开终端后直接执行：

```text
agent-demo
```

发布包不要求安装 Rust，也不依赖 GitHub。卸载程序及 PATH 条目、同时保留 session 和配置：

```text
agent-demo uninstall
```

开发者可从源码安装：`cargo install --path .`。开发期间也可使用 `cargo run --bin agent-demo --`。

## 配置

首次启动若没有 API Key，会自动进入配置向导；也可手动执行：

```text
agent-demo config
```

`agent-demo --config` 是等价入口。向导配置模型、API 地址、本地用户、工作目录和默认权限。DeepSeek API Key 存入操作系统凭据管理器，不会写入 `config.json`。

环境变量 `DEEPSEEK_API_KEY` 的优先级高于凭据管理器，适用于 CI 或无界面主机。可选覆盖项为 `DEEPSEEK_MODEL` 和 `DEEPSEEK_BASE_URL`。

## Session 与交互命令

无参数启动会自动创建一个持久 session：

```text
agent-demo
```

REPL 命令：

```text
/new                         新建 session
/resume [session-id]         列出或恢复 session
/sessions                    查看最近 session
/history [limit]             查看恢复后的对话历史
/permission [mode]           查看或修改执行权限
/paste                       输入多行任务，以单独一行 `.` 提交
/trace on|off                开关详细工具输出
/status                      查看当前状态
/config                      查看配置位置
/help                        查看帮助
/exit                        退出
```

普通多行粘贴会作为一个请求；`/paste` 是兼容性兜底。Agent 工作时按 `Esc` 可停止当前请求且不丢失 session；`Ctrl-C` 清空当前输入，方向键可浏览本进程输入历史。

首条用户消息会生成 session 标题，空 session 在退出时删除。`/resume` 恢复对话、工具观察、摘要 memory 和 session todo，并显示最近 20 条用户/Agent 消息；`/history 100` 可查看更多。界面不会回显内部 reasoning 和原始工具 JSON。

权限可在运行中切换：

```text
/permission require-approval
/permission full-access
```

`require-approval` 在执行前展示已验证的工具批次并请求确认；`full-access` 直接执行。

## 单次执行模式

用于脚本或 CI：

```text
agent-demo run "使用 calculator 计算 23*19"
agent-demo run --session s-123 --permission full-access "继续之前的任务"
agent-demo run --json --permission full-access "运行测试"
```

JSON 模式只向 stdout 输出结果对象，Runtime 日志保留在 stderr。

## 工具与 Runtime

六个 Schema 注册工具：

- `shell`：运行命令和测试
- `read_file`：分页读取文本文件
- `edit_file`：新建文件或进行一次精确替换
- `calculator`：计算表达式
- `search`：确定性 mock 搜索
- `todo`：新增、列出和完成 session todo

全只读批次并行执行；只要包含写操作，整个批次就按模型顺序串行执行。无效、未知或被拒绝的调用也会获得结构化结果。副作用通过 `(user_id, session_id, tool_call_id)` 保证幂等。

默认限制为 80 次逻辑 LLM 调用、120 次工具调用、60 分钟截止时间，以及连续三次相同观察触发无进展保护。

## Memory 与 Context

SQLite 保存 session、标题、完整审计消息、摘要、todo 和工具运行记录，所有操作都按用户与 session 隔离。每次 LLM 调用前，Runtime 读取当前 session 的摘要和未压缩消息并放入 context。每个工具结果在执行后立即追加到对应的 assistant tool call 之后。

只有估算 Prompt 接近约 900K tokens 时才触发基础压缩：仅总结已经完成的旧用户轮次，保留最近轮次和活跃工具链。DeepSeek thinking 模式要求的工具调用 `reasoning_content` 会持久化并按协议重放。

## 验证

```text
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo test --test live_deepseek -- --ignored --nocapture
cargo test --test keyring -- --ignored --nocapture
```

真实 DeepSeek 测试会消耗 API 额度；普通离线测试使用脚本化假 LLM。

## 执行边界

本项目明确不实现操作系统沙箱。`shell`、绝对路径读取和编辑都使用当前进程权限。批准机制提供人工介入，但不等于隔离；只应在可信或可丢弃环境中使用 `full-access`。Shell 子进程不会继承 `DEEPSEEK_API_KEY`，但仍可访问当前用户本来就有权限访问的其他主机资源。

更多资料见 `docs/DESIGN.md`、`docs/AI_DEVLOG.md` 和 `docs/demo-script.md`。
