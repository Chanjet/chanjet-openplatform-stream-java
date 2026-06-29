# 畅捷通 Open Streaming Connector - Go SDK Demo

本目录提供了一个完整的基于 Go SDK 的使用示例，展示了如何连接网关、接收并解密系统消息（如应用票据、临时授权码等）。

## 准备工作

1. **环境要求**：Go 1.18+
2. **凭证准备**：请前往畅捷通开放平台获取您的：
   - `APP_KEY` (应用 Key)
   - `APP_SECRET` (应用 Secret)
   - `ENCRYPT_KEY` (消息加解密 Key)

## 运行步骤

### 1. 配置环境变量

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

### 2. 下载依赖

在 `demo` 目录下运行：
```bash
go mod tidy
```

> **注意**：此处默认使用本地的相对路径 (`replace com.chanjet/connector-sdk-go => ..`) 链接外层的 SDK 源码。如果您将此 Demo 文件夹拷贝到别处作为独立项目使用，请删除 `go.mod` 中的 `replace` 指令，并通过执行 `go get github.com/Chanjet/chanjet-openplatform-stream-go` 来下载依赖。

### 3. 启动示例

运行以下命令启动程序：

```bash
go run main.go
```

启动成功后，控制台会打印：

```text
🚀 [Go Demo] 正在启动 Go SDK Demo...
```

当畅捷通开放平台向您的应用推送消息时（例如每 10 分钟一次的应用票据推送），Demo 将会自动解密消息并打印出具体的业务数据：

```text
🎫 [Go Demo] 收到应用票据: a1b2c3d4...
```
