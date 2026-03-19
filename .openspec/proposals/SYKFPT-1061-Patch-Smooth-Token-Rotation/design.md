# Design Patch: Smooth Token Rotation (SYKFPT-1061-Patch-Token)

## 1. 配置项定义 (Properties)
```java
@ConfigurationProperties(prefix = "connector")
public record ConnectorProperties(
    List<String> internalTokens, // 升级为列表
    String nodeId
) {}
```

## 2. 校验逻辑实现
在 `WebhookController` 或自定义拦截器中：
```java
public boolean isAuthorized(String requestToken) {
    if (properties.internalTokens() == null) return false;
    // 只要命中列表中的任意一个即为合法
    return properties.internalTokens().contains(requestToken);
}
```

## 3. 发送逻辑实现
在 `RestP2PClient` 中：
```java
String token = properties.internalTokens().get(0); // 始终使用最新的（首位）令牌
restClient.post().header("X-Internal-Secret", token)...
```

## 4. 滚动更新示例 (application.yml)
```yaml
connector:
  # 滚动切换期间的配置
  internal-tokens:
    - ${NEW_TOKEN}
    - ${OLD_TOKEN}
```
通过这种方式，发送端始终用 `NEW_TOKEN`，但接收端同时认可 `OLD_TOKEN`，从而保证了双向连通。
