package main

import (
	"fmt"
	"strings"
	"sync"
)

// ============================================================
//
//	Memory — 记忆系统
//
// ============================================================
type Memory struct {
	mu      sync.RWMutex
	items   []MemoryItem
	maxSize int
}

func NewMemory(maxSize int) *Memory {
	if maxSize <= 0 {
		maxSize = 200
	}
	return &Memory{items: make([]MemoryItem, 0, maxSize), maxSize: maxSize}
}

func (m *Memory) Remember(t, content string) {
	m.mu.Lock()
	defer m.mu.Unlock()
	if len(m.items) >= m.maxSize {
		m.items = m.items[1:]
	}
	m.items = append(m.items, MemoryItem{Type: t, Content: content})
}

func (m *Memory) Recall(t string) []MemoryItem {
	m.mu.RLock()
	defer m.mu.RUnlock()
	if t == "" {
		r := make([]MemoryItem, len(m.items))
		copy(r, m.items)
		return r
	}
	var r []MemoryItem
	for _, item := range m.items {
		if item.Type == t {
			r = append(r, item)
		}
	}
	return r
}

func (m *Memory) Clear() {
	m.mu.Lock()
	defer m.mu.Unlock()
	m.items = make([]MemoryItem, 0, m.maxSize)
}

func (m *Memory) Stats() string {
	m.mu.RLock()
	defer m.mu.RUnlock()
	typeCount := make(map[string]int)
	for _, item := range m.items {
		typeCount[item.Type]++
	}
	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("🧠 记忆: %d / %d 条\n", len(m.items), m.maxSize))
	for t, c := range typeCount {
		sb.WriteString(fmt.Sprintf("   %s: %d\n", t, c))
	}
	return sb.String()
}
