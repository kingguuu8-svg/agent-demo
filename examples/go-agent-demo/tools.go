package main

import (
	"bytes"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
)

// ============================================================
//
//	ToolRegistry — 工具注册与调度
//
// ============================================================
type ToolRegistry struct {
	tools map[string]Tool
}

func NewToolRegistry() *ToolRegistry {
	r := &ToolRegistry{tools: make(map[string]Tool)}
	r.registerBuiltin()
	return r
}

func (r *ToolRegistry) Register(t Tool)              { r.tools[t.Name] = t }
func (r *ToolRegistry) Get(name string) (Tool, bool) { t, ok := r.tools[name]; return t, ok }
func (r *ToolRegistry) List() []Tool {
	list := make([]Tool, 0, len(r.tools))
	for _, t := range r.tools {
		list = append(list, t)
	}
	return list
}

func (r *ToolRegistry) Execute(name string, args []string) ToolResult {
	t, ok := r.Get(name)
	if !ok {
		return ToolResult{Success: false, Error: fmt.Sprintf("未知工具: %s", name)}
	}
	return t.Execute(args)
}

// ============================================================
//
//	8 个内置工具
//
// ============================================================
func (r *ToolRegistry) registerBuiltin() {
	r.Register(Tool{
		Name: "read_file", Description: "读取文件内容。参数: path（文件路径）",
		Schema: ToolSchema{Args: []ToolArg{{Name: "path", Type: "string", Required: true, Description: "文件路径"}}},
		Execute: func(args []string) ToolResult {
			if len(args) < 1 {
				return ToolResult{Success: false, Error: "缺少 path 参数"}
			}
			data, err := os.ReadFile(args[0])
			if err != nil {
				return ToolResult{Success: false, Error: err.Error()}
			}
			return ToolResult{Success: true, Data: string(data)}
		},
	})

	r.Register(Tool{
		Name: "write_file", Description: "写入文件。参数: path（路径）content（内容）",
		Schema: ToolSchema{Args: []ToolArg{
			{Name: "path", Type: "string", Required: true, Description: "文件路径"},
			{Name: "content", Type: "string", Required: true, Description: "文件内容"},
		}},
		Execute: func(args []string) ToolResult {
			if len(args) < 2 {
				return ToolResult{Success: false, Error: "缺少参数: path content"}
			}
			path := args[0]
			content := strings.Join(args[1:], " ")
			os.MkdirAll(filepath.Dir(path), 0755)
			if err := os.WriteFile(path, []byte(content), 0644); err != nil {
				return ToolResult{Success: false, Error: err.Error()}
			}
			return ToolResult{Success: true, Data: fmt.Sprintf("已写入 %d 字节到 %s", len(content), path)}
		},
	})

	r.Register(Tool{
		Name: "edit_file", Description: "编辑文件（替换文本）。参数: path（路径）old_text（原文本）new_text（新文本）",
		Schema: ToolSchema{Args: []ToolArg{
			{Name: "path", Type: "string", Required: true, Description: "文件路径"},
			{Name: "old_text", Type: "string", Required: true, Description: "被替换的文本"},
			{Name: "new_text", Type: "string", Required: true, Description: "新文本"},
		}},
		Execute: func(args []string) ToolResult {
			if len(args) < 3 {
				return ToolResult{Success: false, Error: "缺少参数: path old_text new_text"}
			}
			path, oldT, newT := args[0], args[1], args[2]
			data, err := os.ReadFile(path)
			if err != nil {
				return ToolResult{Success: false, Error: err.Error()}
			}
			content := string(data)
			if !strings.Contains(content, oldT) {
				return ToolResult{Success: false, Error: fmt.Sprintf("未在 %s 中找到要替换的文本", path)}
			}
			newContent := strings.ReplaceAll(content, oldT, newT)
			if err := os.WriteFile(path, []byte(newContent), 0644); err != nil {
				return ToolResult{Success: false, Error: err.Error()}
			}
			return ToolResult{Success: true, Data: fmt.Sprintf("已替换 %s 中的文本", path)}
		},
	})

	r.Register(Tool{
		Name: "list_dir", Description: "列出目录内容。参数: path（目录路径，可选）",
		Schema: ToolSchema{Args: []ToolArg{{Name: "path", Type: "string", Required: false, Description: "目录路径，默认当前目录"}}},
		Execute: func(args []string) ToolResult {
			path := "."
			if len(args) > 0 && args[0] != "" {
				path = args[0]
			}
			entries, err := os.ReadDir(path)
			if err != nil {
				// 尝试作为文件名模式
				if matches, err := filepath.Glob(path); err == nil && len(matches) > 0 {
					var buf bytes.Buffer
					for _, m := range matches {
						info, _ := os.Stat(m)
						if info != nil {
							buf.WriteString(fmt.Sprintf("%s (%d bytes)\n", m, info.Size()))
						} else {
							buf.WriteString(m + "\n")
						}
					}
					return ToolResult{Success: true, Data: buf.String()}
				}
				return ToolResult{Success: false, Error: err.Error()}
			}
			var buf bytes.Buffer
			for _, e := range entries {
				info, _ := e.Info()
				mark := " "
				if e.IsDir() {
					mark = "d"
				}
				if info != nil {
					buf.WriteString(fmt.Sprintf("%s %8d  %s\n", mark, info.Size(), e.Name()))
				} else {
					buf.WriteString(fmt.Sprintf("%s %8s  %s\n", mark, "-", e.Name()))
				}
			}
			return ToolResult{Success: true, Data: buf.String()}
		},
	})

	r.Register(Tool{
		Name: "shell", Description: "执行 shell 命令。参数: command（要执行的命令）",
		Schema: ToolSchema{Args: []ToolArg{{Name: "command", Type: "string", Required: true, Description: "要执行的命令"}}},
		Execute: func(args []string) ToolResult {
			if len(args) < 1 {
				return ToolResult{Success: false, Error: "缺少 command 参数"}
			}
			cmdStr := strings.Join(args, " ")
			var cmd *exec.Cmd
			shell := os.Getenv("SHELL")
			if shell == "" {
				cmd = exec.Command("powershell", "-Command", cmdStr)
			} else {
				cmd = exec.Command(shell, "-c", cmdStr)
			}
			var stdout, stderr bytes.Buffer
			cmd.Stdout = &stdout
			cmd.Stderr = &stderr
			err := cmd.Run()
			output := stdout.String()
			if stderr.Len() > 0 {
				if output != "" {
					output += "\n"
				}
				output += stderr.String()
			}
			if err != nil {
				return ToolResult{Success: false, Data: output, Error: fmt.Sprintf("命令失败: %v", err)}
			}
			return ToolResult{Success: true, Data: output}
		},
	})

	r.Register(Tool{
		Name: "search", Description: "搜索知识库。参数: query（搜索关键词）",
		Schema: ToolSchema{Args: []ToolArg{{Name: "query", Type: "string", Required: true, Description: "搜索关键词"}}},
		Execute: func(args []string) ToolResult {
			if len(args) < 1 {
				return ToolResult{Success: false, Error: "缺少 query 参数"}
			}
			query := strings.Join(args, " ")
			knowledge := map[string]string{
				"go":               "Go 是 Google 开发的静态强类型编译语言。",
				"golang":           "Go 语言的别称 (golang.org)",
				"agent":            "Agent 是能自主感知并行动以达成目标的系统。",
				"coding agent":     "Coding Agent 是专用于编程任务的智能体。",
				"runtime":          "Runtime 是程序运行时的执行环境。",
				"llm":              "Large Language Model，大语言模型。",
				"deepseek":         "DeepSeek 提供 OpenAI 兼容的 Chat API。",
				"function calling": "LLM 调用外部工具的标准方式。",
			}
			ql := strings.ToLower(query)
			tokens := strings.Fields(ql)
			type match struct {
				key, val string
				score    int
			}
			var ms []match
			for k, v := range knowledge {
				kl, vl := strings.ToLower(k), strings.ToLower(v)
				score := 0
				if strings.Contains(kl, ql) || strings.Contains(vl, ql) {
					score += 3
				}
				for _, t := range tokens {
					if len(t) < 2 {
						continue
					}
					if strings.Contains(kl, t) {
						score += 2
					}
					if strings.Contains(vl, t) {
						score++
					}
				}
				if score > 0 {
					ms = append(ms, match{k, v, score})
				}
			}
			// 按分数排序
			for i := 0; i < len(ms); i++ {
				for j := i + 1; j < len(ms); j++ {
					if ms[j].score > ms[i].score {
						ms[i], ms[j] = ms[j], ms[i]
					}
				}
			}
			if len(ms) == 0 {
				return ToolResult{Success: true, Data: "未找到相关结果。"}
			}
			var b strings.Builder
			for i, m := range ms {
				if i > 0 {
					b.WriteString("\n")
				}
				b.WriteString(fmt.Sprintf("• %s: %s", m.key, m.val))
			}
			return ToolResult{Success: true, Data: b.String()}
		},
	})

	r.Register(Tool{
		Name: "calculator", Description: "数学计算。参数: expression（数学表达式）",
		Schema: ToolSchema{Args: []ToolArg{{Name: "expression", Type: "string", Required: true, Description: "数学表达式"}}},
		Execute: func(args []string) ToolResult {
			if len(args) < 1 {
				return ToolResult{Success: false, Error: "缺少 expression 参数"}
			}
			expr := strings.Join(args, " ")
			val, err := calc(expr)
			if err != nil {
				return ToolResult{Success: false, Error: fmt.Sprintf("计算错误: %v", err)}
			}
			return ToolResult{Success: true, Data: fmt.Sprintf("%s = %v", expr, val)}
		},
	})

	r.Register(Tool{
		Name: "finish", Description: "标记任务完成，输出总结。参数: summary（可选）",
		Schema: ToolSchema{Args: []ToolArg{{Name: "summary", Type: "string", Required: false, Description: "完成总结"}}},
		Execute: func(args []string) ToolResult {
			summary := "任务完成"
			if len(args) > 0 {
				summary = strings.Join(args, " ")
			}
			return ToolResult{Success: true, Data: fmt.Sprintf("[完成] %s", summary)}
		},
	})
}

// ============================================================
//
//	四则运算计算器 (递归下降)
//
// ============================================================
type token struct {
	typ byte
	val float64
}

func calc(s string) (float64, error) {
	toks := tokenize(s)
	if len(toks) == 0 {
		return 0, fmt.Errorf("空表达式")
	}
	v, _, err := expr(toks, 0)
	return v, err
}

func tokenize(s string) []token {
	var t []token
	i := 0
	for i < len(s) {
		c := s[i]
		if c == ' ' {
			i++
			continue
		}
		if (c >= '0' && c <= '9') || c == '.' {
			start := i
			for i < len(s) && ((s[i] >= '0' && s[i] <= '9') || s[i] == '.') {
				i++
			}
			v, _ := strconv.ParseFloat(s[start:i], 64)
			t = append(t, token{'n', v})
			continue
		}
		if c == '+' || c == '-' || c == '*' || c == '/' || c == '(' || c == ')' {
			t = append(t, token{c, 0})
			i++
			continue
		}
		return nil
	}
	return t
}

func expr(toks []token, pos int) (float64, int, error) {
	l, pos, err := term(toks, pos)
	if err != nil {
		return 0, pos, err
	}
	for pos < len(toks) && (toks[pos].typ == '+' || toks[pos].typ == '-') {
		op := toks[pos].typ
		pos++
		r, np, er := term(toks, pos)
		if er != nil {
			return 0, np, er
		}
		pos = np
		if op == '+' {
			l += r
		} else {
			l -= r
		}
	}
	return l, pos, nil
}

func term(toks []token, pos int) (float64, int, error) {
	l, pos, err := factor(toks, pos)
	if err != nil {
		return 0, pos, err
	}
	for pos < len(toks) && (toks[pos].typ == '*' || toks[pos].typ == '/') {
		op := toks[pos].typ
		pos++
		r, np, er := factor(toks, pos)
		if er != nil {
			return 0, np, er
		}
		pos = np
		if op == '*' {
			l *= r
		} else {
			if r == 0 {
				return 0, pos, fmt.Errorf("除以零")
			}
			l /= r
		}
	}
	return l, pos, nil
}

func factor(toks []token, pos int) (float64, int, error) {
	if pos >= len(toks) {
		return 0, pos, fmt.Errorf("意外的结尾")
	}
	if toks[pos].typ == '(' {
		pos++
		v, np, err := expr(toks, pos)
		if err != nil {
			return 0, np, err
		}
		if np >= len(toks) || toks[np].typ != ')' {
			return 0, np, fmt.Errorf("缺少 )")
		}
		return v, np + 1, nil
	}
	if toks[pos].typ == 'n' {
		return toks[pos].val, pos + 1, nil
	}
	return 0, pos, fmt.Errorf("意外 token: %c", toks[pos].typ)
}
