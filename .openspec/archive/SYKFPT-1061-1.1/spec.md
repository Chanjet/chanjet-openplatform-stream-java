# Spec: Monorepo Standards (SYKFPT-1061-1.1)

## 1. 命名规范 (Naming Conventions)
- **服务目录**: `services/gateway-{language}-{role}/`
- **SDK 目录**: `sdk/{language}/`
- **Proto 命名**: `proto/{scope}/{service}.proto`

## 2. 构建契约 (Build Contract)
所有服务子目录必须支持以下指令（通过子目录 Makefile 或构建工具）：
- `build`: 编译并生成二进制产物。
- `test`: 运行单元测试。
- `clean`: 清理构建缓存。

## 3. 部署规范 (Deployment Spec)
- 每个服务在 `infra/docker/` 下应有对应的 `Dockerfile`。
- 根目录 `docker-compose.yml` 用于一键启动本地开发环境。
