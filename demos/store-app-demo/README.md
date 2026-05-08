# Cowen Store App Mode Demo

这是一个基于 **Node.js** 的演示项目，展示了 ISV 如何在 **Store App (商店应用)** 模式下使用 `cowen`。

## 🎯 核心展示
1. **Sidecar 部署**: 使用 Docker Compose 演示 `app` 与 `cowen` 的边车协作。
2. **多租户 API 调用**: 演示如何通过 `orgId` Header 让 `cowen` 代理自动处理不同租户的 Token。
3. **消息推送接收**: 演示如何接收 `cowen` 转发的业务消息（Webhook）。

## 🚀 快速开始

### 1. 配置环境
在 `docker-compose.yml` 中填写您的应用信息：
- `COWEN_APP_KEY`: 您的 AppKey
- `COWEN_APP_SECRET`: 您的 AppSecret
- `COWEN_ENCRYPT_KEY`: 16位加密密钥

### 2. 启动服务
```bash
docker-compose up -d
```

### 3. 演示授权流程
1. 访问首页：`http://localhost:5000`
2. 点击 **“立即发起授权同步”** 按钮。
3. 授权完成后，开放平台会跳转回 `http://localhost:5000/callback`。
4. 在回调页面，您可以看到接收到的 `code`，并点击按钮测试 API 调用。

### 4. 手动测试 API 调用
如果您已有授权租户，也可以直接测试：
```bash
curl http://localhost:5000/api-test?orgId=YOUR_TEST_ORG_ID
```

Your app will internally call:
```bash
curl http://cowen:8000/v1/user/info -H "x-org-id: YOUR_TEST_ORG_ID"
```

### 4. 模拟消息推送
当开放平台向 `cowen` 推送消息时，`cowen` 会将其转发给 `app` 的 `/webhook` 接口。您可以在 `app` 的日志中观察到输出：
```bash
docker-compose logs -f app
```

## 🏗️ 架构说明
- **网络共享**: `app` 容器通过 `network_mode: "service:cowen"` 与 `cowen` 共享网络命名空间。这允许 `app` 通过 `127.0.0.1` 访问 `cowen` 的代理端口，且 `cowen` 转发 Webhook 时也能直接到达 `127.0.0.1:5000`。
- **持久化存储**: 使用 Redis 存储租户的 Token 状态，支持多实例横向扩展。
