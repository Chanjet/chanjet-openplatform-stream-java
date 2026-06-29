# Chanjet Stream Gateway Rust Demo

这是 畅捷通 Stream Gateway 的 Rust SDK 官方示例代码，帮助您快速体验连接与消息分发能力。

## 运行前提

- 安装了 Rust 工具链 (`rustc`, `cargo`)
- 准备好 畅捷通开放平台 的应用凭证：`APP_KEY` 和 `APP_SECRET`

## 快速运行

1. 复制环境配置模板，并填写您的应用凭证：
   ```bash
   cp .env.example .env
   ```
   
   修改 `.env` 文件：
   ```env
   APP_KEY=your_app_key
   APP_SECRET=your_app_secret
   # 仅在有额外设置时填写
   # ENCRYPT_KEY=your_encrypt_key
   ```

2. 启动示例代码：
   ```bash
   cargo run
   ```

3. 观察控制台输出，正常情况下，你会看到初始化、连接成功、以及收到应用票据（AppTicket）或其它消息的打印。

## 代码结构说明

- `src/main.rs`: 包含了初始化配置、如何使用 `MessageDispatcher` 注册事件处理器以及启动 SDK 的完整流程。
- `Cargo.toml`: 包含了 `tokio`、`dotenvy` 以及本地引用的 `connector-sdk` 依赖配置。
