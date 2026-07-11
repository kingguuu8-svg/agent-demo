package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"
)

// ============================================================
//
//	LLM Client — DeepSeek OpenAI 兼容 API
//
// ============================================================
type LLMClient struct {
	config LLMConfig
	client *http.Client
}

func NewLLMClient(cfg LLMConfig) *LLMClient {
	return &LLMClient{
		config: cfg,
		client: &http.Client{Timeout: 180 * time.Second},
	}
}

// Chat 发送聊天请求，返回回复消息
func (c *LLMClient) Chat(messages []ChatMessage, tools []*ToolDef) (*ChatMessage, *Usage, error) {
	// 校验 API Key
	key := strings.TrimSpace(c.config.APIKey)
	if key == "" {
		return nil, nil, fmt.Errorf("API Key 为空，请设置 DEEPSEEK_API_KEY")
	}
	// 检查是否有非法字符（比如换行）
	for _, r := range key {
		if r == '\n' || r == '\r' || r == '\ufeff' {
			return nil, nil, fmt.Errorf("API Key 包含非法字符 (换行/BOM)，请检查配置")
		}
	}
	url := c.config.BaseURL + "/chat/completions"

	req := ChatRequest{
		Model:       c.config.Model,
		Messages:    messages,
		Temperature: 0.3,
		MaxTokens:   4096,
	}
	if len(tools) > 0 {
		req.Tools = tools
	}

	body, _ := json.Marshal(req)
	httpReq, err := http.NewRequest("POST", url, bytes.NewReader(body))
	if err != nil {
		return nil, nil, fmt.Errorf("创建请求失败: %w", err)
	}
	httpReq.Header.Set("Content-Type", "application/json")
	httpReq.Header.Set("Authorization", "Bearer "+c.config.APIKey)

	resp, err := c.client.Do(httpReq)
	if err != nil {
		return nil, nil, fmt.Errorf("API 请求失败: %w", err)
	}
	defer resp.Body.Close()

	respBody, _ := io.ReadAll(resp.Body)

	if resp.StatusCode != 200 {
		return nil, nil, fmt.Errorf("API 返回 %d: %s", resp.StatusCode, string(respBody))
	}

	var chatResp ChatResponse
	if err := json.Unmarshal(respBody, &chatResp); err != nil {
		return nil, nil, fmt.Errorf("解析响应失败: %w", err)
	}
	if chatResp.Error != nil {
		return nil, nil, fmt.Errorf("API 错误: %s (%s)", chatResp.Error.Message, chatResp.Error.Type)
	}
	if len(chatResp.Choices) == 0 {
		return nil, nil, fmt.Errorf("API 返回空 choices")
	}

	return &chatResp.Choices[0].Message, chatResp.Usage, nil
}

// ============================================================
//
//	System Prompt
//
// ============================================================
func BuildSystemPrompt(tools []Tool) string {
	p := `你是 Coding Agent，运行在 Go Agent Runtime 中。

## 能力
你可以调用以下工具来完成任务。

## 工具列表
`
	for _, t := range tools {
		p += fmt.Sprintf("\n### %s\n%s\n", t.Name, t.Description)
		for _, a := range t.Schema.Args {
			req := ""
			if a.Required {
				req = " (必填)"
			}
			p += fmt.Sprintf("  - %s (%s)%s: %s\n", a.Name, a.Type, req, a.Description)
		}
	}

	p += `
## 规则
1. 分析用户目标，决定调用哪些工具
2. 每次工具调用后你会收到执行结果
3. 根据结果决定下一步：继续调用工具，或回复用户
4. 任务完成后用中文回复总结

开始工作！`
	return p
}
