# Spec: TCK Verification Standards (SYKFPT-1061-6.2)

## 1. TCK 通过标准 (Exit Criteria)
- **100% Case Pass**: TCK 中定义的所有 4 个场景必须全部通过。
- **Latency Benchmark**: 在单客户端场景下，Webhook POST 到网关收到 200 的全链路时延应 < 50ms (本地测试)。
- **Clean Environment**: 测试完成后，不得在本地 Redis 留下任何残留 Key。

## 2. 跨语言一致性要求
- 本次虽然实现 Java 版 TCK，但其定义的消息顺序、状态码及 ACK 逻辑将作为未来 Go/Rust 版 SDK 的唯一验收标准。

## 3. CI/CD 集成规范
- TCK 必须能够通过 `mvn verify` 指令在构建阶段自动触发。
- 由于涉及 Docker (TestContainers)，CI 环境必须具备 Docker-in-Docker (DinD) 支持。
