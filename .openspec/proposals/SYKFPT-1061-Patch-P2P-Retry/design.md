# Design Patch: P2P Forwarding Resilience (SYKFPT-1061-Patch-P2P-Retry)

## 1. 核心流程伪代码
```java
private boolean doDispatch(EventFrame frame) {
    // 1. 本地优先 (略)
    
    // 2. 集群重试逻辑
    Set<String> allRoutes = routeStore.getNodes(frame.appKey());
    int maxAttempts = Math.min(allRoutes.size(), 3); // 最多尝试 3 个节点
    
    for (int i = 0; i < maxAttempts; i++) {
        Optional<String> selected = loadBalancer.select(allRoutes);
        if (selected.isEmpty()) break;
        
        String route = selected.get();
        allRoutes.remove(route); // 移除已选，防止下次重复
        
        try {
            if (p2pClient.forward(targetNodeId, targetedFrame)) {
                return true; // 转发成功，立即退出
            }
        } catch (Exception e) {
            log.error("P2P attempt {} failed for node {}: {}", i+1, targetNodeId, e.getMessage());
            // 继续循环尝试下一个节点
        }
    }
    return false;
}
```

## 2. IP2PClient 接口变更
将 `forward` 方法的返回值从 `void` 改为 `boolean`，以便分发器感知底层 HTTP 是否成功响应。

## 3. 防环路机制
在 P2P 请求的 Header 中增加 `X-GW-Hop-Count`。如果跳数超过 1，目标节点在本地推送失败后禁止再次发起 P2P 转发，防止 A -> B -> A。
