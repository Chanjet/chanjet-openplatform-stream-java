# Proposal: Cowen CLI v0.2.0 PDU 3 - Integration & UI/UX

## Why (驱动背景)
我们已经在引擎层（PDU 1）和生命周期层（PDU 2）完成了 OAuth2 PKCE 的核心逻辑。现在需要将其无缝组装进 `cowen init` 指令中，为用户提供“一键式”的授权配置体验。

## What Changes (变更内容)
- **Init 指令重构**：
  - 更新 `src/cmd/init.rs`，增加对 `app_mode` 的识别。
  - 对于 OAuth2 模式：
    - 自动选择一个可用端口并启动 `OAuth2CallbackListener`。
    - 生成基于 `DEF_MARKET_URL` 的授权链接。
    - 在终端展示授权 URL 及其对应的 QR Code。
    - 进入等待循环，实时反馈授权进度。
- **UI/UX 优化**：
  - 使用 `qrcode` 库展示 QR Code，方便移动端快速扫描。
  - 授权成功后，自动执行换票并打印成功信息，随后引导用户进行第一次 API 调用。
- **错误恢复**：
  - 处理用户中断（Ctrl+C）及超时场景，确保残留的 `AuthSession` 被妥善清理。

## Impact (影响范围)
- **指令交互**：`cowen init` 将支持完全自动化的 OAuth2 引导流程（不再需要手动填写 AppSecret 和 Certificate，具体取决于模式）。
- **配置**：`Init` 后的配置文件将包含 `app_mode: oauth2`。

## Verification Plan (验证计划)
- **手动验证**：在本地启动模拟授权页，验证 `cowen init` 能够正常拉起监听、展示 QR 并捕获回调完成初始化。
- **异常测试**：验证在授权过程中中断 CLI 是否会导致状态泄漏。
