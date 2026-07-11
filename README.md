# Agent Demo

An execution-capable coding agent runtime written from scratch in Rust. It calls the real DeepSeek Chat Completions API directly and does not use LangGraph, OpenHands, OpenClaw, or another agent framework.

## Submission and Self-Hosting Proof

The [submission checklist](SUBMISSION.md) maps every interview deliverable to concrete evidence. As an end-to-end capability proof, this Rust Agent was asked to create a second agent implementation in Go. The resulting [Go Agent Demo](examples/go-agent-demo/README.md) is included with its generation prompt, issue-resolution log, deterministic tests, and an opt-in real DeepSeek regression. It is intentionally smaller than the primary runtime; the root Rust project remains the requirements-complete submission.

## Install and Start

On Windows, place `install.cmd` beside `agent-demo.exe`, then double-click `install.cmd` or run it from any terminal. It installs the command for the current user and opens first-time setup. Open a new terminal and start with:

```text
agent-demo
```

No Rust installation or GitHub connection is required by the release package. To remove the executable and PATH entry while preserving sessions and configuration:

```text
agent-demo uninstall
```

Developers building from source can use `cargo install --path .`. During development, replace `agent-demo` with `cargo run --bin agent-demo --`.

## Configure

If no API key exists, normal startup opens setup automatically. It can also be opened explicitly:

```powershell
agent-demo config
```

`agent-demo --config` is an alias. The wizard configures the model, API base URL, local user, workspace, and default permission. The DeepSeek API key is stored in the operating-system credential manager, never in `config.json`.

`DEEPSEEK_API_KEY` takes priority over the credential manager, which is useful on CI or a headless Linux host:

```powershell
$env:DEEPSEEK_API_KEY="your-key"
agent-demo
```

Optional environment overrides are `DEEPSEEK_MODEL` and `DEEPSEEK_BASE_URL`.

## Start and Resume Sessions

Start with no arguments. A new persistent session is created automatically:

```powershell
agent-demo
```

Inside the REPL:

```text
/new                         create a new session
/resume [session-id]         list or resume saved sessions
/sessions                    list recent sessions
/history [limit]             show restored conversation history
/permission [mode]           show or change execution permission
/paste                       enter a multi-line request; finish with `.`
/trace on|off                toggle detailed tool output
/status                      show active session state
/config                      show the configuration location
/help                        show commands
/exit                        quit
```

Normal terminal paste is kept as one request. `/paste` is a compatibility fallback for terminals without bracketed-paste support. While the agent is working, press `Esc` to stop the current request without losing the session. `Ctrl-C` clears the current input line, and Up/Down recall input history for the current process.

The first user message becomes a short session title. Empty sessions are discarded. `/resume` restores conversation history, tool observations, summary memory, and session-scoped todos, then prints the latest 20 user/agent messages. `/history 100` can show more. Internal reasoning and raw tool payloads are deliberately not printed.

Permissions can change without restarting:

```text
/permission require-approval
/permission full-access
```

`require-approval` shows a validated tool batch and asks before execution. `full-access` executes immediately.

## One-Shot Mode

For scripts or CI:

```powershell
agent-demo run "Use the calculator tool to compute 23*19"
agent-demo run --session s-123 --permission full-access "Continue the task"
agent-demo run --json --permission full-access "Run the tests"
```

JSON mode writes only the result envelope to stdout; runtime logs remain on stderr.

## Tools and Runtime

Six schema-registered tools are available:

- `shell`: run commands and tests
- `read_file`: paginated text reads
- `edit_file`: create files or perform one exact replacement
- `calculator`: evaluate expressions
- `search`: deterministic mock search
- `todo`: add, list, and complete session todos

All-read-only batches execute concurrently. A batch containing a mutation runs sequentially in model order. Every call ID receives a structured result, including invalid, unknown, or denied calls. Side effects are idempotent by `(user_id, session_id, tool_call_id)`.

Defaults are 80 logical LLM calls, 120 tool calls, a 60-minute deadline, and a three-identical-observation circuit breaker.

## Memory and Context

SQLite stores sessions, titles, the complete audit transcript, summaries, todos, and tool runs. Every operation is scoped by user and session. At roughly 900K estimated prompt tokens, only completed old user-turn blocks are summarized; recent turns and active tool chains remain intact. Tool-call `reasoning_content` is persisted and replayed as required by DeepSeek thinking mode.

## Verify

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo test --test live_deepseek -- --ignored --nocapture
cargo test --test keyring -- --ignored --nocapture
```

The DeepSeek test spends API credit. Offline tests use a scripted LLM.

## Execution Boundary

This project deliberately does **not** implement an OS sandbox. `shell`, absolute file reads, and absolute file edits run with the current process permissions. Approval provides human intervention, not isolation. Use `full-access` only in a trusted or disposable environment. Child shell processes do not inherit `DEEPSEEK_API_KEY`, but they can access other host resources available to the user.

See `docs/DESIGN.md`, `docs/PROMPTS.md`, `docs/AI_DEVLOG.md`, and `docs/demo-script.md`.
