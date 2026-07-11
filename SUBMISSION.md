# Interview Submission

## Code

- Repository: [github.com/kingguuu8-svg/agent-demo](https://github.com/kingguuu8-svg/agent-demo)
- Primary implementation: Rust runtime under `src/`
- Self-hosting proof: Go runtime under `examples/go-agent-demo/`

Neither implementation uses an Agent framework. Both call the DeepSeek OpenAI-compatible API directly. API keys are supplied at runtime and are not committed.

## Run and Verify

```text
agent-demo
cargo test
```

The Windows release package contains `agent-demo.exe` and `install.cmd`. For the generated proof project:

```text
cd examples/go-agent-demo
set DEEPSEEK_API_KEY=your-key
go test ./...
go test -run TestLiveDeepSeekAgentLoop -v
go run .
```

## Recording

The terminal operation recording is supplied separately with the interview submission. It shows the Rust Agent receiving the Go project prompt, inspecting the workspace, creating multiple files, invoking tools, building/testing, correcting problems, and completing the generated Agent.

## Design and Memory Evidence

- Root [README](README.md): installation, commands, tools, sessions, and context policy.
- [System design](docs/DESIGN.md): loop state machine, persistence, scheduling, permissions, context compression, and tracing.
- [Go README](examples/go-agent-demo/README.md): generated runtime and exact memory placement and limits.

The Rust runtime recalls the session's SQLite summary and uncompacted messages before every LLM call. Tool observations are appended immediately after their corresponding assistant tool calls. Compression runs only near the configured context threshold and preserves recent complete turns. The Go proof keeps active-loop messages and tool observations in request order but deliberately does not duplicate durable cross-goal recall; that limitation is recorded in its README.

## AI Prompts and Problem-Solving Record

- Root [prompts](docs/PROMPTS.md) and [development log](docs/AI_DEVLOG.md)
- Go [generation prompt](examples/go-agent-demo/PROMPTS.md)
- Go [review and resolution log](examples/go-agent-demo/AI_DEVLOG.md)

## Test Evidence

Normal suites are deterministic and offline. Explicit ignored/gated tests exercise the real DeepSeek API and operating-system credential manager. The final submission was also manually regressed against DeepSeek using both the Rust runtime and the Go tool-decision test.
