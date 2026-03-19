# Design: Rest-based P2P Forwarding (SYKFPT-1061-P2P)

## 1. 内部协议映射 (Endpoint)
- **POST** `http://{target_node}/internal/v1/p2p/push`
- 消息体采用 `connector-common` 中已定义的 `EventFrame` Record。

## 2. 核心类设计

### 2.1 `RestP2PClient` (Infra 层)
- **实现接口**: `IP2PClient`。
- **底层技术**: `RestClient` (配置专用内部超时)。
- **错误映射**: 将 HTTP 503/504 映射为 `RemoteNodeUnavailableException`。

### 2.2 控制器扩展 (`WebhookController`)
- 新增方法处理 P2P 入口。
- 该方法直接调用 `LocalConnectionManager.push()`，因为它已经是精确寻址。

## 3. TDD 测试矩阵 (Integration)
- `shouldForwardSuccessfullyToRemoteNode()`: 模拟远程节点返回 200。
- `shouldFailWhenRemoteNodeReturnsError()`: 模拟远程节点繁忙。
- `shouldHandleConnectionTimeoutCorrectly()`: 模拟网络分区或节点宕机。
