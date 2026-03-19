# Spec: Internal P2P Contract (SYKFPT-1061-P2P)

## 1. 转发 Header 规范
必须显式透传以下原始 Header 以保证幂等与追踪一致性：
- `X-C-APP_KEY`
- `X-MSG-ID`
- `X-Trace-Id`

## 2. 性能规范
- **内部延迟**: 节点间单次转发增加的时延应控制在 10ms 以内（局域网环境）。
- **连接池**: P2P 专用 `RestClient` 必须配置连接池，最大连接数 1000。

## 3. 安全规范
- 内部接口必须通过 `application.yml` 中的 `connector.internal-token` 进行校验。
- 只有具备合法令牌的请求才允许执行物理推送。
