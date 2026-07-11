package main

import (
	"fmt"
	"strings"
	"time"
)

// ============================================================
//
//	LLMAgent — ReAct 循环: Thought → ToolCall → Observation
//
// ============================================================
type LLMAgent struct {
	llm      *LLMClient
	tools    *ToolRegistry
	memory   *Memory
	messages []ChatMessage
	maxSteps int
	step     int
}

func NewLLMAgent(llm *LLMClient, tools *ToolRegistry, memory *Memory) *LLMAgent {
	return &LLMAgent{
		llm:      llm,
		tools:    tools,
		memory:   memory,
		maxSteps: 20,
	}
}

// Run 执行用户目标，返回最终回答
func (a *LLMAgent) Run(userGoal string) string {
	a.step = 0
	a.messages = nil

	// 系统提示词
	sysPrompt := BuildSystemPrompt(a.tools.List())
	a.messages = append(a.messages, ChatMessage{Role: "system", Content: sysPrompt})
	a.messages = append(a.messages, ChatMessage{Role: "user", Content: userGoal})
	a.memory.Remember("goal", fmt.Sprintf("用户: %s", userGoal))

	toolDefs := BuildToolDefs(a.tools.List())

	fmt.Println("\n" + strings.Repeat("━", 50))
	fmt.Printf("🤖 Agent 开始 (LLM: %s)\n", a.llm.config.Model)
	fmt.Println(strings.Repeat("━", 50))
	fmt.Printf("目标: %s\n\n", userGoal)

	startTime := time.Now()

	for a.step < a.maxSteps {
		a.step++

		fmt.Printf("⏳ [%d/%d] LLM 推理中...\n", a.step, a.maxSteps)

		reply, usage, err := a.llm.Chat(a.messages, toolDefs)
		if err != nil {
			return fmt.Sprintf("❌ LLM 调用失败: %v", err)
		}

		if usage != nil {
			fmt.Printf("   📊 tokens: %d in + %d out = %d\n",
				usage.PromptTokens, usage.CompletionTokens, usage.TotalTokens)
		}

		// 记录 LLM 回复到记忆
		if reply.Content != "" {
			short := reply.Content
			if len(short) > 200 {
				short = short[:200] + "..."
			}
			a.memory.Remember("llm", fmt.Sprintf("[%d] %s", a.step, short))
		}

		if len(reply.ToolCalls) > 0 {
			// LLM 请求调用工具
			a.messages = append(a.messages, *reply)

			for _, tc := range reply.ToolCalls {
				name := tc.Function.Name
				args := ParseToolArgs(tc.Function.Arguments)

				fmt.Printf("   🔧 %s(%v)\n", name, args)

				result := a.tools.Execute(name, args)

				content := result.Data
				if !result.Success {
					content = fmt.Sprintf("错误: %s\n输出: %s", result.Error, result.Data)
				}
				if len(content) > 3000 {
					content = content[:3000] + "\n...(截断)"
				}

				a.messages = append(a.messages, ChatMessage{
					Role: "tool", ToolCallID: tc.ID, Content: content,
				})

				icon := "✅"
				if !result.Success {
					icon = "❌"
				}
				short := result.Data
				if len(short) > 80 {
					short = short[:80] + "..."
				}
				a.memory.Remember("tool", fmt.Sprintf("%s %s → %s", icon, name, short))

				time.Sleep(200 * time.Millisecond)
			}
		} else {
			// LLM 返回文本回复 → 结束
			a.messages = append(a.messages, *reply)

			elapsed := time.Since(startTime)
			fmt.Printf("\n✅ 完成 (%s, %d 步)\n", fmtDuration(elapsed), a.step)

			a.memory.Remember("goal", fmt.Sprintf("✅ 完成: %s", userGoal))
			return reply.Content
		}
	}

	elapsed := time.Since(startTime)
	return fmt.Sprintf("⚠️ 达到最大步骤 %d 限制 (%s)", a.maxSteps, fmtDuration(elapsed))
}

func (a *LLMAgent) Reset() {
	a.messages = nil
	a.step = 0
}

func (a *LLMAgent) History() []ChatMessage {
	return a.messages
}

func fmtDuration(d time.Duration) string {
	if d < time.Second {
		return fmt.Sprintf("%dms", d.Milliseconds())
	}
	if d < time.Minute {
		return fmt.Sprintf("%.1fs", d.Seconds())
	}
	return fmt.Sprintf("%.1fm", d.Minutes())
}
