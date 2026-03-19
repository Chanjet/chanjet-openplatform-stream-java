# Spec: Webhook API Specification (SYKFPT-1061-5.2)

## 1. 接口细节 (Endpoint)
- **URL**: `/internal/v1/webhook/dispatch`
- **Content-Type**: `application/json` (或其他文本格式)
- **Max Body Size**: 2MB (可配置)

## 2. Header 协议字段定义
| Header | 含义 | 必须 | 说明 |
| :--- | :--- | :--- | :--- |
| `X-C-APP_KEY` | 应用标识 | 是 | 用于路由寻址 |
| `X-MSG-ID` | 消息唯一标识 | 是 | 用于去重与 ACK 关联 |
| `X-Trace-Id` | 追踪 ID | 否 | 用于全链路日志关联 |
| `X-C-ORG_ID` | 企业标识 | 否 | 透传字段 |

## 3. 性能规范
- **吞吐量**: 单节点支持 2000+ RPS (Request Per Second)。
- **解析开销**: HTTP 协议处理与对象包装耗时必须 < 2ms。

## 4. 安全规范
- **Source IP Whitelist**: 仅允许 Core 节点和内网网段访问（后续通过 Spring Security 或 Nginx 增强）。
