package main

import (
	"os"
	"strings"
	"testing"
)

func TestLiveDeepSeekAgentLoop(t *testing.T) {
	key := os.Getenv("DEEPSEEK_API_KEY")
	if key == "" {
		t.Skip("set DEEPSEEK_API_KEY to run the live integration test")
	}
	config := LoadLLMConfig()
	tools := NewToolRegistry()
	memory := NewMemory(20)
	agent := NewLLMAgent(NewLLMClient(config), tools, memory)
	result := agent.Run("You must use the calculator tool to compute 123 * 456, then answer with the result.")
	if !strings.Contains(result, "56088") {
		t.Fatalf("expected final result 56088, got %q", result)
	}
	if len(memory.Recall("tool")) == 0 {
		t.Fatal("the live agent did not record a calculator observation")
	}
}
