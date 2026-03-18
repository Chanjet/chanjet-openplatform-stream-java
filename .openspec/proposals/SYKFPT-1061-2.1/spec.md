# Spec: Protobuf Coding Standards (SYKFPT-1061-2.1)

## 1. 命名规范 (Naming Conventions)
- **文件**: `snake_case.proto`。
- **消息**: `CamelCase`。
- **字段**: `snake_case` (Protobuf 3 官方推荐)。
- **枚举值**: `CAPS_SNAKE_CASE`。

## 2. 兼容性准则 (Compatibility)
- **严禁删除字段**: 只能废弃 (Deprecated)。
- **严禁修改字段编号**: 编号必须永久固定。
- **新增字段**: 必须设为可选（Protobuf 3 默认所有字段均为可选）。
- **保留字段**: 使用 `reserved` 关键字保留已废弃的编号或名称。

## 3. 打包规范 (Packaging)
- 根包: `com.chanjet.connector`。
- 子包定义:
  - `proto.model`: 通用消息模型。
  - `proto.internal`: 内部 RPC 协议。
  - `proto.gateway`: 外部接口定义。

## 4. 枚举值安全性
- 枚举值的第一个值必须是 `_UNSPECIFIED` 且编号为 0。
- 用于处理未识别的值或作为默认值。
