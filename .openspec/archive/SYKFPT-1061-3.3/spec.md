# Spec: Resilience Thresholds (SYKFPT-1061-3.3)

## 1. 默认阈值规范
- **Node Max Concurrent**: 5,000 (可配置)。
- **Tenant Max Concurrent**: 100 (可配置)。
- **Circuit Breaker Threshold**: 50% 失败率。
- **Circuit Breaker Window**: 滚动窗口内至少 20 次请求。
- **Circuit Breaker Sleep Duration**: 60 秒。

## 2. 响应行为规范
- **Node Overload**: 返回 `HTTP 503 Service Unavailable`。
- **Tenant Limited**: 返回 `HTTP 429 Too Many Requests`。
- **Circuit Open**: 返回 `HTTP 503 Service Unavailable`。

## 3. 监控指标 (Prometheus)
必须暴露以下指标：
- `gateway_resilience_denied_total{reason="node_full|tenant_full|circuit_open"}`
- `gateway_active_requests{app_key="..."}`
