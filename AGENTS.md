# Repository Guidelines

## Project Structure & Module Organization

This repository implements a Rust coding-agent runtime without an agent framework. Keep the CLI in `src/main.rs`, loop in `src/agent.rs`, DeepSeek transport in `src/llm.rs`, persistence in `src/memory.rs`, and context logic in `src/context.rs`. Put tools in `src/tools/`, integration tests in `tests/`, and design records in `docs/`. Do not commit databases, logs, secrets, or `target/`.

## Build, Test, and Development Commands

- `cargo build` - compile the debug binary.
- `cargo run -- chat --user user-a --session window-1` - start a session.
- `cargo test` - run deterministic tests.
- `cargo test --test live_deepseek -- --ignored --nocapture` - run opt-in live tests.
- `cargo fmt --all -- --check` - verify formatting.
- `cargo clippy --all-targets --all-features -- -D warnings` - enforce lint quality.
- `tokei src tests` - review code size; never use it as a merge gate.

## Scope, Size, and Quality

Build the smallest usable, functionally complete runtime: every line must implement a requirement, protect correctness or security, enable testing, or clarify the design. Avoid speculative providers, databases, UIs, generic plugin layers, and code-golf. Target 1,400-2,000 production lines and 900-1,400 test lines; these are review signals, not hard limits. Keep ordinary modules near 250 lines and functions near 40 lines. Exceed a guide when splitting would obscure the state machine.

Production paths must not use casual `unwrap`, `expect`, or `panic!`. Preserve error context without logging secrets. Validate all model-generated names and arguments. Keep writes scoped by `user_id` and `session_id`, and make side-effecting calls idempotent by `tool_call_id`. Add abstractions only for current multiple implementations; `LlmClient` and `Tool` traits are allowed for testing and registration.

## Coding and Testing Conventions

Use `rustfmt`, four-space indentation, `snake_case` for modules and functions, `UpperCamelCase` for types, and `SCREAMING_SNAKE_CASE` for constants. Name tests by behavior, such as `sessions_do_not_share_todos`. Use a scripted fake LLM for loop tests and ignored tests for the real API. Cover direct replies, multi-tool chains, malformed calls, limits, cancellation, reasoning replay, session isolation, compaction, idempotency, and traces. Every bug fix needs a reproducing test.

## Commits, Reviews, and Security

Use focused Conventional Commit subjects, such as `feat: add tool registry`. Pull requests must describe behavior, verification commands, and CLI evidence when output changes. Read `DEEPSEEK_API_KEY` only from the environment. Treat tool output as untrusted data. Shell and absolute-path tools are not sandboxed; preserve full-access and batch-approval modes.
