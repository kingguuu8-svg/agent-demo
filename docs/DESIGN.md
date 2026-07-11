# System Design

## Scope

Mini Coding Agent is a single-process CLI runtime. The model decides whether to answer or call tools from JSON Schemas; the runtime owns validation, permissions, execution, persistence, budgets, and tracing. There is no keyword router and no agent framework.

## CLI Product Layer

The installed binary is `agent-demo`. No arguments starts a new persistent REPL session. A pure slash-command parser intercepts `/new`, `/resume`, `/sessions`, `/permission`, `/paste`, `/trace`, `/status`, `/config`, and `/exit`; these commands never reach the LLM. The line editor preserves bracketed multi-line paste as one request. During execution an Esc watcher cancels the in-flight HTTP or tool future; shell children use kill-on-drop, and the runtime closes the interrupted conversation turn so the session remains valid. `agent-demo run` provides one-shot automation and JSON output.

Ordinary configuration is JSON in the platform config directory. The API key is resolved from `DEEPSEEK_API_KEY` first, otherwise from the operating-system credential manager. The key is never serialized with application configuration.

## State Machine

```text
user input -> persist -> build context -> DeepSeek
  final content -> persist -> return
  tool calls -> persist exact assistant wire message
             -> parse and validate every call
             -> approve batch or create denial results
             -> parallel read-only / serial mutation execution
             -> persist one result per call ID -> repeat
```

Recoverable failures are observations, not runtime crashes. Invalid JSON, unknown tools, validation failures, non-zero shell exits, edit mismatches, missing todos, and user denial are returned to the model as structured tool results.

## Tool Contract

Each `Tool` supplies its name, description, JSON Schema, typed validation, effect classification, and async execution. `ToolOutput` is always either `{ok:true,data:...}` or `{ok:false,error:{code,message}}`. The registry supplies definitions to DeepSeek and resolves calls by exact name.

## Ordering and Idempotency

DeepSeek may return several tool calls. If every validated call is read-only, `join_all` executes them concurrently and preserves input order. Otherwise the entire batch runs in model order. `tool_runs` is unique on `(user_id, session_id, call_id)`; a repeated call ID reuses its stored output instead of repeating a side effect.

## Session Memory

`sessions`, `messages`, `todos`, and `tool_runs` are SQLite tables. Message sequence allocation uses an immediate transaction. Complete transcript rows remain available for audit even after their prefix is replaced by a summary in model context.

Sessions have short generated IDs, a title derived from the first user message, and an activity timestamp. Empty sessions are hidden and deleted on exit or session switch. `/resume` changes only the active `(user_id, session_id)` key; the same Runtime and database continue serving the REPL.

## Context Compaction

The estimator conservatively counts serialized Unicode characters. Compaction starts near the configured model limit, groups history by user turns, summarizes only old completed groups, and retains recent groups intact. The compaction request consumes the same logical LLM budget as normal decisions.

## Trust Boundary

Permission mode is either direct execution or one approval for the normalized batch. Approval occurs after parsing and validation. It is not a sandbox: shell commands and absolute file paths can access the host with process permissions. This limitation is intentional and visible in the CLI and README.
