# Design: Message Dispatching Logic (SYKFPT-1061-3.1)

## 1. 领域对象流转
`EventFrame` -> `MessageDispatcher` -> `IRouteStore` -> `Routing Decision` -> `Local/Remote Executor`.

## 2. 核心类职责设计
### 2.1 MessageDispatcher (领域服务)
- `void dispatch(EventFrame frame)`: 核心入口。
- 逻辑流:
    1. 获取 `routes = routeStore.getNodes(frame.appKey())`。
    2. 如果 `routes.isEmpty()`，抛出 `NoOnlineClientException`。
    3. 选择目标路由 `target = loadBalancer.select(routes)`。
    4. 判断 `target.nodeId().equals(thisNodeId)`。
    5. 若是本地：`connectionManager.push(target.clientId(), frame)`。
    6. 若是远程：`p2pClient.forward(target.nodeId(), frame)`。

### 2.2 LoadBalancer (策略模式)
- 初始实现：`RandomLoadBalancer` 或 `RoundRobinLoadBalancer`。

## 3. TDD 测试矩阵 (Test Cases)
- `shouldPushToLocalSessionWhenRouteMatchesCurrentNode()`: 验证本地推送路径。
- `shouldForwardToRemoteNodeWhenRouteIsOtherNode()`: 验证跨节点转发路径。
- `shouldThrowExceptionWhenNoRoutesFound()`: 验证离线异常处理。
- `shouldLoadBalanceWhenMultipleRoutesExist()`: 验证负载均衡策略。
