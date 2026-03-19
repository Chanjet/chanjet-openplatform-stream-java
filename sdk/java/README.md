# 畅捷通 Stream Gateway Java SDK

[![Java Version](https://img.shields.io/badge/Java-21+-oracle.svg)](https://www.oracle.com/java/technologies/downloads/)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)

本 SDK 是专为畅捷通开放平台 ISV 打造的轻量级通讯框架。它不仅屏蔽了底层 WebSocket 的连接管理细节，还内置了强大的消息自动解密、POJO 自动分发及好系列业务适配能力。

## 🚀 核心特性

- **自动驾驶级连接管理**：内置心跳保活、断线指数退避重连、Nonce 自动挑战。
- **智能消息分发器 (MessageDispatcher)**：支持根据 `msgType` 自动路由到不同的业务处理器。
- **透明安全保障**：自动解析 `{"encryptMsg": "..."}` 包装层并执行 AES-128-CBC 解密，开发者仅需处理明文 POJO。
- **语义化监听**：提供 `onAppTicket`、`onAppNotice` 等语义化方法，代码即文档。
- **无损上下文传递**：好系列通知（`APP_NOTICE`）同时提供完整消息头与业务负载，确保 `orgId` 等关键信息不丢失。

## 📦 快速开始

### 1. 引入依赖 (Maven)

```xml
<dependency>
    <groupId>com.chanjet.connector</groupId>
    <artifactId>connector-sdk-java</artifactId>
    <version>0.1.0-SNAPSHOT</version>
</dependency>
```

### 2. 编写业务处理器

```java
// 1. 创建分发器
MessageDispatcher dispatcher = new MessageDispatcher();

// 2. 订阅系统消息
dispatcher.onAppTicket(msg -> {
    System.out.println("收到最新票据: " + msg.getBizContent().getAppTicket());
    return true; // 返回 true 自动回复 ACK
});

// 3. 订阅好系列业务消息 (如：销货单)
// 处理器参数说明: (完整消息对象, 业务内容负载)
dispatcher.onAppNotice("GoodsIssue", (msg, content) -> {
    System.out.printf("企业 %s 的操作人 %s 提交了单据 %s\n", 
        msg.getOrgId(), content.getUserName(), content.getCode());
    return true;
});

// 4. 初始化并启动客户端
GatewayClient client = GatewayClient.builder()
    .appKey("your_app_key")
    .appSecret("your_app_secret_32_chars")
    .gatewayUrl("wss://open.chanjet.com/gateway")
    .build();

client.useDispatcher(dispatcher);
client.start();
```

## 🛠️ 高级用法

### 订阅自定义业务消息
如果平台推出了 SDK 尚未内置的消息类型，您可以通过 `register` 方法快速扩展：

```java
// 1. 定义您的 POJO (继承 BaseMessage)
public class MyNewMessage extends BaseMessage {
    private MyContent bizContent;
    // ... getters/setters
}

// 2. 注册订阅
dispatcher.register("MY_NEW_MSG_TYPE", MyNewMessage.class, msg -> {
    // 处理逻辑
    return true;
});
```

### 静默忽略机制
`MessageDispatcher` 默认开启静默忽略：收到未注册类型的消息时，SDK 会打印警告日志并自动返回成功 (200 ACK)，以防止网关因重试导致的资源浪费。

## 📋 内置模型参考

SDK 现已内置以下标准模型，您可以直接在监听器中使用：

| 消息类型 (msgType) | 语义化监听方法 | 模型类 | 描述 |
| :--- | :--- | :--- | :--- |
| `APP_TICKET` | `onAppTicket` | `AppTicketMessage` | 应用票据推送 |
| `TEMP_AUTH_CODE` | `onEntAuthCode` | `EntAuthCodeMessage` | 企业临时授权码 |
| `PAY_ORDER_SUCCESS`| `onOrderStatus` | `OrderStatusMessage` | 订单支付成功回执 |
| `APP_CANCEL_...` | `onEntUnauth`等 | `EntUnauthMessage` | 授权/开通状态变更 |
| `APP_NOTICE` | `onAppNotice` | `AppNoticeMessage` | 好系列标准业务通知 |

## 📖 示例项目
更多详尽的使用场景（如 Spring Boot 集成、复杂复合键分发等），请参考源码树中的：
👉 [**sdk-java-demo**](../java-demo/README.md)

## ⚖️ 许可
Apache License 2.0
