# Terminal Recording Script

Long prompts can be pasted directly. If the recording terminal splits pasted lines, use `/paste`, paste the task, then enter `.` on its own line. Press `Esc` during execution to demonstrate cancellation without losing the session.

1. Install and verify:

   ```powershell
   cargo install --path .
   cargo test
   ```

2. Run `agent-demo config`. Show the wizard but never reveal the API key. Explain that the key enters the operating-system credential manager.
3. Start with no arguments:

   ```powershell
   agent-demo
   ```

4. Ask: `Read Cargo.toml, calculate 23*19, add a todo to review the result, then run cargo test.` Approve the displayed tool batch and show readable tool progress.
5. Run `/status`, `/sessions`, and `/trace on`.
6. Switch permission without restarting:

   ```text
   /permission full-access
   ```

7. Ask: `Create demo.txt, read it back, and use shell to verify its contents.` Show that no approval is requested.
8. Exit with `/exit`, run `agent-demo` again, then use `/resume` to select the previous titled session.
9. Ask: `What did we change, and which todo remains?` This demonstrates restored conversation and structured session state.
10. Show one-shot automation:

    ```powershell
    agent-demo run --json --permission full-access "Use calculator to compute 2468*1357"
    ```

Do not display the API key, Credential Manager entry, environment contents, or a real `.env` file.
