# AI Development Log

## Problem Framing

The assignment requested a minimal agent loop, schema-based tools, sessions, context management, errors, traces, tests, a real LLM API, documentation, and an AI problem-solving record. The initial design was a conversational tool orchestrator. Review identified that a functionally complete coding agent also needs observation, mutation, and verification, so `read_file`, `edit_file`, and `shell` became core tools.

## Major Decisions

1. Rust was selected to demonstrate an explicit runtime state machine and typed protocol handling.
2. DeepSeek is called with raw `reqwest`, avoiding an OpenAI SDK and all agent frameworks.
3. Named calculator and search tools remain because the assignment explicitly requests them; shell does not replace their schemas.
4. Todo actions are combined into one tool to reduce duplicated registration code while demonstrating session state.
5. Only fully read-only batches run concurrently. Any mutation makes the batch sequential.
6. Permission control has two modes: direct execution or one user approval for a validated batch. No sandbox is claimed.
7. Tool errors and denial are returned to the model so it can recover instead of terminating the runtime.
8. Full assistant tool-call messages, including reasoning content, are replayed to satisfy DeepSeek thinking-mode protocol.
9. Context compression happens late and only at complete user-turn boundaries.

## AI-Assisted Review

Independent review agents were asked to attack the architecture and test plan. They highlighted DeepSeek reasoning replay, one-result-per-call-ID protocol integrity, ordered parallel results, transactional message sequences, SQLite connection boundaries, Windows/Unix shell differences, child output deadlocks, UTF-8-safe truncation, and deterministic concurrency tests. Those findings were incorporated before verification.

## Known Boundary

The runtime executes commands with host permissions. Approval is human intervention, not isolation. A production service would add an OS sandbox, stronger authorization, and encrypted memory, but those are intentionally outside this minimum implementation.
