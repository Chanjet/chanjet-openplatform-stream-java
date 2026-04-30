# Specification Delta: Storage Management Command

## MODIFIED Requirements

### Requirement: 存储后端配置命令 (Storage Configuration Command)
SYSTEM SHALL provide a dedicated `store set` command to configure global storage and cache settings.

#### Scenario: 成功设置存储后端
GIVEN 系统未配置或需修改存储配置
WHEN 用户执行 `cowen store set --store mysql --db-url <DB_URL>`
THEN 系统 SHALL 验证连接性
AND 系统 SHALL 更新全局 `app.yaml` 配置文件
AND 系统 SHALL 返回成功提示。

### Requirement: 存储状态自检命令 (Storage Status Command)
SYSTEM SHALL provide a `store status` command to display current storage configuration and verify connectivity.

#### Scenario: 查看存储状态
WHEN 用户执行 `cowen store status`
THEN 系统 SHALL 显示当前存储后端类型、URL 及缓存配置
AND 系统 SHALL 执行连接性检查并显示健康状态。

### Requirement: 初始化命令精简 (Streamlined Init Command)
SYSTEM SHALL remove storage-related parameters from the `init` command.

#### Scenario: 执行初始化
WHEN 用户执行 `cowen init`
THEN 系统 SHALL 直接使用全局配置中的存储后端
AND 系统 SHALL 仅处理 Profile 相关的认证与参数配置。

## ADDED Requirements

### Requirement: 存储连接性验证 (Storage Connectivity Validation)
WHEN 配置新的存储后端,
系统 SHALL 尝试建立连接。

#### Scenario: 连接失败处理
GIVEN 数据库服务器不可达
WHEN 用户尝试 `store set` 或执行 `store status`
THEN 系统 SHALL 报告具体的连接错误
AND 系统 SHALL 将状态标记为 ERROR。
