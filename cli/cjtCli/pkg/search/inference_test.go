package search

import (
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

// TestONNXEmbedder_InvalidPath 验证当模型路径不存在时，NewONNXEmbedder 应返回错误而非崩溃
func TestONNXEmbedder_InvalidPath(t *testing.T) {
	embedder, err := NewONNXEmbedder("dummy/path/to/model.onnx", "dummy/tokenizer.json")
	assert.Error(t, err, "expected error when model path does not exist")
	assert.Nil(t, embedder)
}

// TestONNXEmbedder_RealInference 使用真实嵌入的 ONNX 模型和 tokenizer 进行端到端推理验证
func TestONNXEmbedder_RealInference(t *testing.T) {
	// 1. 准备运行环境：从嵌入资源释放 ONNX Runtime 动态库和模型文件
	_, modelPath, tokenizerPath, err := EnsureEnvironmentReady()
	if err != nil {
		t.Skipf("AI environment not ready (expected in CI): %v", err)
	}

	// 2. 创建 ONNXEmbedder
	embedder, err := NewONNXEmbedder(modelPath, tokenizerPath)
	require.NoError(t, err, "NewONNXEmbedder should succeed with valid model and tokenizer paths")
	defer embedder.Close()

	// 3. 验证 Embedder 接口实现
	var _ Embedder = embedder

	// 4. 基础推理：验证输出向量维度和归一化
	vec := embedder.Embed("invoice billing")
	assert.Equal(t, 512, len(vec), "bge-small-zh-v1.5 should output 512-dim vectors")
	var norm float32
	for _, v := range vec {
		norm += v * v
	}
	assert.InDelta(t, 1.0, float64(norm), 0.01, "output vector should be L2-normalized")

	// 5. 语义一致性：相同文本应产生一致的向量
	vec2 := embedder.Embed("invoice billing")
	sim := CosineSimilarity(vec, vec2)
	assert.InDelta(t, 1.0, float64(sim), 0.001, "same text should produce identical vectors")

	// 6. 语义区分度：不同语义的中文文本相似度应较低
	// 注: bge-small-zh-v1.5 支持中文词表
	vecFinance := embedder.Embed("余额查询")
	vecLogistics := embedder.Embed("物流配送")
	simCross := CosineSimilarity(vecFinance, vecLogistics)
	assert.Less(t, float64(simCross), 0.95,
		"semantically different texts should produce different embeddings, got sim=%.4f", simCross)
}
