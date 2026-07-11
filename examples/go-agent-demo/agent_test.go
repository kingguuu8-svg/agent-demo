package main

import (
	"fmt"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync/atomic"
	"testing"
)

func TestCalculator(t *testing.T) {
	value, err := calc("2+(3*4)")
	if err != nil || value != 14 {
		t.Fatalf("calc returned %v, %v", value, err)
	}
}

func TestMemoryKeepsLatestItems(t *testing.T) {
	memory := NewMemory(2)
	memory.Remember("goal", "one")
	memory.Remember("tool", "two")
	memory.Remember("goal", "three")
	items := memory.Recall("")
	if len(items) != 2 || items[0].Content != "two" || items[1].Content != "three" {
		t.Fatalf("unexpected memory: %#v", items)
	}
}

func TestAgentRunsToolLoop(t *testing.T) {
	var calls atomic.Int32
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		if calls.Add(1) == 1 {
			fmt.Fprint(w, `{"choices":[{"message":{"role":"assistant","tool_calls":[{"id":"c1","type":"function","function":{"name":"calculator","arguments":"{\"expression\":\"6*7\"}"}}]}}]}`)
			return
		}
		fmt.Fprint(w, `{"choices":[{"message":{"role":"assistant","content":"42"}}]}`)
	}))
	defer server.Close()

	memory := NewMemory(20)
	agent := NewLLMAgent(
		NewLLMClient(LLMConfig{APIKey: "test", BaseURL: server.URL, Model: "test"}),
		NewToolRegistry(),
		memory,
	)
	if result := agent.Run("calculate"); result != "42" {
		t.Fatalf("unexpected result: %q", result)
	}
	if calls.Load() != 2 {
		t.Fatalf("expected two LLM calls, got %d", calls.Load())
	}
	if len(memory.Recall("tool")) != 1 {
		t.Fatal("tool observation was not recorded")
	}
}

func TestConfigReadsKeyFromEnvironment(t *testing.T) {
	t.Setenv("DEEPSEEK_API_KEY", " environment-key ")
	if key := LoadLLMConfig().APIKey; key != "environment-key" {
		t.Fatalf("environment key was not trimmed: %q", key)
	}
	if strings.Contains(LoadLLMConfig().APIKey, "\n") {
		t.Fatal("key contains a newline")
	}
}
