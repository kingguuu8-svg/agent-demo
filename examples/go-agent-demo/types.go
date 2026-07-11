package main

import (
	"encoding/json"
)

// ============================================================
//
//	Status 枚举
//
// ============================================================
type Status string

const (
	StatusPending    Status = "pending"
	StatusInProgress Status = "in_progress"
	StatusDone       Status = "done"
	StatusFailed     Status = "failed"
)

// ============================================================
//
//	Goal & Step
//
// ============================================================
type Goal struct {
	ID          string
	Description string
	Status      Status
	Result      string
}

// ============================================================
//
//	Tool 系统
//
// ============================================================
type ToolArg struct {
	Name        string
	Type        string
	Required    bool
	Description string
}

type ToolSchema struct {
	Args []ToolArg
}

type Tool struct {
	Name        string
	Description string
	Schema      ToolSchema
	Execute     func(args []string) ToolResult
}

type ToolResult struct {
	Success bool
	Data    string
	Error   string
}

// ============================================================
//
//	Memory
//
// ============================================================
type MemoryItem struct {
	Type    string
	Content string
}

// ============================================================
//
//	Runtime Config
//
// ============================================================
type RuntimeConfig struct {
	MaxSteps int
	WorkDir  string
}

func DefaultConfig() RuntimeConfig {
	return RuntimeConfig{
		MaxSteps: 20,
		WorkDir:  ".",
	}
}

// ============================================================
//
//	LLM Types — OpenAI 兼容 Chat API
//
// ============================================================
type LLMConfig struct {
	APIKey  string
	BaseURL string
	Model   string
}

type ChatMessage struct {
	Role       string     `json:"role"`
	Content    string     `json:"content,omitempty"`
	ToolCalls  []ToolCall `json:"tool_calls,omitempty"`
	ToolCallID string     `json:"tool_call_id,omitempty"`
}

type ToolCall struct {
	ID       string       `json:"id"`
	Type     string       `json:"type"`
	Function ToolCallFunc `json:"function"`
}

type ToolCallFunc struct {
	Name      string `json:"name"`
	Arguments string `json:"arguments"`
}

type ChatRequest struct {
	Model       string        `json:"model"`
	Messages    []ChatMessage `json:"messages"`
	Tools       []*ToolDef    `json:"tools,omitempty"`
	Temperature float64       `json:"temperature,omitempty"`
	MaxTokens   int           `json:"max_tokens,omitempty"`
}

type ChatResponse struct {
	Choices []Choice  `json:"choices"`
	Usage   *Usage    `json:"usage,omitempty"`
	Error   *APIError `json:"error,omitempty"`
}

type Choice struct {
	Index        int         `json:"index"`
	Message      ChatMessage `json:"message"`
	FinishReason string      `json:"finish_reason"`
}

type Usage struct {
	PromptTokens     int `json:"prompt_tokens"`
	CompletionTokens int `json:"completion_tokens"`
	TotalTokens      int `json:"total_tokens"`
}

type APIError struct {
	Message string `json:"message"`
	Type    string `json:"type"`
}

// ============================================================
//
//	ToolDef — 发送给 LLM 的 Function Calling 定义
//
// ============================================================
type ToolDef struct {
	Type     string      `json:"type"`
	Function FunctionDef `json:"function"`
}

type FunctionDef struct {
	Name        string         `json:"name"`
	Description string         `json:"description"`
	Parameters  FunctionParams `json:"parameters"`
}

type FunctionParams struct {
	Type       string                 `json:"type"`
	Properties map[string]PropertyDef `json:"properties"`
	Required   []string               `json:"required,omitempty"`
}

type PropertyDef struct {
	Type        string `json:"type"`
	Description string `json:"description"`
}

// BuildToolDefs 将本地工具转为 LLM 工具定义
func BuildToolDefs(tools []Tool) []*ToolDef {
	defs := make([]*ToolDef, 0)
	for _, t := range tools {
		props := make(map[string]PropertyDef)
		req := make([]string, 0)
		for _, a := range t.Schema.Args {
			props[a.Name] = PropertyDef{Type: a.Type, Description: a.Description}
			if a.Required {
				req = append(req, a.Name)
			}
		}
		if len(props) == 0 {
			props["input"] = PropertyDef{Type: "string", Description: "工具输入"}
		}
		defs = append(defs, &ToolDef{
			Type: "function",
			Function: FunctionDef{
				Name:        t.Name,
				Description: t.Description,
				Parameters:  FunctionParams{Type: "object", Properties: props, Required: req},
			},
		})
	}
	return defs
}

// ParseToolArgs 将 LLM 返回的 JSON 参数转为字符串切片
func ParseToolArgs(raw string) []string {
	var m map[string]interface{}
	if err := json.Unmarshal([]byte(raw), &m); err != nil {
		return []string{raw}
	}
	var out []string
	for _, v := range m {
		if s, ok := v.(string); ok {
			out = append(out, s)
		}
	}
	if len(out) == 0 {
		return []string{raw}
	}
	return out
}
