# Design: Monorepo Scaffolding (SYKFPT-1061-1.1)

## 1. 骨架拓扑 (Skeleton Topology)
根目录 Makefile 将作为分发器，调用各子目录的构建工具：
- `make build-java` -> `(cd services/gateway-java && mvn package)`
- `make proto` -> `(cd proto && protoc --java_out=...)`

## 2. 核心目录预设
- `proto/`: 存储跨语言 IDL 文件。
- `services/`: 存储独立进程的源码。
- `sdk/`: 存储多语言客户端实现。
- `infra/`: 存储 K8s Deployment 和 Dockerfile。

## 3. 工程化配置
- `.gitignore`: 配置全局忽略规则，如 `.DS_Store`, `target/`, `.idea/`。
- `Makefile`: 定义全局变量 `PROJECT_NAME`, `VERSION` 等。
