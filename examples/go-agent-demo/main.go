package main

import (
	"fmt"
	"os"
	"strings"
)

func main() {
	llmCfg := LoadLLMConfig()

	if llmCfg.APIKey == "" {
		fmt.Println(strings.Repeat("═", 50))
		fmt.Println("  ❌ 未设置 DeepSeek API Key")
		fmt.Println(strings.Repeat("═", 50))
		fmt.Println()
		fmt.Println("设置方式:")
		fmt.Println()
		fmt.Println("  1. 环境变量:")
		fmt.Println("     set DEEPSEEK_API_KEY=your-key")
		fmt.Println()
		fmt.Println("  可选:")
		fmt.Println("     DEEPSEEK_BASE_URL  (默认: https://api.deepseek.com/v1)")
		fmt.Println("     DEEPSEEK_MODEL     (默认: deepseek-chat)")
		fmt.Println()
		os.Exit(1)
	}

	cfg := DefaultConfig()
	rt := NewRuntime(cfg, llmCfg)
	rt.Run()
}
