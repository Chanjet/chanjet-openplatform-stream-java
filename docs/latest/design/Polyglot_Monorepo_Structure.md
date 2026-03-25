# 畅捷通 Stream Gateway 多语言单仓 (Monorepo) 目录规划 v1.0

## 1. 总体目录树 (Global Directory Tree)

项目采用 Monorepo 模式组织，初期以 Java 21 实现为核心，同时预留多语言扩展空间。

```text
open-streaming-connector/
open-streaming-connector/
├── docs/                          # [共享] 业务需求与技术设计文档
├── proto/                         # [核心契约] 跨语言协议定义 (Protobuf)
├── connector-server/              # [服务端] 网关核心实现 (Spring Boot)
├── connector-core/                # [服务端] 消息分发逻辑中心
├── connector-api/                 # [服务端] SPI 接口契约
├── connector-infra/               # [服务端] Redis 路由与 Nacos 集成实现
├── connector-common/              # [共享] 跨模块协议 Record 帧定义
├── connector-java-sdk/            # [SDK] 官方 Java SDK 接入包
├── sdk/                           # [多语言 SDK 与 Demo]
│   ├── go/                        # Go SDK
│   ├── go-demo/                   # Go 示例程序
│   ├── nodejs/                    # Node.js SDK
│   ├── nodejs-demo/               # Node.js 示例程序
│   ├── java-demo/                 # Java 示例程序
│   └── python/                    # Python SDK (规划中)
├── infra/                         # [基础设施] Docker 与 K8s 部署模板
├── scripts/                       # [辅助工具] 自动化验证与稳定性测试脚本
└── Makefile                       # [统一入口] 构建与测试指令

---

## 2. 核心目录职责说明

### 2.1 `proto/` (契约驱动)
**职责**：存放所有跨端通讯的 IDL 文件，确保多语言 SDK 与服务端对 `EventFrame` 的序列化理解一致。

### 2.2 `connector-*` (网关实现)
**职责**：基于 Java 21 虚拟线程的高性能网关。
- **connector-api**: 定义核心 SPI。
- **connector-core**: 跨节点分发逻辑。
- **connector-server**: 提供 Webhook 接收与 WebSocket 连接入口。

### 2.3 `sdk/` 与 `connector-java-sdk` (接入层)
**职责**：为不同技术栈的 ISV 提供高度一致的接入体验。
- **一致性**: 所有语言 SDK 均遵循指数退避重连算法、AES-128-ECB 解密逻辑及独立 `encryptKey` 安全模型。

---

## 3. 多语言演进现状

1.  **Java, Node.js & Go (v0.1.0)**: 已完成。提供了成熟的 Java, Node.js 和 Go SDK。
2.  **Stability Validation**: 通过 `scripts/stability_test_runner.sh` 完成了 2 小时级别的稳定性压测。
3.  **Security Baseline**: 全面采用 AES-128-ECB 加密及独立消息秘钥。

---
**更新日期**: 2026-03-19
