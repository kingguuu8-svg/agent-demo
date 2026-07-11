# Terminal Recording Script

1. Show `cargo test` passing.
2. Export a newly issued `DEEPSEEK_API_KEY` without showing its value.
3. Start window 1:

   ```powershell
   cargo run -- chat --user user-a --session window-1 --permission require-approval
   ```

4. Ask: `Read Cargo.toml, calculate 23*19, add a todo to review the result, then run cargo test.` Approve the displayed batch and show tool traces.
5. Ask a follow-up: `Complete the todo you just added.`
6. Start window 2 with `--session window-2`, ask it to list todos, and show that window 1's todo is absent.
7. In window 2 ask it to create `demo.txt`, read it back, and run a shell command that verifies its contents.
8. Return to window 1 and ask a pure conversational follow-up to demonstrate persistent history.
9. Briefly show `.mini-agent.db`, `docs/DESIGN.md`, and JSON trace mode:

   ```powershell
   cargo run -- --json-logs chat --user user-a --session trace-demo --permission full-access
   ```

Do not display the API key or paste `.env` contents in the recording.
