package search

import (
	"encoding/json"
	"fmt"
	"math"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

// Document 代表一个向量化的接口元数据
type Document struct {
	ID       string    `json:"id"`       // e.g. "GET /v1/orders"
	Metadata string    `json:"metadata"` // 原始文本摘要
	Vector   []float32 `json:"vector"`   // 高维浮点向量
}

// Result 搜索结果
type Result struct {
	ID      string  `json:"id"`
	Summary string  `json:"summary"`
	Score   float64 `json:"score"`
}

type Engine struct {
	Docs []Document `json:"docs"`
}

func NewEngine(docs []Document) *Engine {
	return &Engine{Docs: docs}
}

// Search 执行 Top-K 向量相似度搜索
func (e *Engine) Search(queryVector []float32, queryText string, limit int) []Result {
	if len(queryVector) == 0 {
		return nil
	}

	var results []Result
	queryTextLower := strings.ToLower(queryText)
	
	// 生成 N-Gram (2字, 3字, 4字窗口) 用于中文字符级模糊匹配
	var nGrams []string
	runes := []rune(queryTextLower)
	for windowSize := 2; windowSize <= 4; windowSize++ {
		if len(runes) < windowSize {
			continue
		}
		for i := 0; i < len(runes)-windowSize+1; i++ {
			nGrams = append(nGrams, string(runes[i:i+windowSize]))
		}
	}

	for _, doc := range e.Docs {
		score := CosineSimilarity(queryVector, doc.Vector)
		metadataLower := strings.ToLower(doc.Metadata)
		idLower := strings.ToLower(doc.ID)

		// 补丁逻辑 (N-Gram Hybrid Search): 基于语义碎片命中率加分
		if len(nGrams) > 0 {
			hitCount := 0
			for _, gram := range nGrams {
				if strings.Contains(metadataLower, gram) || strings.Contains(idLower, gram) {
					hitCount++
				}
			}
			// 根据命中比例加分，最高加 0.98
			boost := float32(hitCount) / float32(len(nGrams)) * 0.98
			if boost > 0 {
				score += boost
			}
		}

		if score > 0.3 { // 相似度阈值
			results = append(results, Result{
				ID:      doc.ID,
				Summary: doc.Metadata,
				Score:   float64(score),
			})
		}
	}

	sort.Slice(results, func(i, j int) bool {
		return results[i].Score > results[j].Score
	})

	if len(results) > limit {
		results = results[:limit]
	}
	return results
}

// CosineSimilarity 计算两个向量的余弦相似度
func CosineSimilarity(v1, v2 []float32) float32 {
	if len(v1) != len(v2) || len(v1) == 0 {
		return 0
	}

	var dotProduct, mag1, mag2 float32
	for i := 0; i < len(v1); i++ {
		dotProduct += v1[i] * v2[i]
		mag1 += v1[i] * v1[i]
		mag2 += v2[i] * v2[i]
	}

	if mag1 == 0 || mag2 == 0 {
		return 0
	}

	return dotProduct / (float32(math.Sqrt(float64(mag1))) * float32(math.Sqrt(float64(mag2))))
}

// RebuildIndexFromSpec 从 OpenAPI 数据构建语义搜索索引
func RebuildIndexFromSpec(specData interface{}, indexPath string, embedder func(string) []float32) error {
	spec, ok := specData.(map[string]interface{})
	if !ok {
		return fmt.Errorf("invalid spec data format")
	}

	paths, ok := spec["paths"].(map[string]interface{})
	if !ok {
		return fmt.Errorf("spec does not contain paths")
	}

	var docs []Document
	for path, methods := range paths {
		for method, detail := range methods.(map[string]interface{}) {
			summary := ""
			if d, ok := detail.(map[string]interface{}); ok {
				if s, ok := d["summary"].(string); ok {
					summary = s
				}
			}
			text := strings.TrimSpace(summary + " " + path)
			
			// 调用推理机生成向量
			vector := embedder(text)

			docs = append(docs, Document{
				ID:       fmt.Sprintf("%s %s", strings.ToUpper(method), path),
				Metadata: summary,
				Vector:   vector,
			})
		}
	}

	engine := NewEngine(docs)
	raw, err := json.Marshal(engine)
	if err != nil {
		return err
	}

	os.MkdirAll(filepath.Dir(indexPath), 0755)
	return os.WriteFile(indexPath, raw, 0644)
}

func LoadEngine(indexPath string) (*Engine, error) {
	data, err := os.ReadFile(indexPath)
	if err != nil {
		return nil, err
	}
	var e Engine
	if err := json.Unmarshal(data, &e); err != nil {
		return nil, err
	}
	return &e, nil
}
