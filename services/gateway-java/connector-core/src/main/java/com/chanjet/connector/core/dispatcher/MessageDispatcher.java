package com.chanjet.connector.core.dispatcher;

import com.chanjet.connector.api.connection.IConnectionManager;
import com.chanjet.connector.api.connection.IP2PClient;
import com.chanjet.connector.api.resilience.IResilienceManager;
import com.chanjet.connector.api.store.ILoadBalancer;
import com.chanjet.connector.api.store.IRouteStore;
import com.chanjet.connector.common.protocol.AcquisitionResult;
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
        // 1. 背压与限流保护
        AcquisitionResult result = resilienceManager.tryAcquire(frame.appKey());
        if (result != AcquisitionResult.ALLOWED) {
            // TODO: 在入口 Controller 中根据 result 返回对应 HTTP 状态码
            return;
        }

        boolean success = false;
        try {
            success = doDispatch(frame);
        } finally {
            // 2. 释放许可并反馈结果
            resilienceManager.release(frame.appKey(), success);
        }
    }

    private boolean doDispatch(EventFrame frame) {
        Set<String> routes = routeStore.getNodes(frame.appKey());
        
        if (routes == null || routes.isEmpty()) {
            toleranceManager.handleFailure(frame.appKey(), System.currentTimeMillis());
            return false;
        }

        Optional<String> selectedRoute = loadBalancer.select(routes);
        if (selectedRoute.isEmpty()) {
            toleranceManager.handleFailure(frame.appKey(), System.currentTimeMillis());
            return false;
        }

        String route = selectedRoute.get();
        int lastColonIndex = route.lastIndexOf(":");
        if (lastColonIndex == -1) return false;

        String targetNodeId = route.substring(0, lastColonIndex);
        String clientId = route.substring(lastColonIndex + 1);

        if (targetNodeId.equals(nodeId)) {
            return connectionManager.push(clientId, frame);
        } else {
            p2pClient.forward(targetNodeId, frame);
            return true; // 假设转发发起即为逻辑成功，具体的转发失败由 P2P 层处理
        }
    }
}
