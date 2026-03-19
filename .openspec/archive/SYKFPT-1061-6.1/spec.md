# Spec: SDK Compliance & Usage (SYKFPT-1061-6.1)

## 1. 最小依赖规范
- **Runtime**: Java 21+。
- **External Libs**: Jackson (JSON), Slf4j (Logging)。尽量避免引入大型框架以保持轻量。

## 2. 接口契约 (API)
```java
GatewayClient client = GatewayClient.builder()
    .appKey("...")
    .appSecret("...")
    .gatewayUrl("wss://...")
    .build();

client.onEvent(frame -> {
    System.out.println("Received: " + frame.getPayload());
    return true; // 自动返回 200 ACK
});

client.start();
```

## 3. 安全规范
- **No Log Secret**: SDK 内部日志严禁打印 `appSecret`。
- **Nonce Usage**: 每次重连必须获取全新的 Nonce。

## 4. 性能规范
- **Footprint**: 内存占用在闲置时应 < 10MB。
- **Threads**: 默认不创建额外线程池，利用 Java 21 虚拟线程或调用方的上下文。
