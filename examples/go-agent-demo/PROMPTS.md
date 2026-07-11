# Generation Prompt

The Rust Agent received the following task in a fresh directory:

> Create a project named `go-agent-demo` in Go. From scratch, implement a minimal coding-agent runtime without an existing agent framework and call the DeepSeek OpenAI-compatible Chat API directly. Implement the user/LLM/tool loop; schema-registered `shell`, `read_file`, and `edit_file`; tool observations; a maximum-loop guard; memory recall; trace output; tests; and a README explaining architecture and context strategy. Prefer the standard library, read the key only from `DEEPSEEK_API_KEY`, run `gofmt`, `go vet`, and `go test ./...`, and create and verify the project rather than only describing it.
