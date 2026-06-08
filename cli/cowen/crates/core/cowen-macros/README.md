# Cowen Macros (`cowen-macros`)

本 Crate 提供了面向切面（AOP）的 gRPC 鉴权宏，专门用于 `tonic` 控制器方法的 RBAC（基于角色的访问控制）权限拦截。

通过在方法或 `impl` 块上添加声明式属性宏，你可以在进入核心业务逻辑前，自动且安全地执行声明式的权限与身份校验。

## 快速概览

### 1. 基础用法
如果你的控制器方法不属于同一个特定的业务域，你可以直接在独立的方法上通过 `scope` 声明完整权限路径：

```rust
use cowen_macros::rbac;

#[tonic::async_trait]
impl MyService for MyController {
    #[rbac(scope = "native.api.registry:read")]
    async fn get_data(&self, request: Request<Req>) -> Result<Response<Res>, Status> {
        // 只有拥有 "native.api.registry:read" 权限的调用方才能进入
        // ...
    }
}
```

### 2. 业务域复用 (`#[rbac_controller]`)
当同一个 Controller 下的所有方法都属于同一个业务域时，你可以使用 `#[rbac_controller]` 将域路径提取到 `impl` 级别，并在方法上直接使用简写的 `action` 属性：

> **注意：** `#[rbac_controller]` 必须放置在 `#[tonic::async_trait]` 的**上方**，以确保预编译阶段的 AST 宏展开顺序正确。

```rust
use cowen_macros::{rbac, rbac_controller};

#[rbac_controller(domain = "native.api.registry")]
#[tonic::async_trait]
impl ApiRegistryService for ApiRegistryController {
    // 在编译阶段将被自动重写展开为: #[rbac(scope = "native.api.registry:search")]
    #[rbac(action = "search")]
    async fn api_list(&self, request: Request<ApiListRequest>) -> Result<Response<ApiListResponse>, Status> {
        // ...
    }

    // 自动展开为: #[rbac(scope = "native.api.registry:execute")]
    #[rbac(action = "execute")]
    async fn call_api(&self, request: Request<CallApiRequest>) -> Result<Response<CallApiResponse>, Status> {
        // ...
    }
}
```

### 3. 多权限校验 (AND 与 OR)
宏底层完全支持接收多个相同的属性键，会自动在内部将其转换为对多个权限的组合并发校验：

#### AND 逻辑（必须同时具备）
使用多次 `action` 或 `scope`，表示调用者必须**同时具备**所声明的所有权限。

```rust
#[rbac(action = "read", action = "write")]
async fn process_data(...) {
    // 必须同时具备 domain:read 和 domain:write 权限
}

#[rbac(scope = "global:admin", scope = "global:operator")]
async fn global_process(...) {
    // 必须同时具备这两种全路径权限
}
```

#### OR 逻辑（具备其一即可）
使用 `any_action` 或 `any_scope`，表示调用者只要**具备其中任意一个**权限即可通过放行。

```rust
#[rbac(any_action = "view", any_action = "edit")]
async fn open_dashboard(...) {
    // 只要有 domain:view 或者 domain:edit 任意一个权限即可进入
}
```

#### 混合组合
你可以同时混用 AND 和 OR 的检查约束。当它们共存时：调用方必须满足所有声明的 `action/scope`，并且只要额外满足 `any_action/any_scope` 数组中的其中一个即可。

### 4. 身份一致性校验 (`profile`)
如果你的 RPC 业务请求本身包含 `profile` 标识符（通常代表工作空间或租户 ID），你可以动态要求访问者的身份凭证（JWT subject）必须严格等于请求报文中的 profile，以防越权：

```rust
#[rbac(profile = "request.get_ref().profile.as_str()")]
async fn start_worker(&self, request: Request<StartWorkerRequest>) {
    // 首先校验请求携带的 JWT 身份是否等于 request.profile
}
```

你可以将 `profile` 与 `action/scope` 组合使用，以达到既验证身份所有权、又验证操作权限的强约束目的：
```rust
#[rbac(profile = "request.get_ref().profile.as_str()", action = "read")]
async fn read_worker_info(...) {
    // ...
}
```

## 技术原理
由于 `#[tonic::async_trait]` 会将所有的 `async fn` 重新包装成同步返回 `Pin<Box<dyn Future>>` 的方法，本宏通过巧妙地在 `Box::pin` 创建之前，提前注入权限拦截校验块。
如果权限不满足，它会立即返回一个携带 `Status::permission_denied` 错误的已完结 Future (`Box::pin(async move { Err(e) })`)。这种底层实现方式不仅保证了最高的运行性能，还无缝兼容了 `tonic` 的 gRPC 规范限制，使得业务函数的纯净度得以保留。
