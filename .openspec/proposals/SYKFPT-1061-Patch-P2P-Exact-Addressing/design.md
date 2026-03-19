# Design Patch: Exact Client Addressing (SYKFPT-1061-Patch-P2P)

## 1. 协议变更 (`connector-common`)
```java
public record EventFrame(
    String msgType,
    String msgId,
    String traceId,
    String appKey,
    String targetClientId, // 新增字段，用于 P2P 精确寻址
    Map<String, String> headers,
    String payload,
    long timestamp
) {}
```

## 2. 逻辑调整

### 2.1 `MessageDispatcher` (发送端)
在调用 `p2pClient.forward` 前：
```java
String clientId = ... // 从选中的 route 字符串中解析
EventFrame p2pFrame = new EventFrame(..., clientId, ...);
p2pClient.forward(targetNodeId, p2pFrame);
```

### 2.2 `WebhookController` (接收端)
```java
@PostMapping("/internal/v1/p2p/push")
public void receiveP2P(@RequestBody EventFrame frame) {
    String targetId = frame.targetClientId() != null ? 
                      frame.targetClientId() : 
                      frame.appKey() + "@local";
    connectionManager.push(targetId, frame);
}
```

## 3. TDD 验证
在 `TckIntegrationTest` 中增加 `tck03_shouldForwardToExactClientInMultiClientScenario`。
- 注册 Client A 和 Client B（同一 AppKey）。
- 模拟路由指向 Client B。
- 验证只有 Client B 收到了消息。
