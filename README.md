# Mini Coding Agent

A small, execution-capable coding agent runtime written from scratch in Rust. It calls the real DeepSeek Chat Completions API directly and does not use LangGraph, OpenHands, OpenClaw, or another agent framework.

## Features

- Native DeepSeek tool calling with `content`, `reasoning_content`, and `tool_calls` parsing
- Six schema-registered tools: `shell`, `read_file`, `edit_file`, `calculator`, mock `search`, and session-scoped `todo`
- Parallel execution for all-read-only batches; deterministic serial execution when a batch mutates state
- `full-access` and per-batch `require-approval` permission modes
- Persistent, isolated `(user_id, session_id)` conversations and todos in SQLite
- Idempotent tool side effects by `tool_call_id`
- Long runs with loop, tool, time, and repeated-no-progress guards
- Late context compaction near DeepSeek V4's context limit
- Structured tracing and deterministic Fake-LLM tests

## Prerequisites

- Rust stable toolchain
- A newly issued DeepSeek API key

The API key previously pasted into a conversation must be revoked. Never reuse or commit it.

## Run

```powershell
$env:DEEPSEEK_API_KEY="your-new-key"
$env:DEEPSEEK_MODEL="deepseek-v4-flash"
cargo run -- chat --user user-a --session window-1 --permission require-approval
```

For a trusted disposable environment:

```powershell
cargo run -- chat --user user-a --session window-1 --permission full-access
```

Open a second terminal with `--session window-2` to demonstrate isolation. The default database is `.mini-agent.db`; select another with `--database path.db`.

## Verify

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo test --test live_deepseek -- --ignored --nocapture
```

The live test is opt-in and requires `DEEPSEEK_API_KEY`. Offline tests use a scripted LLM and never spend API credit.

## Runtime Design

For each user message the runtime loads the session summary and uncompacted message tail, sends the registered tool schemas to DeepSeek, and parses the first assistant choice. A final `content` ends the run. Tool calls are parsed and validated before approval. Every call ID receives a tool result, including unknown tools, malformed arguments, and denied batches. Results are appended in model order and the loop continues.

All-read-only batches execute concurrently. A batch containing `shell`, `edit_file`, or a mutating Todo action executes sequentially. Defaults are 80 logical LLM calls, 120 tool calls, a 60-minute deadline, and a three-identical-observation circuit breaker.

## Memory and Context

SQLite stores the full audit transcript, session summary, todos, and tool runs. Every lookup and mutation is scoped by both user and session. Recent messages, active tool chains, the current user input, and the summary enter model context. Todo state is recalled through the `todo` tool rather than copied into every prompt.

At roughly 900K estimated prompt tokens, the runtime summarizes only completed old user-turn blocks, keeps four recent complete turns, and marks the summarized prefix as compacted. Assistant tool-call messages are never separated from their tool results. Tool-call `reasoning_content` is persisted and replayed as required by DeepSeek thinking mode.

## Execution Boundary

This project deliberately does **not** implement an OS sandbox. `shell`, absolute file reads, and absolute file edits run with the permissions of the current process. `require-approval` provides controllable human intervention, not isolation. Use `full-access` only in a disposable workspace or restricted machine. Child shell processes do not inherit `DEEPSEEK_API_KEY`, but they may still access other host resources available to the user.

See [docs/DESIGN.md](docs/DESIGN.md), [docs/PROMPTS.md](docs/PROMPTS.md), [docs/AI_DEVLOG.md](docs/AI_DEVLOG.md), and [docs/demo-script.md](docs/demo-script.md).
