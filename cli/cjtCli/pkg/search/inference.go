package search

import (
	"fmt"
	"math"

	"github.com/daulet/tokenizers"
	ort "github.com/yalue/onnxruntime_go"
)

// Embedder 定义了将文本转换为向量的接口
type Embedder interface {
	Embed(text string) []float32
}

// ONNXEmbedder 使用 ONNX Runtime 和 HuggingFace Tokenizer 进行真实的向量推理
type ONNXEmbedder struct {
	session   *ort.AdvancedSession
	tokenizer *tokenizers.Tokenizer
	dimension int
	maxLen    int64 // tokenizer 最大序列长度

	// 预分配的输入输出张量（复用避免 GC）
	inputIDs      *ort.Tensor[int64]
	attentionMask *ort.Tensor[int64]
	tokenTypeIDs  *ort.Tensor[int64]
	output        *ort.Tensor[float32]
}

func NewONNXEmbedder(modelPath, tokenizerPath string) (*ONNXEmbedder, error) {
	const maxSeqLen int64 = 512
	const embDim int64 = 512

	// 1. 初始化 ONNX Runtime 环境（全局单例，幂等）
	if !ort.IsInitialized() {
		if err := ort.InitializeEnvironment(); err != nil {
			return nil, fmt.Errorf("初始化 ONNX Runtime 环境失败: %w", err)
		}
	}

	// 2. 加载 HuggingFace Tokenizer
	tk, err := tokenizers.FromFile(tokenizerPath)
	if err != nil {
		return nil, fmt.Errorf("加载 tokenizer 失败 (%s): %w", tokenizerPath, err)
	}

	// 3. 创建输入张量 (int64, shape=[1, maxSeqLen])
	inputShape := ort.NewShape(1, maxSeqLen)

	inputIDs, err := ort.NewEmptyTensor[int64](inputShape)
	if err != nil {
		tk.Close()
		return nil, fmt.Errorf("创建 input_ids 张量失败: %w", err)
	}

	attentionMask, err := ort.NewEmptyTensor[int64](inputShape)
	if err != nil {
		tk.Close()
		inputIDs.Destroy()
		return nil, fmt.Errorf("创建 attention_mask 张量失败: %w", err)
	}

	tokenTypeIDs, err := ort.NewEmptyTensor[int64](inputShape)
	if err != nil {
		tk.Close()
		inputIDs.Destroy()
		attentionMask.Destroy()
		return nil, fmt.Errorf("创建 token_type_ids 张量失败: %w", err)
	}

	// 4. 创建输出张量 (float32, shape=[1, maxSeqLen, embDim])
	outputShape := ort.NewShape(1, maxSeqLen, embDim)
	output, err := ort.NewEmptyTensor[float32](outputShape)
	if err != nil {
		tk.Close()
		inputIDs.Destroy()
		attentionMask.Destroy()
		tokenTypeIDs.Destroy()
		return nil, fmt.Errorf("创建 output 张量失败: %w", err)
	}

	// 5. 创建 ONNX Session
	inputNames := []string{"input_ids", "attention_mask", "token_type_ids"}
	outputNames := []string{"last_hidden_state"}
	inputs := []ort.Value{inputIDs, attentionMask, tokenTypeIDs}
	outputs := []ort.Value{output}

	session, err := ort.NewAdvancedSession(modelPath, inputNames, outputNames, inputs, outputs, nil)
	if err != nil {
		tk.Close()
		inputIDs.Destroy()
		attentionMask.Destroy()
		tokenTypeIDs.Destroy()
		output.Destroy()
		return nil, fmt.Errorf("创建 ONNX Session 失败 (%s): %w", modelPath, err)
	}

	return &ONNXEmbedder{
		session:       session,
		tokenizer:     tk,
		dimension:     int(embDim),
		maxLen:        maxSeqLen,
		inputIDs:      inputIDs,
		attentionMask: attentionMask,
		tokenTypeIDs:  tokenTypeIDs,
		output:        output,
	}, nil
}

func (o *ONNXEmbedder) Embed(text string) []float32 {
	if o == nil || o.session == nil {
		return nil
	}

	// 1. Tokenize (使用 EncodeWithOptions 以获取 AttentionMask 和 TypeIDs)
	encoding := o.tokenizer.EncodeWithOptions(text, true,
		tokenizers.WithReturnAttentionMask(),
		tokenizers.WithReturnTypeIDs(),
	)

	// 2. 填充输入张量
	ids := o.inputIDs.GetData()
	mask := o.attentionMask.GetData()
	types := o.tokenTypeIDs.GetData()

	// 清零
	for i := range ids {
		ids[i] = 0
		mask[i] = 0
		types[i] = 0
	}

	// 填入 tokens（不超过 maxSeqLen）
	seqLen := len(encoding.IDs)
	if int64(seqLen) > o.maxLen {
		seqLen = int(o.maxLen)
	}
	for i := 0; i < seqLen; i++ {
		ids[i] = int64(encoding.IDs[i])
		if len(encoding.AttentionMask) > i {
			mask[i] = int64(encoding.AttentionMask[i])
		} else {
			mask[i] = 1 // 默认有效 token
		}
		if len(encoding.TypeIDs) > i {
			types[i] = int64(encoding.TypeIDs[i])
		}
	}

	// 3. 执行推理
	if err := o.session.Run(); err != nil {
		return nil
	}

	// 4. 提取 [CLS] token 的嵌入（输出 shape=[1, seqLen, dim]，取 index=0）
	outputData := o.output.GetData()
	vec := make([]float32, o.dimension)
	copy(vec, outputData[:o.dimension])

	// 5. L2 归一化
	var norm float32
	for _, v := range vec {
		norm += v * v
	}
	norm = float32(math.Sqrt(float64(norm)))
	if norm > 0 {
		for i := range vec {
			vec[i] /= norm
		}
	}

	return vec
}

func (o *ONNXEmbedder) Close() {
	if o == nil {
		return
	}
	if o.session != nil {
		o.session.Destroy()
	}
	if o.tokenizer != nil {
		o.tokenizer.Close()
	}
	if o.inputIDs != nil {
		o.inputIDs.Destroy()
	}
	if o.attentionMask != nil {
		o.attentionMask.Destroy()
	}
	if o.tokenTypeIDs != nil {
		o.tokenTypeIDs.Destroy()
	}
	if o.output != nil {
		o.output.Destroy()
	}
}
