package com.chanjet.connector.core.dispatcher;

import com.chanjet.connector.api.connection.IConnectionManager;
import com.chanjet.connector.api.connection.IP2PClient;
import com.chanjet.connector.api.exception.NoOnlineClientException;
import com.chanjet.connector.api.resilience.IResilienceManager;
import com.chanjet.connector.api.store.ILoadBalancer;
import com.chanjet.connector.api.store.IRouteStore;
import com.chanjet.connector.common.protocol.AcquisitionResult;
import com.chanjet.connector.common.protocol.EventFrame;
import com.chanjet.connector.core.state.ToleranceManager;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.*;

/**
 * 消息分发器核心逻辑实现。
 * 策略：本地优先单播 + 远程失败重试 (Resilient Clustered Unicast)。
 */
public class MessageDispatcher {

    private static final Logger log = LoggerFactory.getLogger(MessageDispatcher.class);

    private final String nodeId;
    private final IRouteStore routeStore;
    private final IConnectionManager connectionManager;
    private final IP2PClient p2pClient;
    private final ILoadBalancer loadBalancer;
    private final ToleranceManager toleranceManager;
    private final IResilienceManager resilienceManager;

    public MessageDispatcher(String nodeId,
                             IRouteStore routeStore,
                             IConnectionManager connectionManager,
                             IP2PClient p2pClient,
                             ILoadBalancer loadBalancer,
                             ToleranceManager toleranceManager,
                             IResilienceManager resilienceManager) {
        this.nodeId = nodeId;
        this.routeStore = routeStore;
        this.connectionManager = connectionManager;
        this.p2pClient = p2pClient;
        this.loadBalancer = loadBalancer;
        this.toleranceManager = toleranceManager;
        this.resilienceManager = resilienceManager;
    }

    public void dispatch(EventFrame frame) {
        AcquisitionResult result = resilienceManager.tryAcquire(frame.appKey());
        if (result != AcquisitionResult.ALLOWED) {
            log.warn("[THROTTLED] MsgId: {}, AppKey: {}", frame.msgId(), frame.appKey());
            return;
        }

        log.info("[DISPATCH_START] Node: {}, MsgId: {}, AppKey: {}, TraceId: {}", 
            nodeId, frame.msgId(), frame.appKey(), frame.traceId());

        boolean success = false;
        try {
            success = doDispatch(frame);
        } finally {
            resilienceManager.release(frame.appKey(), success);
        }
    }

    private boolean doDispatch(EventFrame frame) {
        String appKey = frame.appKey();

        // 1. 本地优先策略
        List<String> localClients = connectionManager.getClientsByAppKey(appKey);
        if (localClients != null && !localClients.isEmpty()) {
            log.debug("Local-First: Found {} clients on current node {}. Pushing locally. MsgId: {}", localClients.size(), nodeId, frame.msgId());
            boolean anySuccess = false;
            for (String clientId : localClients) {
                if (connectionManager.push(clientId, frame)) {
                    anySuccess = true;
                }
            }
            if (anySuccess) {
                toleranceManager.handleReconnect(appKey);
                return true;
            }
            // 如果本地所有连接都推送失败（可能是僵尸连接），则 fallback 到集群查找或失败处理
            log.warn("Local-First: All {} local clients failed to receive push for AppKey [{}]. MsgId: {}", localClients.size(), appKey, frame.msgId());
        }

        // 2. 防环路检查：如果该帧已经经过转发且本地没有连接，则不再继续转发
        String hopCountStr = frame.headers().getOrDefault("X-GW-Hop-Count", "0");
        if (Integer.parseInt(hopCountStr) > 0) {
            log.warn("P2P Loop Prevention: Message [{}] already hopped, local push failed. Dropping. MsgId: {}", frame.msgId(), frame.msgId());
            return false;
        }

        // 3. 集群重试逻辑
        Set<String> availableRoutes = routeStore.getNodes(appKey);
        if (availableRoutes == null || availableRoutes.isEmpty()) {
            log.warn("[DISPATCH_ERROR] No online clients/routes found for AppKey: {}. MsgId: {}", appKey, frame.msgId());
            toleranceManager.handleFailure(appKey, System.currentTimeMillis());
            throw new NoOnlineClientException(appKey);
        }

        // 最多尝试 3 个不同节点进行转发
        int maxAttempts = Math.min(availableRoutes.size(), 3);
        Set<String> triedNodes = new HashSet<>();

        for (int i = 0; i < maxAttempts; i++) {
            // 排除已尝试过的路由
            Set<String> remainingRoutes = new HashSet<>(availableRoutes);
            triedNodes.forEach(node -> remainingRoutes.removeIf(r -> r.startsWith(node + ":")));
            
            if (remainingRoutes.isEmpty()) break;

            Optional<String> selectedRoute = loadBalancer.select(remainingRoutes);
            if (selectedRoute.isEmpty()) break;

            String route = selectedRoute.get();
            int lastColonIndex = route.lastIndexOf(":");
            if (lastColonIndex == -1) continue;

            String targetNodeId = route.substring(0, lastColonIndex);
            String targetClientId = route.substring(lastColonIndex + 1);
            triedNodes.add(targetNodeId);

            // 如果负载均衡选到了本节点（可能 Redis 还没更新），已经在第 1 步处理过了
            if (targetNodeId.equals(nodeId)) continue;

            EventFrame targetedFrame = new EventFrame(
                    frame.msgType(), frame.msgId(), frame.traceId(), appKey,
                    targetClientId, frame.headers(), frame.payload(), frame.timestamp()
            );

            log.info("Dispatching attempt {}: MsgId: [{}] -> remote node [{}]", i + 1, frame.msgId(), targetNodeId);
            if (p2pClient.forward(targetNodeId, targetedFrame)) {
                log.info("[FORWARD_SUCCESS] MsgId: {}, TargetNode: {}", frame.msgId(), targetNodeId);
                toleranceManager.handleReconnect(appKey);
                return true; // 转发成功，流程结束
            }
            log.warn("[FORWARD_FAILED] MsgId: {}, TargetNode: {}", frame.msgId(), targetNodeId);
            log.warn("P2P attempt {} failed for node {}, trying next...", i + 1, targetNodeId);
        }

        log.error("[DISPATCH_FAILED] All P2P attempts failed for message [{}] under AppKey [{}]", frame.msgId(), appKey);
        toleranceManager.handleFailure(appKey, System.currentTimeMillis());
        return false;
    }
}
