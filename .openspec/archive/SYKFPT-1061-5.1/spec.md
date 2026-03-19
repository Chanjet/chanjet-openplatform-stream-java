# Spec: WebSocket Protocol Implementation (SYKFPT-1061-5.1)

## 1. 连接协议规范 (Handshake)
- **Endpoint**: `ws://{host}:{port}/connect?app_key={...}&client_id={...}&...`
- **身份验证**: 握手拦截器应在升级前校验签名（后续 Task 5.3 完善）。

## 2. 消息帧格式 (Frame Format)
- **数据编码**: 全文本 JSON。
- **字符集**: UTF-8。
- **最大帧限制**: 1MB（防止恶意大报文内存攻击）。

## 3. 物理可靠性规范
- **Binary Support**: 禁用（仅支持文本帧）。
- **Session Timeout**: 默认 30 分钟（若无应用级心跳）。
- **Concurrency**: 支持基于虚拟线程的异步非阻塞发送。

## 4. 依赖注入契约
- `connector-server` 必须提供 `IConnectionManager` 的单例 Bean 供 `connector-core` 使用。
