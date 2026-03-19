# Design Patch: Dynamic Configuration Refresh (SYKFPT-1061-Patch-Refresh)

## 1. 模型重构 (POJO with Setters)
```java
@Component
@ConfigurationProperties(prefix = "connector")
@RefreshScope // 开启动态刷新作用域
public class ConnectorProperties {
    private List<String> internalTokens;
    private String nodeId;

    // Getters and Setters
}
```

## 2. 线程安全性考量
由于 `internalTokens` 是一个 `List`，在被 Setter 修改的一瞬间，其他虚拟线程可能正在读取它。
- **优化**: 使用 `volatile` 修饰字段，或在读取时进行防御性拷贝。
- **最佳实践**: Spring Cloud 的 `@RefreshScope` 通过创建一个全新的 Proxy 实例来解决该问题，它是天然线程安全的。

## 3. 依赖变更
需要在 `connector-server` 模块中确保引入了 `spring-cloud-starter-bootstrap` 或确保 Nacos Config 已被正确加载。
