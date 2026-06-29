# 畅捷通 Open Streaming Connector - Node.js SDK Demo

本目录提供了一个完整的基于 Node.js SDK 的使用示例，展示了如何连接网关、接收并解密系统消息（如应用票据、订单状态更新、应用授权等）。

## 准备工作

1. **环境要求**：Node.js >= 16.0.0
2. **凭证准备**：请前往畅捷通开放平台获取您的：
   - `APP_KEY` (应用 Key)
   - `APP_SECRET` (应用 Secret)
   - `ENCRYPT_KEY` (消息加解密 Key)

## 运行步骤

### 1. 安装依赖

在 `demo` 目录下运行：
```bash
npm install
```

> **注意**：此处默认使用相对路径 (`file:..`) 链接外层的 SDK。如果您将此 Demo 拷贝为独立项目使用，请修改 `package.json` 中的依赖为实际的包名版本（如 `npm i @chanjet/connector-sdk`）。

### 2. 配置环境变量

复制环境配置文件示例，并重命名为 `.env`：

```bash
cp .env.example .env
```

编辑 `.env` 文件，填入您的真实凭证：
```env
APP_KEY=您的AppKey
APP_SECRET=您的AppSecret
ENCRYPT_KEY=您的EncryptKey
# GATEWAY_URL 默认连接至生产环境，如无特殊需求无需配置
```

### 3. 启动示例

运行以下命令启动程序：

```bash
node main.js
```

启动成功后，您将会看到如下日志：

```
🚀 [Demo] 正在从环境变量启动 Node.js SDK Demo...
📍 AppKey: 您的AppKey
🔑 消息秘钥: xxxx****
[Connector] WebSocket 连接已建立，尝试注册会话...
```

当畅捷通开放平台向您的应用推送消息时（例如每 10 分钟一次的应用票据推送，或真实的业务单据推送），Demo 将会自动解密消息并打印出具体的业务数据。
