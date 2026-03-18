# Design: Protobuf Message Schema (SYKFPT-1061-2.1)

## 1. 消息拓扑 (Message Schema)

### 1.1 EventFrame (核心数据推送)
- `msg_id` (string): 网关生成 UUID。
- `trace_id` (string): 全链路追踪 ID。
- `app_key` (string): ISV 应用标识。
- `headers` (map<string, string>): 业务 Headers (透传白名单)。
- `payload` (string/bytes): 原始业务数据 (保持稳定性)。
- `timestamp` (int64): 毫秒级时间戳。

### 1.2 AckFrame (处理确认)
- `msg_id` (string): 引用 EventFrame 的 ID。
- `code` (int32): 业务状态码 (200=成功)。
- `message` (string): 简短说明。

### 1.3 SystemFrame (协议控制)
- `type` (enum): CONNECTED, RECONNECT, TIMEOUT, HEARTBEAT_PING/PONG。
- `data` (map<string, string>): 随类型变化的元数据。

## 2. 存储布局 (Redis Route Record)
- `node_id` (string): 实例标识 (ip:port)。
- `client_id` (string): 客户端自生成 ID。
- `connect_time` (int64): 建连时间。
- `tags` (map<string, string>): 扩展标签。

## 3. 生成代码配置
- **Java**: `option java_package = "com.chanjet.connector.proto";`, `option java_multiple_files = true;`。
- **Go/Rust**: 遵循各语言标准的命名空间规范。
