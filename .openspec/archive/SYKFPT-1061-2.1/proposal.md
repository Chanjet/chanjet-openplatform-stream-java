# OpenSpec 提案：Protobuf 协议定义 (SYKFPT-1061-2.1)

| 属性 | 内容 |
| --- | --- |
| **提案 ID** | SYKFPT-1061-2.1 |
| **状态** | 草案 (Draft) |
| **负责人** | Gemini CLI |
| **相关任务** | Task 2.1: Protobuf 协议定义 |

---

## 1. 问题背景 (Context)
在多语言微服务架构中，Java, Go, 和 Rust 节点之间以及网关与 ISV 客户端之间需要高度一致的消息格式。采用 Protobuf 3 能够提供强类型契约、高效的二进制序列化以及良好的跨语言支持。

## 2. 目标 (Objectives)
- 在 `proto/` 目录下定义核心业务模型。
- 涵盖 Webhook 推送帧 (`EventFrame`)、客户端响应帧 (`AckFrame`)、路由记录 (`RouteRecord`) 以及系统通知帧 (`SystemFrame`)。
- 确保字段定义兼容后续可能的 P2P gRPC 调用。

## 3. 技术设计 (Technical Design)

### 3.1 协议版本
- 使用 **Protocol Buffers v3**。

### 3.2 核心模型定义规划
- `EventFrame`: 承载原始 Webhook Body 及关键 Headers。
- `AckFrame`: ISV 返回的处理结果确认。
- `SystemFrame`: 建立连接、超时、重连建议等控制帧。
- `RouteRecord`: 存储在 Redis 中的路由元数据模型。

## 4. 实施计划 (Implementation Plan)
1.  **创建目录结构**: 在 `proto/` 下按领域划分子目录（model, internal, gateway）。
2.  **编写 .proto 文件**: 按照规范定义消息结构。
3.  **Makefile 集成**: 预留 `make proto` 编译逻辑（虽然当前仅做定义，但需考虑自动化路径）。

## 5. 验证策略 (Verification Strategy)
- **语法验证**: 使用 `protoc --decode_raw` 或 linter 检查协议合法性。
- **跨语言兼容性**: 验证生成的 Java 代码是否符合 `connector-common` 的需求。

---
**审批意见**：待评审。
