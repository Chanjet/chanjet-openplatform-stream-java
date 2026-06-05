# Cowen AI

畅捷通 Cowen CLI 的智能接口发现与向量检索引擎。

## 🎯 职责 (Responsibility)
- **语义理解 (Semantic Understanding)**: 将自然语言意图转化为高维向量。
- **混合检索 (Hybrid Search)**: 结合向量相似度与传统文本 N-Gram 匹配，实现精准的 API 搜索。
- **本地化推理 (Edge Inference)**: 在本地零依赖运行神经网络模型，确保隐私与极低延迟。

## 🛠️ 核心能力 (Capabilities)
- **ONNXEmbedder**: 基于 `ort` (ONNX Runtime) 的轻量级嵌入模型封装。
- **SearchIndex**: 优化的本地向量索引结构，支持快速 Top-K 检索。
- **AssetManagement**: 自动提取并管理本地嵌入式神经网络模型资产。

## 📦 外部依赖 (Key Dependencies)
- `ort`: ONNX Runtime Rust 绑定。
- `tokenizers`: HuggingFace 分词器支持。
- `ndarray`: 张量计算。

## ⚠️ 注意事项 (Constraints)
- **资源受限**: 本模块是 CPU 密集型的，应在专用的异步线程池或阻塞任务中运行，避免阻塞主事件循环。
- **版本对齐**: 向量模型与索引数据必须保持版本一致。
