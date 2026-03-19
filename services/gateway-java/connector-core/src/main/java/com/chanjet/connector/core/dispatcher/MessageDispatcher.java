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

import java.util.List;
import java.util.Optional;
import java.util.Set;

/**
 * 消息分发器核心逻辑实现。
 * 策略：本地优先单播 (Local-First Unicast)。
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
            log.warn("Request throttled for AppKey: {}", frame.appKey());
            return;
        }

        boolean success = false;
        try {
            success = doDispatch(frame);
        } finally {
            resilienceManager.release(frame.appKey(), success);
        }
    }

    private boolean doDispatch(EventFrame frame) {
        String appKey = frame.appKey();

        // 1. 本地优先策略：检查本节点是否有连接
        List<String> localClients = connectionManager.getClientsByAppKey(appKey);
        if (localClients != null && !localClients.isEmpty()) {
            log.debug("Local-First: Found {} clients on current node {}. Pushing locally.", localClients.size(), nodeId);
            localClients.forEach(clientId -> connectionManager.push(clientId, frame));
            return true;
        }

        // 2. 本地无连接，执行集群单播分发逻辑
        Set<String> routes = routeStore.getNodes(appKey);
        if (routes == null || routes.isEmpty()) {
            log.info("No routes found in cluster for AppKey: {}", appKey);
            toleranceManager.handleFailure(appKey, System.currentTimeMillis());
            throw new NoOnlineClientException(appKey);
        }

        // 通过负载均衡选出一个目标路由 (格式 nodeId:clientId)
        Optional<String> selectedRoute = loadBalancer.select(routes);
        if (selectedRoute.isEmpty()) {
            toleranceManager.handleFailure(appKey, System.currentTimeMillis());
            throw new NoOnlineClientException(appKey);
        }

        String route = selectedRoute.get();
        int lastColonIndex = route.lastIndexOf(":");
        if (lastColonIndex == -1) return false;

        String targetNodeId = route.substring(0, lastColonIndex);
        String targetClientId = route.substring(lastColonIndex + 1);

        // 构造带有精准目标 ClientID 的帧
        EventFrame targetedFrame = new EventFrame(
                frame.msgType(),
                frame.msgId(),
                frame.traceId(),
                appKey,
                targetClientId,
                frame.headers(),
                frame.payload(),
                frame.timestamp()
        );

        if (targetNodeId.equals(nodeId)) {
            // 虽然前面查过本地，但如果在高并发下 Redis 还没来得及更新，这里作为二次防线
            return connectionManager.push(targetClientId, targetedFrame);
        } else {
            // 本地没有，且负载均衡选到了远程节点，执行单播 P2P 转发
            log.info("Dispatching via P2P: [{}] -> remote node [{}]", frame.msgId(), targetNodeId);
            p2pClient.forward(targetNodeId, targetedFrame);
            return true;
        }
    }
}
