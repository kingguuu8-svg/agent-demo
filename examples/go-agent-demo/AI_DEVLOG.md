# AI Development Record

## One-Prompt Generation

The Rust `agent-demo` received the task recorded in `PROMPTS.md` in a fresh directory. It inspected the Go environment, chose a standard-library-first layout, created the API client, loop, registry, eight tools, bounded memory, and interactive CLI, then built the program. The interview terminal recording captures this process.

## Submission Preparation

The generated runtime is intentionally preserved rather than rewritten into a second production implementation. Submission preparation made only security and evidence changes:

- excluded the plaintext development key and generated Windows executables;
- made `DEEPSEEK_API_KEY` the only key source;
- added deterministic tests around the generated calculator, bounded log, and two-call Agent loop;
- added an opt-in real DeepSeek end-to-end tool-loop test;
- documented the generated version's narrower memory semantics honestly.

## Known Limits

This proof version has process-local memory, resets message history between goals, executes tool calls sequentially, and has no approval UI, session persistence, context compression, or cancellation. Those features are implemented in the root Rust runtime. The Go artifact is evidence that the Rust Agent can autonomously create, build, and exercise a coherent multi-file Agent—not a claim that both runtimes have identical scope.
