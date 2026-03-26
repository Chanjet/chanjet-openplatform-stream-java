package search

import (
	"fmt"
	"os"
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestSearchHybridFlowVerification(t *testing.T) {
	// 0. 初始化 ONNX 运行环境
	_, modelPath, tokenizerPath, err := EnsureEnvironmentReady()
	if err != nil {
		t.Skipf("AI environment not ready: %v", err)
	}
	embedder, err := NewONNXEmbedder(modelPath, tokenizerPath)
	if err != nil {
		t.Skipf("ONNX embedder init failed: %v", err)
	}
	defer embedder.Close()

	// 1. Prepare Rich Mock OpenApiSpec (Realistic Business Case)
	mockSpec := map[string]interface{}{
		"paths": map[string]interface{}{
			"/v1/invoice/issue/blue": map[string]interface{}{
				"post": map[string]interface{}{"summary": "为已成交订单开具增值税电子普通发票(蓝票)"},
			},
			"/v1/finance/balance": map[string]interface{}{
				"get": map[string]interface{}{"summary": "查询 ISV 在开放平台的实时可用资金余额"},
			},
			"/v1/sys/circuit-breaker": map[string]interface{}{
				"put": map[string]interface{}{"summary": "手动触发服务熔断以应对突发的海量洪峰流量"},
			},
		},
	}

	indexPath := "./test_api_hybrid_search.idx"
	defer os.Remove(indexPath)

	// 2. Build Index
	err = RebuildIndexFromSpec(mockSpec, indexPath, embedder.Embed)
	assert.NoError(t, err)

	// 3. Load Engine
	engine, err := LoadEngine(indexPath)
	assert.NoError(t, err)

	// 4. Verify Hybrid Search (Semantic + Keyword Boost)
	testCases := []struct {
		query      string
		expectedID string
	}{
		{"发票", "POST /v1/invoice/issue/blue"},
		{"余额", "GET /v1/finance/balance"},
		{"熔断", "PUT /v1/sys/circuit-breaker"},
	}

	for _, tc := range testCases {
		t.Run(fmt.Sprintf("Find_%s", tc.query), func(t *testing.T) {
			queryVector := embedder.Embed(tc.query)
			results := engine.Search(queryVector, tc.query, 1)

			assert.NotEmpty(t, results, "Should find result for query: "+tc.query)
			assert.Equal(t, tc.expectedID, results[0].ID)
		})
	}
}
