package main

import (
	"bufio"
	"fmt"
	"os"
	"strings"
)

// ============================================================
//
//	Runtime — 交互式运行环境
//
// ============================================================
type Runtime struct {
	Config RuntimeConfig
	Agent  *LLMAgent
	Tools  *ToolRegistry
	Memory *Memory
}

func NewRuntime(cfg RuntimeConfig, llmCfg LLMConfig) *Runtime {
	mem := NewMemory(200)
	tools := NewToolRegistry()
	llm := NewLLMClient(llmCfg)
	agent := NewLLMAgent(llm, tools, mem)
	return &Runtime{
		Config: cfg,
		Agent:  agent,
		Tools:  tools,
		Memory: mem,
	}
}

// Run 启动交互式 CLI
func (r *Runtime) Run() {
	fmt.Println()
	fmt.Println(strings.Repeat("═", 56))
	fmt.Println("  🤖  Go Coding Agent Runtime")
	fmt.Printf("  📡 %s\n", r.Agent.llm.config.Model)
	fmt.Println(strings.Repeat("═", 56))
	fmt.Println("  输入目标, help, tools, memory, clear, history, exit")
	fmt.Println()

	scanner := bufio.NewScanner(os.Stdin)
	for {
		fmt.Print("🎯 > ")
		if !scanner.Scan() {
			break
		}

		input := strings.TrimSpace(scanner.Text())
		if input == "" {
			continue
		}

		lower := strings.ToLower(input)

		switch lower {
		case "exit", "quit", "q":
			fmt.Println("👋 再见！")
			return

		case "help":
			fmt.Println("\n📖 命令:")
			fmt.Println(strings.Repeat("─", 36))
			fmt.Println("  <目标>     LLM Agent 自动处理")
			fmt.Println("  tools      列出所有工具")
			fmt.Println("  memory     查看记忆")
			fmt.Println("  history    查看对话历史")
			fmt.Println("  clear      清空记忆和历史")
			fmt.Println("  exit       退出")
			fmt.Println()

		case "tools":
			fmt.Println("\n📦 工具:")
			fmt.Println(strings.Repeat("─", 36))
			for _, t := range r.Tools.List() {
				fmt.Printf("  %-14s %s\n", t.Name, t.Description)
			}
			fmt.Println()

		case "memory":
			items := r.Memory.Recall("")
			if len(items) == 0 {
				fmt.Println("📭 暂无记忆")
			} else {
				fmt.Printf("📝 记忆 (%d):\n", len(items))
				for i, item := range items {
					text := item.Content
					if len(text) > 100 {
						text = text[:100] + "..."
					}
					fmt.Printf("  %d. [%s] %s\n", i+1, item.Type, text)
				}
			}

		case "history":
			hist := r.Agent.History()
			if len(hist) == 0 {
				fmt.Println("📭 无历史")
			} else {
				fmt.Printf("📜 对话 (%d 条):\n", len(hist))
				for i, m := range hist {
					role := m.Role
					content := m.Content
					if content == "" {
						content = fmt.Sprintf("[工具调用: %d]", len(m.ToolCalls))
					}
					if len(content) > 80 {
						content = content[:80] + "..."
					}
					fmt.Printf("  %d. %s: %s\n", i+1, role, content)
				}
			}

		case "clear":
			r.Agent.Reset()
			r.Memory.Clear()
			fmt.Println("🧹 已清空")

		default:
			result := r.Agent.Run(input)
			fmt.Println("\n📬", result)
		}

		fmt.Println()
	}
}

// LoadLLMConfig 从环境变量加载 LLM 配置
func LoadLLMConfig() LLMConfig {
	apiKey := os.Getenv("DEEPSEEK_API_KEY")

	baseURL := os.Getenv("DEEPSEEK_BASE_URL")
	if baseURL == "" {
		baseURL = "https://api.deepseek.com/v1"
	}

	model := os.Getenv("DEEPSEEK_MODEL")
	if model == "" {
		model = "deepseek-chat"
	}

	return LLMConfig{
		APIKey:  strings.TrimSpace(apiKey),
		BaseURL: baseURL,
		Model:   model,
	}
}
