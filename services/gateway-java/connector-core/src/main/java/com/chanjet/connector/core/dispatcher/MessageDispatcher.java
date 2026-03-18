package com.chanjet.connector.core.dispatcher;

import com.chanjet.connector.api.connection.IConnectionManager;
import com.chanjet.connector.api.connection.IP2PClient;
import com.chanjet.connector.api.store.ILoadBalancer;
import com.chanjet.connector.api.store.IRouteStore;
import com.chanjet.connector.common.protocol.EventFrame;
import com.chanjet.connector.core.state.ToleranceManager;

import java.util.Optional;
import java.util.Set;

/**
 * 消息分发器核心逻辑实现。
 */
public class MessageDispatcher {

    private final String nodeId;
    private final IRouteStore routeStore;
    private final IConnectionManager connectionManager;
    private final IP2PClient p2pClient;
    private final ILoadBalancer loadBalancer;
    private final ToleranceManager toleranceManager;

    public MessageDispatcher(String nodeId,
                             IRouteStore routeStore,
                             IConnectionManager connectionManager,
                             IP2PClient p2pClient,
                             ILoadBalancer loadBalancer,
                             ToleranceManager toleranceManager) {
        this.nodeId = nodeId;
        this.routeStore = routeStore;
        this.connectionManager = connectionManager;
        this.p2pClient = p2pClient;
        this.loadBalancer = loadBalancer;
        this.toleranceManager = toleranceManager;
    }

    public void dispatch(EventFrame frame) {
        Set<String> routes = routeStore.getNodes(frame.appKey());
        
        if (routes == null || routes.isEmpty()) {
            // 触发容忍期状态机逻辑
            toleranceManager.handleFailure(frame.appKey(), System.currentTimeMillis());
            return;
        }

        // 通过负载均衡器选择一个目标路由
        Optional<String> selectedRoute = loadBalancer.select(routes);
        if (selectedRoute.isEmpty()) {
            toleranceManager.handleFailure(frame.appKey(), System.currentTimeMillis());
            return;
        }

        String route = selectedRoute.get();
        int lastColonIndex = route.lastIndexOf(":");
        if (lastColonIndex == -1) return;

        String targetNodeId = route.substring(0, lastColonIndex);
        String clientId = route.substring(lastColonIndex + 1);

        if (targetNodeId.equals(nodeId)) {
            connectionManager.push(clientId, frame);
        } else {
            p2pClient.forward(targetNodeId, frame);
        }
    }
}
